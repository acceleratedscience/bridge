use tracing_subscriber::{filter::LevelFilter, prelude::*};

pub struct Logger;

impl Logger {
    pub fn start(level: LevelFilter) {
        // let file = std::fs::File::create("./log").unwrap();
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_thread_ids(true)
                    // .with_writer(Arc::new(file))
                    .with_filter(level),
            )
            .init()
    }
}
