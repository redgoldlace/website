---
title = "Building this site"
published = 2021-06-24T21:35:00
---

I'd like to think I did a decent job building this site - in fact, I'm pretty happy with it.
It's taken me the last couple days, and I don't have any other ideas for a first post.
So let's stick with this.

# 1: Rekindling old ideas
I had the idea of starting a blog or similar a long while back - 6 months ago, in fact - but I was busy and
it didn't really happen. I was speaking to a certain someone a couple days ago and they mentioned that they'd
had the idea of writing a blog of some sort. That conversation ended up being the spark I needed and led me to
frantically obsess over CSS and templating for the next couple of days. It really pains me to say that I lost sleep
because I was thinking about web design, but here we are.

Anyway. I wanted to write a blog. Where the hell do I start?
I had a couple of options at my disposal:
- Write a simple static page or two and throw those on GitHub pages (which I've done previously)
- Write something a bit more complex, and throw it onto my VPS. I haven't SSH'd into that thing in.. months.
- Dive down the deep end and actually learn a bit.

I figure, y'know, I've got time. I can learn.
I've been writing a lot of Rust lately and I've accidentally fallen in love with Markdown.
In fact it's what I'm writing this page in now - but more on that later.

I've "dabbled" in Rust web frameworks before, if you can call getting frustrated at docs and giving up in 20 minutes
"dabbling" - *I'm looking at you, Actix*. I didn't wanna deal with Actix, and lots of the other options didn't seem
too great. Warp is just.. kinda bleh? Rocket is another big player in this space, and I messed around with it a bit
when I was still picking up Rust basics. I've since heard negative things about it, and I didn't remember much of my 
experience with it so I wasn't really planning on giving it a go. Or well, I wasn't, until I found out that a release
candidate of version 0.5 had become available recently. Version 0.5 is a big step for a lot of reasons, and encompasses
a *lot* of development progress - as evidenced by the changelog, which is *massive*.

And so, out of foolishness or... something else, I decided to give Rocket, with the Tera templating engine a go. This
is what that friend I mentioned decided to go with as well, and I thought it'd be fun to try. In the end, I was right!

Another big part of my set of ideas was not writing much HTML. I don't really like the idea of writing all my blog pages
in HTML because I just don't find it pleasant after a while. It's not difficult or anything like that - I just don't
enjoy it. I think it's because it feels tedious? I had a couple of options here, but I was already familiar enough with
Markdown and knew that it could be rendered to plain HTML in one way or another. Googling "rust markdown renderer"
yielded some helpful results - namely [Comrak](https://github.com/kivikakk/comrak), which supports the entire CommonMark
spec as well as the GFM extensions that I've come to love. And the top of their
[doc page](https://docs.rs/comrak/0.10.1/comrak/) had exactly what I needed, laid out nice and simply:
```rust
use comrak::{markdown_to_html, ComrakOptions};
assert_eq!(markdown_to_html("Hello, **世界**!", &ComrakOptions::default()),
           "<p>Hello, <strong>世界</strong>!</p>\n");
```
(not formatted with rustfmt though, which is a bit of a pet peeve)

But oh well, that's easy enough.
At this point, I was pretty sure I knew what I wanted to do:
1. User hits a `/blog/post/...` route
2. We load and render the markdown for that page
3. We push the rendered markdown through a Tera template to add some page features and make it pretty

So let's get started!

# 2: First steps
First steps with Rocket are *super* simple, and the library has overall been a joy to use.
My only major gripe with it is that it's given me something like 800 dependences, which takee a while for Rust-analyser
to index. But oh well, it's Rust. I should've expected this.

Anyways, starting off, my `Cargo.toml` looks like this:
```toml
[package]
name = "woeblog"
version = "0.1.0"
edition = "2018"

[dependencies]
comrak = "0.10.1"
rocket = "0.5.0-rc.1"
rocket_dyn_templates = { version="0.1.0-rc.1", features=["tera"] }
# ... snip
```
`tera` needs to be explicitly enabled via a feature flag, which isn't too much of a surprise.
I was a bit surprised that `rocket_dyn_templates` was its own package, but I imagine there's a decent reason for it.

With all that installed, let's jump into something simple:
```rs
use rocket::{self, routes};

#[rocket::get("/about")]
pub async fn about_me() -> String {
    "I am a person who exists.".to_owned()
}

#[rocket::main]
async fn main() {
    let _ = rocket::build()
        .mount("/", routes![about_me])
        .launch()
        .await;
}

```
And this is a complete Rocket application? Which is pretty crazy to me.

The next step was wrestling with Comrak so that it'd behave in the way I wanted - this is pretty simple to accomplish;
you just pass in a reference to a `ComrakOptions` struct, and that's that.
Setting it up for my needs was a pretty trivial task, but it did result in me pulling in `lazy_static` as a dependency.
I don't think the dependency is *really* necessary here, but what's one more, aye?
```rs
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
```
I was originally using just a plain call to `ComrakOptions::default()`, but that doesn't enable any of the GFM
extensions. So I wound up doing it manually. You'll also notice me setting `unsafe_` to `true`. Spooky I know!
This is because some of my Markdown has a *tiny* bit of styled HTML in it that doesn't apply under my normal
global styling rules:
```html
<div class="image-row">
  <img src="/cat.png">
  <img src="/cat2.jpg">
</div>
```
I trust my own Markdown input, so this isn't a real big deal to me personally. But you should be **very** careful
otherwise. There is a *lot* of space for things to go wrong if you parse untrusted markdown without sanitisation.

Here I ran into a bit of a roadblock, and an odd one at that: I'd set `unsafe_` to `true`, but my raw HTML was still
being scrubbed. I had absolutely no clue why this was happening, and wound up digging through Comrak's source to
figure out if it was a strange logic bug. As it turns out, Comrak is fine! But something strange was happening with
my build cache that was causing this; I think the options I was passing may have been inlined across crate boundaries
somehow? I'm not too sure. Regardless, I eventually had the idea of running `cargo clean` (a coping mechanism gained
after traumatic experiences building `llvm-sys`) and that was enough to fix it. I haven't been able to reproduce anything
similar since so who knows what was really going on. Maybe I should just start assuming that all compilers are evil?

Now I needed to render templates. Doing this in Rocket is *incredibly* straightforward.

First, you attach a fairing so that templates get loaded - by default Rocket will read templates from `/templates/`,
which is a nice batteries-included way to handle this. It's also possible to change where Rocket reads templates from,
but I didn't end up needing that.
```rs
// ...
use rocket_dyn_templates::Template;

#[rocket::main]
async fn main() {
    let _ = rocket::build()
        // ...
        .attach(Template::fairing())
        // ...
        .launch()
        .await;
}
```
And then, inside one of your routes, you can just return an instance of `Template`:
```rs
Template::render("template-name", context);
```
`Template::render` is *lazy*, and doesn't actually render the template if you throw the instance of it away - despite
the name of the function, rendering actually happens at response time, which is a nice touch.
Another neat thing here is that `context` can just be any serde-compatible value. It's not limited to a specific type
or something like that.

# 3: Wiring things up
Now that I had everything I needed, the next thing to do was attach all this machinery together.
I got a bit carried away with this, and wound up with a kinda-abstraction and project structure that I think is mostly
pretty nice. And I wrote a macro for it. Oh boy.

One of my routes looks a bit like this:
```rs
#[rocket::catch(default)]
pub fn default_catcher(status: Status, _: &Request) -> Page {
    Page::new(
        PageKind::Error,
        context! {
            "reason" => format!("Status code {}: {}.", status.code, status.reason_lossy())
        },
    )
}
```
The `PageKind` specifies the template that's used, and the `context!` macro builds a context for you.
`Page` implements the `Responder` trait, and is what actually renders the template with the context you give it.
It's still a bit of a thin wrapper around `Template`, but I'm mostly happy with it.
And this isn't a massive project either. So I think this kinda thing is fine for now.

I also wound up messing with Rocket's managed state a bit. Essentially I wanted to be able to just commit pages
as markdown, then have a git hook pull down changes and have the new pages "registered" automatically.
As it turns out Rocket's managed state is actually *really* cool and it works really well.
You initialise it in a similar way to how you initialise template functionality:
```rs
// ...
use rocket_dyn_templates::Template;

#[rocket::main]
async fn main() {
    let _ = rocket::build()
        // ...
        .manage(value)
        // ...
        .launch()
        .await;
}
```
Here `value` can be any type that's both `Send` and `Sync`. Rocket will happy manage any number of types that
satisfy these constraints, but will only manage at most *one* value of each type, which is sensible.
Actually getting a *hold* of this state is really easy too. You can use `&State<T>` as the type of a route parameter
to get it passed to you there, or you can retrieve it with the `state<T>()` method on a `Rocket` instance.
Since `Rocket` instances are fairly pervasive, you don't really run into issues where you need state but can't get to
it. I'm honestly pretty floored by how easy getting this set up was - I was expecting state to be a bit of a nightmare.

At this point I realised that I needed a metadata/manifest file to store some basic post information. I'm a big fan of
TOML so I decided that I'd use that, along with the `toml` package.  
My config wound up looking a bit like this:
```toml
# This document stores page configurations for blog posts, such as titles and upload dates.

[pages.building_this_site]
title = "Building this site"
published = 2021-06-22T21:55:00
```
Pretty clean and not too hard to expand later. So I'm happy with it.
Rocket has its own facilities for managing configs, but I didn't really want to deal with that. I'm lazy.

Since the `toml` package is based on Serde, deserialising into my own struct was very easy:
```rs
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

// Then, to actually deserialise:
let config: Config = toml::from_str(&page_config)?;
```
The `crate = "rocket::serde"` attribute is necessary since Rocket itself uses Serde, and re-exports it - it's not
a dependency I pulled in myself. Because `#[derive(Deserialize)]` (and co) implement Serde's traits via macro, they
needs to know the path to the crate, or else they'll produce invalid code. The
`deserialize_with = "deserialize_config_date"` attribute is a little cheat I use to deserialise from a TOML
date into a form that's a bit easier to work with - the `Datetime` struct in the `toml` package is... very limited.
I instead use Chrono's `DateTime`, which is a lot more sophisticated and has more functionality available than just
converting to a string. I really don't know why the maintainer of the `toml` package thought that was a good idea.

Anyway. The implementation of that function is pretty simple:
```rs
fn deserialize_config_date<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
where
    D: de::Deserializer<'de>,
{
    let toml_date: toml::value::Datetime = Deserialize::deserialize(deserializer)?;
    Local
        .datetime_from_str(&toml_date.to_string(), "%Y-%m-%dT%H:%M:%S")
        .map_err(|_| de::Error::custom("failed to parse date"))
}
```
Since my config only has one form of date, this was all I needed.

I also ended up using `IndexMap` since I wanted to keep information about these posts sorted by upload date.
I have a page that'll display a list of blog posts, and I don't wanna re-sort on each request. It's also helpful
to have them sorted from the get-go if I want to paginate things later down the line - I won't require a full sort
to display posts 51-100.

I wanted my config to update itself every so often. In Rust, this isn't entirely trivial because of borrowing rules,
but in my case it was pretty easy. I knew I wanted a `RwLock` but I was concerned about fair queueing - that is, if
multiple readers make a request to read, and a single writer makes a request to write in the middle, will that order
be honoured? Or will the read operations be grouped? It turns out that Tokio's `RwLock` is both asynchronous and
implements fair queueing on all platforms, which is exactly what I needed; I wasn't able to use a synchronous `RwLock`
here since that had the potential to block the thread - which is *not* what I wanted.  
Recall that I mentioned how Rocket's managed state needs to be both `Send` and `Sync`. This meant that I wound up
wrapping my `Config` struct in `RwLock`, and then wrapping *that* in `Arc`.  
Part of the reason for this is that I needed non-borrowed access to it in two places; The first being a "loop"
function that would update the config every so often, and the second being in my managed state.  
That loop function wound up looking like this:
```rs
async fn refresh_config(config: WrappedConfig) {
    // Realistically it's probably better to only do this if we detect changes. But this is fine for now.

    loop {
        if let Err(error) = config.write().await.try_update().await {
            eprintln!("Unable to update configuration: {}", error);
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}
```
(`WrappedConfig` is an alias to `Arc<RwLock<Config>>` because.. I'm lazy)

You may be wondering "oh, what's this `try_update()` method" and if so, I have bad news.
Because this function is *evil*. I love it, but it is evil. Do not copy it. Ever.
You have been warned.
```rs
// This is incredibly fucking evil.
pub async fn try_update(&mut self) -> Result<(), Error> {
    let mut result = Config::try_new().await?;
    mem::swap(self, &mut result);

    Ok(())
}
```
I even added a little comment to remind myself. Just in case I forgot.
If you're wondering how this actually *works*, here's an explanation:
1.  We try to create a new instance of `Config`.
    If this fails, the error is returned and control goes back to the caller - this is the `?` operator at play.
2.  We swap the memory position of the *actual* instance, and the new instance. In essence, our old config instance
    gets moved into the `result` variable, and then the new config instance gets put where it used to be.
3.  The method returns the unit type wrapped in `Ok()` to symbolise success, and control goes back to the caller.
    Except this is Rust! So when the method returns, destructors are called to clean up any values in scope.
    Because of our evil with memory swapping, this means our *old* config gets cleaned up as part of this.

It's such a dumb and hacky solution to this kinda problem. But it makes me giggle, so I guess it's worth it.

Anyways! It's starting to get late and man I am *tired*. Writing is hard. I'm gonna save this and chuck it onto a new
VPS somewhere. Probably something on DigitalOcean or Linode. Something cheap and easy.

See you on the flipside, and thanks for sticking with my terrible writing until the end!

\- Kaylynn