use std::str::FromStr;

use crate::{
    context,
    page::{Page, PageKind},
    WrappedPostMap, SECRET,
};
use hex::ToHex;
use hmac::{Hmac, Mac, NewMac};
use rocket::{
    data::ToByteUnit, http::Status, outcome::IntoOutcome, request::FromRequest,
    response::content::Xml, Data, Request, Shutdown, State,
};
use serde_json::Value;
use sha2::Sha256;

#[rocket::catch(default)]
pub fn default_catcher(status: Status, _: &Request) -> Page {
    Page::new(
        PageKind::Error,
        context! {
            "reason" => format!("Status code {}: {}.", status.code, status.reason_lossy())
        },
    )
}

async fn render_simple(title: &str, path: &str, description: &str) -> Option<Page> {
    let result = Page::new(
        PageKind::Simple,
        context! {
            "title" => title,
            "content" => Page::render_markdown(path).await.ok()?,
            "og_title" => title,
            "og_description" => description,
        },
    );

    Some(result)
}

#[rocket::get("/")]
pub async fn home() -> Option<Page> {
    render_simple(
        "Home",
        "pages/home.md",
        "Computers, cats, and eternal sleepiness",
    )
    .await
}

#[rocket::get("/about")]
pub async fn about_me() -> Option<Page> {
    render_simple(
        "About me",
        "pages/about.md",
        "It's me!",
    )
    .await
}

#[rocket::get("/blog")]
pub async fn post_list(config: &State<WrappedPostMap>) -> Page {
    let posts: Vec<_> = config
        .read()
        .await
        .iter()
        .map(|(slug, info)| {
            context! {
                "slug" => slug.to_owned(),
                "title" => info.title.to_owned(),
                "published" => info.published.to_rfc3339(),
            }
        })
        .collect();

    Page::new(
        PageKind::PostList,
        context! {
            "title" => "Kaylynn's blog",
            "posts" => posts,
        },
    )
}

#[rocket::get("/blog/post/<slug>")]
pub async fn post(config: &State<WrappedPostMap>, slug: String) -> Option<Page> {
    let posts = config.read().await;
    let info = posts.get(&slug)?;
    let result = Page::new(
        PageKind::Post,
        context! {
            "title" => info.title.as_str(),
            "og_title" => info.title.as_str(),
            "og_description" => info.description.as_str(),
            "published" => info.published.to_rfc3339(),
            "content" => info.rendered.to_owned(),
        },
    );

    Some(result)
}

#[rocket::get("/blog/feed.rss")]
pub async fn rss_feed(config: &State<WrappedPostMap>) -> Xml<String> {
    let posts = config.read().await;
    let rss = posts.rss();

    // Panic safety: Vectors will grow when required.
    let buffer = rss.pretty_write_to(Vec::new(), b' ', 2).unwrap();

    // Panic safety: The various inputs are already valid UTF-8, so realistically it should be impossible for this to fail.
    Xml(String::from_utf8(buffer).unwrap())
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

pub struct Secret<'r>(&'r str);

impl<'r> Secret<'r> {
    fn value(&self) -> &str {
        self.0
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Secret<'r> {
    type Error = &'static str;

    async fn from_request(request: &'r Request<'_>) -> rocket::request::Outcome<Self, Self::Error> {
        request
            .headers()
            .get_one("X-Hub-Signature-256")
            .into_outcome((Status::Unauthorized, "Missing signature"))
            .and_then(|secret| {
                secret
                    .trim()
                    .strip_prefix("sha256=")
                    .into_outcome((Status::BadRequest, "Invalid signature format"))
                    .map(Secret)
            })
    }
}

#[rocket::post("/deploy", data = "<data>")]
pub async fn deploy(
    shutdown: Shutdown,
    data: Data<'_>,
    request_secret: Secret<'_>,
) -> Result<(), (Status, &'static str)> {
    let mut body = Vec::new();

    data.open(25.megabytes())
        .stream_to(&mut body)
        .await
        .unwrap();

    let secret = SECRET
        .as_deref()
        .ok_or((Status::InternalServerError, "No secret configured"))?
        .as_bytes();

    let sha = Hmac::<Sha256>::new_from_slice(secret)
        .unwrap()
        .with_data(body.as_ref())
        .finalize()
        .into_bytes()
        .encode_hex::<String>();

    if sha != request_secret.value() {
        return Err((Status::Unauthorized, "Invalid signature"));
    }

    let raw = String::from_utf8_lossy(body.as_ref());
    let payload = Value::from_str(&raw).unwrap();

    // We don't want to trigger a shutdown until the actions run is completed and a new image is present on Docker Hub
    if payload["action"] != "completed" {
        return Ok(());
    }

    shutdown.notify();

    Ok(())
}
