use std::any::{Any, TypeId};

use futures::TryStreamExt;
use mongodb::{bson::Document, Client, Collection};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    config::CONFIG,
    errors::{GuardianError, Result},
};

use super::Database;

pub struct DB<'d> {
    mongo_client: Client,
    database: &'d str,
}

impl<'d> DB<'d> {
    pub async fn new(database: &'d str) -> Result<Self> {
        let db = &CONFIG
            .get()
            .ok_or_else(|| {
                GuardianError::GeneralError("Could not obtain configuration".to_string())
            })?
            .db;
        let mongo_client = Client::with_uri_str(&db.url).await?;
        Ok(Self {
            mongo_client,
            database,
        })
    }

    #[inline]
    fn get_collection<Z>(d: &DB, collection: &str) -> Collection<Z>
    where
        Z: Send + Sync + Serialize + DeserializeOwned,
    {
        let db = d.mongo_client.database(d.database);
        db.collection::<Z>(collection)
    }
}

impl<'c, R1> Database<Document, &'c str, R1, TypeId, u64> for DB<'_>
where
    R1: Send + Sync + Serialize + DeserializeOwned,
{
    async fn find(&self, query: Document, collection: &'c str) -> Result<R1> {
        let col = Self::get_collection(self, collection);
        col.find_one(query)
            .await?
            .ok_or_else(|| GuardianError::GeneralError("Could not find any document".to_string()))
    }

    async fn find_many(&self, query: Document, collection: &'c str) -> Result<Vec<R1>> {
        let mut docs = Vec::new();
        let col = Self::get_collection(self, collection);
        let mut cursor = col.find(query).await?;

        while let Some(doc) = cursor.try_next().await? {
            docs.push(doc);
        }

        if docs.is_empty() {
            return Err(GuardianError::GeneralError(
                "Could not find any documents".to_string(),
            ));
        }
        Ok(docs)
    }

    async fn insert(&self, query: Document, collection: &'c str) -> Result<TypeId> {
        let col = Self::get_collection(self, collection);
        Ok(col.insert_one(query).await?.type_id())
    }

    async fn insert_many(&self, query: Vec<Document>, collection: &'c str) -> Result<Vec<TypeId>> {
        let mut types = Vec::new();
        let col = Self::get_collection(self, collection);
        let r = col.insert_many(query).await?;

        for id in r.inserted_ids {
            types.push(id.type_id());
        }
        Ok(types)
    }

    async fn update(
        &self,
        query: Document,
        update: Document,
        collection: &'c str,
    ) -> Result<TypeId> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.update_one(query, update).await?;
        Ok(r.type_id())
    }

    async fn update_many(
        &self,
        query: Document,
        update: Vec<Document>,
        collection: &'c str,
    ) -> Result<u64> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.update_many(query, update).await?;

        Ok(r.modified_count)
    }

    async fn delete(&self, filter: Document, collection: &'c str) -> Result<TypeId> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.delete_one(filter).await?;

        Ok(r.type_id())
    }

    async fn delete_many(&self, filter: Document, collection: &'c str) -> Result<u64> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.delete_many(filter).await?;

        Ok(r.deleted_count)
    }
}

#[cfg(test)]
mod tests {
    use mongodb::bson::doc;

    use crate::{config, db::models::User};

    use super::*;

    #[tokio::test]
    async fn test_mongo_connection() {
        config::init_once();
        let db = DB::new("guardian").await.unwrap();

        let result: User = db
            .find(
                doc! {
                    "sub": "choi@ibm.com"
                },
                "users",
            )
            .await
            .unwrap();
        assert_eq!(result.email, "choi@ibm.com");
    }
}
