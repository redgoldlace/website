use crate::{
    context,
    page::{Page, PageKind},
    WrappedConfig,
};
use rocket::{http::Status, Request, State};
use std::path::PathBuf;

#[rocket::catch(default)]
pub fn default_catcher(status: Status, _: &Request) -> Page {
    Page::new(
        PageKind::Error,
        context! {
            "reason" => format!("Status code {}: {}.", status.code, status.reason_lossy())
        },
    )
}

async fn render_simple(title: &str, path: &str) -> Option<Page> {
    let result = Page::new(
        PageKind::Simple,
        context! {
            "title" => title,
            "content" => Page::render_markdown(path).await.ok()?
        },
    );

    Some(result)
}

#[rocket::get("/")]
pub async fn home() -> Option<Page> {
    render_simple("Home", "pages/home.md").await
}

#[rocket::get("/about")]
pub async fn about_me() -> Option<Page> {
    render_simple("About me", "pages/about.md").await
}

#[rocket::get("/blog")]
pub async fn post_list(config: &State<WrappedConfig>) -> Page {
    let posts: Vec<_> = config
        .read()
        .await
        .pages
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
pub async fn post(config: &State<WrappedConfig>, slug: PathBuf) -> Option<Page> {
    let config_entry = config.read().await;
    let info = config_entry.pages.get(slug.to_str()?)?;
    let path = PathBuf::from("blog-pages").join(slug).with_extension("md");
    let result = Page::new(
        PageKind::Post,
        context! {
            "title" => info.title.to_owned(),
            "published" => info.published.to_rfc3339(),
            "content" => Page::render_markdown(path).await.ok()?,
        },
    );

    Some(result)
}
