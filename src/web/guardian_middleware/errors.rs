use actix_web::{
    body::BoxBody,
    dev::ServiceResponse,
    http::StatusCode,
    middleware::{ErrorHandlerResponse, ErrorHandlers},
    HttpResponse,
};
use serde_json::json;

pub fn custom_404_handle() -> ErrorHandlers<BoxBody> {
    ErrorHandlers::new().handler(StatusCode::NOT_FOUND, |res| {
        let request = res.into_parts().0;
        Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
            request,
            {
                HttpResponse::NotFound()
                    .json(json!({
                        "error": "Not Found",
                        "message": "Check the uri and try again",
                    }))
                    .map_into_left_body()
            },
        )))
    })
}
