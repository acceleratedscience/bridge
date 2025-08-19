use std::time::{Duration, Instant};

use futures::{Stream, StreamExt, stream};
use num_bigint::BigUint;
use redis::AsyncCommands;
use reqwest::Client;
use tokio::time::timeout;
use url::Url;

use crate::{
    db::keydb::CacheDB,
    errors::{BridgeError, Result},
};

pub struct InferenceServicesHealth<'a> {
    services: &'a Vec<(Url, String)>,
    client: Client,
    cache: Option<&'a CacheDB>,
}

pub struct ListBuilder<'a> {
    outer_body: (&'a str, &'a str),
    inner_body: String,
}

impl<'a> InferenceServicesHealth<'a> {
    pub fn new(
        services: &'a Vec<(Url, String)>,
        client: Client,
        cache: Option<&'a CacheDB>,
    ) -> Self {
        InferenceServicesHealth {
            services,
            client,
            cache,
        }
    }

    pub fn builder(&self) -> ListBuilder<'_> {
        let outer_body = (r##"<div class="status-card small">"##, r##"</div>"##);
        ListBuilder {
            outer_body,
            inner_body: String::new(),
        }
    }

    pub fn create_stream(
        &'a self,
    ) -> impl Stream<Item = Result<(bool, &'a String, u128)>> + use<'a> {
        let requests = stream::iter(self.services.iter().map(|(url, name)| {
            let client = self.client.clone();
            let cache = self.cache.map(|c| c.get_connection());

            // If caching is available, we hit the cache first and immediately return the
            // value if it exists. Otherwise, we make the request. We only cache if caching is
            // available.
            async move {
                if let Some(mut conn) = cache.clone()
                    && let Some(t) = conn
                        .get::<'_, _, Option<u128>>(String::from("health:") + name)
                        .await?
                {
                    return Ok((true, name, t));
                }

                let now = Instant::now();

                let fut = client.get(url.as_str()).send();
                let response = timeout(Duration::from_secs(10), fut).await.map_err(|_| {
                    BridgeError::GeneralError("Call to inference service timed out".to_string())
                })??;

                let elapsed = now.elapsed();

                if let Some(mut conn) = cache {
                    let big_uint = BigUint::from(elapsed.as_millis());
                    conn.set_ex::<'_, _, _, ()>(String::from("health:") + name, big_uint, 60 * 30)
                        .await?;
                }

                Ok((response.status().is_success(), name, elapsed.as_millis()))
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
            r##"<div><b>{name}</b></div>
				<div>Service is currently {status}.</div>
				<div class="{state}">{elapsed} ms</div>"##
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
        let inference_services = InferenceServicesHealth::new(&services, client, None);
        let stream = inference_services.create_stream();

        let results = stream.collect::<Vec<Result<(bool, &String, u128)>>>().await;
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
