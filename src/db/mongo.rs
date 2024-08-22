use std::{marker::PhantomData, sync::OnceLock};

use futures::{StreamExt, TryStreamExt};
use mongodb::{
    bson::{doc, Bson, Document},
    options::IndexOptions,
    Client, Collection, Database as MongoDatabase, IndexModel,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    config::CONFIG,
    errors::{GuardianError, Result},
};

use super::{
    models::{Group, User, GROUP, USER},
    Database,
};

#[derive(Clone)]
pub struct DB {
    mongo_database: MongoDatabase,
}

pub static DBCONN: OnceLock<DB> = OnceLock::new();

static COLLECTIONS: [&str; 2] = [USER, GROUP];

impl DB {
    pub async fn init_once(database: &'static str) -> Result<()> {
        let db = &CONFIG.db;
        let mongo_database = Client::with_uri_str(&db.url).await?.database(database);

        // check if the collections exists, if not create them
        let all_collection = mongo_database.list_collection_names().await?;
        for collection in COLLECTIONS.iter() {
            if !all_collection.contains(&collection.to_string()) {
                mongo_database.create_collection(*collection).await?;
            }
        }

        let dbs = Self { mongo_database };
        // create the unique indexes if they do not exist
        Self::create_index::<User, _>(&dbs, USER, "email", 1).await?;
        Self::create_index::<Group, _>(&dbs, GROUP, "name", "text").await?;

        DBCONN.get_or_init(|| dbs);

        Ok(())
    }

    async fn create_index<Z, T>(db: &DB, collection: &str, field: &str, index_type: T) -> Result<()>
    where
        Z: Send + Sync + Serialize + DeserializeOwned,
        T: Into<Bson>,
    {
        let col = Self::get_collection::<Z>(db, collection);
        let mut indexes = col.list_indexes().await?;
        while let Some(Ok(index)) = indexes.next().await {
            if collection == index.keys.to_string() {
                return Ok(());
            }
        }
        // create index
        let index_model = IndexModel::builder()
            .keys(doc! {field: index_type})
            .options(
                IndexOptions::builder()
                    .name(Some(field.to_string()))
                    .unique(true)
                    .build(),
            )
            .build();
        col.create_index(index_model).await?;

        Ok(())
    }

    #[inline]
    fn get_collection<Z>(d: &DB, collection: &str) -> Collection<Z>
    where
        Z: Send + Sync + Serialize + DeserializeOwned,
    {
        d.mongo_database.collection::<Z>(collection)
    }
}

impl<'c, R1> Database<Document, &'c str, R1, Bson, u64> for DB
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

    async fn insert(&self, query: R1, collection: &'c str) -> Result<Bson> {
        let col = Self::get_collection(self, collection);
        Ok(col.insert_one(query).await?.inserted_id)
    }

    async fn insert_many(&self, query: Vec<R1>, collection: &'c str) -> Result<Vec<Bson>> {
        let mut types = Vec::new();
        let col = Self::get_collection(self, collection);
        let r = col.insert_many(query).await?;

        for id in r.inserted_ids {
            types.push(id.1);
        }
        Ok(types)
    }

    async fn update(
        &self,
        query: Document,
        update: Document,
        collection: &'c str,
        _model: PhantomData<R1>,
    ) -> Result<u64> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.update_one(query, update).await?;
        Ok(r.modified_count) // should always be 1
    }

    async fn update_many(
        &self,
        query: Document,
        update: Vec<Document>,
        collection: &'c str,
        _model: PhantomData<R1>,
    ) -> Result<u64> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.update_many(query, update).await?;

        Ok(r.modified_count)
    }

    async fn delete(
        &self,
        filter: Document,
        collection: &'c str,
        _model: PhantomData<R1>,
    ) -> Result<u64> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.delete_one(filter).await?;

        Ok(r.deleted_count)
    }

    async fn delete_many(
        &self,
        filter: Document,
        collection: &'c str,
        _model: PhantomData<R1>,
    ) -> Result<u64> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.delete_many(filter).await?;

        Ok(r.deleted_count)
    }
}

#[cfg(test)]
mod tests {
    use mongodb::bson::{doc, oid::ObjectId, to_bson};

    use crate::{
        config,
        db::models::{User, UserType, USER},
    };

    use super::*;

    #[tokio::test]
    // You will need to have a local instance of MongoDB running to run this test successfully
    // Look into the justfile for the command to run
    async fn test_mongo_connection_n_queries() {
        config::init_once();
        DB::init_once("guardian").await.unwrap();

        let db = DBCONN.get().unwrap();
        let time = time::OffsetDateTime::now_utc();

        let _id = db
            .insert(
                User {
                    _id: ObjectId::new(),
                    sub: "choi.mina@gmail.com".to_string(),
                    user_name: "Daniel".to_string(),
                    email: "choi.mina@gmail.com".to_string(),
                    groups: vec!["ibm".to_string()],
                    user_type: UserType::SystemAdmin,
                    token: None,
                    created_at: time,
                    updated_at: time,
                    last_updated_by: "choi.mina@gmail.com".to_string(),
                },
                USER,
            )
            .await
            .unwrap();

        let user: User = db
            .find(
                doc! {
                    "sub": "choi.mina@gmail.com",
                },
                USER,
            )
            .await
            .unwrap();
        let group = user.groups.first().unwrap();
        assert_eq!(group, "ibm");

        let new_time = time::OffsetDateTime::now_utc();
        let n = db
			.update(
				doc! {"sub": "choi.mina@gmail.com"},
				doc! {"$set": doc! {"email": "someone@gmail.com", "updated_at": to_bson(&new_time).unwrap()}},
				USER,
				PhantomData::<User>,
			)
			.await
			.unwrap();
        assert_eq!(n, 1);

        let result: User = db
            .find(
                doc! {
                    "sub": "choi.mina@gmail.com"
                },
                USER,
            )
            .await
            .unwrap();
        assert_eq!(result.email, "someone@gmail.com");
        assert_eq!(result.updated_at, new_time);

        let n = db
            .delete(
                doc! {"sub": "choi.mina@gmail.com"},
                USER,
                PhantomData::<User>,
            )
            .await
            .unwrap();
        assert_eq!(n, 1);
    }
}
