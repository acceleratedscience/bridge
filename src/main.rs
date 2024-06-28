use guardian::{
    auth::openid, config, logger::Logger, web::{services, start_server}
};
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
    openid::init_once().await;

    let _ = start_server(true).await;
}
