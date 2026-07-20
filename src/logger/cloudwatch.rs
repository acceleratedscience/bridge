use std::{
    io::Write,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aws_config::BehaviorVersion;
use aws_sdk_cloudwatchlogs::Client;
use crossbeam::channel::{Receiver, bounded};
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

pub struct CloudWatch {
    sender: Sender<(i64, Option<Vec<u8>>)>,
    pool_recv: Receiver<Vec<u8>>,
    handler: Option<JoinHandle<()>>,
}

impl CloudWatch {
    pub fn new(tx: BSender<()>) -> Self {
        let (sender, recv) = channel(100);
        let (pool_sender, pool_recv) = bounded(100);

        let handler = tokio::spawn(async move {
            let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
            let client = aws_sdk_cloudwatchlogs::Client::new(&config);

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

                    if let Err(e) = CloudWatch::send_batch(&client, &events).await {
                        eprintln!("Failed to send log events: {}", e);
                    }

                    // shutdown stop
                    if term_inner.try_recv().is_ok() {
                        break;
                    }

                    // prune and send pre-allocated events back to pool
                    for mut event in events {
                        let mut inner_event = event.1.take().unwrap_or(Vec::with_capacity(1024));
                        inner_event.clear();
                        // if the send fails or w/e reason, we just deallocate
                        let _ = pool_sender.try_send(inner_event);
                    }
                }
            }
        });

        Self {
            sender,
            handler: Some(handler),
            pool_recv,
        }
    }

    async fn send_batch(
        client: &Client,
        batch: &[(i64, Option<Vec<u8>>)],
    ) -> Result<(), String> {
        client
            .put_log_events()
            .log_group_name(&CONFIG.log_group)
            .log_stream_name("bridge")
            .set_log_events(Some(
                batch
                    .iter()
                    .filter_map(|v| {
                        // packaging
                        aws_sdk_cloudwatchlogs::types::InputLogEvent::builder()
                            .timestamp(v.0)
                            .message(String::from_utf8_lossy(
                                // This should always be full of data
                                v.1.as_ref().expect("No slice of bytes"),
                            ))
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

        let mut payload = self
            .pool_recv
            .try_recv()
            .unwrap_or_else(|_| Vec::with_capacity(buf.len()));

        payload.extend_from_slice(buf);
        if let Err(e) = self.sender.try_send((timestamp, Some(payload))) {
            eprintln!("Failed to send log event: {}", e);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
