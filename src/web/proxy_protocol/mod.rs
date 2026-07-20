use std::pin::Pin;
use std::task::{Context, Poll};
use std::{io::Result, net::SocketAddr};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf, Ready};
use tokio::net::TcpStream;

mod parse;

use parse::{Parsed, SIGNATURE, parse_pp_v2};

pub struct ProxiedStream<S> {
    inner: S,
    pub src: Option<SocketAddr>,
    socket_peer: Option<SocketAddr>,
    prefix: Vec<u8>,
    pos: usize,
}

impl<S> ProxiedStream<S> {
    fn new(
        inner: S,
        src: Option<SocketAddr>,
        socket_peer: Option<SocketAddr>,
        prefix: Vec<u8>,
    ) -> Self {
        Self {
            inner,
            src,
            socket_peer,
            prefix,
            pos: 0,
        }
    }

    /// Effective client address: PROXY-advertised source if present,
    /// otherwise the real socket peer (direct connection / health check).
    pub fn peer(&self) -> Option<SocketAddr> {
        self.src.or(self.socket_peer)
    }
}

pub async fn accept(mut tcp: TcpStream) -> Result<ProxiedStream<TcpStream>> {
    let socket_peer = tcp.peer_addr().ok();
    let mut fixed = [0u8; 16];
    tcp.read_exact(&mut fixed).await?;

    // Dispatch on the signature only — the address block isn't in these 16
    // bytes yet, so we must NOT full-parse here. Non-PROXY: replay the bytes.
    if fixed[..12] != SIGNATURE {
        return Ok(ProxiedStream::new(tcp, None, socket_peer, fixed.to_vec()));
    }

    // It is PROXY v2: read exactly the advertised address block, then parse
    // the complete header. LOCAL / UNSPEC parse to None -> socket peer.
    let addr_len = u16::from_be_bytes([fixed[14], fixed[15]]) as usize;
    let mut full = fixed.to_vec();
    full.resize(16 + addr_len, 0);
    tcp.read_exact(&mut full[16..]).await?;
    let src = match parse_pp_v2(&full) {
        Parsed::Proxied(s) => Some(s),
        _ => None,
    };
    Ok(ProxiedStream::new(tcp, src, socket_peer, Vec::new()))
}

impl<S: actix_rt::net::ActixStream> actix_rt::net::ActixStream for ProxiedStream<S> {
    fn poll_read_ready(&self, cx: &mut Context<'_>) -> Poll<Result<Ready>> {
        S::poll_read_ready(&self.inner, cx)
    }
    fn poll_write_ready(&self, cx: &mut Context<'_>) -> Poll<Result<Ready>> {
        S::poll_write_ready(&self.inner, cx)
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for ProxiedStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        if self.pos < self.prefix.len() {
            let rem = &self.prefix[self.pos..];
            let n = rem.len().min(buf.remaining());
            buf.put_slice(&rem[..n]);
            self.pos += n;
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for ProxiedStream<S> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, b: &[u8]) -> Poll<Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, b)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
