use reqwest::Client;
use tracing_subscriber::{filter::LevelFilter, prelude::*};

use crate::config::CONFIG;

#[cfg(feature = "observe")]
mod observability;

pub fn start_logger(level: LevelFilter, _client: Client) {
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
        if let Some((ref api_key, ref endpoint)) = CONFIG.observability_cred {
            let writer = observability::Observe::new(api_key, endpoint, _client)
                .expect("Failed to create observability for logger");
            ts.with(writer.wrap_layer(level))
        } else {
            panic!("Observability credentials are not set in the configuration")
        }
    };

    ts.init()
}
