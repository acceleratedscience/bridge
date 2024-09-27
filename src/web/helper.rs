use mongodb::bson::{to_bson, Bson};
use tera::Context;

use crate::{auth::jwt::validate_token, config::CONFIG, errors::GuardianError};

/// This macro logs the error, warn, info, or debug level of the error message.
/// Macro is used instead of a helper function to leverage debug symbols and print out line
/// numbers.
macro_rules! log_with_level {
    ($res:expr, error) => {{
        let result = $res;
        match result {
            Ok(_) => result,
            Err(ref e) => {
                tracing::error!("Error: {}", e);
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

pub fn delimited_string_to_vec(s: Vec<String>, delimiter: &str) -> Vec<String> {
    let col = Vec::new();
    (0..s.len()).fold(col, |mut acc, i| {
        s[i].split(delimiter).for_each(|s| {
            acc.push(s.to_string());
        });
        acc
    })
}

pub fn add_token_exp_to_tera(tera: &mut Context, token: &str) {
    let res = validate_token(token, &CONFIG.decoder, &CONFIG.validation);
    match res {
        Ok(claims) => {
            tera.insert("token_exp", &claims.token_exp_as_string());
        }
        Err(_) => {
            tera.insert("token_exp", "Not a valid token");
        }
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

        // sigh... this is a workaround due to reqwest and actix-web use different versions of the
        // http crate. At least we can use two versios of the http crate and not get stuck with
        // dependency hell like python.
        // Discussion on this can be found here: https://github.com/actix/actix-web/issues/3384
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
            forwarded_req
                .send()
                .await
                .map_err(|e| { GuardianError::GeneralError(e.to_string()) }),
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
            // because actix-web and reqwest use different versions of http. Once Actix-web
            // updates their http version, we can remove this.
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
        web::{self},
        HttpRequest, HttpResponse,
    };

    use futures::{SinkExt, StreamExt};
    use reqwest::StatusCode;
    use tokio_tungstenite::tungstenite::{self, handshake::client::Request};

    use crate::errors::{GuardianError, Result};

    pub async fn manage_connection<T>(
        req: HttpRequest,
        pl: web::Payload,
        url: T,
    ) -> Result<HttpResponse>
    where
        T: AsRef<str> + Sync + Send,
    {
        let websocket_url = url.as_ref();

        let mut request = Request::builder().uri(websocket_url);
        for (header_name, header_value) in req.headers().iter() {
            if let Ok(val) = header_value.to_str() {
                // this header causes some weird behavior over wss, so we ignore it for now
                if val == "v1.kernel.websocket.jupyter.org" {
                    continue;
                }
                request = request.header(header_name.to_string(), val);
            }
        }
        let request = request.body(()).unwrap();

        let (stream, res) = tokio_tungstenite::connect_async(request).await.unwrap();
        if !res.status().eq(&StatusCode::SWITCHING_PROTOCOLS) {
            return Err(GuardianError::GeneralError(
                "Failed to establish websocket connection".to_string(),
            ));
        }
        // downstream server
        let (mut w, mut s_stream) = stream.split();
        // client
        let (res, mut s, mut c_stream) = actix_ws::handle(&req, pl).unwrap();

        rt::spawn(async move {
            loop {
                tokio::select! {
                    // data to be sent to downstream server
                    resp = c_stream.next() => {
                        match resp {
                            Some(result) => {
                                if let Ok(msg) = result {
                                    match msg {
                                        actix_ws::Message::Text(t) => {
                                            let _ = log_with_level!(
                                                w.send(tungstenite::Message::Text(t.to_string())).await,
                                                error
                                            );
                                        }
                                        actix_ws::Message::Binary(b) => {
                                            let _ = log_with_level!(
                                                w.send(tungstenite::Message::Binary(b.to_vec())).await,
                                                error
                                            );
                                        }
                                        actix_ws::Message::Ping(p) => {
                                            let _ =
                                            log_with_level!(w.send(tungstenite::Message::Ping(p.to_vec())).await, error);
                                        }
                                        actix_ws::Message::Pong(p) => {
                                            let _ =
                                            log_with_level!(w.send(tungstenite::Message::Pong(p.to_vec())).await, error);
                                        }
                                        actix_ws::Message::Close(_) => {
                                            let _ = log_with_level!(w.send(tungstenite::Message::Close(None)).await, error);
                                            break;
                                        }
                                        _ => break,
                                    }
                                }
                            },
                            None => return,
                        }
                    }
                    // data to be sent to the client
                    resp = s_stream.next() => {
                        match resp {
                            Some(result) => {
                                if let Ok(msg) = result {
                                    match msg {
                                        tungstenite::Message::Text(t) => {
                                            let _ = log_with_level!(s.text(t).await, error);
                                        }
                                        tungstenite::Message::Binary(b) => {
                                            let _ = log_with_level!(s.binary(b).await, error);
                                        }
                                        tungstenite::Message::Pong(p) => {
                                            let _ = log_with_level!(s.pong(&p).await, error);
                                        }
                                        tungstenite::Message::Ping(p) => {
                                            let _ = log_with_level!(s.ping(&p).await, error);
                                        }
                                        tungstenite::Message::Close(_) => {
                                            let _ = log_with_level!(s.close(None).await, error);
                                            break;
                                        }
                                        _ => break,
                                    }
                                }
                            },
                            None => return,
                        }
                    }
                }
            }
        });

        // websocket handshake with the client
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delimited_string_to_vec() {
        let s = vec!["a,b,c".to_string(), "d,e,f".to_string()];
        let delimiter = ",";
        let res = delimited_string_to_vec(s, delimiter);
        assert_eq!(
            res,
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
                "f".to_string()
            ]
        );
    }
}
