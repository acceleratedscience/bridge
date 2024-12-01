use std::io::Result;

use guardian::web::start_server;

#[tokio::main]
async fn main() -> Result<()> {
    start_server(true).await
}
