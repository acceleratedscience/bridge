use actix_web::web;

use crate::web::guardian_middleware::CookieCheck;

pub mod user;

pub fn config_portal(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/portal").wrap(CookieCheck).service(user::user));
}
