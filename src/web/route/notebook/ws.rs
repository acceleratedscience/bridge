pub mod websocket {
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
