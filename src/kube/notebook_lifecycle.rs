use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::{FutureExt, Stream};
use k8s_openapi::api::core::v1::Pod;
use pin_project::pin_project;
use serde::Deserialize;
use time::OffsetDateTime;
use tokio::time::{sleep, Instant, Sleep};
use tracing::info;

use crate::{errors::Result, kube::KubeAPI};

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
    dat: fn() -> F,
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
    fn new(dat: fn() -> F) -> Self {
        Self {
            state: State::Prep,
            dat,
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
                self.fut = Box::pin((self.dat)());
                self.state = State::Go;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            State::Go => match self.fut.poll_unpin(cx) {
                Poll::Ready(r) => {
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

pub async fn notebook_lifecycle() -> Result<()> {
    // get all the notesbook
    let pods = KubeAPI::<Pod>::get_all_pods().await?;
    // get the name of the notebook and ip address
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

    println!("{:?}", pods_detail);

    // call the kernel and get data for each notebook
    // filter the data by those idle for 24 hours or more
    // shutdown the notebook
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use serde_json::json;
    use tokio::time::timeout;

    async fn test() -> Result<()> {
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

        assert!(notebook_lifecycle().await.is_ok());
    }
}
