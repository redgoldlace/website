use page::Config;
use rocket::{
    self, catchers,
    fs::FileServer,
    routes,
    tokio::{self, sync::RwLock},
};
use rocket_dyn_templates::Template;
use std::{sync::Arc, time::Duration};

mod page;
mod routes;
mod tera_utils;

/// Alias for convenience
pub type WrappedConfig = Arc<RwLock<Config>>;

async fn refresh_config(config: WrappedConfig) {
    // Realistically it's probably better to only do this if we detect changes. But this is fine for now.

    loop {
        if let Err(error) = config.write().await.try_update().await {
            eprintln!("Unable to update configuration: {}", error);
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

#[rocket::main]
async fn main() {
    let config = Arc::new(RwLock::new(
        page::Config::try_new()
            .await
            .expect("unable to open page config"),
    ));

    tokio::spawn(refresh_config(Arc::clone(&config)));

    let _ = rocket::build()
        .register("/", catchers![routes::default_catcher])
        .mount(
            "/",
            routes![
                routes::home,
                routes::about_me,
                routes::post,
                routes::post_list
            ],
        )
        .mount("/", FileServer::from("static/"))
        .attach(Template::custom(|engine| {
            engine
                .tera
                .register_filter("humanise", tera_utils::humanise)
        }))
        .manage(config)
        .launch()
        .await;
}
