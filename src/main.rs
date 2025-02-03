use std::io::Result;

use mimalloc::MiMalloc;

use openbridge::web::start_server;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() -> Result<()> {
    start_server(true).await
}
