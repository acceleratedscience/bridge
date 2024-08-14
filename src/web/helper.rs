use std::error::Error;

use mongodb::bson::{to_bson, Bson};

use crate::errors::GuardianError;

macro_rules! log_with_level {
    ($res:expr, error) => {{
        let result = $res;
        match result {
            Ok(_) => result,
            Err(ref e) => {
                $crate::web::helper::error(e);
                result
            }
        }
    }};
    ($res:expr, warn) => {{
        let result = $res;
        match result {
            Ok(_) => result,
            Err(ref e) => {
                tracing::warn!("Warning: {}", e);
                result
            }
        }
    }};
    ($res:expr, info) => {{
        let result = $res;
        match result {
            Ok(_) => result,
            Err(ref e) => {
                tracing::info!("Info: {}", e);
                result
            }
        }
    }};
    ($res:expr, debug) => {{
        let result = $res;
        match result {
            Ok(_) => result,
            Err(ref e) => {
                tracing::debug!("Debug: {}", e);
                result
            }
        }
    }};
    ($res:expr, $level:tt) => {
        compile_error!("Invalid log level. Use error, warn, info, or debug.")
    };
}

pub fn error(e: impl Error){
    tracing::error!("Error: {}", e);
}

pub(crate) use log_with_level;

pub fn bson<T>(t: T) -> Result<Bson, GuardianError>
where
    T: serde::Serialize,
{
    match to_bson(&t) {
        Ok(bson) => Ok(bson),
        Err(e) => Err(GuardianError::GeneralError(e.to_string())),
    }
}

/// http proxying utilities
pub mod forwarding {
    use std::str::FromStr;

    use actix_web::{
        dev::PeerAddr,
        http::{
            header::{HeaderName, HeaderValue},
            Method,
        },
        web, HttpRequest, HttpResponse,
    };
    use futures::StreamExt;
    use tokio::sync::mpsc;
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use tracing::error;

    use actix_web::http::StatusCode;

    use crate::errors::{GuardianError, Result};

    // No inline needed... generic are inherently inlined
    pub async fn forward<T>(
        _req: HttpRequest,
        mut payload: web::Payload,
        method: Method,
        peer_addr: Option<PeerAddr>,
        client: web::Data<reqwest::Client>,
        new_url: T,
    ) -> Result<HttpResponse>
    where
        T: AsRef<str> + Send + Sync,
    {
        let (tx, rx) = mpsc::unbounded_channel();

        actix_web::rt::spawn(async move {
            while let Some(chunk) = payload.next().await {
                if let Err(e) = tx.send(chunk) {
                    error!("{:?}", e);
                    return;
                }
            }
        });

        // sigh... this is a workaround due to reqwest and actix-web use different versions of
        // hyper. At least we can use two version of hyper and not get stuck in dependency hell
        // like python.
        let method = match method.as_str() {
            "OPTIONS" => reqwest::Method::OPTIONS,
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "HEAD" => reqwest::Method::HEAD,
            "TRACE" => reqwest::Method::TRACE,
            "CONNECT" => reqwest::Method::CONNECT,
            "PATCH" => reqwest::Method::PATCH,
            _ => {
                return Err(GuardianError::GeneralError(
                    "Unsupported HTTP method".to_string(),
                ))
            }
        };

        let forwarded_req = client
            .request(method, new_url.as_ref())
            .body(reqwest::Body::wrap_stream(UnboundedReceiverStream::new(rx)));

        // TODO: This forwarded implementation is incomplete as it only handles the unofficial
        // X-Forwarded-For header but not the official Forwarded one.
        let forwarded_req = match peer_addr {
            Some(PeerAddr(addr)) => forwarded_req.header("X-Forwarded-For", addr.ip().to_string()),
            None => forwarded_req,
        };

        let res = log_with_level!(
            forwarded_req.send().await.map_err(|e| {
                GuardianError::GeneralError(e.to_string())
            }),
            error
        )?;

        let status = res.status().as_u16();
        let status =
            StatusCode::from_u16(status).map_err(|e| GuardianError::GeneralError(e.to_string()))?;

        let mut client_resp = HttpResponse::build(status);
        // Removing `Connection` and 'keep-alive' as per
        // https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Connection#Directives
        // Also removing "content-length" since we are streaming the response, and content-length
        // is not necessarily needed.
        for (header_name, header_value) in res
            .headers()
            .iter()
            .filter(|(h, _)| *h != "connection" && *h != "keep-alive" && *h != "content-length")
        {
            // Again copy over seem incredibly inefficient. It sure is, but like before, we do this
            // because actix-web and reqwest use different versions of hyper. Once Actix-web
            // updates their hyper version, we can remove this.
            let name = header_name.to_string();
            let value = header_value.to_str().unwrap();

            let name = HeaderName::from_str(&name).unwrap();
            let value = HeaderValue::from_str(value).unwrap();

            client_resp.insert_header((name, value));
        }

        Ok(client_resp.streaming(res.bytes_stream()))
    }
}

pub mod ws {
    use actix_web::{
        rt,
        web::{self, BytesMut},
        HttpRequest, HttpResponse,
    };
    use awc::http::StatusCode;
    use futures::{channel::mpsc::unbounded, SinkExt, StreamExt};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::errors::{GuardianError, Result};

    #[inline]
    pub async fn manage_connection<T>(
        req: HttpRequest,
        mut pl: web::Payload,
        url: T,
    ) -> Result<HttpResponse>
    where
        T: AsRef<str> + Sync + Send,
    {
        let websocket_url = url.as_ref();
        // start the websocket handshake with the downstream server
        let mut wsc = awc::Client::new().ws(websocket_url);

        let cookies = req.headers().get_all("cookie");
        for cookie in cookies {
            wsc = wsc.header("cookie", cookie.to_str().unwrap());
        }

        let (res, frame) = wsc.connect().await?;
        if !res.status().eq(&StatusCode::SWITCHING_PROTOCOLS) {
            return Err(GuardianError::GeneralError(
                "Failed to establish websocket connection".to_string(),
            ));
        }
        let mut io = frame.into_parts().io;

        // websocket handshake with the client
        let mut resp = actix_web_actors::ws::handshake(&req)?;
        // for (key, value) in res.headers().iter() {
        //     resp.insert_header((key.as_str(), value.as_bytes()));
        // }

        let (mut tx, rx) = unbounded();
        let mut buf = BytesMut::new();

        rt::spawn(async move {
            loop {
                tokio::select! {
                    // body from source.
                    res = pl.next() => {
                        match res {
                            None => return,
                            Some(body) => {
                                let body = body.unwrap();
                                io.write_all(&body).await.unwrap();
                            }
                        }
                    }

                    // body from dest.
                    res = io.read_buf(&mut buf) => {
                        let size = res.unwrap();
                        let bytes = buf.split_to(size).freeze();
                        tx.send(Ok::<_, actix_web::Error>(bytes)).await.unwrap();
                    }
                }
            }
        });

        Ok(resp.streaming(rx))
    }
}
