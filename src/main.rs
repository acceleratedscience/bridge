use guardian::config;
use guardian::web::services::{self};
use guardian::{logger::Logger, web::start_server};
use tracing_subscriber::filter::LevelFilter;

#[tokio::main]
async fn main() {
    if cfg!(debug_assertions) {
        Logger::start(LevelFilter::INFO);
    } else {
        Logger::start(LevelFilter::WARN);
    }

    config::init_once();
    services::init_once();

    let _ = start_server(true).await;
}
