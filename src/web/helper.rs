use std::error::Error;

use tracing::error;

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
