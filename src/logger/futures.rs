use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use parking_lot::Mutex;
use pin_project::pin_project;
use tokio::{
    sync::{broadcast::error::RecvError, mpsc::Receiver},
    time::{Sleep, sleep},
};

static MAX_CAP: usize = 100;

#[pin_project]
pub struct FutureBatch<T, F> {
    fut: Arc<Mutex<Receiver<T>>>,
    #[pin]
    timer: Sleep,
    events: Option<Vec<T>>,
    #[pin]
    term: F,
}

impl<T, F> FutureBatch<T, F> {
    pub fn new(fut: Arc<Mutex<Receiver<T>>>, term: F, timer: Duration) -> Self {
        // Create a timer that will sleep for 60 minutes
        let sleep = sleep(timer);
        let events = Vec::with_capacity(MAX_CAP);
        Self {
            fut,
            timer: sleep,
            events: Some(events),
            term,
        }
    }
}

impl<T, F> Future for FutureBatch<T, F>
where
    F: Future<Output = Result<(), RecvError>>,
{
    type Output = Option<Vec<T>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if this.timer.poll(cx).is_ready() {
            return Poll::Ready(this.events.take());
        }

        // the channel is buffered and multiple events can be received, so we should drain it
        loop {
            if this.term.as_mut().poll(cx).is_ready() {
                return Poll::Ready(this.events.take());
            }

            // TODO: replace this with poll_recv_many
            match this.fut.lock().poll_recv(cx) {
                Poll::Ready(Some(event)) => {
                    if let Some(events) = this.events.as_mut() {
                        events.push(event);
                        if events.len() >= MAX_CAP {
                            return Poll::Ready(this.events.take());
                        }
                    }
                }
                Poll::Ready(None) => return Poll::Ready(this.events.take()),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
