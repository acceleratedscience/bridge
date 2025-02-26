use std::future::{ready, Ready};

use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, HttpResponse,
};
use futures::{future::LocalBoxFuture, FutureExt, TryFutureExt};
use tracing::warn;

use crate::{auth::COOKIE_NAME, db::models::BridgeCookie};

const RESOURCE_PREFIX: &str = "/resource";

pub struct ResourceCookieCheck;

impl<S, B> Transform<S, ServiceRequest> for ResourceCookieCheck
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = ResourceCookieCheckMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ResourceCookieCheckMW { service }))
    }
}

pub struct ResourceCookieCheckMW<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for ResourceCookieCheckMW<S>
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
                    Ok(bc) => {
                        if let Some(ref resource_allowed) = bc.resources {
                            let path = req
                                .uri()
                                .path()
                                .strip_prefix(RESOURCE_PREFIX)
                                .unwrap_or(req.uri().path());
                            let resource_requested =
                                path.split("/").nth(1).unwrap_or("").to_string();

                            if resource_allowed.contains(&resource_requested) {
                                req.extensions_mut().insert((bc, resource_requested));
                                return self
                                    .service
                                    .call(req)
                                    .map_ok(ServiceResponse::map_into_left_body)
                                    .boxed_local();
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Bridge cookie deserialization error: {:?}", e);
                        let res = HttpResponse::InternalServerError()
                            .finish()
                            .map_into_right_body();
                        return Box::pin(async { Ok(req.into_response(res)) });
                    }
                }
                warn!(
                    "User not allowed to access resource {} from ip {:?}",
                    req.uri().path(),
                    req.connection_info().realip_remote_addr()
                );
            }
            None => {
                // Make sure "X-Forwarded-For" is present in the header
                warn!(
                    "Bridge cookie not found from ip {:?}",
                    req.connection_info().realip_remote_addr()
                );
            }
        }

        Box::pin(async {
            Ok(req.into_response(HttpResponse::Forbidden().finish().map_into_right_body()))
        })
    }
}
