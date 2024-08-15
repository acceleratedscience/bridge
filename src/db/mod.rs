use std::future::Future;
use std::marker::PhantomData;

use crate::errors::Result;

pub mod deserialize;
pub mod models;
pub mod mongo;

// Database interface
// Q is a generic type that represents a query
// C is a generic type that represents a collection or a table in the DB
// R1 is a generic type that represents a model, such as User and Group
// R2 is a generic type that represents some ID that is linked to some record in the DB
// R3 is a generic type that represents the number of records affected
pub trait Database<Q, C, R1, R2, R3> {
    fn find(&self, query: Q, collection: C) -> impl Future<Output = Result<R1>>;
    fn find_many(&self, query: Q, collection: C) -> impl Future<Output = Result<Vec<R1>>>;

    fn insert(&self, query: R1, collection: C) -> impl Future<Output = Result<R2>>;
    fn insert_many(&self, query: Vec<R1>, collection: C) -> impl Future<Output = Result<Vec<R2>>>;

    fn update(
        &self,
        query: Q,
        update: Q,
        collection: C,
        _model: PhantomData<R1>,
    ) -> impl Future<Output = Result<R3>>;
    fn update_many(
        &self,
        query: Q,
        update: Vec<Q>,
        collection: C,
        _model: PhantomData<R1>,
    ) -> impl Future<Output = Result<R3>>;

    fn delete(
        &self,
        filter: Q,
        collection: C,
        _model: PhantomData<R1>,
    ) -> impl Future<Output = Result<R3>>;
    fn delete_many(
        &self,
        filter: Q,
        collection: C,
        _model: PhantomData<R1>,
    ) -> impl Future<Output = Result<R3>>;
}
