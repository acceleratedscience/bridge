use std::process::exit;

use guardian::{
    auth::openid,
    db::mongo::DB,
    logger::Logger,
    web::{services, start_server},
};
use tracing::error;
use tracing_subscriber::filter::LevelFilter;

#[tokio::main]
async fn main() {

    if cfg!(debug_assertions) {
        Logger::start(LevelFilter::INFO);
    } else {
        Logger::start(LevelFilter::WARN);
    }

    services::init_once();
    openid::init_once().await;
    if let Err(e) = DB::init_once("guardian").await {
        error!("{e}");
        exit(1);
    }

    let _ = start_server(true).await;
}
