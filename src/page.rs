use chrono::{DateTime, Local, TimeZone};
use comrak::{
    nodes::{AstNode, NodeValue},
    Arena, ComrakExtensionOptions, ComrakOptions, ComrakRenderOptions,
};
use indexmap::IndexMap;
use lazy_static::lazy_static;
use rocket::{
    response::Responder,
    serde::{de, Deserialize, Serialize},
    tokio::fs::{read_dir, read_to_string},
    Request,
};
use rocket_dyn_templates::Template;
use serde_json::Value;
use std::{fmt::Display, io::Error as IoError, path::Path};
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
            front_matter_delimiter: Some("---".to_owned()),
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

type R = Result<(), ()>;

fn iter_nodes<'a>(node: &'a AstNode<'a>, mut f: impl FnMut(&'a AstNode<'a>) -> R) -> R {
    fn _iter_nodes<'a>(node: &'a AstNode<'a>, f: &mut impl FnMut(&'a AstNode<'a>) -> R) -> R {
        f(node)?;
        for c in node.children() {
            _iter_nodes(c, f)?;
        }

        Ok(())
    }

    _iter_nodes(node, &mut f)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct PostInfo {
    pub title: String,
    #[serde(deserialize_with = "deserialize_config_date")]
    pub published: DateTime<Local>,
    #[serde(skip_deserializing)]
    pub content: String,
}

pub struct Config {
    pub pages: IndexMap<String, PostInfo>,
}

impl Config {
    pub fn new(pages: IndexMap<String, PostInfo>) -> Self {
        Self { pages }
    }

    pub async fn try_new() -> Result<Self, Error> {
        let mut pages = IndexMap::new();
        let mut entries = read_dir("blog-pages").await?;

        while let Some(entry) = entries.next_entry().await? {
            let filename = entry
                .path()
                .file_stem()
                .map_or_else(String::new, |filename| {
                    filename.to_string_lossy().into_owned()
                });

            let page_content = read_to_string(entry.path()).await?;
            let arena = Arena::new();
            let root = comrak::parse_document(&arena, &page_content, &OPTIONS);

            let mut front_matter = None;

            let _ = iter_nodes(root, |node| match &node.data.borrow().value {
                NodeValue::FrontMatter(bytes) => {
                    front_matter.replace(String::from_utf8_lossy(bytes).into_owned());
                    Err(())
                }
                _ => Ok(()),
            });

            let page = toml::from_str::<PostInfo>(
                front_matter
                    .unwrap_or_else(String::new)
                    .trim()
                    .trim_start_matches("---")
                    .trim_end_matches("---"),
            )
            .map(|page| PostInfo {
                content: page_content,
                ..page
            });

            match page {
                Ok(page) => {
                    pages.insert(filename, page);
                }
                Err(error) => eprintln!("Error while opening post at {}: {}", filename, error),
            }
        }

        pages.sort_by(|_, a, _, b| b.published.cmp(&a.published));

        Ok(Self::new(pages))
    }

    pub async fn try_update(&mut self) -> Result<(), Error> {
        *self = Config::try_new().await?;
        Ok(())
    }
}
