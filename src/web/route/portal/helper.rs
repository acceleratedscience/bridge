use std::any::Any;
#[cfg(feature = "notebook")]
use std::ops::Deref;

#[cfg(feature = "notebook")]
use actix_web::cookie::{Cookie, SameSite};
use actix_web::web::ReqData;
use mongodb::bson::doc;
#[cfg(feature = "notebook")]
use tera::Context;

use crate::{
    db::{
        models::{Group, GuardianCookie, User, UserType, GROUP},
        mongo::DB,
        Database,
    },
    errors::{GuardianError, Result},
    web::helper,
};

#[cfg(feature = "notebook")]
use crate::{
    auth::{NOTEBOOK_COOKIE_NAME, NOTEBOOK_STATUS_COOKIE_NAME},
    db::models::{NotebookCookie, NotebookStatusCookie, UserNotebook},
    kube::KubeAPI,
    web::{notebook_helper, route::notebook::NOTEBOOK_SUB_NAME},
};
#[cfg(feature = "notebook")]
use k8s_openapi::api::core::v1::Pod;

#[allow(dead_code)]
#[allow(unused_variables)]
pub(super) fn portal_hygienic_group(gc: &GuardianCookie, doc: &dyn Any) -> Result<bool> {
    // Downcast to Group
    if let Some(group) = doc.downcast_ref::<Group>() {
        // group_admin cannot edit groups
        return Ok(false);
    }

    if let Some(user) = doc.downcast_ref::<User>() {
        // if user.groups.contains(gc.user_type)
        return Ok(true);
    }

    Err(GuardianError::GeneralError(
        "Portal Hygienic Error".to_string(),
    ))
}

pub(super) fn check_admin(
    subject: Option<ReqData<GuardianCookie>>,
    admin: UserType,
) -> Result<GuardianCookie> {
    Ok(match subject {
        // System admin
        Some(cookie_subject) if cookie_subject.user_type == admin => cookie_subject.into_inner(),
        // All other users
        Some(g) => {
            return helper::log_with_level!(
                Err(GuardianError::UserNotFound(format!(
                    "User {} is not a system admin",
                    g.into_inner().subject
                ))),
                error
            )
        }
        None => {
            return helper::log_with_level!(
                Err(GuardianError::UserNotFound(
                    "No user passed from middleware... subject not passed from middleware"
                        .to_string(),
                )),
                error
            )
        }
    })
}

/// This is a helper function to get all Groups from the database
pub(super) async fn get_all_groups(db: &DB) -> Result<Vec<Group>> {
    let result: Result<Vec<Group>> = db.find_many(doc! {}, GROUP).await;
    Ok(match result {
        Ok(groups) => groups,
        Err(e) => return helper::log_with_level!(Err(e), warn),
    })
}

#[cfg(feature = "notebook")]
/// This is a helper function that takes care of the notebook setup for all users
/// Is the user does not have access to notebooks, None is returned
pub(super) async fn notebook_bookkeeping<'c, C>(
    user: &User,
    nsc: Option<ReqData<NotebookStatusCookie>>,
    ctx: &mut Context,
    subscription: Vec<C>,
) -> Result<Option<[Cookie<'c>; 2]>>
where
    C: Deref<Target = str>,
{
    // Check is user is allowed to access the notebook
    if subscription
        .iter()
        .map(Deref::deref)
        .collect::<Vec<&str>>()
        .contains(&NOTEBOOK_SUB_NAME)
    {
        // For notwbook UI component
        let mut user_notebook = Into::<UserNotebook>::into(user);

        // Check if the user has running notebook
        match nsc {
            Some(nsc) => {
                let nsc = nsc.into_inner();
                user_notebook.status = nsc.status;
            }
            None => {
                // There may be a case where the user has no notebook status cookie... perhaps
                // cleared the browser history while the notebook was still running. If the
                // notebook_status_cookie is not present, there is a pretty high chance the
                // notebook_cookie isn't there either...
                if let Some(nb_start) = &user.notebook {
                    let sub = notebook_helper::make_notebook_name(&user._id.to_string());
                    match KubeAPI::<Pod>::check_pod_running(&(sub.clone() + "-0")).await {
                        Ok(running) => {
                            if running {
                                user_notebook.status = "Ready".to_string();
                                ctx.insert("notebook", &user_notebook);

                                let ip = KubeAPI::<Pod>::get_pod_ip(&(sub + "-0")).await?;
                                let notebook_cookie = NotebookCookie {
                                    subject: user._id.to_string(),
                                    ip,
                                };
                                let nc_json = serde_json::to_string(&notebook_cookie)?;
                                let nc_cookie = Cookie::build(NOTEBOOK_COOKIE_NAME, nc_json)
                                    .path("/notebook")
                                    .same_site(SameSite::Strict)
                                    .secure(true)
                                    .http_only(true)
                                    .max_age(time::Duration::days(1))
                                    .finish();

                                let notebook_status_cookie = NotebookStatusCookie {
                                    status: "Ready".to_string(),
                                    start_time: nb_start
                                        .start_time
                                        .map(|t| t.to_string())
                                        .unwrap_or_default(),
                                    start_url: nb_start.start_up_url.clone(),
                                };
                                let nsc_json = serde_json::to_string(&notebook_status_cookie)?;
                                let nsc_cookie =
                                    Cookie::build(NOTEBOOK_STATUS_COOKIE_NAME, nsc_json)
                                        .path("/")
                                        .same_site(SameSite::Strict)
                                        .secure(true)
                                        .http_only(true)
                                        .max_age(time::Duration::days(1))
                                        .finish();

                                return Ok(Some([nc_cookie, nsc_cookie]));
                            }
                        }
                        Err(err) => return Err(err),
                    }
                }
            }
        }

        ctx.insert("notebook", &user_notebook);
    }

    Ok(None)
}
