#![allow(dead_code)]

use futures::{stream, Stream, StreamExt};
use reqwest::Client;
use url::Url;

struct InferenceServices<'a> {
    services: Vec<&'a Url>,
    client: Client,
}

impl<'a> InferenceServices<'a> {
    fn new(services: Vec<&Url>, client: Client) -> InferenceServices {
        InferenceServices { services, client }
    }

    fn steam(&'a self) -> impl Stream<Item = Result<bool, reqwest::Error>> + 'a {
        let requests = stream::iter(self.services.iter().map(|url| {
            let client = self.client.clone();
            async move {
                let response = client.get(url.as_str()).send().await?;
                Ok(response.status().is_success())
            }
        }));
        requests.buffer_unordered(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inference_services() {
        let url1 = Url::parse("https://postman-echo.com/get").unwrap(); // 200 response
        let url2 = Url::parse("https://postman-echo.com/hello").unwrap(); // 404 response
        let services = vec![&url1, &url2];
        let client = Client::new();
        let inference_services = InferenceServices::new(services, client);
        let stream = inference_services.steam();

        let results: Vec<Result<bool, reqwest::Error>> = stream.collect().await;
        assert_eq!(results.len(), 2);

        let mut ok_cnt = 0;
        for result in results {
            assert!(result.is_ok());
            dbg!(&result);
            if result.unwrap() {
                ok_cnt += 1;
            }
        }
        assert_eq!(ok_cnt, 1);
    }
}
