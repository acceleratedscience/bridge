use tera::{Tera, Value, Result as TeraResult};
use tracing::error;
use chrono::{DateTime, Utc};
use chrono_humanize::HumanTime;
use std::collections::HashMap;

pub fn start_template_eng() -> Tera {
    match Tera::new("templates/**/*.html") {
        Ok(mut tera) => {
            // Add custom filter for relative time
            tera.register_filter("time_ago", time_ago_filter);
            tera
        },
        Err(e) => {
            error!("Parsing error(s): {}", e);
            ::std::process::exit(1);
        }
    }
}

fn time_ago_filter(value: &Value, _: &HashMap<String, Value>) -> TeraResult<Value> {
    let time_str = value.as_str().ok_or_else(|| {
        tracing::error!("time_ago filter received non-string value: {:?}", value);
        tera::Error::msg("time_ago filter can only be applied to strings")
    })?;
    
    tracing::debug!("time_ago filter processing timestamp: {}", time_str);
    
    // Parse the timestamp - handle the format "2025-08-27 14:49:07.498643 +00:00:00"
    // Note: the timezone has an extra :00 at the end, so we need to handle that
    let normalized_time_str = if time_str.len() > 6 && time_str.ends_with(":00") {
        let trimmed = &time_str[..time_str.len() - 3]; // Remove the last ":00"
        tracing::debug!("Normalized timestamp from '{}' to '{}'", time_str, trimmed);
        trimmed
    } else {
        time_str
    };
    
    let parsed_time = DateTime::parse_from_str(normalized_time_str, "%Y-%m-%d %H:%M:%S%.f %z")
        .or_else(|_| {
            tracing::debug!("Failed to parse with first format, trying RFC3339");
            DateTime::parse_from_rfc3339(normalized_time_str)
        })
        .map_err(|e| {
            tracing::error!("Failed to parse timestamp '{}': {}", time_str, e);
            tera::Error::msg(format!("Failed to parse timestamp '{}': {}", time_str, e))
        })?;
    
    let utc_time: DateTime<Utc> = parsed_time.with_timezone(&Utc);
    let human_time = HumanTime::from(utc_time);
    let result = human_time.to_string();
    
    tracing::debug!("time_ago filter result: {}", result);
    Ok(Value::String(result))
}
