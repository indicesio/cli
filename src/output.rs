use serde_json::{Map, Value};

use crate::config::OutputMode;
use crate::errors::CliError;

const PRIORITY_KEYS: &[&str] = &[
    "id",
    "uuid",
    "name",
    "display_name",
    "task_id",
    "run_id",
    "current_state",
    "status",
    "success",
    "created_at",
    "finished_at",
    "updated_at",
];

pub fn print_response(value: &Value, mode: OutputMode) -> Result<(), CliError> {
    match mode {
        OutputMode::Json => {
            println!("{}", serde_json::to_string_pretty(value)?);
        }
        OutputMode::Markdown => print_markdown(value),
    }

    Ok(())
}

fn print_markdown(value: &Value) {
    match value {
        Value::Array(items) => print_markdown_array(items),
        Value::Object(map) => {
            if let Some(Value::Array(items)) = map.get("data") {
                print_markdown_array(items);
                print_markdown_metadata(map, "data");
            } else {
                print_markdown_object(map);
            }
        }
        _ => println!("- `{}`", markdown_inline_code(&stringify_value(value))),
    }
}

fn print_markdown_metadata(map: &Map<String, Value>, skip_key: &str) {
    let keys = ordered_keys(map)
        .into_iter()
        .filter(|key| key.as_str() != skip_key)
        .collect::<Vec<_>>();

    if !keys.is_empty() {
        println!("\n## Metadata");
        for key in keys {
            if let Some(value) = map.get(&key) {
                print_markdown_field(&key, value);
            }
        }
    }
}

fn print_markdown_array(items: &[Value]) {
    if items.is_empty() {
        println!("_No results._");
        return;
    }

    if items.iter().all(Value::is_object) {
        for (index, item) in items.iter().enumerate() {
            let object = item
                .as_object()
                .expect("checked object shape before markdown object rendering");
            println!("## {}", markdown_item_title(index, object));
            print_markdown_object(object);
            if index + 1 != items.len() {
                println!();
            }
        }
        return;
    }

    for item in items {
        println!("- `{}`", markdown_inline_code(&stringify_value(item)));
    }
}

fn markdown_item_title(index: usize, object: &Map<String, Value>) -> String {
    let marker = object
        .get("id")
        .or_else(|| object.get("uuid"))
        .and_then(Value::as_str)
        .unwrap_or("");

    if marker.is_empty() {
        format!("Item {}", index + 1)
    } else {
        format!("Item {} ({marker})", index + 1)
    }
}

fn print_markdown_object(map: &Map<String, Value>) {
    if map.is_empty() {
        println!("_No fields._");
        return;
    }

    for key in ordered_keys(map) {
        if let Some(value) = map.get(&key) {
            print_markdown_field(&key, value);
        }
    }
}

fn ordered_keys(map: &Map<String, Value>) -> Vec<String> {
    let mut keys = map.keys().cloned().collect::<Vec<_>>();
    keys.sort_by_key(|key| {
        PRIORITY_KEYS
            .iter()
            .position(|priority| priority == key)
            .unwrap_or(usize::MAX)
    });

    // Keep non-priority keys deterministic by alphabetical order.
    let mut non_priority = keys
        .iter()
        .filter(|key| !PRIORITY_KEYS.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    non_priority.sort();

    let priority = keys
        .into_iter()
        .filter(|key| PRIORITY_KEYS.contains(&key.as_str()))
        .collect::<Vec<_>>();

    priority.into_iter().chain(non_priority).collect()
}

fn print_markdown_field(key: &str, value: &Value) {
    match value {
        Value::Array(_) | Value::Object(_) => {
            println!("- `{key}`:");
            print_json_block(value);
        }
        Value::String(raw) => {
            if let Some(parsed) = parse_json_like_string(raw) {
                println!("- `{key}`:");
                print_json_block(&parsed);
            } else if raw.contains('\n') {
                println!("- `{key}`:");
                println!("```text");
                println!("{}", raw.trim_end());
                println!("```");
            } else {
                println!("- `{key}`: `{}`", markdown_inline_code(raw));
            }
        }
        _ => {
            println!(
                "- `{key}`: `{}`",
                markdown_inline_code(&stringify_value(value))
            );
        }
    }
}

fn print_json_block(value: &Value) {
    println!("```json");
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| stringify_value(value))
    );
    println!("```");
}

fn parse_json_like_string(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if !((trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']')))
    {
        return None;
    }

    serde_json::from_str(trimmed).ok()
}

fn markdown_inline_code(value: &str) -> String {
    value
        .replace('`', "'")
        .replace('\r', "")
        .replace('\n', "\\n")
}

fn stringify_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.to_string(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}
