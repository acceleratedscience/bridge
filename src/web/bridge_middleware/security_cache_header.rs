use std::future::{ready, Ready};

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{self, HeaderValue, CACHE_CONTROL},
    Error,
};
use futures::future::LocalBoxFuture;

pub struct SecurityCacheHeader;

impl<S, B> Transform<S, ServiceRequest> for SecurityCacheHeader
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = SecurityCacheHeaderMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SecurityCacheHeaderMW { service }))
    }
}

pub struct SecurityCacheHeaderMW<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for SecurityCacheHeaderMW<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // add csp header
        let response = self.service.call(req);
        Box::pin(async move {
            let mut res = response.await?;
            let header = res.headers_mut();
            header.insert(
             header::CONTENT_SECURITY_POLICY,
             HeaderValue::from_str("default-src 'self'; img-src *; style-src 'self' https://unpkg.com; script-src 'self' https://unpkg.com;")?,
            );
            header.insert(CACHE_CONTROL, HeaderValue::from_str("no-cache")?);
            Ok(res)
        })
    }
}
