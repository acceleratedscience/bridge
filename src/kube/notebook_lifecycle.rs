use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::{FutureExt, Stream};
use k8s_openapi::api::core::v1::Pod;
use mongodb::bson::doc;
use pin_project::pin_project;
use reqwest::Client;
use serde::Deserialize;
use time::OffsetDateTime;
use tokio::time::{sleep, Instant, Sleep};
use tracing::info;

use crate::{
    db::{
        models::{User, USER},
        mongo::{ObjectID, DBCONN},
        Database,
    },
    errors::{GuardianError, Result},
    kube::KubeAPI,
    web::{
        notebook_helper::{make_forward_url, make_notebook_name},
        utils,
    },
};

// TODO: move this to notebook.toml... max_idle_time already exists
const MAX_IDLE_TIME: u64 = 60 * 60 * 24;

#[pin_project]
pub struct Medium<T, F> {
    expiration: OffsetDateTime,
    #[pin]
    sleep: Sleep,
    slept: bool,
    sleep_min: Duration,
    exp_min: Duration,
    #[pin]
    fut: T,
    #[pin]
    sigterm: F,
}

impl<T, F> Medium<T, F> {
    /// Create a new Medium instance
    pub fn new(exp_min: Duration, sleep_min: Duration, fut: T, sigterm: F) -> Self {
        let expiration = OffsetDateTime::now_utc() + exp_min;
        Self {
            expiration,
            sleep: sleep(sleep_min),
            sleep_min,
            exp_min,
            slept: false,
            fut,
            sigterm,
        }
    }
}

impl<T, F> Future for Medium<T, F>
where
    T: Stream,
    F: Future,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if !*this.slept {
            match this.sleep.as_mut().poll(cx) {
                Poll::Ready(_) => {
                    *this.slept = true;
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }

        // check if we need to shutdown
        if this.sigterm.poll(cx).is_ready() {
            info!("Received SIGTERM, shutting down lifecycle");
            return Poll::Ready(());
        }

        let now = OffsetDateTime::now_utc();

        if now >= *this.expiration {
            match this.fut.poll_next(cx) {
                Poll::Ready(Some(_)) => {
                    info!("Notebook lifecycling has finished");
                    *this.expiration = OffsetDateTime::now_utc() + *this.exp_min;
                }
                Poll::Ready(None) => {
                    return Poll::Ready(());
                }
                Poll::Pending => {
                    info!("Notebook lifecycling is still pending");
                    return Poll::Pending;
                }
            }
        }

        // reset the sleep timer and timer
        this.sleep.reset(Instant::now() + *this.sleep_min);
        *this.slept = false;
        Poll::Pending
    }
}

struct NotebookLifecycle<F> {
    state: State,
    dat: fn(Client) -> F,
    client: Client,
    fut: Pin<Box<dyn Future<Output = Result<()>>>>,
}

enum State {
    Prep,
    Go,
}

impl<F> NotebookLifecycle<F>
where
    F: Future<Output = Result<()>>,
{
    fn new(dat: fn(Client) -> F) -> Self {
        Self {
            state: State::Prep,
            dat,
            client: Client::new(),
            fut: Box::pin(async { Ok(()) }),
        }
    }
}

impl<F> Stream for NotebookLifecycle<F>
where
    F: Future<Output = Result<()>> + 'static,
{
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.state {
            State::Prep => {
                self.fut = Box::pin((self.dat)(self.client.clone()));
                self.state = State::Go;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            State::Go => match self.fut.poll_unpin(cx) {
                Poll::Ready(_r) => {
                    self.state = State::Prep;
                    cx.waker().wake_by_ref();
                    Poll::Ready(Some(()))
                }
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct Kernel {
    // id: String,
    // name: String,
    last_activity: String,
    // execution_state: String,
    connections: u32,
}

impl From<&Kernel> for OffsetDateTime {
    fn from(value: &Kernel) -> Self {
        let format = time::macros::format_description!(
            version = 1,
            "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z"
        );
        let last_activity = &value.last_activity;
        time::PrimitiveDateTime::parse(last_activity, &format)
            .map(|t| t.assume_utc())
            .unwrap_or(OffsetDateTime::now_utc())
    }
}

pub async fn notebook_lifecycle(client: Client) -> Result<()> {
    info!("Running notebook lifecycle");

    let db = DBCONN
        .get()
        .ok_or(GuardianError::GeneralError("DB connection failed".into()))?;

    let pods = KubeAPI::<Pod>::get_all_pods().await?;

    // get the ip and the subject id of all the notebooks
    let pods_detail: Vec<(String, String)> = pods
        .into_iter()
        .filter_map(|mut pod| {
            Some((
                pod.status.as_mut()?.pod_ip.take()?,
                pod.metadata
                    .name
                    .and_then(|s| s.split("-").next().map(|s| s.to_owned()))?,
            ))
        })
        .collect();

    // map ip into a forward url
    let addresses = pods_detail
        .into_iter()
        .map(|(ip, id)| {
            (
                make_forward_url(&ip, &make_notebook_name(&id), "http", None) + "/api/kernels",
                id,
            )
        })
        .collect::<Vec<(String, String)>>();

    let now = OffsetDateTime::now_utc();

    // call the kernel and get data for each notebook
    // filter the data by those idle for 24 hours or more
    let mut results = Vec::with_capacity(addresses.len());
    let mut results_db_chk = Vec::new();
    for (url, id) in addresses.iter() {
        info!("Checking notebook {}", id);

        let resp = client.get(url).send().await?;
        if !resp.status().is_success() {
            info!("Notebook {} failed to respond... skipping", id);
            continue;
        }
        let body = resp.text().await?;
        let kernels: Vec<Kernel> = serde_json::from_str(&body)?;
        // If the notebook was never opened...
        if kernels.is_empty() {
            info!(
                "Notebook {} has no kernel... checking DB for latest activity",
                id
            );
            // check DB for last activity
            results_db_chk.push(ObjectID::new(id).into_inner());
            continue;
        }
        // convert last_activity to OffsetDateTime, sort to get latest
        let mut last_activities = kernels
            .into_iter()
            .filter(|k| k.connections.eq(&0))
            .map(|k| (&k).into())
            .collect::<Vec<OffsetDateTime>>();
        last_activities.sort();
        if let Some(t) = last_activities.last() {
            if (now - *t) >= Duration::from_secs(MAX_IDLE_TIME) {
                results.push(id.clone());
            }
        }
    }

    // If the user never open their notebook after spinning it up.. the kernel has no information.
    // In fact, the json is an empty array. Since we cannot let these linger forever...
    if !results_db_chk.is_empty() {
        // TODO: look into optimizing this with FindOptions to only get id and start_time
        let users: Vec<User> = db
            .find_many(doc! {"_id": {"$in": results_db_chk}}, USER)
            .await?;

        results.extend(users.into_iter().filter_map(|u| {
            u.notebook
                .and_then(|n| n.start_time)
                .filter(|t| (now - *t) >= Duration::from_secs(MAX_IDLE_TIME))
                .map(|_| u._id.to_string())
        }));
    }

    // shutdown the notebook
    for id in results {
        match utils::notebook_destroy(db, &id, true, "system").await {
            Ok(_) => info!(
                "Notebook {} has been destroyed for being idle for +{} seconds",
                id, MAX_IDLE_TIME
            ),
            Err(e) => info!("Notebook {} failed to be destroyed: {}", id, e),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::mongo::DB;
    use crate::logger;

    use super::*;
    use futures::StreamExt;
    use serde_json::json;
    use tokio::time::timeout;
    use tracing::level_filters::LevelFilter;

    async fn test(client: Client) -> Result<()> {
        // ping postman echo
        let resp = client.get("https://postman-echo.com/get").send().await?;
        assert!(resp.status().is_success());
        Ok(())
    }

    #[test]
    fn test_kernel_deserialize() {
        let kernel_json = json!([
            {
                "id": "d0dfe0cc-2733-4820-9314-32dc64aa67f0",
                "name": "python3",
                "last_activity": "2024-12-19T03:45:33.622387Z",
                "execution_state": "idle",
                "connections": 1
            },
            {
                "id": "8b4c5df2-ee6f-49f2-908f-75e23d47bf88",
                "name": "python3",
                "last_activity": "2024-12-19T03:45:01.038322Z",
                "execution_state": "idle",
                "connections": 0
            },
            {
                "id": "1c80cdad-3606-4fce-9c9d-dadeae542171",
                "name": "python3",
                "last_activity": "2024-12-19T03:48:34.163393Z",
                "execution_state": "idle",
                "connections": 1
            }
        ]);

        let k: Vec<Kernel> = serde_json::from_value(kernel_json).unwrap();
        let mut col = k
            .into_iter()
            .map(|ref k| k.into())
            .collect::<Vec<OffsetDateTime>>();

        col.sort();

        col.windows(2)
            .for_each(|w| assert!(w[0] <= w[1], "{:?} <= {:?}", w[0], w[1]));
    }

    #[test]
    fn test_kernel_deserialize_empty() {
        let kernel_json = json!([]);
        let k: Vec<Kernel> = serde_json::from_value(kernel_json).unwrap();
        assert!(k.is_empty());
    }

    #[tokio::test]
    async fn test_notebook_lifecycle() {
        let mut fut = NotebookLifecycle::new(test);
        let mut count = 0;
        for _ in 0..10 {
            match fut.next().await {
                Some(_) => (),
                None => break,
            }
            count += 1;
        }
        assert_eq!(count, 10);
    }

    #[tokio::test]
    async fn test_medium() {
        let exp_min = Duration::from_secs(1);
        let sleep_min = Duration::from_secs(1);
        let fut = NotebookLifecycle::new(test);
        let sigterm = sleep(Duration::from_secs(5));
        let lifecycle = Medium::new(exp_min, sleep_min, fut, sigterm);

        timeout(Duration::from_secs(10), lifecycle).await.unwrap();
    }

    #[tokio::test]
    async fn test_notebook_fn() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("Cannot install default provider");

        crate::kube::init_once().await;
        DB::init_once("guardian").await.unwrap();
        logger::Logger::start(LevelFilter::INFO);
        let result = notebook_lifecycle(Client::new()).await;
        println!("{:?}", result);
        assert!(result.is_ok());
    }
}
