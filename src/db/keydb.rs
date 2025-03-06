use std::sync::OnceLock;

use actix_web::web::Bytes;
use redis::{
    AsyncCommands as _, Client, Msg,
    aio::{MultiplexedConnection, PubSubStream},
};

use crate::{config::CONFIG, errors::Result};

pub static CACHEDB: OnceLock<CacheDB> = OnceLock::new();

pub struct CacheDB {
    client: Client,
    conn: MultiplexedConnection,
}

impl CacheDB {
    pub async fn init_once() -> Result<()> {
        let url = &CONFIG.cache.url;
        let client = Client::open(url.clone()).expect("Failed to connect to cache");
        let conn = client.get_multiplexed_async_connection().await?;
        CACHEDB.get_or_init(|| CacheDB { client, conn });
        Ok(())
    }

    pub fn get_connection(&self) -> MultiplexedConnection {
        self.conn.clone()
    }

    pub async fn get_async_sub<T: AsRef<str>>(&self, channel: T) -> Result<PubSubStream> {
        let (mut sink, stream) = self.client.get_async_pubsub().await?.split();
        sink.subscribe(channel.as_ref()).await?;
        Ok(stream)
    }

    pub async fn set_session_id<T: AsRef<str>>(
        &self,
        session_id: T,
        sub: T,
        expiration: u64,
    ) -> Result<()> {
        let session_id = session_id.as_ref();
        let sub = sub.as_ref();
        let _: () = self
            .get_connection()
            .set_ex(format!("session:{}", sub), session_id, expiration)
            .await?;
        Ok(())
    }

    pub async fn get_session_id<T: AsRef<str>>(&self, sub: T) -> Result<String> {
        let sub = sub.as_ref();
        let session_id: String = self
            .get_connection()
            .get(format!("session:{}", sub))
            .await?;
        Ok(session_id)
    }

    pub async fn del_session_id<T: AsRef<str>>(&self, sub: T) -> Result<()> {
        let sub = sub.as_ref();
        let _: () = self
            .get_connection()
            .del(format!("session:{}", sub))
            .await?;
        Ok(())
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
        CacheDB::init_once().await.unwrap();
        let result = CACHEDB.get();
        assert!(result.is_some());
        let cache = result.unwrap();
        let mut conn = cache.get_connection();

        let data_in = json!({"hello": ["world", "foo", "bar"]});
        let data_str = data_in.to_string();
        let _: () = conn.set_ex("test", data_str, 10).await.unwrap();
        let data_out: String = conn.get("test").await.unwrap();
        let data_out_value: serde_json::Value = serde_json::from_str(&data_out).unwrap();
        assert_eq!(data_in, data_out_value);
        println!("data_out: {}", data_out);

        let not_exist: Option<String> = conn.get("nothing").await.unwrap();
        println!("{:?}", not_exist);

        // let mut stream = cache.get_async_sub("maintenance").await.unwrap();
        // let msg = stream.next().await.unwrap();
        // assert_ne!(Into::<MaintenanceMSG>::into(msg), MaintenanceMSG::None);
    }
}
