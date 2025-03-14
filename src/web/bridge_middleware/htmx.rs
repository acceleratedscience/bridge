use std::future::{Ready, ready};

use actix_web::{
    Error, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use futures::{FutureExt, TryFutureExt, future::LocalBoxFuture};
use tracing::warn;

pub struct Htmx;

pub static HTMX_ERROR_RES: &str = "HTMX-Error-Response";

impl<S, B> Transform<S, ServiceRequest> for Htmx
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = HtmxMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(HtmxMW { service }))
    }
}

pub struct HtmxMW<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for HtmxMW<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // if request is not an htmx request, return bad request
        match req.headers().get("HX-Request") {
            Some(v) if v == "true" => self
                .service
                .call(req)
                .map_ok(ServiceResponse::map_into_left_body)
                .boxed_local(),
            _ => {
                warn!(
                    "Request is not an htmx request from {:?}",
                    req.connection_info().realip_remote_addr()
                );
                let res = HttpResponse::BadRequest().finish().map_into_right_body();
                Box::pin(async { Ok(req.into_response(res)) })
            }
        }
    }
}
