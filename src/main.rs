use hmac::Hmac;
use lazy_static::lazy_static;
use page::Config;
use rocket::{self, catchers, fs::FileServer, routes, tokio::sync::RwLock};
use rocket_dyn_templates::Template;
use sha2::Sha256;

mod page;
mod routes;
mod tera_utils;

/// Alias for convenience
pub type WrappedConfig = RwLock<Config>;
pub type WrappedSecret = Hmac<Sha256>;

lazy_static! {
    pub static ref SECRET: String = std::env::var("WEBHOOK_SECRET")
        .expect("the WEBHOOK_SECRET environment variable is required");
}

#[rocket::launch]
async fn launch() -> _ {
    // This is bad but I'm tired. Essentially we want to have this built here so that we panic at startup if things go
    // wrong.
    &*SECRET;

    rocket::build()
        .register("/", catchers![routes::default_catcher])
        .mount(
            "/",
            routes![
                routes::home,
                routes::about_me,
                routes::post,
                routes::post_list,
                routes::refresh_pages,
            ],
        )
        .mount("/", FileServer::from("static/"))
        .attach(Template::custom(|engine| {
            engine
                .tera
                .register_filter("humanise", tera_utils::humanise)
        }))
        .manage(RwLock::new(
            page::Config::try_new()
                .await
                .expect("unable to open page config"),
        ))
}
