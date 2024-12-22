use std::{future::Future, marker::PhantomData};

use crate::errors::Result;

pub mod deserialize;
pub mod models;
pub mod mongo;

// Database interface
// Q is a generic type that represents a query
// N is a generic type that represents a string to search a collection
// C is a generic type that represents a collection or a table in the DB
// R1 is a generic type that represents a model, such as User and Group
// R2 is a generic type that represents some ID that is linked to some record in the DB
// R3 is a generic type that represents the number of records affected
pub trait Database<R1> {
    type Q;
    type N<'a>;
    type C;
    type R2;
    type R3;

    fn find(
        &self,
        query: Self::Q,
        collection: Self::C,
    ) -> impl Future<Output = Result<R1>>;
    fn find_one_update(
        &self,
        query: Self::Q,
        update: Self::Q,
        collection: Self::C,
    ) -> impl Future<Output = Result<R1>>;
    fn find_many(
        &self,
        query: Self::Q,
        collection: Self::C,
    ) -> impl Future<Output = Result<Vec<R1>>>;

    fn insert(
        &self,
        query: R1,
        collection: Self::C,
    ) -> impl Future<Output = Result<Self::R2>>;
    fn insert_many(
        &self,
        query: Vec<R1>,
        collection: Self::C,
    ) -> impl Future<Output = Result<Vec<Self::R2>>>;

    fn update(
        &self,
        query: Self::Q,
        update: Self::Q,
        collection: Self::C,
        _model: PhantomData<R1>,
    ) -> impl Future<Output = Result<Self::R3>>;
    fn update_many(
        &self,
        query: Self::Q,
        update: Vec<Self::Q>,
        collection: Self::C,
        _model: PhantomData<R1>,
    ) -> impl Future<Output = Result<Self::R3>>;

    fn delete(
        &self,
        filter: Self::Q,
        collection: Self::C,
        _model: PhantomData<R1>,
    ) -> impl Future<Output = Result<Self::R3>>;
    fn delete_many(
        &self,
        filter: Self::Q,
        collection: Self::C,
        _model: PhantomData<R1>,
    ) -> impl Future<Output = Result<Self::R3>>;

    // Application specific methods
    fn search_users(
        &self,
        name: Self::N<'_>,
        collection: Self::C,
        _model: PhantomData<R1>,
    ) -> impl Future<Output = Result<Vec<R1>>>;
}
