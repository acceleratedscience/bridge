use std::future::{ready, Ready};

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{self, HeaderValue},
    Error,
};
use futures::future::LocalBoxFuture;

pub struct SecurityHeader;

impl<S, B> Transform<S, ServiceRequest> for SecurityHeader
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = SecurityHeaderMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SecurityHeaderMW { service }))
    }
}

pub struct SecurityHeaderMW<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for SecurityHeaderMW<S>
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
            res.headers_mut().insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_str("default-src 'self'; img-src *; style-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net/npm/bootstrap@5.3.3/dist/css/bootstrap.min.css; script-src 'self' 'unsafe-eval' 'nonce-guardian-group-form-val' 'nonce-guardian-theme-selector' https://cdn.jsdelivr.net/npm/bootstrap@5.3.3/dist/js/bootstrap.bundle.min.js https://unpkg.com/htmx.org@2.0.1;")?,
            );
            Ok(res)
        })
    }
}
