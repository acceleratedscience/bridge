use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use reqwest::{Client, header::CONTENT_TYPE};
use serde_json::json;
use tokio::{
    sync::{
        broadcast::Sender as BSender,
        mpsc::{Sender, channel},
    },
    task::JoinHandle,
};
use tracing::{Subscriber, error, field::Visit, level_filters::LevelFilter, warn};
use tracing_subscriber::{
    Layer, Registry, filter,
    fmt::{self, MakeWriter, format},
    layer,
};
use url::Url;

use crate::{
    db::{
        Database,
        models::{OBSERVE, ObserveEventEntry},
        mongo::{DB, helper::i64_to_bson_datatime},
    },
    errors::{BridgeError, Result},
};

use super::futures::FutureRace;

pub const MESSAGE_DELIMITER: &str = "*~*~*";
pub const PERSIST_META: &str = "persist_to_db";
// 6 months
pub const PERSIST_TIME: i64 = 15778463;

pub struct Observe {
    sender: Sender<String>,
    handler: Option<JoinHandle<()>>,
}

pub struct ObserveEvents {
    sender: Sender<ObserveEventEntry>,
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

const CHANNEL_SIZE: usize = 150;

struct ObserveEventTrace {
    sub: Option<String>,
    // group: Option<String>,
    property: Option<String>,
    request_date: Option<i64>,
    expire_soon_after: Option<i64>,
}

impl Observe {
    /// Warning! Ensure you don't instrustment this with sensitive data, as it will be sent to the
    /// observability endpoint.
    pub fn new(api_key: &'static str, endpoint: &'static str, client: Client) -> Result<Self> {
        let (sender, mut recv) = channel(CHANNEL_SIZE);
        let endpoint = Url::parse(endpoint)
            .map_err(|_| BridgeError::GeneralError("Invalid URL".to_string()))?;

        let handler = tokio::spawn(async move {
            let endpoint = endpoint.as_str();
            let api_key = api_key;
            while let Some(msg) = recv.recv().await {
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

    pub fn send_message<T: ToString>(&self, msg: T) {
        if let Err(e) = self.sender.try_send(msg.to_string()) {
            match e {
                tokio::sync::mpsc::error::TrySendError::Full(_) => warn!(
                    "Observability channel is full, dropping message: {}",
                    msg.to_string()
                ),
                tokio::sync::mpsc::error::TrySendError::Closed(_) => error!(
                    "Observability channel is closed, cannot send message: {}",
                    msg.to_string()
                ),
            }
        }
    }

    pub async fn close(mut self) -> Result<()> {
        let handler = self.handler.take();
        drop(self);
        Ok(handler.unwrap().await?)
    }
}

impl ObserveEvents {
    pub fn new(db: &'static DB, tx: BSender<()>) -> Self {
        let (sender, recv) = channel(CHANNEL_SIZE);

        let handler = tokio::spawn(async move {
            let recv = Arc::new(Mutex::new(recv));
            let mut term_outer = tx.subscribe();
            loop {
                let mut term_inner = tx.subscribe();
                let get_events = FutureRace::new(recv.clone(), term_inner.recv());
                if let Some(events) = get_events.await {
                    if events.is_empty() {
                        continue;
                    }
                    if let Err(e) = db.insert_many(events, OBSERVE).await {
                        error!("Failed to insert observability events: {}", e);
                    }
                    // shutdown stop
                    if term_outer.try_recv().is_ok() {
                        break;
                    }
                }
            }
        });

        Self {
            sender,
            handler: Some(handler),
        }
    }

    pub fn send_message(&self, event: ObserveEventEntry) {
        let message = format!("{:?}", &event);
        if let Err(e) = self.sender.try_send(event) {
            match e {
                tokio::sync::mpsc::error::TrySendError::Full(_) => warn!(
                    "Observability channel is full, dropping message: {:?}",
                    message
                ),
                tokio::sync::mpsc::error::TrySendError::Closed(_) => error!(
                    "Observability channel is closed, cannot send message: {:?}",
                    message
                ),
            }
        }
    }

    #[allow(dead_code)]
    pub async fn close(mut self) -> Result<()> {
        let handler = self.handler.take();
        drop(self);
        Ok(handler.unwrap().await?)
    }
}

impl Write for &Observe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let message = String::from_utf8(buf.to_vec()).unwrap_or_default();
        if let Some(message) = message.split(MESSAGE_DELIMITER).nth(1)
            && !message.is_empty()
        {
            self.send_message(message);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // We write the data directly to the underlying sink, so no flush is needed.
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Observe {
    type Writer = &'a Observe;

    fn make_writer(&'a self) -> Self::Writer {
        self
    }
}

impl Visit for ObserveEventTrace {
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name() == "request_date" {
            self.request_date = Some(value);
        } else if field.name() == "expire_soon_after" {
            self.expire_soon_after = Some(value);
        } else {
            unreachable!("unimplemented field: {}", field.name());
        }
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "sub" => self.sub = Some(value.to_string()),
            // "group" => self.group = Some(value.to_string()),
            "property" => self.property = Some(value.to_string()),
            _ => unreachable!("unimplmented field: {}", field.name()),
        }
    }
    // NOOPS
    fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {}
}

impl<S> Layer<S> for ObserveEvents
where
    S: Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: layer::Context<'_, S>) {
        if event.metadata().target() == PERSIST_META {
            let mut trace = ObserveEventTrace {
                sub: None,
                // group: None,
                property: None,
                request_date: None,
                expire_soon_after: None,
            };
            event.record(&mut trace);

            if let (Some(request_date), Some(expire_soon_after)) =
                (trace.request_date, trace.expire_soon_after)
            {
                let entry = ObserveEventEntry {
                    sub: trace.sub.take().unwrap_or_default(),
                    // group: trace.group.take().unwrap_or_default(),
                    property: trace.property.take().unwrap_or_default(),
                    request_date: time::OffsetDateTime::from_unix_timestamp(request_date)
                        .unwrap_or(time::OffsetDateTime::now_utc()),
                    expire_soon_after: i64_to_bson_datatime(expire_soon_after),
                };
                self.send_message(entry);
            }
        }
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
