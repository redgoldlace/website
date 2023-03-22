use axum::response::Html;
use chrono::{DateTime, FixedOffset, Local, TimeZone};
use comrak::Arena;
use serde::{de::DeserializeOwned, de::Error, Deserialize, Deserializer};
use std::{borrow::Cow, path::Path};
use tera::{Context, Value};
use toml::value::Datetime as TomlDateTime;

use crate::{
    context,
    error::Result,
    markdown::{self, NodeArena, NodeRef},
    templates::Engine,
};

const PREVIEW_CHARACTER_LIMIT: usize = 200;

#[derive(Debug)]
pub struct Page {
    template_name: Cow<'static, str>,
    context: Context,
}

impl Page {
    pub fn new(template_name: impl Into<Cow<'static, str>>, context: Context) -> Self {
        Self {
            template_name: template_name.into(),
            context,
        }
    }

    pub fn simple(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let arena = Arena::new();
        let page = Self::build::<StaticMetadata>(&arena, &content)?;

        Ok(page)
    }

    pub fn build<'a, M>(arena: NodeArena<'a>, content: &str) -> Result<Self>
    where
        M: DeserializeOwned + IntoPage + 'static,
    {
        let (metadata, document) = markdown::parse::<M>(&arena, content)?;

        Ok(metadata.into_page(document))
    }

    pub fn title(&self) -> Option<String> {
        self.context
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    pub fn description(&self) -> Option<String> {
        self.context
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    pub fn published(&self) -> Option<DateTime<FixedOffset>> {
        self.context
            .get("published")
            .and_then(Value::as_str)
            .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
    }

    pub fn render(&self, engine: &Engine) -> Result<Html<String>> {
        let result = engine.render(&format!("{}.html.tera", self.template_name), &self.context)?;

        Ok(result)
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}

pub trait IntoPage {
    fn into_page<'a>(self, document: NodeRef<'a>) -> Page;
}

#[derive(Deserialize)]
pub struct StaticMetadata {
    title: String,
    description: String,
}

#[derive(Deserialize)]
pub struct PostMetadata {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(deserialize_with = "toml_date")]
    published: DateTime<Local>,
}

impl IntoPage for StaticMetadata {
    fn into_page<'a>(self, document: NodeRef<'a>) -> Page {
        Page::new(
            "page",
            context! {
                "title" => self.title,
                "description" => self.description,
                "content" => markdown::render(document),
            },
        )
    }
}

impl IntoPage for PostMetadata {
    fn into_page<'a>(self, document: NodeRef<'a>) -> Page {
        let description = self
            .description
            .or_else(|| markdown::preview(document, PREVIEW_CHARACTER_LIMIT))
            .unwrap_or_else(|| "(no description provided)".to_owned());

        Page::new(
            "post",
            context! {
                "title" => self.title,
                "description" => description,
                "published" => self.published.to_rfc3339(),
                "content" => markdown::render(document),
                "is_blog_post" => true,
            },
        )
    }
}

fn toml_date<'de, D>(deserializer: D) -> std::result::Result<DateTime<Local>, D::Error>
where
    D: Deserializer<'de>,
{
    let date = TomlDateTime::deserialize(deserializer)?.to_string();

    Local
        .datetime_from_str(&date, "%Y-%m-%dT%H:%M:%S")
        .map_err(|_| D::Error::custom("failed to parse toml date"))
}
