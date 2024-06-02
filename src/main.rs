use guardian::{logger::Logger, web::start_server};
use tracing_subscriber::filter::LevelFilter;

#[tokio::main]
async fn main() {
    if cfg!(debug_assertions) {
        Logger::start(LevelFilter::DEBUG);
    } else {
        Logger::start(LevelFilter::WARN);
    }
    let _ = start_server(false).await;
}
