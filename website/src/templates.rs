use axum::response::Html;
use chrono::{self, DateTime, Local};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use std::collections::HashMap;
use tera::{Context, Error, Result, Tera, Value};

#[macro_export]
macro_rules! context {
    ($($key:expr => $value:expr,)+) => { context! {$($key => $value),*} };
    ($($key:expr => $value:expr),*) => {{
        let mut context: ::tera::Context = ::tera::Context::new();
        $(context.insert($key, &$value);)*
        context
    }};
}

/// The template engine used for rendering templates.
#[derive(Debug)]
pub struct Engine(Tera);

impl Engine {
    pub fn new(tera: Tera) -> Self {
        Self(tera)
    }

    pub fn render(&self, template_name: &str, context: &Context) -> Result<Html<String>> {
        self.0.render(template_name, context).map(Html)
    }
}

pub fn humanize(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let post_date = value
        .as_str()
        .ok_or_else(|| Error::msg("Value is not a string"))?
        .parse::<DateTime<Local>>()
        .map_err(|_| Error::msg("Unable to parse time"))?;

    let duration = post_date - Local::now();
    let human_time = HumanTime::from(duration);
    let result = human_time.to_text_en(Accuracy::Rough, Tense::Past);

    Ok(Value::from(result))
}
