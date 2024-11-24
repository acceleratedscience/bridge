use std::future::{ready, Ready};

use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, HttpResponse,
};
use futures::{future::LocalBoxFuture, FutureExt, TryFutureExt};
use tracing::warn;

use crate::{auth::NOTEBOOK_COOKIE_NAME, db::models::NotebookCookie};

pub struct NotebookCookieCheck;

impl<S, B> Transform<S, ServiceRequest> for NotebookCookieCheck
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = NotebookCookieCheckMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(NotebookCookieCheckMW { service }))
    }
}

pub struct NotebookCookieCheckMW<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for NotebookCookieCheckMW<S>
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
        match req.cookie(NOTEBOOK_COOKIE_NAME).map(|c| c.value().to_string()) {
            Some(v) => {
                let guardian_cookie_result = serde_json::from_str::<NotebookCookie>(&v);
                match guardian_cookie_result {
                    Ok(gcs) => {
                        req.extensions_mut().insert(gcs);
                        self.service
                            .call(req)
                            .map_ok(ServiceResponse::map_into_left_body)
                            .boxed_local()
                    }
                    Err(e) => {
                        warn!("Guardian cookie deserialization error: {:?}", e);
                        let res = HttpResponse::InternalServerError()
                            .finish()
                            .map_into_right_body();
                        Box::pin(async { Ok(req.into_response(res)) })
                    }
                }
            }
            None => {
                warn!("Guardian cookie not found from ip {:?}", req.peer_addr());
                let res = HttpResponse::Forbidden().finish().map_into_right_body();
                Box::pin(async { Ok(req.into_response(res)) })
            }
        }
    }
}
