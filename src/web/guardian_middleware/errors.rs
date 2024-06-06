use actix_web::{
    body::BoxBody,
    dev::ServiceResponse,
    http::{header::ContentType, StatusCode},
    middleware::{ErrorHandlerResponse, ErrorHandlers},
    web::Data,
    HttpResponse,
};
use tera::{Context, Tera};
use tracing::warn;

pub fn custom_code_handle(data: Data<Tera>) -> ErrorHandlers<BoxBody> {
    let body = data.render("404.html", &Context::new()).unwrap();
    ErrorHandlers::new().handler(StatusCode::NOT_FOUND, move |res: ServiceResponse| {
        let request = res.into_parts().0;
        Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
            request,
            {
                HttpResponse::build(StatusCode::NOT_FOUND)
                    .content_type(ContentType::html())
                    .body(body.clone())
                    .map_into_left_body()
            },
        )))
    }).handler(StatusCode::INTERNAL_SERVER_ERROR, move |res: ServiceResponse| {
        let body = data.render("500.html", &Context::new()).unwrap();
        let request = res.into_parts().0;
        Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
            request,
            {
                HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                    .content_type(ContentType::html())
                    .body(body.clone())
                    .map_into_left_body()
            },
        )))
    })
}
