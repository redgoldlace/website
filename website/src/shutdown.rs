use axum::{extract::FromRequestParts, http::request::Parts, Extension};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tokio::sync::oneshot::{channel, Receiver, Sender};

/// A `Shutdown` value is a cheaply cloneable handle to an asynchronous shutdown signal that can be shared freely
/// between threads. Once a shutdown is desired, the `notify()` method may be used to make the associated `Signal`'s
/// future resolve.
///
/// `Shutdown` implements `FromRequestParts`, so it can be used as an extractor in axum applications. The underlying
/// `FromRequestParts` implementation delegates to `Extension(FromRequestParts)`, so you must register the `Shutdown`
/// value using request extensions for the extractor to complete successfully.
#[derive(Debug, Clone)]
pub struct Shutdown(Arc<Mutex<Option<Sender<()>>>>);

impl Shutdown {
    /// Create a new `(shutdown, signal)` pair. `signal.await` will block until `shutdown.notify()` is called.
    ///
    /// Note that this function returns a *new* shutdown/signal pair each time it is called. Calling `Shutdown.notify()` does
    /// not cause **all** `Signal`s to resolve; only the `Signal` it was created with is resolved.
    ///
    /// Since `Shutdown` is cheaply cloneable, you should avoid calling this function multiple times.
    pub fn new() -> (Self, Signal) {
        let (tx, rx) = channel();

        let shutdown = Shutdown(Arc::new(Mutex::new(Some(tx))));
        let signal = Signal(rx);

        (shutdown, signal)
    }

    /// Notify the server that it should prepare for graceful shutdown.
    ///
    /// # Panic
    ///
    /// Panics if the associated `Shutdown` listener was dropped.
    pub fn notify(&self) {
        if let Some(tx) = self.0.lock().unwrap().take() {
            tx.send(()).expect("shutdown listener already dropped");
        }
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for Shutdown
where
    S: Send + Sync,
{
    type Rejection = <Extension<Shutdown> as FromRequestParts<S>>::Rejection;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Extension(state) = Extension::<Shutdown>::from_request_parts(parts, state).await?;

        Ok(state)
    }
}

/// A `Future` that only resolves when `shutdown.notify()` is called.
pub struct Signal(Receiver<()>);

impl Future for Signal {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let receiver = unsafe { self.map_unchecked_mut(|inner| &mut inner.0) };

        receiver.poll(cx).map(|_| ())
    }
}
