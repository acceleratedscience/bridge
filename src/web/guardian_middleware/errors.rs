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

use super::htmx::HTMX_ERROR_RES;

static ERROR_HTMLS: OnceLock<HashMap<&str, String>> = OnceLock::new();

// TODO: refactor this using middleware pattern
pub fn custom_code_handle(data: Data<Tera>) -> ErrorHandlers<BoxBody> {
	let template = ERROR_HTMLS.get_or_init(|| {
		let mut map = HashMap::new();
		map.insert(
			"404",
			data.render("pages/404.html", &Context::new())
				.expect("Failed to render 404.html"),
		);
		map.insert(
			"500",
			data.render("pages/500.html", &Context::new())
				.expect("Failed to render 500.html"),
		);
		map.insert(
			"400",
			data.render("pages/400.html", &Context::new())
				.expect("Failed to render 400.html"),
		);
		map.insert(
			"401",
			data.render("pages/401.html", &Context::new())
				.expect("Failed to render 401.html"),
		);
		map.insert(
			"403",
			data.render("pages/403.html", &Context::new())
				.expect("Failed to render 403.html"),
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
			let response = res.response();
			let htmx_header = response.headers().get(HTMX_ERROR_RES).map(|header| {
				header
					.to_str()
					.unwrap_or("Htmx message retrieval failed")
					.to_string()
			});
			let request = res.into_parts().0;

			Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
				request,
				{
					let contents = if let Some(hh) = htmx_header {
						(hh, ContentType::form_url_encoded())
					} else {
						(template.get("400").unwrap().to_string(), ContentType::html())
					};

					HttpResponse::build(StatusCode::BAD_REQUEST)
						.content_type(contents.1)
						.body(contents.0)
						.map_into_left_body()
				},
			)))
		})
		.handler(StatusCode::UNAUTHORIZED, move |res: ServiceResponse| {
			let response = res.response();
			let headers = response.headers();
			let request = res.request().clone();
			Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
				request,
				{
					let mut response = HttpResponse::build(StatusCode::UNAUTHORIZED);

					for (key, value) in headers.iter() {
						response.insert_header((key.clone(), value.clone()));
					}
					response
						.content_type(ContentType::html())
						.body(template.get("401").unwrap().to_string())
						.map_into_left_body()
				},
			)))
		})
		.handler(StatusCode::FORBIDDEN, move |res: ServiceResponse| {
			let request = res.into_parts().0;
			Ok(ErrorHandlerResponse::Response(ServiceResponse::new(
				request,
				{
					HttpResponse::build(StatusCode::FORBIDDEN)
						.content_type(ContentType::html())
						.body(template.get("403").unwrap().to_string())
						.map_into_left_body()
				},
			)))
		})
}
