use std::{marker::PhantomData, str::FromStr};

mod htmx;

use actix_web::{
    delete, get,
    http::header::ContentType,
    patch, post,
    web::{self, Data, ReqData},
    HttpRequest, HttpResponse,
};
use mongodb::bson::{doc, oid::ObjectId};
use tera::Tera;
use tracing::instrument;

use crate::{
    db::models::{AdminTab, AdminTabs, GroupForm},
    web::{
        guardian_middleware::Htmx,
        helper::bson,
        route::portal::helper::{check_admin, payload_to_struct},
        services::CATALOG_URLS,
    },
};
use crate::{
    db::{
        models::{Group, GuardianCookie, User, UserType, GROUP, USER},
        mongo::DB,
        Database,
    },
    errors::{GuardianError, Result},
    web::helper::{self},
};

use self::htmx::{GroupContent, CREATE, MODIFY};

const USER_PAGE: &str = "system/admin.html";

#[get("")]
#[instrument(skip(data, db, subject))]
pub(super) async fn system(
    data: Data<Tera>,
    req: HttpRequest,
    subject: Option<ReqData<GuardianCookie>>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    // get the subject id from middleware
    let guardian_cookie = check_admin(subject, UserType::SystemAdmin)?;

    let id = ObjectId::from_str(&guardian_cookie.subject)
        .map_err(|e| GuardianError::GeneralError(e.to_string()))?;

    // check the db using objectid and get info on user
    let result: Result<User> = db
        .find(
            doc! {
                "_id": id,
            },
            USER,
        )
        .await;

    let user = match result {
        Ok(user) => user,
        Err(e) => return helper::log_errors(Err(e)),
    };

    match user.user_type {
        UserType::SystemAdmin => {}
        _ => {
            return Err(GuardianError::UserNotAllowedOnPage(USER_PAGE.to_string()));
        }
    }

    let mut ctx = tera::Context::new();
    ctx.insert("name", &user.user_name);
    ctx.insert("group", &user.groups.join(", "));
    let content = helper::log_errors(data.render(USER_PAGE, &ctx))?;

    return Ok(HttpResponse::Ok().body(content));
}

#[instrument(skip(db, pl))]
#[post("group")]
async fn system_create_group(
    db: Data<&DB>,
    pl: web::Payload,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    // TODO: do this at the middleware level
    let _ = check_admin(subject, UserType::SystemAdmin)?;
    let gf = payload_to_struct::<GroupForm>(pl).await?;
    let now = time::OffsetDateTime::now_utc();

    let group = Group {
        _id: ObjectId::new(),
        name: gf.name.clone(),
        subscriptions: gf.subscriptions,
        created_at: now,
        updated_at: now,
        last_updated_by: gf.last_updated_by,
    };

    // TODO: check if group already exists, and not rely one dup key from DB
    let result = helper::log_errors(db.insert(group, GROUP).await);
    let content = match result {
        Ok(r) => format!("<p>Group created with id: {}</p>", r),
        Err(e) if e.to_string().contains("dup key") => {
            format!("<p>Group named {} already exists</p>", gf.name)
        }
        Err(e) => return Err(e),
    };

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

// This is commented out because no one should be able to delete a group for now, unless biz
// requires. For now, you can delete them through the db
// #[delete("group")]
// async fn system_delete_group(db: Data<&DB>, form: web::Form<Group>) -> Result<HttpResponse> {
//     let group = form.into_inner();
//     let _ = db
//         .delete(doc! {"name": &group.name}, GROUP, PhantomData::<Group>)
//         .await?;
//
//     let content = format!("<p>Group named {} has been deleted</p>", group.name);
//     Ok(HttpResponse::Ok()
//         .content_type(ContentType::form_url_encoded())
//         .body(content))
// }

#[patch("group")]
async fn system_update_group(
    db: Data<&DB>,
    pl: web::Payload,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    // TODO: do this at the middleware level
    let gc = check_admin(subject, UserType::SystemAdmin)?;
    let gf = payload_to_struct::<GroupForm>(pl).await?;

    let _ = db
        .update(
            doc! {"name": &gf.name},
            doc! {"$set": doc! {
                "name": &gf.name,
                "subscriptions": &gf.subscriptions,
                "updated_at": bson(time::OffsetDateTime::now_utc())?,
                "last_updated_by": gf.last_updated_by,
            }},
            GROUP,
            PhantomData::<Group>,
        )
        .await?;

    let content = format!("<p>Group named {} has been updated</p>", gf.name);
    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

#[patch("user")]
async fn system_update_user(
    db: Data<&DB>,
    form: web::Form<User>,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    // TODO: do this at the middleware level
    let gc = check_admin(subject, UserType::SystemAdmin)?;

    let user = form.into_inner();
    let _ = db
        .update(
            doc! {"user_name": &user.user_name},
            doc! {"$set": doc! {
            "user_name": &user.user_name,
            "groups": &user.groups,
            "user_type": bson(user.user_type)?,
            "updated_at": bson(time::OffsetDateTime::now_utc())?,
            "last_updated_by": gc.subject,            }},
            USER,
            PhantomData::<User>,
        )
        .await?;

    let content = format!("<p>User named {} has been updated</p>", user.user_name);
    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

#[delete("user")]
async fn system_delete_user(
    db: Data<&DB>,
    form: web::Form<User>,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    // TODO: do this at the middleware level
    let _ = check_admin(subject, UserType::SystemAdmin)?;

    let user = form.into_inner();
    let _ = db
        .delete(
            doc! {"user_name": &user.user_name},
            USER,
            PhantomData::<User>,
        )
        .await?;

    let content = format!("<p>User named {} has been deleted</p>", user.user_name);
    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

#[get("tab")]
async fn system_tab_htmx(
    req: HttpRequest,
    db: Data<&DB>,
    data: Data<Tera>,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    let gc = check_admin(subject, UserType::SystemAdmin)?;

    let id =
        ObjectId::from_str(&gc.subject).map_err(|e| GuardianError::GeneralError(e.to_string()))?;

    // get user from objectid
    let user: User = db
        .find(
            doc! {
                "_id": id,
            },
            USER,
        )
        .await?;

    let query = req.query_string();
    // deserialize into SystemAdminTab
    let tab = helper::log_errors(serde_urlencoded::from_str::<AdminTabs>(query))?;

    let mut group_form = GroupContent::new();

    CATALOG_URLS
        .get()
        .ok_or_else(|| GuardianError::GeneralError("Catalog urls not found".to_string()))?
        .iter()
        .for_each(|(_, service_name)| group_form.add(service_name.to_owned()));

    let content = match tab.tab {
        AdminTab::Profile => r#"<br><p class="lead">Profile tab</p>"#.to_string(),
        AdminTab::GroupCreate => group_form.render(&user.sub, data, CREATE)?,
        AdminTab::GroupModify => group_form.render(&user.sub, data, MODIFY)?,
        AdminTab::User => r#"<br><p class="lead">User tab</p>"#.to_string(),
    };

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

pub fn config_system(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/system_admin").service(system).service(
            web::scope("/hx")
                .wrap(Htmx)
                .service(system_tab_htmx)
                .service(system_create_group)
                .service(system_update_group)
                .service(system_update_user),
        ),
        // .service(system_delete_group)
    );
}
