use guardian::{logger::Logger, web::start_server};
use tracing_subscriber::filter::LevelFilter;

#[tokio::main]
async fn main() {
    if cfg!(debug_assertions) {
        Logger::start(LevelFilter::INFO);
    } else {
        Logger::start(LevelFilter::WARN);
    }
    let _ = start_server(true).await;
}
