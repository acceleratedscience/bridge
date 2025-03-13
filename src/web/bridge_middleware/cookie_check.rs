use std::{
    future::{Ready, ready},
    rc::Rc,
};

use actix_web::{
    Error, HttpMessage, HttpResponse,
    body::EitherBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use futures::{FutureExt, future::LocalBoxFuture};
use tracing::warn;

use crate::{
    auth::{COOKIE_NAME, NOTEBOOK_STATUS_COOKIE_NAME},
    db::{
        keydb::CACHEDB,
        models::{BridgeCookie, NotebookStatusCookie},
    },
};

pub struct CookieCheck;

impl<S, B> Transform<S, ServiceRequest> for CookieCheck
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = CookieCheckMW<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CookieCheckMW {
            service: Rc::new(service),
        }))
    }
}

pub struct CookieCheckMW<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for CookieCheckMW<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
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

                        let session_id = gcs.session_id.clone();
                        req.extensions_mut().insert(gcs);
                        let service = self.service.clone();
                        async move {
                            if let Some(cache) = CACHEDB.get() {
                                if let Some(session_id) = session_id {
                                    warn!("Session id: {:?}", session_id);
                                // TODO: check with redis if session_id is still valid
                                } else {
                                    return Ok(req.into_response(
                                        HttpResponse::Unauthorized().finish().map_into_right_body(),
                                    ));
                                }
                            }

                            Ok(service.call(req).await?.map_into_left_body())
                        }
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
