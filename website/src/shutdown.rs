use axum::{extract::FromRequestParts, http::request::Parts, Extension};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::{
    oneshot::{channel, Receiver, Sender},
    Mutex,
};

#[derive(Debug, Clone)]
pub struct Shutdown(Arc<Mutex<Option<Sender<()>>>>);

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

pub struct Signal(Receiver<()>);

impl Future for Signal {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let receiver = unsafe { self.map_unchecked_mut(|inner| &mut inner.0) };

        receiver.poll(cx).map(|_| ())
    }
}

impl Shutdown {
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
    pub async fn notify(&self) {
        if let Some(tx) = self.0.lock().await.take() {
            tx.send(())
                .expect("shutdown listener was dropped before shutdown was notified");
        }
    }
}
