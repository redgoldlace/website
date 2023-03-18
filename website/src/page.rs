use chrono::{DateTime, FixedOffset, Local, TimeZone};
use serde::{de::Error, Deserialize, Deserializer};
use serde_json::Value;
use std::borrow::Cow;
use toml::value::Datetime as TomlDateTime;

use crate::{
    context,
    markdown::{self, NodeArena, NodeRef, TomlResult},
};

const PREVIEW_CHARACTER_LIMIT: usize = 200;

pub struct Page {
    template_name: Cow<'static, str>,
    context: Value,
}

impl Page {
    pub fn new(template_name: impl Into<Cow<'static, str>>, context: Value) -> Self {
        Self {
            template_name: template_name.into(),
            context,
        }
    }

    pub fn build<'a, 'i, M>(arena: NodeArena<'a>, content: &'i str) -> TomlResult<Self>
    where
        M: Deserialize<'i> + IntoPage,
    {
        let (metadata, document) = markdown::parse::<M>(&arena, content)?;

        Ok(metadata.into_page(document))
    }

    pub fn title(&self) -> Option<String> {
        self.context["title"].as_str().map(str::to_owned)
    }

    pub fn description(&self) -> Option<String> {
        self.context["description"].as_str().map(str::to_owned)
    }

    pub fn published(&self) -> Option<DateTime<FixedOffset>> {
        self.context["published"]
            .as_str()
            .and_then(|date| DateTime::parse_from_rfc3339(date).ok())
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
            },
        )
    }
}

fn toml_date<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
where
    D: Deserializer<'de>,
{
    let date = TomlDateTime::deserialize(deserializer)?.to_string();

    Local
        .datetime_from_str(&date, "%Y-%m-%dT%H:%M:%S")
        .map_err(|_| D::Error::custom("failed to parse toml date"))
}
