use std::error::Error;

use mongodb::bson::{to_bson, Bson};
use tracing::error;

use crate::errors::GuardianError;

#[inline]
pub fn log_errors<T, E>(res: Result<T, E>) -> Result<T, E>
where
    E: Error,
{
    match res {
        Ok(_) => res,
        Err(ref e) => {
            error!("Error: {}", e);
            res
        }
    }
}

pub fn bson<T>(t: T) -> Result<Bson, GuardianError>
where
    T: serde::Serialize,
{
    match to_bson(&t) {
        Ok(bson) => Ok(bson),
        Err(e) => Err(GuardianError::GeneralError(e.to_string())),
    }
}
