use comrak::{
    arena_tree::NodeEdge,
    nodes::{AstNode, NodeHtmlBlock, NodeValue},
    Arena, ComrakExtensionOptions, ComrakOptions, ComrakRenderOptions,
};
use lazy_static::lazy_static;
use serde::de::DeserializeOwned;
use syntect::{
    html::{ClassStyle, ClassedHTMLGenerator},
    util::LinesWithEndings,
};
use toml::de::Error as TomlError;

use crate::SYNTAX_SET;

pub type NodeArena<'a> = &'a Arena<AstNode<'a>>;
pub type NodeRef<'a> = &'a AstNode<'a>;
pub type TomlResult<T> = Result<T, TomlError>;

lazy_static! {
    static ref COMRAK_OPTIONS: ComrakOptions = ComrakOptions {
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

/// Render a Markdown AST as HTML.
///
/// # Panics
///
/// This function panics if the output contains invalid UTF-8.
pub fn render<'a>(document: NodeRef<'a>) -> String {
    let mut buffer = Vec::new();
    comrak::format_html(document, &COMRAK_OPTIONS, &mut buffer).expect("writing output failed");

    String::from_utf8(buffer).expect("output contained invalid UTF-8")
}

/// Parse raw Markdown source into an AST.
///
/// This function returns a tuple of (metadata, AST) when successful. The metadata is extracted from the document's
/// front matter and deserialized into the type `M`. The front matter is assumed to be in TOML format.
///
/// This function returns an error if deserializing into `M` fails.
pub fn parse<'a, 'de, M>(arena: NodeArena<'a>, content: &str) -> TomlResult<(M, NodeRef<'a>)>
where
    M: DeserializeOwned,
{
    let document = comrak::parse_document(&arena, content, &COMRAK_OPTIONS);

    highlight(document);

    let front_matter = traverse(document)
        .find_map(|node| match node.data.borrow().value {
            NodeValue::FrontMatter(ref bytes) => Some(
                String::from_utf8_lossy(bytes)
                    .trim()
                    .trim_matches('-')
                    .to_owned(),
            ),
            _ => None,
        })
        .unwrap_or_default();

    let metadata: M = toml::from_str(&front_matter)?;

    Ok((metadata, document))
}

/// Return an iterator over each child node in the provided Markdown AST.
pub fn traverse<'a>(root: &'a AstNode<'a>) -> impl Iterator<Item = &'a AstNode<'a>> {
    root.traverse().filter_map(|edge| match edge {
        NodeEdge::Start(node) => Some(node),
        NodeEdge::End(_) => None,
    })
}

/// Perform syntax highlighting on the Markdown AST in-place.
///
/// For each fenced codeblock in the AST, the codeblock is parsed and syntax highlighting is performed. Then the
/// original AST node is replaced with an inline HTML node containing the highlighted output.
pub fn highlight<'a>(root: &'a AstNode<'a>) {
    let syntax_set = SYNTAX_SET.read().unwrap();

    for node in traverse(root) {
        let mut data = node.data.borrow_mut();

        if let NodeValue::CodeBlock(ref codeblock) = data.value {
            // Panic safety: I solemnly swear I will never include invalid UTF-8 inside of my website.
            let language = std::str::from_utf8(&codeblock.info).unwrap();
            let code = std::str::from_utf8(&codeblock.literal).unwrap();

            let syntax_reference = syntax_set
                .find_syntax_by_extension(language)
                .or_else(|| syntax_set.find_syntax_by_name(language));

            let syntax_reference = match syntax_reference {
                Some(reference) => reference,
                None => continue,
            };

            let mut html_generator = ClassedHTMLGenerator::new_with_class_style(
                syntax_reference,
                &syntax_set,
                ClassStyle::SpacedPrefixed { prefix: "hl-" },
            );

            for line in LinesWithEndings::from(code) {
                let _ = html_generator.parse_html_for_line_which_includes_newline(line);
            }

            // What follows may be considered a crime
            let mut new_node = NodeHtmlBlock::default();
            let rendered = html_generator.finalize();
            new_node.literal = format!("<pre><code>{}</code></pre>\n", rendered).into_bytes();

            data.value = NodeValue::HtmlBlock(new_node);
        }
    }
}

/// Extract a "preview" paragraph from the Markdown AST.
///
/// Returns the first paragraph of text content in the AST, truncated at `character_limit` characters. Overflow is
/// represented by appending a space followed by "[...]".
///
/// This function returns `None` if the AST does not contain a paragraph, or if the paragraph is empty.
pub fn preview<'a>(document: &'a AstNode<'a>, character_limit: usize) -> Option<String> {
    let first_paragraph = document
        .children()
        .find(|node| matches!(node.data.borrow().value, NodeValue::Paragraph))?
        .children();

    fn process_text<'a>(buffer: &mut String, nodes: impl Iterator<Item = &'a AstNode<'a>>) {
        for node in nodes {
            match &node.data.borrow().value {
                NodeValue::Text(bytes) => buffer.push_str(&String::from_utf8_lossy(bytes)),
                NodeValue::SoftBreak => buffer.push(' '),
                NodeValue::LineBreak => buffer.push('\n'),
                NodeValue::Emph
                | NodeValue::Strong
                | NodeValue::Strikethrough
                | NodeValue::Superscript => process_text(buffer, node.children()),
                _ => continue,
            }
        }
    }

    let mut preview = String::new();
    process_text(&mut preview, first_paragraph);

    if preview.is_empty() {
        return None;
    }

    let trim_at = preview
        .char_indices()
        .map(|(index, char)| index + char.len_utf8())
        .zip(1..)
        .find_map(|(byte_offset, n)| (n >= character_limit).then_some(byte_offset));

    match trim_at {
        Some(end) => Some(format!("{} [...]", &preview[..end].trim())),
        None => Some(preview),
    }
}
