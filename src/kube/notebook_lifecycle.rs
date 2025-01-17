use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::{FutureExt, Stream};
use k8s_openapi::api::core::v1::Pod;
use mongodb::bson::{doc, oid::ObjectId};
use pin_project::pin_project;
use reqwest::Client;
use serde::Deserialize;
use time::OffsetDateTime;
use tokio::time::{sleep, Instant, Sleep};
use tracing::info;

use crate::{
    db::{
        models::{NotebookInfo, User, USER},
        mongo::{ObjectID, DB, DBCONN},
        Database,
    },
    errors::{BridgeError, Result},
    kube::KubeAPI,
    web::{
        notebook_helper::{make_forward_url, make_notebook_name},
        utils,
    },
};

// TODO: move this to notebook.toml... max_idle_time already exists
const MAX_IDLE_TIME: u64 = 60 * 60 * 24;
const BG_LEASE: &str = "bridge-lease";

#[pin_project]
pub struct Medium<T, F> {
    expiration: OffsetDateTime,
    #[pin]
    sleep: Sleep,
    slept: bool,
    sleep_min: Duration,
    exp_min: Duration,
    db: &'static DB,
    #[pin]
    stream: T,
    #[pin]
    sigterm: F,
    fut: Pin<Box<dyn Future<Output = Result<()>> + Send>>,
    leased: bool,
}

impl<T, F> Medium<T, F> {
    /// Create a new Medium instance
    pub fn new(
        exp_min: Duration,
        sleep_min: Duration,
        db: &'static DB,
        stream: T,
        sigterm: F,
    ) -> Self {
        let expiration = OffsetDateTime::now_utc() + exp_min;
        Self {
            expiration,
            sleep: sleep(sleep_min),
            sleep_min,
            exp_min,
            db,
            slept: false,
            stream,
            sigterm,
            fut: Box::pin(db.get_lease(BG_LEASE, exp_min.as_secs() as i64)),
            leased: false,
        }
    }
}

impl<T, F> Future for Medium<T, F>
where
    T: Stream + Send,
    F: Future + Send,
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
            info!("Shutting down lifecycle");
            return Poll::Ready(());
        }

        let now = OffsetDateTime::now_utc();

        if now >= *this.expiration {
            if !*this.leased {
                // try to get the advisory lock lease...
                match this.fut.as_mut().poll(cx) {
                    Poll::Ready(r) => {
                        *this.fut =
                            Box::pin(this.db.get_lease(BG_LEASE, this.exp_min.as_secs() as i64));
                        if r.is_ok() {
                            info!("Look at me, I'm the captain now...");
                            // lease acquired move to streaming
                            *this.leased = true;
                        } else {
                            *this.expiration = OffsetDateTime::now_utc() + *this.exp_min;
                        }
                    }
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                }
            }

            if *this.leased {
                match this.stream.poll_next(cx) {
                    Poll::Ready(Some(_)) => {
                        info!("Notebook lifecycling has finished");
                        // reset
                        *this.expiration = OffsetDateTime::now_utc() + *this.exp_min;
                        // lease expired
                        *this.leased = false;
                    }
                    Poll::Ready(None) => {
                        return Poll::Ready(());
                    }
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                }
            }
        }

        this.sleep.reset(Instant::now() + *this.sleep_min);
        *this.slept = false;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

pub struct LifecycleStream<F> {
    state: State,
    dat: fn(Client) -> F,
    client: Client,
    fut: Pin<Box<dyn Future<Output = Result<()>> + Send>>,
}

enum State {
    Prep,
    Go,
}

impl<F> LifecycleStream<F>
where
    F: Future<Output = Result<()>>,
{
    pub fn new(dat: fn(Client) -> F) -> Self {
        Self {
            state: State::Prep,
            dat,
            client: Client::new(),
            fut: Box::pin(async { Ok(()) }),
        }
    }
}

impl<F> Stream for LifecycleStream<F>
where
    F: Future<Output = Result<()>> + Send + 'static,
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
            // yo time be hard... please don't break
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
        .ok_or(BridgeError::GeneralError("DB connection failed".into()))?;

    let pods = KubeAPI::<Pod>::get_all_pods().await?;
    if pods.is_empty() {
        info!("No running notebooks found");
        return Ok(());
    }

    // get the subject id and corresponding ip of all the notebooks
    let pods_detail: Vec<(String, String)> = pods
        .into_iter()
        .filter_map(|mut pod| {
            Some((
                pod.metadata
                    .name
                    .and_then(|s| s.split("-").next().map(|s| s.to_owned()))?,
                pod.status.as_mut()?.pod_ip.take()?,
            ))
        })
        .collect();

    // hashmap key: subject id and value: forward url
    let mut addresses = pods_detail
        .into_iter()
        .map(|(id, ip)| {
            let url =
                make_forward_url(&ip, &make_notebook_name(&id), "http", None) + "/api/kernels";
            (id, (url, None))
        })
        .collect::<HashMap<String, (String, Option<User>)>>();

    let all_ids = addresses
        .keys()
        .map(|id| ObjectID::new(id).into_inner())
        .collect::<Vec<ObjectId>>();

    // TODO: look into optimizing this with FindOptions to only get id and start_time
    let users: Vec<User> = crate::log_with_level!(
        db.find_many(
            doc! {
                "_id": {"$in": all_ids}
            },
            USER,
        )
        .await,
        error
    )?;

    users.into_iter().for_each(|u| {
        if u.notebook.is_some() {
            let id = u._id.to_string();
            let v = addresses.get_mut(&id).unwrap();
            v.1 = Some(u);
        }
    });

    let now = OffsetDateTime::now_utc();

    // call the kernel and get data for each notebook
    // filter the data by those idle for 24 hours or more
    let mut notebook_to_shutdown = Vec::with_capacity(addresses.len());
    for (id, (url, user)) in addresses.into_iter() {
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
            if let Some(User {
                notebook:
                    Some(NotebookInfo {
                        start_time: Some(t),
                        ..
                    }),
                ..
            }) = user
            {
                if (now - t) >= Duration::from_secs(MAX_IDLE_TIME) {
                    notebook_to_shutdown.push((id, user));
                }
                continue;
            }

            // One or more not present: kernel, notebook, start_time. In these situation, mark for
            // deletion
            notebook_to_shutdown.push((id, None));
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
                notebook_to_shutdown.push((id, user));
            }
        }
    }

    // shutdown the notebook
    for (id, user) in notebook_to_shutdown {
        let persist_pvc = match user {
            Some(u) => u.notebook.map(|n| n.persist_pvc).unwrap_or(false),
            None => false,
        };

        match utils::notebook_destroy(db, &id, persist_pvc, "system").await {
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
        println!("{:?}", resp);
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
        let mut fut = LifecycleStream::new(test);
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
        let fut = LifecycleStream::new(test);
        let sigterm = sleep(Duration::from_secs(5));
        DB::init_once("guardian").await.unwrap();
        let db = DBCONN.get().unwrap();
        let lifecycle = Medium::new(exp_min, sleep_min, db, fut, sigterm);

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
