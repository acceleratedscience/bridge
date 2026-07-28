use actix_web::{HttpRequest, HttpResponse, dev::PeerAddr, rt, web};
use actix_ws::AggregatedMessage;
use bytestring::ByteString;
use futures::{SinkExt, StreamExt};
use http::{HeaderName, HeaderValue, StatusCode, Uri};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{
    self, Utf8Bytes, protocol::WebSocketConfig,
};
use url::Url;

use crate::errors::{BridgeError, Result};

const WS_STRIP_REQ: &[&str] = &[
    "host",
    "connection",
    "upgrade",
    "sec-websocket-key",
    "sec-websocket-version",
    "sec-websocket-accept",
    "sec-websocket-extensions",
    "content-length",
    "keep-alive",
    "transfer-encoding",
];

// The upper bound for the maximum size of a WebSocket continuation frame
const MAX_CONTINUATION_SIZE: usize = 64 * 1024 * 1024; // 64MB

#[inline]
pub fn is_websocket(req: &HttpRequest) -> bool {
    let upgrade = req
        .headers()
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"));

    let connection = req
        .headers()
        .get("connection")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("upgrade"));

    upgrade && connection
}

pub async fn forward_ws(
    req: HttpRequest,
    pl: web::Payload,
    peer_addr: Option<PeerAddr>,
    upstream_url: Url,
) -> Result<HttpResponse> {
    let raw = upstream_url.as_str();

    let normalized = if let Some(rest) = raw.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = raw.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        raw.to_string()
    };
    let uri: Uri = normalized
        .parse()
        .map_err(|e: http::uri::InvalidUri| BridgeError::GeneralError(e.to_string()))?;

    let mut upstream_req = uri
        .into_client_request()
        .map_err(|e| BridgeError::GeneralError(e.to_string()))?;

    let header = upstream_req.headers_mut();

    for (name, value) in req.headers() {
        if WS_STRIP_REQ.contains(&name.as_str()) {
            continue;
        }

        let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_str().as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) else {
            continue;
        };
        header.append(n, v);
    }

    if let Some(PeerAddr(addr)) = peer_addr {
        let prior = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok());
        let xff = match prior {
            Some(p) => format!("{p}, {}", addr.ip()),
            None => addr.ip().to_string(),
        };
        if let Ok(v) = HeaderValue::from_str(&xff) {
            header.insert("x-forwarded-for", v);
        }
    }
    header.insert("x-forwarded-proto", HeaderValue::from_static("https"));

    let (stream, res) = tokio_tungstenite::connect_async_with_config(
        upstream_req,
        Some(WebSocketConfig::default().accept_unmasked_frames(true)),
        false,
    )
    .await
    .map_err(|e| BridgeError::GeneralError(e.to_string()))?;

    if !res.status().eq(&StatusCode::SWITCHING_PROTOCOLS) {
        return Err(BridgeError::GeneralError(format!(
            "Upstream server did not switch protocols: {}",
            res.status()
        )));
    }

    // Capture the subprotocol the upstream accepted so we can echo it to the
    // client's 101 response. Without this a client that offered a subprotocol
    // (e.g. Jupyter's `v1.kernel.websocket.jupyter.org`) sees an empty
    // `ws.protocol` and silently drops to legacy framing. Crosses the http 1.x
    // (tungstenite) -> http 0.2 (actix) boundary the same way forward.rs does.
    let upstream_subprotocol = res
        .headers()
        .get(http::header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| actix_web::http::header::HeaderValue::from_str(v).ok());

    let (mut s_sink, mut s_stream) = stream.split();
    let (mut response, mut session, c_stream) =
        actix_ws::handle(&req, pl).map_err(|e| BridgeError::GeneralError(e.to_string()))?;

    let mut c_stream = c_stream
        .aggregate_continuations()
        .max_continuation_size(MAX_CONTINUATION_SIZE);

    if let Some(proto) = upstream_subprotocol {
        response
            .headers_mut()
            .insert(actix_web::http::header::SEC_WEBSOCKET_PROTOCOL, proto);
    }

    macro_rules! shutdown {
        // bail is not an error.. check token needed to eval Result
        (check $res:expr, $msg:expr) => {{
            if let Err(e) = $res {
                tracing::warn!(error = ?e, message = %$msg);
                shutdown!();
            }
        }};
        ($msg:expr, warn)  => {{ tracing::warn!("{}", $msg);  shutdown!(); }};
        ($msg:expr, error) => {{ tracing::error!("{}", $msg); shutdown!(); }};
        () => {{
            let _ = s_sink.close().await;
            let _ = session.close(None).await;
            break;
        }};
    }

    rt::spawn(async move {
        loop {
            tokio::select! {
                // data to be sent to downstream server
                resp = c_stream.next() => {
                    let msg = match resp {
                        Some(Ok(m)) => m,
                        Some(Err(e)) => {
                            shutdown!(format!("Error reading from client stream: {}", e), error);
                        }
                        None => shutdown!("Client stream closed", warn),
                    };

                    match msg {
                        AggregatedMessage::Text(t) => {
                            let msg = tungstenite::Message::Text(
                                // SAFETY: we are converting a
                                // ByteString which is already a valid UTF8
                                unsafe {
                                    Utf8Bytes::from_bytes_unchecked(t.into_bytes())
                                }
                            );
                            shutdown!(
                                check s_sink.send(msg).await,
                                "Error sending text message to upstream server"
                            );
                        },
                        AggregatedMessage::Binary(b) => {
                            shutdown!(
                                check s_sink.send(tungstenite::Message::Binary(b)).await,
                                "Error sending binary message to upstream server"
                            );
                        },
                        AggregatedMessage::Ping(p) => {
                            shutdown!(
                                check s_sink.send(tungstenite::Message::Ping(p)).await,
                                "Error sending ping message to upstream server"
                            )
                        },
                        AggregatedMessage::Pong(p) => {
                            shutdown!(
                                check s_sink.send(tungstenite::Message::Pong(p)).await,
                                "Error sending pong message to upstream server"
                            )
                        },
                        AggregatedMessage::Close(_) => {
                            shutdown!(
                                check s_sink.send(tungstenite::Message::Close(None)).await,
                                "Error sending close message to upstream server"
                            )
                        },
                    }
                }

                resp = s_stream.next() => {
                    let msg = match resp {
                        Some(Ok(m)) => m,
                        Some(Err(e)) => shutdown!(format!("Error reading from upstream server stream: {}", e), error),
                        None => shutdown!("Upstream server stream closed", warn),
                    };

                    match msg {
                        tungstenite::Message::Text(t) => {
                            let text = unsafe {
                                ByteString::from_bytes_unchecked(t.into())
                            };
                            shutdown!(
                                check session.text(text).await,
                                "Error sending text message to client"
                            );
                        },
                        tungstenite::Message::Binary(b) => {
                            shutdown!(
                                check session.binary(b).await,
                                "Error sending binary message to client"
                            );
                        },
                        tungstenite::Message::Ping(p) => {
                            shutdown!(
                                check session.ping(&p).await,
                                "Error sending ping message to client"
                            );
                        },
                        tungstenite::Message::Pong(p) => {
                            shutdown!(
                                check session.pong(&p).await,
                                "Error sending pong message to client"
                            );
                        },
                        tungstenite::Message::Close(_) => {
                            shutdown!();
                        },
                        tungstenite::Message::Frame(_) => {
                            // NOP
                            tracing::warn!("unexpected raw frame from upstream; ignoring");
                        },
                    }
                }
            }
        }
    });

    Ok(response)
}
