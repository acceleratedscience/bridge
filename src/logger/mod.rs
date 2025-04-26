use tracing_subscriber::{filter::LevelFilter, prelude::*};

mod observability;

pub fn start_logger(level: LevelFilter) {
    // let file = std::fs::File::create("./log").unwrap();
    // let writer = observability::Observe::new("".to_string(), "".to_string());

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_file(true)
                .with_line_number(true)
                .with_thread_ids(true)
                // .with_writer(Arc::new(file))
                .with_filter(level),
        )
        // .with(writer.wrap_layer(level))
        .init()
}
