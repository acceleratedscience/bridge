use std::{marker::PhantomData, ops::Deref, str::FromStr};

mod htmx;

use actix_web::{
    delete, get,
    http::header::ContentType,
    patch, post,
    web::{self, Data, ReqData},
    HttpRequest, HttpResponse,
};
use mongodb::bson::{doc, oid::ObjectId};
use serde::Deserialize;
use tera::Tera;
use tracing::instrument;

use crate::{
    db::models::{AdminTab, AdminTabs, GroupForm, UserDeleteForm, UserForm},
    web::{
        guardian_middleware::{Htmx, HTMX_ERROR_RES},
        helper::{bson, payload_to_struct},
        route::{
            notebook::NOTEBOOK_SUB_NAME,
            portal::helper::{check_admin, get_all_groups},
        },
        services::CATALOG,
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

use self::htmx::{
    GroupContent, UserContent, CREATE_MODIFY_GROUP, MODIFY_USER, VIEW_GROUP, VIEW_USER,
};

const USER_PAGE: &str = "pages/portal_system.html";

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
        Err(e) => return helper::log_with_level!(Err(e), error),
    };

    match user.user_type {
        UserType::SystemAdmin => {}
        _ => {
            return Err(GuardianError::UserNotAllowedOnPage(USER_PAGE.to_string()));
        }
    }

    let group_name = user.groups.first().unwrap_or(&"".to_string()).clone();

    let subscriptions: Result<Group> = db.find(doc! {"name": group_name}, GROUP).await;
    let subs = match subscriptions {
        Ok(g) => g.subscriptions,
        Err(_) => vec![],
    };

    let mut ctx = tera::Context::new();
    ctx.insert("name", &user.user_name);
    ctx.insert("group", &user.groups.join(", "));
    ctx.insert("subscriptions", &subs);
    ctx.insert("token", &user.token);
    if let Some(token) = &user.token {
        helper::add_token_exp_to_tera(&mut ctx, token);
    }
    if subs
        .iter()
        .map(Deref::deref)
        .collect::<Vec<&str>>()
        .contains(&NOTEBOOK_SUB_NAME)
    {
        ctx.insert("notebook", &true);
    }

    let content = helper::log_with_level!(data.render(USER_PAGE, &ctx), error)?;

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

    let subscriptions = helper::delimited_string_to_vec(gf.subscriptions, ",");

    let group = Group {
        _id: ObjectId::new(),
        name: gf.name.clone(),
        subscriptions,
        created_at: now,
        updated_at: now,
        last_updated_by: gf.last_updated_by,
    };

    // TODO: check if group already exists, and not rely one dup key from DB
    let result = helper::log_with_level!(db.insert(group, GROUP).await, error);
    let content = match result {
        Ok(r) => format!("<p>Group created with id: {}</p>", r),
        Err(e) if e.to_string().contains("dup key") => {
            return Ok(HttpResponse::BadRequest()
                .append_header((
                    HTMX_ERROR_RES,
                    format!("<p>Group '{}' already exists</p>", gf.name),
                ))
                .finish());
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
//	 let group = form.into_inner();
//	 let _ = db
//		 .delete(doc! {"name": &group.name}, GROUP, PhantomData::<Group>)
//		 .await?;
//
//	 let content = format!("<p>Group '{}' has been deleted</p>", group.name);
//	 Ok(HttpResponse::Ok()
//		 .content_type(ContentType::form_url_encoded())
//		 .body(content))
// }

#[patch("group")]
async fn system_update_group(
    db: Data<&DB>,
    pl: web::Payload,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    // TODO: do this at the middleware level
    let _ = check_admin(subject, UserType::SystemAdmin)?;
    let gf = payload_to_struct::<GroupForm>(pl).await?;

    // WTH Carbon
    let subscriptions = helper::delimited_string_to_vec(gf.subscriptions, ",");

    let r = db
        .update(
            doc! {"name": &gf.name},
            doc! {"$set": doc! {
                "name": &gf.name,
                "subscriptions": subscriptions,
                "updated_at": bson(time::OffsetDateTime::now_utc())?,
                "last_updated_by": gf.last_updated_by,
            }},
            GROUP,
            PhantomData::<Group>,
        )
        .await?;

    if r.eq(&0) {
        return Ok(HttpResponse::BadRequest()
            .append_header((
                HTMX_ERROR_RES,
                format!("<p>Group '{}' does not exist</p>", gf.name),
            ))
            .finish());
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(format!("<p>Group '{}' has been updated</p>", gf.name)))
}

#[patch("user")]
async fn system_update_user(
    db: Data<&DB>,
    pl: web::Payload,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    // TODO: do this at the middleware level
    let _ = check_admin(subject, UserType::SystemAdmin)?;
    let uf = payload_to_struct::<UserForm>(pl).await?;

    // stop self update
    if uf.email.eq(&uf.last_updated_by) {
        return Ok(HttpResponse::BadRequest()
            .append_header((HTMX_ERROR_RES, "<p>Cannot update self</p>".to_string()))
            .finish());
    }

    let r = db
        .update(
            doc! {"email": &uf.email},
            doc! {"$set": doc! {
            "groups": uf.groups,
            "user_type": bson(uf.user_type)?,
            "updated_at": bson(time::OffsetDateTime::now_utc())?,
            "last_updated_by": uf.last_updated_by }},
            USER,
            PhantomData::<User>,
        )
        .await?;

    if r.eq(&0) {
        return Ok(HttpResponse::BadRequest()
            .append_header((
                HTMX_ERROR_RES,
                format!("<p>User with email address {} does not exist</p>", uf.email),
            ))
            .finish());
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(format!(
            "<p>User with email address {} has been updated</p>",
            uf.email
        )))
}

#[delete("user")]
async fn system_delete_user(
    db: Data<&DB>,
    req: HttpRequest,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    // TODO: do this at the middleware level
    let _ = check_admin(subject, UserType::SystemAdmin)?;
    let query = req.query_string();
    let de = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(query);
    let uf = UserDeleteForm::deserialize(de)?;
    dbg!(&uf);

    // stop self delete
    if uf.email.eq(&uf.last_updated_by) {
        return Ok(HttpResponse::BadRequest()
            .append_header((HTMX_ERROR_RES, "<p>Cannot delete self</p>".to_string()))
            .finish());
    }

    let _ = db
        .delete(doc! {"email": &uf.email}, USER, PhantomData::<User>)
        .await?;

    let content = format!("<p>User with sub {} has been deleted</p>", uf.email);
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
    // deserialize into AdminTab
    let tab = helper::log_with_level!(serde_urlencoded::from_str::<AdminTabs>(query), error)?;

    let content = match tab.tab {
        AdminTab::GroupModify | AdminTab::GroupView | AdminTab::GroupCreate => {
            let mut group_form = GroupContent::new();

            CATALOG
                .0
                .get("services")
                .and_then(|v| v.as_table())
                .ok_or(GuardianError::GeneralError("No services found".to_string()))?
                .iter()
                .for_each(|(k, _)| {
                    // TODO: remove this clone and use &'static str
                    group_form.add(k.clone());
                });

            match tab.tab {
                AdminTab::GroupView => {
                    let groups: Vec<Group> = db.find_many(doc! {}, GROUP).await.unwrap_or(vec![]);
                    let group_names: Vec<String> =
                        groups.into_iter().map(|group| group.name).collect();

                    group_form.render(
                        &user.email,
                        data,
                        VIEW_GROUP,
                        Some(|ctx: &mut tera::Context| {
                            ctx.insert("groups", &group_names);
                        }),
                    )?
                }
                AdminTab::GroupCreate => group_form.render(
                    &user.email,
                    data,
                    CREATE_MODIFY_GROUP,
                    None::<fn(&mut tera::Context)>,
                )?,
                AdminTab::GroupModify => match tab.group {
                    Some(name) => group_form.render(
                        &user.email,
                        data,
                        CREATE_MODIFY_GROUP,
                        Some(|ctx: &mut tera::Context| {
                            ctx.insert("group_name", &name);
                        }),
                    )?,
                    None => {
                        return Err(GuardianError::GeneralError("No group provided".to_string()))
                    }
                },
                _ => unreachable!(),
            }
        }
        AdminTab::UserModify | AdminTab::UserView | AdminTab::UserDelete => {
            let mut user_form = UserContent::new();

            let target_user = tab
                .user
                .as_ref()
                .ok_or(GuardianError::GeneralError("No user provided".to_string()))?;

            get_all_groups(&db)
                .await
                .unwrap_or(vec![])
                .iter()
                .for_each(|g| user_form.add_group(g.name.to_owned()));
            UserType::to_array_str()
                .iter()
                .for_each(|t| user_form.add_user_type(t.to_string()));

            match tab.tab {
                AdminTab::UserView => {
                    user_form.render(&user.email, target_user, data, VIEW_USER, None)?
                }
                AdminTab::UserModify => {
                    user_form.render(&user.email, target_user, data, MODIFY_USER, None)?
                }
                AdminTab::UserDelete => user_form.render(
                    &user.email,
                    target_user,
                    data,
                    MODIFY_USER,
                    Some(|ctx: &mut tera::Context| {
                        ctx.insert("delete", &true);
                    }),
                )?,
                _ => unreachable!("Group variants of enum should not be here"),
            }
        }
        AdminTab::Profile => r#"<br><p>Profile tab</p>"#.to_string(),
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
                .service(system_update_user)
                .service(system_delete_user),
        ),
        // .service(system_delete_group)
    );
}
