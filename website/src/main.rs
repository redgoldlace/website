use axum::{
    body::Body,
    routing::{get, post},
    Extension, Router, Server,
};
use chrono::{DateTime, Utc};
use error::Error;
use lazy_static::lazy_static;
use shutdown::Shutdown;
use state::{Config, State};
use std::{
    net::SocketAddr,
    process::ExitCode,
    sync::{Arc, RwLock},
    time::SystemTime,
};
use syntect::parsing::{SyntaxSet, SyntaxSetBuilder};
use tower::ServiceBuilder;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{event, field::Empty, span, Instrument, Level};
use tracing_subscriber::fmt::{format::Writer, time::FormatTime};

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

struct Timer;

impl FormatTime for Timer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let formatted = DateTime::<Utc>::from(SystemTime::now()).format("[%H:%M:%S] ");
        write!(w, "{formatted} ")
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => {
            event!(Level::INFO, error = Empty, "server closed gracefully");
            ExitCode::SUCCESS
        }
        Err(error) => {
            event!(Level::ERROR, %error, "server closed with error");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_ansi(true)
        .compact()
        .with_timer(Timer)
        .init();

    let profile = match cfg!(debug_assertions) {
        true => "debug",
        false => "release",
    };

    event!(Level::INFO, "Running in \"{}\" mode", profile);

    let config = Config::figment().select(profile).extract::<Config>()?;
    let address = config.host().address();
    let port = config.host().port();

    event!(
        Level::INFO,
        config.webhook_secret = config.webhook_secret(),
        config.content_dir = %config.content_dir().display(),
        config.host.address = %address,
        config.host.port = port,
        "Loaded configuration from environment"
    );

    // This is a really, really evil hack. But doing it this way prevents us from passing it down the call stack when
    // parsing/rendering markdown, which is a lot nicer.
    let mut builder = SyntaxSetBuilder::new();
    builder.add_from_folder(&config.content_dir().join("syntaxes"), true)?;
    *SYNTAX_SET.write().unwrap() = builder.build();

    event!(Level::INFO, "Loaded highlighting syntaxes");

    let (shutdown, signal) = Shutdown::new();
    let state = State::try_new(config)?;

    // This service is just responsible for logging incoming requests. It's not as bad as it looks!
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
        .route("/blog/feed.rss", get(routes::rss_feed))
        .route("/blog/post/:slug", get(routes::post))
        .layer(services);

    event!(Level::INFO, "Starting server...");

    Server::bind(&SocketAddr::new(address, port))
        .serve(router.into_make_service())
        .with_graceful_shutdown(signal)
        .instrument(span!(Level::INFO, "server"))
        .await?;

    Ok(())
}
