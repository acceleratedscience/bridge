use std::future::{ready, Ready};

use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, HttpResponse,
};
use futures::{future::LocalBoxFuture, FutureExt, TryFutureExt};
use tracing::warn;

use crate::{
    auth::{COOKIE_NAME, NOTEBOOK_STATUS_COOKIE_NAME},
    db::models::{BridgeCookie, NotebookStatusCookie},
};

pub struct CookieCheck;

impl<S, B> Transform<S, ServiceRequest> for CookieCheck
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = CookieCheckMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CookieCheckMW { service }))
    }
}

pub struct CookieCheckMW<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for CookieCheckMW<S>
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
        match req.cookie(COOKIE_NAME).map(|c| c.value().to_string()) {
            Some(v) => {
                let bridge_cookie_result = serde_json::from_str::<BridgeCookie>(&v);
                match bridge_cookie_result {
                    Ok(gcs) => {
                        // also insert notebook_status_cookie if available
                        if let Some(ncs) = req.cookie(NOTEBOOK_STATUS_COOKIE_NAME) {
                            if let Ok(ncs) =
                                serde_json::from_str::<NotebookStatusCookie>(ncs.value())
                            {
                                req.extensions_mut().insert(ncs);
                            }
                        }
                        req.extensions_mut().insert(gcs);
                        self.service
                            .call(req)
                            .map_ok(ServiceResponse::map_into_left_body)
                            .boxed_local()
                    }
                    Err(e) => {
                        warn!("Bridge cookie deserialization error: {:?}", e);
                        let res = HttpResponse::InternalServerError()
                            .finish()
                            .map_into_right_body();
                        Box::pin(async { Ok(req.into_response(res)) })
                    }
                }
            }
            None => {
                // Make sure "X-Forwarded-For" is present in the header
                warn!(
                    "Bridge cookie not found from ip {:?}",
                    req.connection_info().realip_remote_addr()
                );
                let res = HttpResponse::Unauthorized().finish().map_into_right_body();
                Box::pin(async { Ok(req.into_response(res)) })
            }
        }
    }
}
