use std::{marker::PhantomData, ops::Deref, str::FromStr};

use actix_web::{
    get,
    http::header::ContentType,
    patch,
    web::{self, Data, ReqData},
    HttpRequest, HttpResponse,
};
use futures::StreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use tera::Tera;
use tracing::instrument;

use crate::{
    db::{
        models::{
            AdminTab, AdminTabs, Group, GuardianCookie, ModifyUser, User, UserGroupMod, UserType,
            GROUP, USER,
        },
        mongo::DB,
        Database,
    },
    errors::{GuardianError, Result},
    web::{
        guardian_middleware::{Htmx, HTMX_ERROR_RES},
        helper::{self, bson},
        route::{notebook::NOTEBOOK_SUB_NAME, portal::helper::check_admin},
    },
};

use self::htmx::ModifyUserGroup;

mod htmx;

const USER_PAGE: &str = "pages/portal_group.html";

#[get("")]
#[instrument(skip(data, db, subject))]
pub(super) async fn group(
    data: Data<Tera>,
    req: HttpRequest,
    db: Data<&DB>,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    // get the subject id from middleware
    let guardian_cookie = check_admin(subject, UserType::GroupAdmin)?;

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
        UserType::GroupAdmin => {}
        _ => {
            return Err(GuardianError::UserNotAllowedOnPage(USER_PAGE.to_string()));
        }
    }

    // get all subscriptions
    let group_name = user.groups.first().unwrap_or(&"".to_string()).clone();

    let subscriptions: Result<Group> = db.find(doc! {"name": group_name}, GROUP).await;
    let subs = match subscriptions {
        Ok(g) => g.subscriptions,
        Err(_) => vec![],
    };

    let notebook = user
        .notebook
        .map(|n| n.to_string())
        .unwrap_or("None".to_string());

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
        ctx.insert("notebook", &notebook);
    }

    let content = helper::log_with_level!(data.render(USER_PAGE, &ctx), error)?;

    return Ok(HttpResponse::Ok().body(content));
}

#[patch("user")]
async fn group_update_user(
    db: Data<&DB>,
    mut pl: web::Payload,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    let _ = check_admin(subject, UserType::GroupAdmin)?;

    let mut body = web::BytesMut::new();
    while let Some(chunk) = pl.next().await {
        let chunk = chunk.unwrap();
        body.extend_from_slice(&chunk);
    }
    let body = String::from_utf8_lossy(&body);
    let form = serde_urlencoded::from_str::<UserGroupMod>(&body)?;

    // get current user information
    let user: Result<User> = db
        .find(
            doc! {
                "email": &form.email,
            },
            USER,
        )
        .await;
    let user = match user {
        Ok(user) => user,
        Err(_) => {
            dbg!("User not found");
            return Ok(HttpResponse::BadRequest()
                .append_header((
                    HTMX_ERROR_RES,
                    format!("<p>User {} does not exist</p>", form.email),
                ))
                .finish());
        }
    };

    let mut current_group = user.groups;

    let confirmation_message = match form.modify_user {
        ModifyUser::Add => {
            current_group.sort();
            match current_group.binary_search(&form.group_name) {
                Ok(_) => {
                    return Ok(HttpResponse::BadRequest()
                        .append_header((
                            HTMX_ERROR_RES,
                            format!(
                                "<p>User named {} already belongs to {}</p>",
                                form.email, form.group_name
                            ),
                        ))
                        .finish());
                }
                Err(_) => {
                    current_group.push(form.group_name.clone());
                    "Added"
                }
            }
        }
        ModifyUser::Remove => {
            current_group.sort();
            match current_group.binary_search(&form.group_name) {
                Ok(i) => {
                    current_group.remove(i);
                    "Removed"
                }
                Err(_) => {
                    return Ok(HttpResponse::BadRequest()
                        .append_header((
                            HTMX_ERROR_RES,
                            format!(
                                "<p>User named {} does not belong to {}</p>",
                                form.email, form.group_name
                            ),
                        ))
                        .finish());
                }
            }
        }
    };

    let _ = db
        .update(
            doc! {"email": &form.email},
            doc! {"$set": doc! {
                "groups": current_group,
                "updated_at": bson(time::OffsetDateTime::now_utc())?,
                "last_updated_by": &form.email,
            }},
            USER,
            PhantomData::<User>,
        )
        .await?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(confirmation_message))
}

#[get("tab")]
async fn group_tab_htmx(
    req: HttpRequest,
    db: Data<&DB>,
    data: Data<Tera>,
    subject: Option<ReqData<GuardianCookie>>,
) -> Result<HttpResponse> {
    let gc = check_admin(subject, UserType::GroupAdmin)?;
    let query = req.query_string();
    // deserialize into AdminTab
    let tab = helper::log_with_level!(serde_urlencoded::from_str::<AdminTabs>(query), error)?;

    // get the group you below to
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

    let content = match tab.tab {
        AdminTab::UserModify => {
            let group_name = user.groups.first().ok_or_else(|| {
                GuardianError::GeneralError(
                    "Group admin doesn't belong to any group... something is not right".to_string(),
                )
            })?;

            // get all members of group
            let members: Vec<User> = db.find_many(doc! {"groups": group_name }, USER).await?;

            let mut user_form = ModifyUserGroup::new();
            members.iter().for_each(|u| {
                user_form.add(u.email.clone());
            });
            helper::log_with_level!(user_form.render(&user.email, group_name, data), error)?
        }
        AdminTab::GroupView => {
            let group_name = user.groups.first().ok_or_else(|| {
                GuardianError::GeneralError(
                    "Group admin doesn't belong to any group... something is not right".to_string(),
                )
            })?;

            let group_members: Vec<User> = db.find_many(doc! {"groups": group_name }, USER).await?;

            let mut context = tera::Context::new();
            context.insert("group_members", &group_members);
            context.insert("group", group_name);
            context.insert("group_admin", &user.email);

            helper::log_with_level!(data.render("components/member_view.html", &context), error)?
        }
        _ => {
            return Ok(HttpResponse::BadRequest()
                .append_header((HTMX_ERROR_RES, format!("Tab {:?} not found", tab.tab)))
                .finish());
        }
    };

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(content))
}

pub fn config_group(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        web::scope("/group_admin")
            .service(
                web::scope("/hx")
                    .wrap(Htmx)
                    .service(group_tab_htmx)
                    .service(group_update_user),
            )
            .service(group),
    );
}
