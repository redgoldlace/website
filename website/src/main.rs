use axum::{
    body::Body,
    routing::{get, post},
    Extension, Router, Server,
};
use lazy_static::lazy_static;
use shutdown::Shutdown;
use state::{Config, State};
use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
};
use syntect::parsing::{SyntaxSet, SyntaxSetBuilder};
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

mod error;
mod markdown;
mod page;
mod posts;
mod routes;
mod shutdown;
mod state;
mod templates;

lazy_static! {
    pub static ref SYNTAX_SET: Arc<RwLock<SyntaxSet>> = Default::default();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let profile = match cfg!(debug_assertions) {
        true => "debug",
        false => "release",
    };

    tracing_subscriber::fmt()
        .with_target(false)
        .with_ansi(true)
        .compact()
        .init();

    let config = Config::figment().select(profile).extract::<Config>()?;
    let address = config.host().address();
    let port = config.host().port();

    // This is a really, really evil hack. But doing it this way prevents us from passing it down the call stack when
    // parsing/rendering markdown, which is a lot nicer.
    let mut builder = SyntaxSetBuilder::new();
    builder.add_from_folder(&config.content_dir().join("syntaxes"), true)?;
    *SYNTAX_SET.write().unwrap() = builder.build();

    let (shutdown, signal) = Shutdown::new();
    let state = State::try_new(config)?;
    let trace_service = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    let services = ServiceBuilder::new()
        .layer(trace_service)
        .layer(Extension(shutdown))
        .layer(Extension(state))
        .layer(axum::middleware::from_fn(error::to_error_page));

    let router = Router::<(), Body>::new()
        .route("/", get(routes::simple("pages/home.md")))
        .route("/about", get(routes::simple("pages/about.md")))
        .route("/deploy", post(routes::deploy))
        .route("/blog", get(routes::post_list))
        .route("/blog/feed", get(routes::rss_feed))
        .route("/blog/post/:slug", get(routes::post))
        .layer(services);

    Server::bind(&SocketAddr::new(address, port))
        .serve(router.into_make_service())
        .with_graceful_shutdown(signal)
        .await?;

    Ok(())
}
