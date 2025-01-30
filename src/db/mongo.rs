use std::{marker::PhantomData, str::FromStr, sync::OnceLock, time::Duration};

use futures::{StreamExt, TryStreamExt};
use mongodb::{
    bson::{doc, oid::ObjectId, Bson, DateTime, Document, Regex},
    options::IndexOptions,
    Client, Collection, Database as MongoDatabase, IndexModel,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    config::CONFIG,
    db::models::Apps,
    errors::{BridgeError, Result},
};

use super::{
    models::{Group, Locks, User, APPS, GROUP, LOCKS, USER},
    Database,
};

#[derive(Clone)]
pub struct DB {
    mongo_database: MongoDatabase,
}

#[derive(Clone)]
pub struct ObjectID(ObjectId);

impl ObjectID {
    pub fn new(s: &str) -> Self {
        ObjectID(ObjectId::from_str(s).unwrap_or_default())
    }

    #[inline]
    pub fn into_inner(&self) -> ObjectId {
        self.0
    }
}

pub static DBCONN: OnceLock<DB> = OnceLock::new();

static COLLECTIONS: [&str; 4] = [USER, GROUP, LOCKS, APPS];

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
        fn unique(f: impl Into<Bson> + ToString) -> IndexOptions {
            IndexOptions::builder()
                .name(Some(f.to_string()))
                .unique(true)
                .build()
        }
        Self::create_index::<User, _>(&dbs, USER, "email", 1, unique).await?;
        Self::create_index::<Group, _>(&dbs, GROUP, "name", "text", unique).await?;
        Self::create_index::<Locks, _>(&dbs, LOCKS, "expireSoonAfter", 1, |f| {
            IndexOptions::builder()
                .name(Some(f.to_string()))
                .expire_after(Duration::from_secs(0)) // 1 hour
                .build()
        })
        .await?;
        Self::create_index::<Locks, _>(&dbs, LOCKS, "leaseName", "text", unique).await?;
        Self::create_index::<Apps, _>(&dbs, APPS, "client_id", "text", unique).await?;

        DBCONN.get_or_init(|| dbs);

        Ok(())
    }

    async fn create_index<'a, Z, T>(
        db: &DB,
        collection: &str,
        field: &'a str,
        index_type: T,
        index_opt: impl Fn(&'a str) -> IndexOptions,
    ) -> Result<()>
    where
        Z: Send + Sync + Serialize + DeserializeOwned,
        T: Into<Bson> + Copy,
    {
        let col = Self::get_collection::<Z>(db, collection);
        let mut indexes = col.list_indexes().await?;
        while let Some(Ok(index)) = indexes.next().await {
            // If the index already exists, return
            if collection == index.keys.to_string() {
                return Ok(());
            }
        }
        // create index
        let index_model = IndexModel::builder()
            .keys(doc! {field: index_type})
            .options(index_opt(field))
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

    pub async fn get_lease(&self, name: &str, duration_sec: i64) -> Result<()> {
        // one hour from now
        let hour_from_now = (time::OffsetDateTime::now_utc()
            + time::Duration::seconds(duration_sec))
        .unix_timestamp_nanos()
            / 1_000_000;

        let mongo_now = DateTime::from_millis(hour_from_now as i64);
        let _ = self
            .insert(
                doc! {"leaseName": name,
                    "expireSoonAfter": mongo_now,
                },
                LOCKS,
            )
            .await?;
        Ok(())
    }

    #[deprecated(note = "Use get_lease instead")]
    pub async fn get_lock(&self, name: &str) -> Result<()> {
        let _ = self.insert(doc! {"leaseName": name}, LOCKS).await?;
        Ok(())
    }

    #[deprecated(note = "Use get_lease instead")]
    pub async fn release_lock(&self, name: &str) -> Result<()> {
        let _ = self
            .delete(doc! {"leaseName": name}, LOCKS, PhantomData::<Locks>)
            .await?;
        Ok(())
    }
}

// pub trait Database<'c, R1 = User, Q = Document, N = &'c str, C = &'c str, R2 = Bson, R3 = u64> {
impl<R1> Database<R1> for DB
where
    R1: Send + Sync + Serialize + DeserializeOwned,
{
    type Q = Document;
    type N<'a> = &'a str;
    type C = &'static str;
    type R2 = Bson;
    type R3 = u64;

    async fn find(&self, query: Document, collection: Self::C) -> Result<R1> {
        let col = Self::get_collection(self, collection);
        col.find_one(query)
            .await?
            .ok_or_else(|| BridgeError::GeneralError("Could not find any document".to_string()))
    }

    async fn find_one_update(
        &self,
        query: Document,
        update: Document,
        collection: Self::C,
    ) -> Result<R1> {
        let col = Self::get_collection(self, collection);
        col.find_one_and_update(query, update)
            .await?
            .ok_or_else(|| BridgeError::GeneralError("Could not find any document".to_string()))
    }

    async fn find_many(&self, query: Document, collection: Self::C) -> Result<Vec<R1>> {
        let mut docs = Vec::new();
        let col = Self::get_collection(self, collection);
        let mut cursor = col.find(query).await?;

        while let Some(doc) = cursor.try_next().await? {
            docs.push(doc);
        }

        if docs.is_empty() {
            return Err(BridgeError::GeneralError(
                "Could not find any documents".to_string(),
            ));
        }
        Ok(docs)
    }

    async fn insert(&self, query: R1, collection: Self::C) -> Result<Bson> {
        let col = Self::get_collection(self, collection);
        Ok(col.insert_one(query).await?.inserted_id)
    }

    async fn insert_many(&self, query: Vec<R1>, collection: Self::C) -> Result<Vec<Bson>> {
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
        collection: Self::C,
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
        collection: Self::C,
        _model: PhantomData<R1>,
    ) -> Result<u64> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.update_many(query, update).await?;

        Ok(r.modified_count)
    }

    async fn delete(
        &self,
        filter: Document,
        collection: Self::C,
        _model: PhantomData<R1>,
    ) -> Result<u64> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.delete_one(filter).await?;

        Ok(r.deleted_count)
    }

    async fn delete_many(
        &self,
        filter: Document,
        collection: Self::C,
        _model: PhantomData<R1>,
    ) -> Result<u64> {
        let col: Collection<R1> = Self::get_collection(self, collection);
        let r = col.delete_many(filter).await?;

        Ok(r.deleted_count)
    }

    async fn search_users(
        &self,
        name: Self::N<'_>,
        collection: Self::C,
        _model: PhantomData<R1>,
    ) -> Result<Vec<R1>> {
        let mut docs = Vec::new();
        let col = Self::get_collection(self, collection);
        let mut cursor = col
            .find(doc! {
                "email": doc! {
                    "$regex": Regex { pattern: "^".to_string() + name, options: "i".to_string() }
                }
            })
            .await?;

        while let Some(doc) = cursor.try_next().await? {
            docs.push(doc);
        }

        if docs.is_empty() {
            return Err(BridgeError::RecordSearchError(
                "Could not find any documents".to_string(),
            ));
        }
        Ok(docs)
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
                    notebook: None,
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
