use std::{error::Error, sync::Arc, time::Duration};

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};

mod forward;
mod ws;

pub use forward::forward;

#[derive(Debug, Clone)]
#[allow(clippy::complexity)]
pub struct ProxyClient(
    Arc<Client<HttpsConnector<HttpConnector>, BoxBody<Bytes, Box<dyn Error + Send + Sync>>>>,
);

impl ProxyClient {
    pub fn new() -> Self {
        let mut http = HttpConnector::new();
        // disable Nagle's algorithm for lower latency
        http.set_nodelay(true);
        http.enforce_http(false);
        // Some proxy for inference take several minutes..
        http.set_connect_timeout(Some(Duration::from_hours(1)));

        let https = HttpsConnectorBuilder::new()
            .with_native_roots()
            .expect("Failed to create HTTPS connector")
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .wrap_connector(http);

        Self(Arc::new(
            Client::builder(TokioExecutor::new())
                .pool_idle_timeout(Duration::from_mins(2))
                .pool_max_idle_per_host(32)
                .http2_adaptive_window(true)
                .build(https),
        ))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::Buf;
    use http_body_util::{BodyExt, Full};
    use hyper::Request;
    use serde_json::json;

    #[tokio::test]
    async fn test_proxy_client() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("Cannot install default provider with ring");

        let client = ProxyClient::new();
        let req = Request::builder()
            .method(http::Method::GET)
            .uri("https://postman-echo.com/get")
            .header("Hello", "World")
            .body(BoxBody::new(
                Full::new(Bytes::from(
                    json!({ "test": "data" }).to_string().into_bytes(),
                ))
                .map_err(|never| match never {}),
            ))
            .expect("Failed to build request");

        let res = client.0.request(req).await.expect("Request failed");
        assert_eq!(res.status(), 200);

        let bod = res
            .collect()
            .await
            .expect("Failed to collect response body")
            .aggregate();
        println!(
            "Response body: {:?}",
            serde_json::from_reader::<_, serde_json::Value>(bod.reader())
        );
    }
}
