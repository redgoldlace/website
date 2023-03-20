use crate::{
    error::Result,
    posts::Posts,
    templates::{self, Engine},
};
use axum::{
    extract::{Extension, FromRequestParts},
    http::request::Parts,
};
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::{
    net::IpAddr,
    path::{Path, PathBuf},
    sync::Arc,
};
use tera::Tera;

#[derive(Debug)]
pub struct StateInner {
    config: Config,
    engine: Engine,
    posts: Posts,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    webhook_secret: Option<String>,
    content_dir: PathBuf,
    host: HostConfig,
}

impl Config {
    pub fn figment() -> Figment {
        Figment::new()
            .merge(Toml::file("App.toml").nested())
            .merge(Env::prefixed("WOEBLOG_"))
    }

    pub fn webhook_secret(&self) -> Option<&str> {
        self.webhook_secret.as_deref()
    }

    pub fn content_dir(&self) -> &Path {
        &self.content_dir
    }

    pub fn host(&self) -> &HostConfig {
        &self.host
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostConfig {
    address: IpAddr,
    port: u16,
}

impl HostConfig {
    pub fn address(&self) -> IpAddr {
        self.address
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

#[derive(Debug, Clone)]
pub struct State(Arc<StateInner>);

impl State {
    pub fn try_new(config: Config) -> Result<Self> {
        let mut posts = Posts::new();
        posts.refresh(&config.content_dir.join("blog-pages"))?;

        let engine = Engine::new({
            let mut tera = Tera::new(
                &config
                    .content_dir
                    .join("templates/*.html.tera")
                    .to_string_lossy(),
            )?;

            tera.register_filter("humanize", templates::humanize);
            tera
        });

        let inner = StateInner {
            config,
            engine,
            posts,
        };

        Ok(State(Arc::new(inner)))
    }

    pub fn config(&self) -> &Config {
        &self.0.config
    }

    pub fn engine(&self) -> &Engine {
        &self.0.engine
    }

    pub fn posts(&self) -> &Posts {
        &self.0.posts
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for State
where
    S: Send + Sync,
{
    type Rejection = <Extension<State> as FromRequestParts<S>>::Rejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        let Extension(state) = Extension::<State>::from_request_parts(parts, state).await?;

        Ok(state)
    }
}
