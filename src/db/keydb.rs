use std::sync::LazyLock;

use actix_web::web::Bytes;
use redis::{
    aio::{MultiplexedConnection, PubSubStream},
    Client, Msg,
};

use crate::{config::CONFIG, errors::Result};

pub static CACHEDB: LazyLock<CacheDB> = LazyLock::new(CacheDB::init_once);

pub struct CacheDB {
    client: Client,
}

impl CacheDB {
    pub fn init_once() -> Self {
        let url = &CONFIG.cache.url;
        CacheDB {
            client: Client::open(url.clone()).expect("Failed to connect to cache"),
        }
    }

    pub async fn get_connection(&self) -> Result<MultiplexedConnection> {
        Ok(self.client.get_multiplexed_async_connection().await?)
    }

    pub async fn get_async_sub<T: AsRef<str>>(&self, channel: T) -> Result<PubSubStream> {
        let (mut sink, stream) = self.client.get_async_pubsub().await?.split();
        sink.subscribe(channel.as_ref()).await?;
        Ok(stream)
    }
}

#[derive(Debug, PartialEq)]
pub enum MaintenanceMSG {
    Start,
    Stop,
    None,
}

impl From<Msg> for MaintenanceMSG {
    fn from(msg: Msg) -> Self {
        let msg = msg.get_payload::<Bytes>().unwrap_or_default();

        match msg.as_ref() {
            b"start" => MaintenanceMSG::Start,
            b"stop" => MaintenanceMSG::Stop,
            _ => MaintenanceMSG::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use redis::AsyncCommands;
    use serde_json::json;

    // use futures::StreamExt;
    use super::*;

    #[tokio::test]
    async fn test_cache_db() {
        let cache = &CACHEDB;
        let result = cache.get_connection().await;
        assert!(result.is_ok());
        let mut conn = result.unwrap();

        let data_in = json!({"hello": ["world", "foo", "bar"]});
        let data_str = data_in.to_string();
        let _: () = conn.set_ex("test", data_str, 10).await.unwrap();
        let data_out: String = conn.get("test").await.unwrap();
        let data_out_value: serde_json::Value = serde_json::from_str(&data_out).unwrap();
        assert_eq!(data_in, data_out_value);
        println!("data_out: {}", data_out);

        // let mut stream = cache.get_async_sub("maintenance").await.unwrap();
        // let msg = stream.next().await.unwrap();
        // assert_ne!(Into::<MaintenanceMSG>::into(msg), MaintenanceMSG::None);
    }
}
