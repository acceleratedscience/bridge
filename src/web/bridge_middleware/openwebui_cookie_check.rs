use std::future::{Ready, ready};

use actix_web::{
    Error, HttpMessage, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use futures::{FutureExt, TryFutureExt, future::LocalBoxFuture};
use tracing::warn;

use crate::{auth::OWUI_COOKIE_NAME, db::models::OWUICookie};

pub struct OWUICookieCheck;

impl<S, B> Transform<S, ServiceRequest> for OWUICookieCheck
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = OWUICookieCheckMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(OWUICookieCheckMW { service }))
    }
}

pub struct OWUICookieCheckMW<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for OWUICookieCheckMW<S>
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
        match req
            .cookie(OWUI_COOKIE_NAME)
            .map(|c| c.value().to_string())
        {
            Some(v) => {
                let bridge_cookie_result = serde_json::from_str::<OWUICookie>(&v);
                match bridge_cookie_result {
                    Ok(gcs) => {
                        req.extensions_mut().insert(gcs);
                        self.service
                            .call(req)
                            .map_ok(ServiceResponse::map_into_left_body)
                            .boxed_local()
                    }
                    Err(e) => {
                        warn!("OWUI cookie deserialization error: {:?}", e);
                        let res = HttpResponse::InternalServerError()
                            .finish()
                            .map_into_right_body();
                        Box::pin(async { Ok(req.into_response(res)) })
                    }
                }
            }
            None => {
                warn!(
                    "OWUI cookie not found from ip {:?}",
                    req.connection_info().realip_remote_addr()
                );
                let res = HttpResponse::Forbidden().finish().map_into_right_body();
                Box::pin(async { Ok(req.into_response(res)) })
            }
        }
    }
}
