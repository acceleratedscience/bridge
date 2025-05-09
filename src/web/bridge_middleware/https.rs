use std::future::{Ready, ready};

use actix_web::{
    Error, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header,
};
use futures::{FutureExt, TryFutureExt, future::LocalBoxFuture};

pub struct HttpRedirect;

impl<S, B> Transform<S, ServiceRequest> for HttpRedirect
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = HttpRedirectMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(HttpRedirectMW { service }))
    }
}

pub struct HttpRedirectMW<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for HttpRedirectMW<S>
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
        if req.connection_info().scheme() == "http" {
            let new_uri = format!("https://{}{}", req.connection_info().host(), req.uri());
            let response = HttpResponse::PermanentRedirect()
                .append_header((header::LOCATION, new_uri))
                .finish()
                .map_into_right_body();

            return Box::pin(async { Ok(req.into_response(response)) });
        }

        self.service
            .call(req)
            .map_ok(ServiceResponse::map_into_left_body)
            .boxed_local()
    }
}
