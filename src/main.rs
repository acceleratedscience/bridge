use guardian::web::start_server;

#[tokio::main]
async fn main() {
    let _ = start_server(true).await;
}
