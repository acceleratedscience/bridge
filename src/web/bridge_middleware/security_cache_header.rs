use std::future::{Ready, ready};

use actix_web::{
    Error,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::{self, CACHE_CONTROL, HeaderValue},
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
                HeaderValue::from_str(
                    "default-src 'self'; img-src *; style-src 'self'; script-src 'self';",
                )?,
            );
            header.insert(CACHE_CONTROL, HeaderValue::from_str("no-cache")?);
            // add HSTS header
            header.insert(
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_str("max-age=31536000; includeSubDomains")?,
            );
            Ok(res)
        })
    }
}
