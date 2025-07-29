use reqwest::Client;
use tokio::sync::broadcast::Sender;
use tracing_subscriber::{filter::LevelFilter, prelude::*};

#[cfg(feature = "observe")]
mod futures;
#[cfg(feature = "observe")]
mod observability;
#[cfg(feature = "observe")]
pub use observability::MESSAGE_DELIMITER;
#[cfg(feature = "observe")]
pub use observability::PERSIST_META;

pub fn start_logger(level: LevelFilter, _client: Client, tx: Sender<()>) {
    // let file = std::fs::File::create("./log").unwrap();

    let ts = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .compact()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            // .with_writer(Arc::new(file))
            .with_filter(level),
    );

    #[cfg(feature = "observe")]
    let ts = {
        use crate::config::CONFIG;

        if let Some((ref api_key, ref endpoint)) = CONFIG.observability_cred {
            use crate::db::mongo::DBCONN;

            let writer = observability::Observe::new(api_key, endpoint, _client)
                .expect("Failed to create observability for logger");
            let observe_layer = observability::ObserveEvents::new(
                DBCONN.get().expect("DB connection not initialized"),
                tx,
            );

            ts.with(writer.wrap_layer(level)).with(observe_layer)
        } else {
            panic!("Observability credentials are not set in the configuration")
        }
    };

    ts.init()
}
