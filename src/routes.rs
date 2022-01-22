use crate::{
    context,
    page::{Page, PageKind},
    WrappedPostMap, SECRET,
};
use hmac::{Hmac, Mac, NewMac};
use rocket::{
    data::ToByteUnit, http::Status, outcome::IntoOutcome, request::FromRequest,
    response::content::Xml, Data, Request, State,
};
use sha2::Sha256;
use std::process::Command;

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
        "It's the home page. I'm not sure what else to tell you?",
    )
    .await
}

#[rocket::get("/about")]
pub async fn about_me() -> Option<Page> {
    render_simple(
        "About me",
        "pages/about.md",
        "Information about a certain someone",
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
    
    // SAFETY: This shouldn't fail, as vectors will grow when required.
    let buffer = rss.pretty_write_to(Vec::new(), b' ', 2).unwrap();

    // SAFETY: The various inputs are already valid UTF-8, so realistically it should be impossible for this to fail.
    Xml(String::from_utf8(buffer).unwrap())
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
            .map(str::trim)
            .into_outcome((Status::Unauthorized, "Missing signature"))
            .map(Secret)
    }
}

#[rocket::post("/githook", data = "<data>")]
pub async fn githook(
    config: &State<WrappedPostMap>,
    data: Data<'_>,
    request_secret: Secret<'_>,
) -> Result<(), (Status, &'static str)> {
    let mut body = Vec::new();
    data.open(25.megabytes())
        .stream_to(&mut body)
        .await
        .unwrap();

    let mut hmac = Hmac::<Sha256>::new_from_slice(SECRET.as_bytes())
        .expect("HMAC supports keys of any size. This shouldn't happen");

    hmac.update(body.as_ref());
    let secret = format!("sha256={}", hex::encode(hmac.finalize().into_bytes()));

    if secret != request_secret.value() {
        return Err((Status::Unauthorized, "Invalid signature"));
    }

    rocket::tokio::task::spawn_blocking(move || {
        Command::new("git")
            .arg("pull")
            .status()
            .expect("updating failed")
    })
    .await
    .unwrap();

    let _ = config.write().await.try_update();

    Ok(())
}
