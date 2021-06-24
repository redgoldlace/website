use chrono::{self, DateTime, Local};
use chrono_humanize::{Accuracy, HumanTime, Tense};
use rocket_dyn_templates::tera::{Error, Result, Value};
use std::collections::HashMap;

pub fn humanise(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let input = match value {
        Value::String(string) => Ok(string),
        _ => Err(Error::msg("Value is not a string")),
    }?;

    let post_date = input
        .parse::<DateTime<Local>>()
        .map_err(|_| Error::msg("Unable to parse time"))?;

    let duration = post_date - Local::now();
    let human_time = HumanTime::from(duration);
    let result = human_time.to_text_en(Accuracy::Rough, Tense::Past);

    Ok(Value::from(result))
}
