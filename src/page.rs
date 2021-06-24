use chrono::{DateTime, Local, TimeZone};
use comrak::{ComrakExtensionOptions, ComrakOptions, ComrakRenderOptions};
use indexmap::IndexMap;
use lazy_static::lazy_static;
use rocket::{
    response::Responder,
    serde::{de, Deserialize, Serialize},
    tokio::fs::read_to_string,
    Request,
};
use rocket_dyn_templates::Template;
use serde_json::Value;
use std::{fmt::Display, io::Error as IoError, mem, path::Path};
use toml::{self, de::Error as TomlDeError};

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

lazy_static! {
    static ref OPTIONS: ComrakOptions = ComrakOptions {
        extension: ComrakExtensionOptions {
            strikethrough: true,
            table: true,
            autolink: true,
            tasklist: true,
            description_lists: true,
            ..Default::default()
        },
        render: ComrakRenderOptions {
            unsafe_: true,
            ..Default::default()
        },
        ..Default::default()
    };
}

#[derive(Debug)]
pub enum Error {
    Io(IoError),
    Invalid(TomlDeError),
}

impl Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(error) => write!(formatter, "IO error: {}", error),
            Error::Invalid(error) => write!(formatter, "TOML error: {}", error),
        }
    }
}

impl std::error::Error for Error {}

impl From<IoError> for Error {
    fn from(error: IoError) -> Self {
        Error::Io(error)
    }
}

impl From<TomlDeError> for Error {
    fn from(error: TomlDeError) -> Self {
        Error::Invalid(error)
    }
}

pub enum PageKind {
    Error,
    Simple,
    Post,
    PostList,
}

impl PageKind {
    fn template_name(&self) -> &'static str {
        match self {
            PageKind::Error => "error",
            PageKind::Simple => "page",
            PageKind::Post => "post",
            PageKind::PostList => "post-list",
        }
    }
}

pub struct Page {
    kind: PageKind,
    context: Value,
}

impl Page {
    pub fn new(kind: PageKind, context: Value) -> Self {
        Self { kind, context }
    }

    pub async fn render_markdown<P>(path: P) -> Result<String, Error>
    where
        P: AsRef<Path>,
    {
        let markdown = read_to_string(path.as_ref())
            .await
            .map_err(Into::<Error>::into)?;

        Ok(comrak::markdown_to_html(&markdown, &OPTIONS))
    }
}

impl<'r, 'o: 'r> Responder<'r, 'o> for Page {
    fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'o> {
        Template::render(self.kind.template_name(), self.context).respond_to(request)
    }
}

fn deserialize_config_date<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let toml_date: toml::value::Datetime = Deserialize::deserialize(deserializer)?;
    Local
        .datetime_from_str(&toml_date.to_string(), "%Y-%m-%dT%H:%M:%S")
        .map_err(|_| de::Error::custom("failed to parse date"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct PostInfo {
    pub title: String,
    #[serde(deserialize_with = "deserialize_config_date")]
    pub published: DateTime<Local>,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Config {
    pub pages: IndexMap<String, PostInfo>,
}

impl Config {
    pub async fn try_new() -> Result<Self, Error> {
        let page_config = read_to_string(&Path::new("Meta.toml"))
            .await
            .map_err(Into::<Error>::into)?;

        let mut config: Config = toml::from_str(&page_config)?;
        config
            .pages
            .sort_by(|_, first, _, second| second.published.cmp(&first.published));

        Ok(config)
    }

    pub async fn try_update(&mut self) -> Result<(), Error> {
        // This is incredibly fucking evil.
        let mut result = Config::try_new().await?;
        mem::swap(self, &mut result);

        Ok(())
    }
}
