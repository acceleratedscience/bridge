use std::io::Result;

use jemallocator::Jemalloc;

use openbridge::web::start_server;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[tokio::main]
async fn main() -> Result<()> {
    start_server(true).await
}
