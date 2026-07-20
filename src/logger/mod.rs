use std::io::{self, BufWriter};
use std::sync::OnceLock;

use reqwest::Client;
use tokio::sync::broadcast::Sender;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{filter::LevelFilter, prelude::*};

#[cfg(feature = "observe")]
mod cloudwatch;
#[cfg(feature = "observe")]
mod futures;
#[cfg(feature = "observe")]
mod observability;
#[cfg(feature = "observe")]
pub use observability::{MESSAGE_DELIMITER, PERSIST_META, PERSIST_TIME};

// This stores the WorkerGuard to ensure that the logging continues even after the return of the
// start_logger fn. This also prevents a value being return to the caller. When the applicatio is
// shut down, the WorkerGuard will be dropped and the logging will stop.
static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

pub fn start_logger(level: LevelFilter, _client: Client, tx: Sender<()>) {
    // let file = std::fs::File::create("./log").unwrap();

    let (stdout, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .lossy(false)
        .finish(BufWriter::new(io::stdout()));

    let _ = LOG_GUARD.set(guard);

    #[cfg(not(feature = "observe"))]
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_file(true)
                .with_line_number(true)
                .with_thread_ids(true)
                // .with_writer(Arc::new(file))
                .with_writer(stdout)
                .with_filter(level),
        )
        .init();

    #[cfg(feature = "observe")]
    {
        use crate::config::CONFIG;

        if let Some((ref api_key, ref endpoint)) = CONFIG.observability_cred {
            use crate::db::mongo::DBCONN;

            let writer = observability::Observe::new(api_key, endpoint, _client)
                .expect("Failed to create observability for logger");
            let observe_layer = observability::ObserveEvents::new(
                DBCONN.get().expect("DB connection not initialized"),
                tx.clone(),
            );
            let cloudwatch = cloudwatch::CloudWatch::new(tx);

            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .compact()
                        .with_file(true)
                        .with_line_number(true)
                        .with_thread_ids(true)
                        .with_writer(stdout.and(writer).and(cloudwatch))
                        .with_filter(level),
                )
                .with(observe_layer)
                .init();
        } else {
            panic!("Observability credentials are not set in the configuration")
        }
    }
}
