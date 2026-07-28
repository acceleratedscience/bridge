use std::{
    io::Write,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aws_config::BehaviorVersion;
use aws_sdk_cloudwatchlogs::Client;
use parking_lot::Mutex;
use tokio::{
    sync::{
        broadcast::Sender as BSender,
        mpsc::{Sender, channel},
    },
    task::JoinHandle,
};
use tracing_subscriber::fmt::MakeWriter;

use super::futures::FutureBatch;
use crate::config::CONFIG;

static LOG_STREAM_NAME: &str = "bridge";

pub struct CloudWatch {
    sender: Sender<(i64, String)>,
    handler: Option<JoinHandle<()>>,
}

impl CloudWatch {
    pub fn new(tx: BSender<()>) -> Self {
        let (sender, recv) = channel(100);

        let handler = tokio::spawn(async move {
            let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
            let client = aws_sdk_cloudwatchlogs::Client::new(&config);

            // try to create log group if it doesn't exist
            if let Err(e) = client
                .create_log_group()
                .log_group_name(&CONFIG.log_group)
                .send()
                .await
                && !e
                    .as_service_error()
                    .is_some_and(|se| se.is_resource_already_exists_exception())
            {
                eprintln!("Failed to create log group: {:?}", e);
            }

            // try to create log stream if it doesn't exist
            if let Err(e) = client
                .create_log_stream()
                .log_group_name(&CONFIG.log_group)
                .log_stream_name(LOG_STREAM_NAME)
                .send()
                .await
                && !e
                    .as_service_error()
                    .is_some_and(|se| se.is_resource_already_exists_exception())
            {
                eprintln!("Failed to create log stream: {:?}", e);
            }

            let recv = Arc::new(Mutex::new(recv));
            let mut term_outer = tx.subscribe();
            let mut term_inner = tx.subscribe();

            loop {
                let get_events =
                    FutureBatch::new(recv.clone(), term_outer.recv(), Duration::from_secs(30));

                if let Some(events) = get_events.await {
                    if events.is_empty() {
                        continue;
                    }

                    if let Err(e) = CloudWatch::send_batch(&client, events).await {
                        eprintln!("Failed to send log events: {:?}", e);
                    }

                    // shutdown stop
                    if term_inner.try_recv().is_ok() {
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

    async fn send_batch(client: &Client, batch: Vec<(i64, String)>) -> Result<(), String> {
        client
            .put_log_events()
            .log_group_name(&CONFIG.log_group)
            .log_stream_name(LOG_STREAM_NAME)
            .set_log_events(Some(
                batch
                    .into_iter()
                    .filter_map(|v| {
                        // packaging
                        aws_sdk_cloudwatchlogs::types::InputLogEvent::builder()
                            .timestamp(v.0)
                            .message(v.1)
                            .build()
                            .ok()
                    })
                    .collect(),
            ))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    #[allow(dead_code)]
    async fn close(mut self) -> crate::errors::Result<()> {
        let handler = self.handler.take();
        drop(self);
        Ok(handler.unwrap().await?)
    }
}

impl<'a> MakeWriter<'a> for CloudWatch {
    type Writer = &'a Self;

    fn make_writer(&'a self) -> Self::Writer {
        self
    }
}

impl Write for &CloudWatch {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // UNIX_EPOCH as i64 should not lead to data truncation
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            // only possible error if clock is pre-1970
            .unwrap_or(Duration::ZERO)
            .as_millis()
            .try_into()
            .unwrap_or(i64::MAX);

        if let Err(e) = self
            .sender
            .try_send((timestamp, String::from_utf8_lossy(buf).to_string()))
        {
            eprintln!("Failed to send log event: {}", e);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
