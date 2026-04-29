use std::future::{Ready, ready};

use actix_web::{
    Error,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::{self, CACHE_CONTROL, HeaderValue},
};
use futures::future::LocalBoxFuture;

use crate::config::CONFIG;

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
        let path = req.path();
        let crc = &CONFIG.custom_resource_csp;
        // TODO: This is not the most robust way of doing this... improve this in later iterations
        let custom_csp = crc.iter().find(|&v| path.contains(v.0));

        // add csp header
        let response = self.service.call(req);
        Box::pin(async move {
            let mut res = response.await?;
            let header = res.headers_mut();

            let csp = match custom_csp {
                Some(m) => m.1,
                None => "default-src 'self'; img-src *; style-src 'self'; script-src 'self';",
            };

            header.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_str(csp)?);
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
