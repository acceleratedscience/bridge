use std::io::{self, Write};

use reqwest::{Client, header::CONTENT_TYPE};
use serde_json::json;
use tokio::{
    sync::mpsc::{Sender, channel},
    task::JoinHandle,
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    Layer, Registry, filter,
    fmt::{self, MakeWriter, format},
    layer,
};
use url::Url;

use crate::errors::{BridgeError, Result};

pub const MESSAGE_DELIMITER: &str = "*~*~*";

pub struct Observe {
    sender: Sender<String>,
    handler: Option<JoinHandle<()>>,
}

type LayerAlias = filter::Filtered<
    fmt::Layer<
        layer::Layered<
            filter::Filtered<
                fmt::Layer<Registry, format::DefaultFields, format::Format<format::Compact>>,
                LevelFilter,
                Registry,
            >,
            Registry,
        >,
        format::DefaultFields,
        format::Format<format::Compact>,
        Observe,
    >,
    LevelFilter,
    layer::Layered<
        filter::Filtered<
            fmt::Layer<Registry, format::DefaultFields, format::Format<format::Compact>>,
            LevelFilter,
            Registry,
        >,
        Registry,
    >,
>;

const CHANNEL_SIZE: usize = 100;

impl Observe {
    pub fn new(api_key: &'static str, endpoint: &'static str, client: Client) -> Result<Self> {
        let (sender, mut recv) = channel(CHANNEL_SIZE);
        let endpoint = Url::parse(endpoint)
            .map_err(|_| BridgeError::GeneralError("Invalid URL".to_string()))?;

        let handler = tokio::spawn(async move {
            let endpoint = endpoint.as_str();
            let api_key = api_key;
            while let Some(msg) = recv.recv().await {
                println!("Sending observability message: {}", msg);
                let msg = json!(
                    {
                        "text": msg
                    }
                )
                .to_string();
                let _ = client
                    .post(endpoint)
                    .bearer_auth(api_key)
                    .header(CONTENT_TYPE, "application/json")
                    .body(msg)
                    .send()
                    .await;
            }
        });

        Ok(Observe {
            sender,
            handler: Some(handler),
        })
    }

    pub fn wrap_layer(self, level: LevelFilter) -> LayerAlias {
        fmt::layer()
            .compact()
            .with_file(true)
            .with_line_number(true)
            .with_writer(self)
            .with_filter(level)
    }

    pub fn send_message<T: ToString>(&self, msg: T) -> Result<()> {
        Ok(self.sender.blocking_send(msg.to_string())?)
    }

    pub async fn close(mut self) -> Result<()> {
        let handler = self.handler.take();
        drop(self);
        Ok(handler.unwrap().await?)
    }
}

impl Write for &Observe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let message = String::from_utf8(buf.to_vec()).unwrap_or_default();
        if let Some(message) = message.split(MESSAGE_DELIMITER).nth(1) {
            if !message.is_empty() {
                self.send_message(message)
                    .map_err(|_| io::Error::other("Failed to send message for observability"))?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Implement the flush logic here
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Observe {
    type Writer = &'a Observe;

    fn make_writer(&'a self) -> Self::Writer {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_observability() {
        let observe = Observe::new("api-key", "https://api.slack.com/api", Client::new()).unwrap();
        let mut writer = &observe;
        let _ = writer
            .write(b"Nothing*~*~*Hello there from open bridge")
            .unwrap();

        assert!(observe.close().await.is_ok());
    }
}
