use super::SYNTAX_SET;
use chrono::{DateTime, Local, TimeZone};
use comrak::{
    nodes::{AstNode, NodeHtmlBlock, NodeValue},
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
use std::{fmt::Display, io::Error as IoError, path::Path, str::from_utf8, string::FromUtf8Error};
use syntect::{
    html::{ClassStyle, ClassedHTMLGenerator},
    util::LinesWithEndings,
};
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
    Encoding(FromUtf8Error),
}

impl Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(error) => write!(formatter, "IO error: {}", error),
            Error::Invalid(error) => write!(formatter, "TOML error: {}", error),
            Error::Encoding(error) => write!(formatter, "UTF-8 conversion error: {}", error),
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

impl From<FromUtf8Error> for Error {
    fn from(error: FromUtf8Error) -> Self {
        Error::Encoding(error)
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

fn iter_nodes<'a>(node: &'a AstNode<'a>, mut f: impl FnMut(&'a AstNode<'a>) -> R) -> bool {
    fn _iter_nodes<'a>(node: &'a AstNode<'a>, f: &mut impl FnMut(&'a AstNode<'a>) -> R) -> R {
        f(node)?;
        for c in node.children() {
            _iter_nodes(c, f)?;
        }

        Ok(())
    }

    _iter_nodes(node, &mut f).is_err()
}

fn highlight<'a>(root: &'a AstNode<'a>) {
    iter_nodes(root, |node| {
        let mut data = node.data.borrow_mut();

        match data.value {
            NodeValue::CodeBlock(ref codeblock) => {
                // SAFETY: I solemnly swear I will never include invalid UTF-8 inside of my website.
                let language = from_utf8(&codeblock.info).unwrap();
                let code = from_utf8(&codeblock.literal).unwrap();

                let syntax_reference = SYNTAX_SET
                    .find_syntax_by_extension(language)
                    .or_else(|| SYNTAX_SET.find_syntax_by_name(language));

                let syntax_reference = match syntax_reference {
                    Some(reference) => reference,
                    None => return Ok(()),
                };

                let mut html_generator = ClassedHTMLGenerator::new_with_class_style(
                    &syntax_reference,
                    &SYNTAX_SET,
                    ClassStyle::SpacedPrefixed { prefix: "hl-" },
                );

                for line in LinesWithEndings::from(code) {
                    html_generator.parse_html_for_line_which_includes_newline(line)
                }

                // What follows may be considered a crime
                let mut new_node = NodeHtmlBlock::default();
                let rendered = html_generator.finalize();
                new_node.literal = format!("<pre><code>{}</code></pre>\n", rendered).into_bytes();

                data.value = NodeValue::HtmlBlock(new_node);

                Ok(())
            }
            _ => Ok(()),
        }
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct PostInfo {
    pub title: String,
    #[serde(deserialize_with = "deserialize_config_date")]
    pub published: DateTime<Local>,
    #[serde(skip_deserializing)]
    pub rendered: String,
}

impl PostInfo {
    async fn build(content: String) -> Result<PostInfo, Error> {
        let arena = Arena::new();
        let root = comrak::parse_document(&arena, &content, &OPTIONS);

        let mut front_matter = None;

        iter_nodes(root, |node| match node.data.borrow().value {
            NodeValue::FrontMatter(ref bytes) => {
                front_matter.replace(String::from_utf8_lossy(bytes).into_owned());
                Err(())
            }
            _ => Ok(()),
        });

        highlight(root);

        let mut buffer = Vec::new();
        comrak::format_html(root, &OPTIONS, &mut buffer)?;
        let rendered = String::from_utf8(buffer)?;

        toml::from_str::<PostInfo>(
            front_matter
                .unwrap_or_else(String::new)
                .trim()
                .trim_matches('-'),
        )
        .map(|page| PostInfo { rendered, ..page })
        .map_err(Into::into)
    }
}

pub struct PostMap {
    pub pages: IndexMap<String, PostInfo>,
}

impl PostMap {
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

            let content = read_to_string(entry.path()).await?;

            match PostInfo::build(content).await {
                Ok(page) => {
                    pages.insert(filename, page);
                }
                Err(error) => eprintln!(
                    "Error while opening and rendering post at {}: {}",
                    filename, error
                ),
            }
        }

        pages.sort_by(|_, a, _, b| b.published.cmp(&a.published));

        Ok(Self::new(pages))
    }

    pub async fn try_update(&mut self) -> Result<(), Error> {
        *self = PostMap::try_new().await?;
        Ok(())
    }
}
