use super::SYNTAX_SET;
use chrono::{DateTime, Local, TimeZone};
use comrak::{
    arena_tree::NodeEdge,
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
use rss::{Channel, ChannelBuilder, GuidBuilder, ImageBuilder, ItemBuilder};
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

fn traverse<'a>(root: &'a AstNode<'a>) -> impl Iterator<Item = &'a AstNode<'a>> {
    root.traverse().filter_map(|edge| match edge {
        NodeEdge::Start(node) => Some(node),
        NodeEdge::End(_) => None,
    })
}

fn highlight<'a>(root: &'a AstNode<'a>) {
    for node in traverse(root) {
        let mut data = node.data.borrow_mut();

        if let NodeValue::CodeBlock(ref codeblock) = data.value {
            // SAFETY: I solemnly swear I will never include invalid UTF-8 inside of my website.
            let language = from_utf8(&codeblock.info).unwrap();
            let code = from_utf8(&codeblock.literal).unwrap();

            let syntax_reference = SYNTAX_SET
                .find_syntax_by_extension(language)
                .or_else(|| SYNTAX_SET.find_syntax_by_name(language));

            let syntax_reference = match syntax_reference {
                Some(reference) => reference,
                None => continue,
            };

            let mut html_generator = ClassedHTMLGenerator::new_with_class_style(
                syntax_reference,
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
        }
    }
}

enum Fragment {
    String(String),
    Char(char),
}

impl From<char> for Fragment {
    fn from(v: char) -> Self {
        Self::Char(v)
    }
}

impl From<String> for Fragment {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

fn description<'a>(root: &'a AstNode<'a>) -> Option<String> {
    const DESCRIPTION_LENGTH: usize = 200;

    let first_paragraph = root
        .children()
        .find(|node| matches!(node.data.borrow().value, NodeValue::Paragraph))?
        .children();

    let mut buffer = String::new();

    fn extend_buffer<'a>(buffer: &mut String, nodes: impl Iterator<Item = &'a AstNode<'a>>) {
        for node in nodes {
            match &node.data.borrow().value {
                NodeValue::Text(bytes) => buffer.push_str(&String::from_utf8_lossy(bytes)),
                NodeValue::SoftBreak => buffer.push(' '),
                NodeValue::LineBreak => buffer.push('\n'),
                NodeValue::Emph
                | NodeValue::Strong
                | NodeValue::Strikethrough
                | NodeValue::Superscript => extend_buffer(buffer, node.children()),
                _ => continue,
            }
        }
    }

    extend_buffer(&mut buffer, first_paragraph);

    let (len, end) = buffer
        .char_indices()
        .zip(1..)
        .map(|((index, char), len)| (len, index + char.len_utf8()))
        .take(DESCRIPTION_LENGTH + 3)
        .last()?;

    if len > DESCRIPTION_LENGTH {
        let offset = buffer[..end]
            .chars()
            .rev()
            .take(3)
            .map(char::len_utf8)
            .sum::<usize>();

        return Some(format!("{} [...]", &buffer[..end - offset].trim()));
    }

    Some(buffer[..end].to_owned())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct PostInfo {
    pub title: String,
    #[serde(deserialize_with = "deserialize_config_date")]
    pub published: DateTime<Local>,
    #[serde(skip_deserializing)]
    pub rendered: String,
    // Realistically this could be some self-referential slice but I'm not that much of a masochist. Nor am I a C
    // programmer.
    #[serde(skip_deserializing)]
    pub description: String,
}

impl PostInfo {
    async fn build(content: String) -> Result<PostInfo, Error> {
        let arena = Arena::new();
        let root = comrak::parse_document(&arena, &content, &OPTIONS);

        highlight(root);

        let front_matter = traverse(root).find_map(|node| match node.data.borrow().value {
            NodeValue::FrontMatter(ref bytes) => Some(String::from_utf8_lossy(bytes).into_owned()),
            _ => None,
        });

        let description =
            description(root).unwrap_or_else(|| "No description provided.".to_owned());

        let mut buffer = Vec::new();
        comrak::format_html(root, &OPTIONS, &mut buffer)?;
        let rendered = String::from_utf8(buffer)?;

        let info = toml::from_str(front_matter.unwrap_or_default().trim().trim_matches('-'))?;

        Ok(PostInfo {
            rendered,
            description,
            ..info
        })
    }
}

pub struct PostMap {
    pages: IndexMap<String, PostInfo>,
    rss: Channel,
}

impl PostMap {
    pub fn new(pages: IndexMap<String, PostInfo>) -> Self {
        let rss = build_rss(&pages);

        Self { pages, rss }
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
        std::mem::swap(self, &mut PostMap::try_new().await?);
        Ok(())
    }

    pub fn get(&self, slug: &str) -> Option<&PostInfo> {
        self.pages.get(slug)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &PostInfo)> {
        self.pages.iter().map(|(slug, post)| (slug.as_str(), post))
    }

    pub fn rss(&self) -> &Channel {
        &self.rss
    }
}

pub fn build_rss(pages: &IndexMap<String, PostInfo>) -> Channel {
    ChannelBuilder::default()
        .title("Kaylynn's blog")
        .link("https://kaylynn.gay/blog")
        .description("Computers, Rust, and other ramblings")
        .webmaster(Some("mkaylynn7@gmail.com (Kaylynn Morgan)".to_owned()))
        .managing_editor(Some("mkaylynn7@gmail.com (Kaylynn Morgan)".to_owned()))
        .last_build_date(pages.first().map(|(_, info)| info.published.to_rfc2822()))
        .pub_date(pages.last().map(|(_, info)| info.published.to_rfc2822()))
        .copyright(Some("Copyright 2021-present, Kaylynn Morgan".to_owned()))
        .image(Some(
            ImageBuilder::default()
                .url("https://kaylynn.gay/favicon.png")
                .link("https://kaylynn.gay/blog")
                .title("Kaylynn's Blog")
                .description(Some("<3".to_owned()))
                .build(),
        ))
        .items(
            pages
                .iter()
                .map(|(slug, info)| {
                    ItemBuilder::default()
                        .author(Some("mkaylynn7@gmail.com".to_owned()))
                        .link(Some(format!("https://kaylynn.gay/blog/post/{slug}")))
                        .title(Some(info.title.clone()))
                        .guid(Some(
                            GuidBuilder::default()
                                .value(format!("https://kaylynn.gay/blog/post/{slug}"))
                                .permalink(true)
                                .build(),
                        ))
                        .description(Some(info.description.clone()))
                        .pub_date(Some(info.published.to_rfc2822()))
                        .build()
                })
                .collect::<Vec<_>>(),
        )
        .build()
}
