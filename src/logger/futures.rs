use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use pin_project::pin_project;
use tokio::{
    sync::{broadcast::error::RecvError, mpsc::Receiver},
    time::{Sleep, sleep},
};

static MAX_CAP: usize = 100;
static WAIT: u64 = 60 * 60;

#[pin_project]
pub struct FutureRace<T, F> {
    fut: Arc<Mutex<Receiver<T>>>,
    #[pin]
    timer: Sleep,
    events: Option<Vec<T>>,
    #[pin]
    term: F,
}

impl<T, F> FutureRace<T, F> {
    pub fn new(fut: Arc<Mutex<Receiver<T>>>, term: F) -> Self {
        // Create a timer that will sleep for 60 minutes
        let sleep = sleep(Duration::from_secs(WAIT));
        let events = Vec::with_capacity(MAX_CAP);
        Self {
            fut,
            timer: sleep,
            events: Some(events),
            term,
        }
    }
}

impl<T, F> Future for FutureRace<T, F>
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

            match this.fut.lock().unwrap().poll_recv(cx) {
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
