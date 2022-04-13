use std::mem;

use pin_project::pin_project;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

use std::future::Future;
use std::pin::Pin;
use std::task::{self, Poll};

pub async fn channel() -> (Signal, Watch, WatchStream<()>) {
    let (tx, mut rx) = watch::channel(());
    rx.changed().await;
    (Signal { tx }, Watch { rx: rx.clone() }, WatchStream::new(rx))
}

pub struct Signal {
    tx: watch::Sender<()>,
}

pub struct Draining(Pin<Box<dyn Future<Output = ()> + Send + Sync>>);

#[derive(Clone)]
pub struct Watch {
    rx: watch::Receiver<()>,
}

#[allow(missing_debug_implementations)]
#[pin_project]
pub struct Watching<F, FN> {
    #[pin]
    future: F,
    state: State<FN>,
    watch: Pin<Box<dyn Future<Output = ()> + Send + Sync>>,
    _rx: watch::Receiver<()>,
}

enum State<F> {
    Watch(F),
    Draining,
}

impl Signal {
    pub fn drain(mut self) -> Draining {
        let _ = self.tx.send(());
        Draining(Box::pin(async move { self.tx.closed().await }))
    }
}

impl Future for Draining {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.as_mut().0).poll(cx)
    }
}

impl Watch {
    pub fn watch<F, FN>(self, future: F, on_drain: FN) -> Watching<F, FN>
    where
        F: Future,
        FN: FnOnce(Pin<&mut F>),
    {
        let Self { mut rx } = self;
        let _rx = rx.clone();
        Watching {
            future,
            state: State::Watch(on_drain),
            watch: Box::pin(async move {
                let _ = rx.changed().await;
            }),
            // Keep the receiver alive until the future completes, so that
            // dropping it can signal that draining has completed.
            _rx,
        }
    }
}

impl<F, FN> Future for Watching<F, FN>
where
    F: Future,
    FN: FnOnce(Pin<&mut F>),
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let mut me = self.project();
        loop {
            match mem::replace(me.state, State::Draining) {
                State::Watch(on_drain) => {
                    match Pin::new(&mut me.watch).poll(cx) {
                        Poll::Ready(()) => {
                            // Drain has been triggered!
                            on_drain(me.future.as_mut());
                        }
                        Poll::Pending => {
                            *me.state = State::Watch(on_drain);
                            return me.future.poll(cx);
                        }
                    }
                }
                State::Draining => return me.future.poll(cx),
            }
        }
    }
}
