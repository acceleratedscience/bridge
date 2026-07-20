use actix_web::{cookie::Cookie, web};
use base64ct::{Base64, Encoding};
use mongodb::bson::{Bson, to_bson};
use rand::{RngExt as _, rng};
use serde::Deserialize;
use tera::Context;
use tokio_stream::StreamExt;
#[cfg(feature = "observe")]
use tracing::instrument;
use tracing::{error, info, warn};

#[cfg(feature = "observe")]
use crate::db::models::BridgeCookie;
use crate::{
    auth::jwt::validate_token,
    config::CONFIG,
    db::keydb::{CACHEDB, MaintenanceMSG},
    errors::{BridgeError, Result},
};

/// This macro logs the error, warn, info, or debug level of the error message.
/// Macro is used instead of a helper function to leverage debug symbols and print out line
/// numbers.
#[macro_export]
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

use super::bridge_middleware::MAINTENANCE_WINDOWS;

pub fn bson<T>(t: T) -> Result<Bson>
where
    T: serde::Serialize,
{
    match to_bson(&t) {
        Ok(bson) => Ok(bson),
        Err(e) => Err(BridgeError::GeneralError(e.to_string())),
    }
}

/// Convert a vector of strings that are delimited by a character to a vector of strings
///
/// # Example
/// ```ignore
/// let s = vec!["a,b,c".to_string(), "d,e,f".to_string()];
/// let delimiter = ",";
/// let res = delimited_string_to_vec(s, delimiter);
/// assert_eq!(res, vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string(),
/// "e".to_string(), "f".to_string()]);
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

#[instrument(skip_all, parent = None)]
#[cfg(feature = "observe")]
#[inline]
pub fn observability_post(msg: &'static str, bc: &BridgeCookie) {
    use crate::logger::MESSAGE_DELIMITER;

    let user = &bc.subject;
    let user_type = &bc.user_type;
    info!(
        "{}User: {} with type: {:?} {} for {}",
        MESSAGE_DELIMITER, user, user_type, msg, CONFIG.company
    );
}

pub(super) async fn payload_to_struct<T>(mut payload: web::Payload) -> Result<T>
where
    T: Deserialize<'static>,
{
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.unwrap();
        body.extend_from_slice(&chunk);
    }
    let body = String::from_utf8_lossy(&body);
    let deserializer = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(&body);
    Ok(log_with_level!(T::deserialize(deserializer), error)?)
}

/// Watch for pubsub message to determine if maintenance window is active. This runs in the
/// background and will not block the main thread. Nothing is persisted here, so no graceful exit
/// needed.
pub fn maintenance_watch() -> Result<()> {
    let cache = if let Some(c) = CACHEDB.get() {
        c
    } else {
        warn!("Cache not initialized. Maintenance window will not be watched.");
        // the caller can ignore this error
        return Ok(());
    };

    tokio::spawn(async move {
        let mut stream = match cache.get_async_sub("maintenance").await {
            Ok(stream) => stream,
            Err(e) => {
                error!("{:?}", e);
                return;
            }
        };

        while let Some(msg) = stream.next().await {
            match Into::<MaintenanceMSG>::into(msg) {
                MaintenanceMSG::Start => {
                    info!("Maintenance window started");
                    let mut mw = MAINTENANCE_WINDOWS.write();
                    *mw = true;
                }
                MaintenanceMSG::Stop => {
                    info!("Maintenance window stopped");
                    let mut mw = MAINTENANCE_WINDOWS.write();
                    *mw = false;
                }
                _ => (),
            }
        }
    });

    Ok(())
}

#[inline]
pub fn generate_salt() -> String {
    let mut rng = rng();
    let salt: Vec<u8> = (0..32).map(|_| rng.random()).collect();
    Base64::encode_string(&salt)
}

#[derive(Default)]
pub struct Config<'a> {
    pub inference: bool,
    pub pack_cookies: bool,
    pub updated_cookie: Option<Cookie<'a>>,
}

#[cfg(feature = "notebook")]
pub mod utils {
    use std::{marker::PhantomData, ops::Deref};

    use k8s_openapi::api::core::v1::PersistentVolumeClaim;
    use mongodb::bson::{Bson, Document, doc};

    use crate::{
        db::{
            Database,
            models::{USER, User},
            mongo::ObjectID,
        },
        errors::Result,
        kube::{KubeAPI, NOTEBOOK_NAMESPACE, Notebook},
        web::{helper::bson, notebook_helper},
    };

    #[inline]
    // Once this issue is fixed with compiler https://github.com/rust-lang/rust/issues/64552, can
    // relax C = &'static str to C<'a> = &'a str
    pub async fn notebook_destroy<O, I>(
        db: O,
        subject: &str,
        persist_pvc: bool,
        user: &str,
    ) -> Result<()>
    where
        O: Deref<Target = I>,
        I: for<'a> Database<
                User,
                Q = Document,
                N<'a> = &'a str,
                C = &'static str,
                R2 = Bson,
                R3 = u64,
            >,
        // pub async fn notebook_destroy(db: &DB, subject: &str, pvc: bool, user: &str) -> Result<()>
    {
        let name = notebook_helper::make_notebook_name(subject);
        let pvc_name = notebook_helper::make_notebook_volume_name(subject);
        log_with_level!(
            KubeAPI::<Notebook>::delete(&name, *NOTEBOOK_NAMESPACE).await,
            error
        )?;
        if !persist_pvc {
            log_with_level!(
                KubeAPI::<PersistentVolumeClaim>::delete(&pvc_name, *NOTEBOOK_NAMESPACE).await,
                error
            )?;
        }
        // TODO: add last_updated_by
        db.update(
            doc! {
                "_id": ObjectID::new(subject).into_inner(),
            },
            doc! {
                "$set": doc! {
                    "updated_at": bson(time::OffsetDateTime::now_utc())?,
                    "notebook": null,
                    "last_updated_by": user,
                },
            },
            USER,
            PhantomData::<User>,
        )
        .await?;

        Ok(())
    }
}

/// Websocket proxying utilities
pub mod ws {
    use actix_web::{
        HttpRequest, HttpResponse, rt,
        web::{self},
    };

    use actix_ws::Item;
    use futures::{SinkExt, StreamExt};
    use reqwest::StatusCode;
    use tokio_tungstenite::tungstenite::{
        self,
        handshake::client::Request,
        protocol::{
            WebSocketConfig,
            frame::coding::{Data, OpCode},
        },
    };
    use tracing::{error, warn};

    use crate::errors::{BridgeError, Result};

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

        let (stream, res) = tokio_tungstenite::connect_async_with_config(
            request,
            Some(WebSocketConfig::default().accept_unmasked_frames(true)),
            false,
        )
        .await
        .map_err(|e| {
            error!("WebSocket connection error: {:?}", e);
            BridgeError::GeneralError(e.to_string())
        })?;
        if !res.status().eq(&StatusCode::SWITCHING_PROTOCOLS) {
            return Err(BridgeError::GeneralError(
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
                                                w.send(tungstenite::Message::Text(t.to_string().into())).await,
                                                error
                                            );
                                        }
                                        actix_ws::Message::Binary(b) => {
                                            let _ = log_with_level!(
                                                w.send(tungstenite::Message::Binary(b.to_vec().into())).await,
                                                error
                                            );
                                        }
                                        actix_ws::Message::Ping(p) => {
                                            let _ =
                                            log_with_level!(w.send(tungstenite::Message::Ping(p.to_vec().into())).await, error);
                                        }
                                        actix_ws::Message::Pong(p) => {
                                            let _ =
                                            log_with_level!(w.send(tungstenite::Message::Pong(p.to_vec().into())).await, error);
                                        }
                                        actix_ws::Message::Close(_) => {
                                            let _ = log_with_level!(w.send(tungstenite::Message::Close(None)).await, error);
                                            let _ = log_with_level!(w.close().await, error);
                                            let _ = log_with_level!(s.close(None).await, error);
                                            break;
                                        }
                                        _ => {
                                            let _ = log_with_level!(w.close().await, error);
                                            let _ = log_with_level!(s.close(None).await, error);
                                            break;
                                        }
                                    }
                                }
                            },
                            None => {
                                let _ = log_with_level!(w.close().await, error);
                                let _ = log_with_level!(s.close(None).await, error);
                                break;
                            }
                        }
                    }
                    // data to be sent to the client
                    resp = s_stream.next() => {
                        match resp {
                            Some(result) => {
                                match result {
                                    Ok(msg) => {
                                        match msg {
                                            tungstenite::Message::Text(t) => {
                                                let _ = log_with_level!(s.text(t.as_str()).await, error);
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
                                                tracing::warn!("Closing connection");
                                                let _ = log_with_level!(s.close(None).await, error);
                                                let _ = log_with_level!(w.close().await, error);
                                                break;
                                            }
                                            tungstenite::Message::Frame(frame) => {
                                                tracing::warn!("Frame: {:?}", frame);
                                                let header = frame.header().opcode;
                                                if let OpCode::Data(Data::Continue) = header {
                                                    let _ = log_with_level!(s.continuation(Item::Continue(frame.into_payload())).await, error);
                                                } else {
                                                    let _ = log_with_level!(s.close(None).await, error);
                                                    let _ = log_with_level!(w.close().await, error);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Error: {:?}", e);
                                        let _ = log_with_level!(s.close(None).await, error);
                                        let _ = log_with_level!(w.close().await, error);
                                        break;
                                    },
                                }
                            },
                            None => {
                                warn!("Closing connection due to None... connection possibly closed.");
                                let _ = log_with_level!(s.close(None).await, error);
                                let _ = log_with_level!(w.close().await, error);
                                break;
                            }
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
