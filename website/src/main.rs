use hmac::Hmac;
use lazy_static::lazy_static;
use page::PostMap;
use rocket::{
    self, catchers,
    fs::{FileServer, Options as FsOptions},
    routes,
    tokio::sync::RwLock,
};
use rocket_dyn_templates::Template;
use sha2::Sha256;
use syntect::parsing::{SyntaxSet, SyntaxSetBuilder};

mod page;
mod routes;
mod tera_util;
mod markdown;
mod posts;

/// Alias for convenience
pub type WrappedPostMap = RwLock<PostMap>;
pub type WrappedSecret = Hmac<Sha256>;

lazy_static! {
    pub static ref SECRET: Option<String> = std::env::var("WEBHOOK_SECRET").ok();
    pub static ref SYNTAX_SET: SyntaxSet = {
        let mut builder = SyntaxSetBuilder::new();
        builder
            .add_from_folder("syntaxes/", true)
            .expect("failed to load syntaxes");

        builder.build()
    };
}

#[macro_export]
macro_rules! context {
    ($($key:expr => $value:expr,)+) => { context! {$($key => $value),*} };
    ($($key:expr => $value:expr),*) => {{
        let mut map: ::serde_json::Map<::std::string::String, ::serde_json::Value> = ::serde_json::Map::new();
        $(map.insert($key.into(), $value.into());)*
        let as_value: ::serde_json::Value = map.into();
        as_value
    }};
}

#[rocket::launch]
async fn launch() -> _ {
    // This is bad but I'm tired. Essentially we want to have this built here so that we panic at startup if things go
    // wrong.
    let _ = &*SYNTAX_SET;

    rocket::build()
        .register("/", catchers![routes::default_catcher])
        .mount(
            "/",
            routes![
                routes::home,
                routes::about_me,
                routes::post,
                routes::post_list,
                routes::rss_feed,
                routes::deploy,
            ],
        )
        .mount(
            "/",
            FileServer::new("static/", FsOptions::NormalizeDirs | FsOptions::default()),
        )
        .attach(Template::custom(|engine| {
            engine
                .tera
                .register_filter("humanise", tera_util::humanise)
        }))
        .manage(RwLock::new(
            page::PostMap::try_new()
                .await
                .expect("unable to open page config"),
        ))
}
