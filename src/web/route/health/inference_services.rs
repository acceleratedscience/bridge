use std::time::{Duration, Instant};

use futures::{stream, Stream, StreamExt};
use reqwest::Client;
use tokio::time::timeout;
use url::Url;

use crate::errors::{GuardianError, Result};

pub struct InferenceServicesHealth<'a> {
    services: &'a Vec<(Url, String)>,
    client: Client,
}

pub struct ListBuilder<'a> {
    outer_body: (&'a str, &'a str),
    inner_body: String,
}

impl<'a> InferenceServicesHealth<'a> {
    pub fn new(services: &'a Vec<(Url, String)>, client: Client) -> Self {
        InferenceServicesHealth { services, client }
    }

    pub fn builder(&self) -> ListBuilder {
        let outer_body = (r##"<div class="status-card small">"##, r##"</div>"##);
        ListBuilder {
            outer_body,
            inner_body: String::new(),
        }
    }

    pub fn create_stream(&'a self) -> impl Stream<Item = Result<(bool, String, u128)>> + 'a {
        let requests = stream::iter(self.services.iter().map(|(url, name)| {
            let client = self.client.clone();
            async move {
                let now = Instant::now();
                let fut = client.get(url.as_str()).send();
                let response = timeout(Duration::from_secs(1), fut).await.map_err(|_| {
                    GuardianError::GeneralError("Call to inference service timed out".to_string())
                })??;
                let elapsed = now.elapsed();
                Ok((
                    response.status().is_success(),
                    name.clone(),
                    elapsed.as_millis(),
                ))
            }
        }));
        requests.buffer_unordered(5)
    }
}

impl ListBuilder<'_> {
    pub fn add_inner_body(&mut self, up: bool, name: &str, elapsed: u128) {
        let status = if up { "up" } else { "down" };
        let state = if elapsed.gt(&500) {
            "status-danger"
        } else {
            "status-success"
        };

        self.inner_body.push_str(&format!(
            r##"<div><b>{}</b></div>
				<div>Service is currently {}.</div>
				<div class="{}">{} ms</div>"##,
            name, status, state, elapsed
        ));
    }

    pub fn render(&self) -> String {
        format!(
            r##"{}{}{}"##,
            self.outer_body.0, self.inner_body, self.outer_body.1
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inference_services() {
        let url1 = Url::parse("https://postman-echo.com/get").unwrap(); // 200 response
        let url2 = Url::parse("https://postman-echo.com/hello").unwrap(); // 404 response
        let services = vec![(url1, "200".to_string()), (url2, "400".to_string())];
        let client = Client::new();
        let inference_services = InferenceServicesHealth::new(&services, client);
        let stream = inference_services.create_stream();

        let results: Vec<Result<(bool, String, u128), reqwest::Error>> = stream.collect().await;
        assert_eq!(results.len(), 2);

        let mut ok_cnt = 0;
        for result in results {
            assert!(result.is_ok());
            if result.unwrap().0 {
                ok_cnt += 1;
            }
        }
        assert_eq!(ok_cnt, 1);
    }
}
