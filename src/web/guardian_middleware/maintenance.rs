use std::{
    future::{ready, Ready},
    sync::RwLock,
};

use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header,
    Error, HttpResponse,
};
use futures::{future::LocalBoxFuture, FutureExt, TryFutureExt};

static MAINTENANCE_WINDOWS: RwLock<bool> = RwLock::new(false);

pub struct Maintainence;

impl<S, B> Transform<S, ServiceRequest> for Maintainence
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = MaintainenceMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(MaintainenceMW { service }))
    }
}

pub struct MaintainenceMW<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for MaintainenceMW<S>
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
        match MAINTENANCE_WINDOWS.try_read() {
            Ok(rg) => {
                if *rg {
                    // Guardian under maintenance
                    return Box::pin(async move {
                        Ok(req.into_response(
                            HttpResponse::Found()
                                .append_header((header::LOCATION, "/maintenance"))
                                .finish()
                                .map_into_right_body(),
                        ))
                    });
                }
            }
            Err(_e) => (),
        }

        self.service
            .call(req)
            .map_ok(ServiceResponse::map_into_left_body)
            .boxed_local()
    }
}
