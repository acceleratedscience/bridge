use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::future::ok;
use k8s_openapi::api::core::v1::Pod;
use pin_project::pin_project;
use time::OffsetDateTime;
use tokio::time::{sleep, Instant, Sleep};
use tracing::info;

use crate::{errors::Result, kube::KubeAPI};

#[pin_project]
pub struct NotebookLifecycle<T, F> {
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

impl<T, F> NotebookLifecycle<T, F> {
    /// Create a new NotebookLifecycle instance
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

impl<T, F> Future for NotebookLifecycle<T, F>
where
    T: Future,
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
            match this.fut.poll(cx) {
                Poll::Ready(_) => {
                    info!("Notebook lifecycling has finished");
                    *this.expiration = OffsetDateTime::now_utc() + *this.exp_min;
                }
                Poll::Pending => {
                    info!("Notebook lifecycling is still pending");
                    return Poll::Pending;
                }
            }
        }

        // reset the sleep timer and timer
        this.sleep.reset(Instant::now() + *this.sleep_min);
        Poll::Pending
    }
}

async fn notebook_lifecycle() -> Result<()> {
    // get all the notesbook
    let pods = KubeAPI::<Pod>::get_all_pods().await?;
    println!("{:?}", pods);
    // call the kernel and get data for each notebook
    // filter the data by those idle for 24 hours or more
    // shutdown the notebook
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{sync::mpsc, time::timeout};

    #[tokio::test]
    async fn test_notebook_lifecycle() {
        let (tx, mut rx) = mpsc::channel(1);
        let exp_min = Duration::from_secs(2);
        let sleep_min = Duration::from_secs(1);
        let fut = tx.send(());
        let sigterm = sleep(Duration::from_secs(5));
        let lifecycle = NotebookLifecycle::new(exp_min, sleep_min, fut, sigterm);

        timeout(Duration::from_secs(10), lifecycle).await.unwrap();
        assert_eq!(rx.recv().await, Some(()));
    }
}
