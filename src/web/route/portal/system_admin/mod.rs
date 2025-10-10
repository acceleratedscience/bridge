use std::{marker::PhantomData, str::FromStr};

mod htmx;

use actix_web::{
    HttpRequest, HttpResponse,
    cookie::{Cookie, SameSite},
    delete, get,
    http::header::ContentType,
    patch, post,
    web::{self, Data, ReqData},
};
use mongodb::bson::{doc, oid::ObjectId};
use serde::Deserialize;
use tera::{Context, Tera};
use tracing::instrument;

use crate::{
    auth::COOKIE_NAME,
    db::{
        Database,
        models::{
            AdminTab, AdminTabs, BridgeCookie, GROUP, Group, GroupForm, GroupPortalRep,
            NotebookStatusCookie, OWUICookie, USER, User, UserDeleteForm, UserForm, UserPortalRep,
            UserType,
        },
        mongo::DB,
    },
    errors::{BridgeError, Result},
    web::{
        bridge_middleware::{HTMX_ERROR_RES, Htmx},
        helper::{self, bson, payload_to_struct},
        route::portal::helper::{check_admin, get_all_groups},
        services::CATALOG,
    },
};

#[cfg(feature = "notebook")]
use crate::web::route::portal::helper::notebook_bookkeeping;

use self::htmx::{
    CREATE_GROUP, GroupContent, MODIFY_GROUP, MODIFY_USER, UserContent, VIEW_GROUP, VIEW_USER,
};

const USER_PAGE: &str = "pages/portal_user.html";

#[get("")]
#[instrument(skip(data, db, subject))]
pub(super) async fn system(
    data: Data<Tera>,
    req: HttpRequest,
    context: Data<Context>,
    subject: Option<ReqData<BridgeCookie>>,
    nsc: Option<ReqData<NotebookStatusCookie>>,
    oc: Option<ReqData<OWUICookie>>,
    db: Data<&DB>,
) -> Result<HttpResponse> {
    // get the subject id from middleware
    #[cfg(feature = "notebook")]
    let mut bridge_cookie = check_admin(subject, UserType::SystemAdmin)?;
    #[cfg(not(feature = "notebook"))]
    let bridge_cookie = check_admin(subject, UserType::SystemAdmin)?;

    let id = ObjectId::from_str(&bridge_cookie.subject)
        .map_err(|e| BridgeError::GeneralError(e.to_string()))?;

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
            return Err(BridgeError::UserNotAllowedOnPage(USER_PAGE.to_string()));
        }
    }

    let group_name = user.groups.first().unwrap_or(&"".to_string()).clone();

    let subscriptions: Result<Group> = db.find(doc! {"name": group_name}, GROUP).await;
    let (subs, group_created_at, group_updated_at, group_last_updated) = match subscriptions {
        Ok(g) => (
            g.subscriptions,
            g.created_at.to_string(),
            g.updated_at.to_string(),
            g.last_updated_by.to_string(),
        ),
        Err(_) => (vec![], "".to_string(), "".to_string(), "".to_string()),
    };

    // // Expand subs into objects with service details
    // let expanded_subs_1: Vec<serde_json::Value> = subs
    //     .iter()
    //     .map(|service_name| {
    //         match CATALOG.get_service(service_name) {
    //             Ok(service_details) => {
    //                 serde_json::json!({
    //                     "service_details": service_details,
    //                 })
    //             }
    //             Err(_) => {
    //                 serde_json::json!({
    //                     "service_name": service_name,
    //                 })
    //             }
    //         }
    //     })
    //     .collect();

    // @placeholder: need to fetch subscription parameters from services.toml
    let subs_expanded: Vec<serde_json::Value> = subs
        .iter()
        .enumerate()
        .map(|(i, service_name)| {
            if i % 2 == 1 {
                serde_json::json!({
                    "type": "mcp",
                    "url": "https://dummy.url",
                    "name": service_name,
                    "code": format!("http://openad.accelerate.science/mcp/{}/mcp", service_name)
                })
            } else {
                serde_json::json!({
                    "type": "openad_model",
                    "url": "https://dummy.url",
                    "name": service_name,
                    "code": format!(
                        "catalog model service from remote 'https://open.accelerate.science/proxy' as {} using (auth_group=default inference-service={})",
                        "foobar",
                        service_name
                    )
                })
            }
        })
        .collect();

    let mut ctx = (**context).clone();
    ctx.insert("name", &user.user_name);
    ctx.insert("user_type", &user.user_type);
    ctx.insert("email", &user.email);
    ctx.insert("group", &user.groups);
    ctx.insert("subscriptions", &subs_expanded);
    ctx.insert("group_created_at", &group_created_at);
    ctx.insert("group_updated_at", &group_updated_at);
    ctx.insert("group_last_updated", &group_last_updated);
    ctx.insert("token", &user.token);
    if let Some(token) = &user.token {
        helper::add_token_exp_to_tera(&mut ctx, token);
    }

    #[cfg(feature = "openwebui")]
    if let Some(owui_cookie) = oc {
        use crate::config::CONFIG;

        ctx.insert("openwebui", &owui_cookie.subject);
        ctx.insert("owui_url", &CONFIG.openweb_url);
    }

    // add notebook tab if user has a notebook subscription
    #[cfg(feature = "notebook")]
    let nb_cookies = notebook_bookkeeping(&user, nsc, &mut bridge_cookie, &mut ctx, subs).await?;

    #[cfg(feature = "notebook")]
    if let Some(ref conf) = bridge_cookie.config {
        ctx.insert("pvc", &conf.notebook_persist_pvc);
    }

    if let Some(ref resources) = bridge_cookie.resources {
        let resources: Vec<(&String, bool)> = resources
            .iter()
            .map(|r| {
                let show = CATALOG
                    .get_details("resources", r, "show")
                    .map(|v| v.as_bool().unwrap_or(false));
                (r, show.unwrap_or(false))
            })
            .collect();
        ctx.insert("resources", &resources);
    }

    let bcj = serde_json::to_string(&bridge_cookie)?;
    let bc = Cookie::build(COOKIE_NAME, bcj)
        .path("/")
        .same_site(SameSite::Strict)
        .secure(true)
        .http_only(true)
        .max_age(time::Duration::days(1))
        .finish();

    let content = helper::log_with_level!(data.render(USER_PAGE, &ctx), error)?;

    #[cfg(feature = "notebook")]
    // no bound checks here
    if let Some([nc, nsc]) = nb_cookies {
        return Ok(HttpResponse::Ok()
            .cookie(nc)
            .cookie(nsc)
            .cookie(bc)
            .body(content));
    }

    return Ok(HttpResponse::Ok().cookie(bc).body(content));
}

#[instrument(skip(db, pl))]
#[post("group")]
async fn system_create_group(
    db: Data<&DB>,
    pl: web::Payload,
    subject: Option<ReqData<BridgeCookie>>,
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
        Ok(r) => format!("Group created with id: {r}"),
        Err(e) if e.to_string().contains("dup key") => {
            return Ok(HttpResponse::BadRequest()
                .append_header((
                    HTMX_ERROR_RES,
                    format!("Group '{}' already exists", gf.name),
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
//	 let content = format!("Group '{}' has been deleted", group.name);
//	 Ok(HttpResponse::Ok()
//		 .content_type(ContentType::form_url_encoded())
//		 .body(content))
// }

#[patch("group")]
async fn system_update_group(
    db: Data<&DB>,
    pl: web::Payload,
    subject: Option<ReqData<BridgeCookie>>,
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
                format!("Group '{}' does not exist", gf.name),
            ))
            .finish());
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(format!("Group '{}' has been updated", gf.name)))
}

#[patch("user")]
async fn system_update_user(
    db: Data<&DB>,
    pl: web::Payload,
    subject: Option<ReqData<BridgeCookie>>,
) -> Result<HttpResponse> {
    // TODO: do this at the middleware level
    let _ = check_admin(subject, UserType::SystemAdmin)?;
    let uf = payload_to_struct::<UserForm>(pl).await?;

    // stop self update
    if uf.email.eq(&uf.last_updated_by) {
        return Ok(HttpResponse::BadRequest()
            .append_header((HTMX_ERROR_RES, "Cannot update self".to_string()))
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
                format!("User with email address {} does not exist", uf.email),
            ))
            .finish());
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body(format!(
            "User with email address {} has been updated",
            uf.email
        )))
}

#[delete("user")]
async fn system_delete_user(
    db: Data<&DB>,
    req: HttpRequest,
    subject: Option<ReqData<BridgeCookie>>,
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
            .append_header((HTMX_ERROR_RES, "Cannot delete self".to_string()))
            .finish());
    }

    let _ = db
        .delete(doc! {"email": &uf.email}, USER, PhantomData::<User>)
        .await?;

    Ok(HttpResponse::Ok()
        .content_type(ContentType::form_url_encoded())
        .body("Deleted"))
}

#[get("tab")]
async fn system_tab_htmx(
    req: HttpRequest,
    db: Data<&DB>,
    data: Data<Tera>,
    subject: Option<ReqData<BridgeCookie>>,
) -> Result<HttpResponse> {
    println!("--- SYSTEM TAB ---");
    let gc = check_admin(subject, UserType::SystemAdmin)?;

    let id =
        ObjectId::from_str(&gc.subject).map_err(|e| BridgeError::GeneralError(e.to_string()))?;

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
        AdminTab::GroupModify
        | AdminTab::GroupList
        | AdminTab::GroupCreate
        | AdminTab::GroupView => {
            println!("--- DEFINE GROUPS ---");
            let mut group_form = GroupContent::new();
            CATALOG.get_all_by_name().iter().for_each(|name| {
                // TODO: remove this clone and use &'static str
                group_form.add(name.clone());
                println!("Added group name: {}", name);
            });

            match tab.tab {
                AdminTab::GroupView => {
                    let group_name = tab.group.unwrap_or_default();
                    let group_members: Vec<User> =
                        db.find_many(doc! {"groups": &group_name}, USER).await?;
                    let group_members: Vec<UserPortalRep> =
                        group_members.into_iter().map(|v| v.into()).collect();

                    let mut context = tera::Context::new();
                    context.insert("group_members", &group_members);
                    context.insert("group", &group_name);
                    context.insert("group_admin", &user.email);
                    context.insert("user_type", &user.user_type);

                    helper::log_with_level!(
                        data.render("components/member_view.html", &context),
                        error
                    )?
                }
                AdminTab::GroupList => {
                    let groups: Vec<Group> = db.find_many(doc! {}, GROUP).await.unwrap_or(vec![]);
                    let groups: Vec<GroupPortalRep> =
                        groups.into_iter().map(|group| group.into()).collect();
                    
                    // println!("Groups: {:#?}", groups);
                    helper::log_with_level!(
                        group_form.render(
                            &user.email,
                            data,
                            VIEW_GROUP,
                            Some(|ctx: &mut tera::Context| {
                                ctx.insert("groups", &groups);
                            }),
                        ),
                        error
                    )?
                }
                AdminTab::GroupCreate => helper::log_with_level!(
                    group_form.render(
                        &user.email,
                        data,
                        CREATE_GROUP,
                        None::<fn(&mut tera::Context)>,
                    ),
                    error
                )?,
                AdminTab::GroupModify => match tab.group {
                    Some(name) => {
                        let group_info: Group = db
                            .find(
                                doc! {
                                    "name": &name,
                                },
                                GROUP,
                            )
                            .await?;

                        let mut selections = group_form
                            .items
                            .iter()
                            .map(|v| (v.clone(), group_info.subscriptions.contains(v)))
                            .collect::<Vec<(String, bool)>>();
                        selections.sort_by_key(|(_, b)| !*b);

                        group_form.render(
                            &user.email,
                            data,
                            MODIFY_GROUP,
                            Some(|ctx: &mut tera::Context| {
                                ctx.insert("group_name", &name);
                                ctx.insert("selections", &selections);
                            }),
                        )?
                    }
                    None => return Err(BridgeError::GeneralError("No group provided".to_string())),
                },
                _ => unreachable!(),
            }
        }
        AdminTab::UserModify | AdminTab::UserView => {
            let mut user_form = UserContent::new();

            let target_user = tab
                .user
                .as_ref()
                .ok_or(BridgeError::GeneralError("No user provided".to_string()))?;

            get_all_groups(**db)
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
                _ => unreachable!("Group variants of enum should not be here"),
            }
        }
        AdminTab::UserList => data.render("components/systems_user.html", &tera::Context::new())?,
        // AdminTab::Main => helper::log_with_level!(
        //     data.render("components/systems_group.html", &tera::Context::new()),
        //     error
        // )?,
        AdminTab::Main => {
            let groups: Vec<Group> = db.find_many(doc! {}, GROUP).await.unwrap_or(vec![]);
            let groups: Vec<GroupPortalRep> =
                groups.into_iter().map(|group| group.into()).collect();

            let mut context = tera::Context::new();
            context.insert("groups", &groups);

            helper::log_with_level!(
                data.render("components/systems_group.html", &context),
                error
            )?
        },

        // @moe-trash
        // AdminTab::Main => {
        //     println!("--- TEST TEST TEST ---");
        //     // let mut group_form = GroupContent::new();

        //     // CATALOG.get_all_by_name().iter().for_each(|name| {
        //     //     // TODO: remove this clone and use &'static str
        //     //     group_form.add(name.clone());
        //     // });

        //     let groups: Vec<Group> = db.find_many(doc! {}, GROUP).await.unwrap_or(vec![]);
        //     let groups: Vec<GroupPortalRep> =
        //         groups.into_iter().map(|group| group.into()).collect();

        //     helper::log_with_level!(
        //         group_form.render(
        //             &user.email,
        //             data,
        //             VIEW_GROUP,
        //             Some(|ctx: &mut tera::Context| {
        //                 ctx.insert("groups", &groups);
        //             }),
        //         ),
        //         error
        //     )?
        // }
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
