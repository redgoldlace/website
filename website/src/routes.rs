use std::{
    future::Future,
    path::{Path as FsPath, PathBuf},
    pin::Pin,
    str::FromStr,
    task::{Context as TaskContext, Poll},
};

use crate::{context, page::Page, shutdown::Shutdown, state::State};
use axum::{
    body::Bytes,
    extract::{FromRequestParts, Path},
    http::{request::Parts, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use hex::ToHex;
use hmac::{Hmac, Mac, NewMac};
use hyper::header;
use serde_json::Value;
use sha2::Sha256;
use tera::Context;

#[derive(Debug, Clone)]
pub struct StaticPage {
    state: State,
    path: PathBuf,
}

impl StaticPage {
    pub fn new(state: State, path: PathBuf) -> Self {
        Self { state, path }
    }
}

impl Future for StaticPage {
    type Output = Result<Html<String>, (StatusCode, String)>;

    fn poll(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let result = Page::simple(&self.path)
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
            .and_then(|page| page.render(&self.state.engine()));

        Poll::Ready(result)
    }
}

/// Create a handler that renders the page at `path`, relative to the application's content directory.
pub fn simple<T>(path: &'static T) -> impl (Fn(State) -> StaticPage) + Clone
where
    T: AsRef<FsPath> + ?Sized,
{
    move |state| {
        let full_path = state.config().content_dir().join(path);

        StaticPage::new(state, full_path)
    }
}

pub async fn post_list(state: State) -> Response {
    let context_for = |slug: &str, context: &Context| -> Option<Value> {
        let mut new = Context::new();
        new.insert("slug", slug);
        new.insert("title", context.get("title")?);
        new.insert("published", context.get("published")?);

        Some(new.into_json())
    };

    let posts: Vec<_> = state
        .posts()
        .iter()
        .filter_map(|(slug, page)| context_for(slug, page.context()))
        .collect();

    let page = Page::new(
        "post-list",
        context! {
            "title" => "Kaylynn's blog",
            "posts" => posts,
        },
    );

    page.render(state.engine()).into_response()
}

pub async fn post(Path(slug): Path<String>, state: State) -> Response {
    state
        .posts()
        .get(&slug)
        .map(|page| page.render(state.engine()))
        .ok_or((StatusCode::NOT_FOUND, "Blog post not found!"))
        .into_response()
}

pub async fn rss_feed(state: State) -> Response {
    let rss = state.posts().rss();
    let headers = [(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/rss+xml; charset=UTF-8"),
    )];

    // Panic safety:
    // A) Vectors will grow when required.
    // B) The various inputs are already valid UTF-8.
    let buffer = rss.pretty_write_to(Vec::new(), b' ', 2).unwrap();
    let xml = String::from_utf8(buffer).unwrap();

    (headers, xml).into_response()
}

trait MacExt {
    fn with_data(self, data: &[u8]) -> Self;
}

impl<M: Mac> MacExt for M {
    fn with_data(mut self, data: &[u8]) -> Self {
        self.update(data);
        self
    }
}

pub struct Secret(String);

impl Secret {
    fn value(&self) -> &str {
        &self.0
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for Secret
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let raw_signature = parts
            .headers
            .get("X-Hub-Signature-256")
            .ok_or((StatusCode::UNAUTHORIZED, "Missing signature"))?
            .as_bytes();

        std::str::from_utf8(raw_signature)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid UTF-8 in signature"))?
            .trim()
            .strip_prefix("sha256=")
            .ok_or((StatusCode::BAD_REQUEST, "Invalid signature format"))
            .map(str::to_owned)
            .map(Secret)
    }
}

pub async fn deploy(
    shutdown: Shutdown,
    request_secret: Secret,
    state: State,
    body: Bytes,
) -> Result<(), (StatusCode, &'static str)> {
    let secret = state
        .config()
        .webhook_secret()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No secret configured"))?
        .as_bytes();

    let sha = Hmac::<Sha256>::new_from_slice(secret)
        .unwrap()
        .with_data(body.as_ref())
        .finalize()
        .into_bytes()
        .encode_hex::<String>();

    if sha != request_secret.value() {
        return Err((StatusCode::UNAUTHORIZED, "Invalid signature"));
    }

    let raw = String::from_utf8_lossy(body.as_ref());
    let payload = Value::from_str(&raw)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid JSON in request body"))?;

    // We only want to trigger a shutdown once the actions run is completed and a new image is present on Docker Hub
    if payload["action"] == "completed" {
        shutdown.notify();
    }

    Ok(())
}
