use std::{collections::HashMap, sync::OnceLock};

use actix_web::{
    body::BoxBody,
    dev::ServiceResponse,
    http::{header::ContentType, StatusCode},
    middleware::{ErrorHandlerResponse, ErrorHandlers},
    web::Data,
    HttpResponse,
};
use tera::{Context, Tera};

static ERROR_HTMLS: OnceLock<HashMap<&str, String>> = OnceLock::new();

pub fn custom_code_handle(data: Data<Tera>) -> ErrorHandlers<BoxBody> {
    let template = ERROR_HTMLS.get_or_init(|| {
        let mut map = HashMap::new();
        map.insert(
            "404",
            data.render("404.html", &Context::new())
                .expect("Failed to render 404.html"),
        );
        map.insert(
            "500",
            data.render("500.html", &Context::new())
                .expect("Failed to render 500.html"),
        );
        map.insert(
            "400",
            data.render("400.html", &Context::new())
                .expect("Failed to render 400.html"),
        );
        map.insert(
            "401",
            data.render("401.html", &Context::new())
                .expect("Failed to render 401.html"),
        );
        map
    });

    ErrorHandlers::new()
        .handler(StatusCode::NOT_FOUND, move |res: ServiceResponse| {
            let request = res.into_parts().0;
            Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
                request,
                {
                    HttpResponse::build(StatusCode::NOT_FOUND)
                        .content_type(ContentType::html())
                        .body(template.get("404").unwrap().to_string())
                        .map_into_left_body()
                },
            )))
        })
        .handler(
            StatusCode::INTERNAL_SERVER_ERROR,
            move |res: ServiceResponse| {
                let request = res.into_parts().0;
                Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
                    request,
                    {
                        HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                            .content_type(ContentType::html())
                            .body(template.get("500").unwrap().to_string())
                            .map_into_left_body()
                    },
                )))
            },
        )
        .handler(StatusCode::BAD_REQUEST, move |res: ServiceResponse| {
            let request = res.into_parts().0;
            Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
                request,
                {
                    HttpResponse::build(StatusCode::BAD_REQUEST)
                        .content_type(ContentType::html())
                        .body(template.get("400").unwrap().to_string())
                        .map_into_left_body()
                },
            )))
        })
        // .handler(StatusCode::UNAUTHORIZED, move |res: ServiceResponse| {
        //     let request = res.into_parts().0;
        //     Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
        //         request,
        //         {
        //             HttpResponse::build(StatusCode::UNAUTHORIZED)
        //                 .content_type(ContentType::html())
        //                 .body(template.get("401").unwrap().to_string())
        //                 .map_into_left_body()
        //         },
        //     )))
        // })
}
