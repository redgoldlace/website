use chrono::DateTime;
use comrak::Arena;
use indexmap::IndexMap;
use rss::{Channel, ChannelBuilder, GuidBuilder, ImageBuilder, ItemBuilder};
use std::{borrow::Borrow, ffi::OsStr, hash::Hash, io::Result as IoResult, path::Path};

use crate::{page::Page, page::PostMetadata};

#[derive(Debug)]
pub struct Posts {
    pages: IndexMap<String, Page>,
    rss: Channel,
}

impl Posts {
    pub fn new() -> Self {
        let pages = Default::default();
        let rss = rss_channel(&pages);

        Posts { pages, rss }
    }

    /// Read posts from the `directory` and update this `Posts` instance.
    pub fn refresh(&mut self, directory: &impl AsRef<Path>) -> IoResult<()> {
        let arena = Arena::new();

        let mut pages = IndexMap::new();
        let mut entries = std::fs::read_dir(directory.as_ref())?;

        while let Some(entry) = entries.next().transpose()? {
            let full_path = entry.path();

            let slug = full_path
                .file_stem()
                .and_then(OsStr::to_str)
                .map(str::to_owned)
                .unwrap_or_default();

            let content = std::fs::read_to_string(&full_path)?;

            match Page::build::<PostMetadata>(&arena, &content) {
                Ok(page) => _ = pages.insert(slug, page),
                Err(error) => eprintln!("error rendering post {}: {}", slug, error),
            };
        }

        // Look, I don't make the rules. But for some reason things need to be swapped around if we want them to be
        // ordered properly.
        let cursed_cmp_helper = |a: &Page, b: &Page| Some(b.published()?.cmp(&a.published()?));
        pages.sort_by(|_, a, _, b| cursed_cmp_helper(a, b).unwrap());

        // It's important that this is done after the sorting step, since `rss_channel` expects the mapping to be in
        // sorted order.
        let rss = rss_channel(&pages);

        *self = Posts { pages, rss };

        Ok(())
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&Page>
    where
        Q: Eq + Hash + ?Sized,
        String: Borrow<Q>,
    {
        self.pages.get(key)
    }

    pub fn rss(&self) -> &Channel {
        &self.rss
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Page)> {
        self.pages.iter().map(|(slug, page)| (slug.as_str(), page))
    }
}

fn rss_channel(pages: &IndexMap<String, Page>) -> Channel {
    let publish_date = |(_, page): (_, &Page)| page.published().as_ref().map(DateTime::to_rfc2822);

    let build_item = |(slug, page): (&String, &Page)| {
        ItemBuilder::default()
            .author(Some("mkaylynn7@gmail.com".to_owned()))
            .link(Some(format!("https://kaylynn.gay/blog/post/{slug}")))
            .title(page.title())
            .guid(Some(
                GuidBuilder::default()
                    .value(format!("https://kaylynn.gay/blog/post/{slug}"))
                    .permalink(true)
                    .build(),
            ))
            .description(page.description())
            .pub_date(page.published().map(|date| date.to_rfc2822()))
            .build()
    };

    let image = ImageBuilder::default()
        .url("https://kaylynn.gay/favicon.png".to_owned())
        .link("https://kaylynn.gay/blog".to_owned())
        .title("Kaylynn's Blog".to_owned())
        .description(Some("Love and be loved".to_owned()))
        .build();

    let channel = ChannelBuilder::default()
        .title("Kaylynn's blog".to_owned())
        .link("https://kaylynn.gay/blog".to_owned())
        .description("Computers, cats, and eternal sleepiness".to_owned())
        .webmaster(Some("mkaylynn7@gmail.com (Kaylynn Morgan)".to_owned()))
        .managing_editor(Some("mkaylynn7@gmail.com (Kaylynn Morgan)".to_owned()))
        .last_build_date(pages.first().and_then(publish_date))
        .pub_date(pages.last().and_then(publish_date))
        .copyright(Some("Copyright 2021-present, Kaylynn Morgan".to_owned()))
        .image(Some(image))
        .items(pages.iter().map(build_item).collect::<Vec<_>>())
        .build();

    channel
}
