use std::future::Future;

use crate::errors::Result;

use self::mongo::DB;

pub mod models;
pub mod mongo;

pub struct DatabaseConn<'a> {
    pub db: DB<'a>,
}

impl<'a> DatabaseConn<'a> {
    pub fn new(db: DB<'a>) -> Self {
        Self { db }
    }
}

// Database interface
// Q query type
// C collection type
// R1 find result type from the models mod
// R2 insert result type is an some id
// R3 update result type is a count of effected documents
pub trait Database<Q, C, R1, R2, R3> {
    fn find(&self, query: Q, collection: C) -> impl Future<Output = Result<R1>>;
    fn find_many(&self, query: Q, collection: C) -> impl Future<Output = Result<Vec<R1>>>;

    fn insert(&self, query: Q, collection: C) -> impl Future<Output = Result<R2>>;
    fn insert_many(&self, query: Vec<Q>, collection: C) -> impl Future<Output = Result<Vec<R2>>>;

    fn update(&self, query: Q, update: Q, collection: C) -> impl Future<Output = Result<R2>>;
    fn update_many(
        &self,
        query: Q,
        update: Vec<Q>,
        collection: C,
    ) -> impl Future<Output = Result<R3>>;

    fn delete(&self, filter: Q, collection: C) -> impl Future<Output = Result<R2>>;
    fn delete_many(&self, filter: Q, collection: C) -> impl Future<Output = Result<R3>>;
}
