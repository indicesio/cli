use serde_json::{Map, Value};

use crate::commands::auth::WhoamiOutput;
use crate::config::OutputMode;
use crate::errors::CliError;

pub fn print_response(value: &Value, mode: OutputMode) -> Result<(), CliError> {
    match mode {
        OutputMode::Json => {
            println!("{}", serde_json::to_string_pretty(value)?);
        }
        OutputMode::Markdown => print_markdown(value),
    }

    Ok(())
}

pub fn print_whoami(output: &WhoamiOutput, mode: OutputMode) -> Result<(), CliError> {
    match mode {
        OutputMode::Json => {
            println!("{}", serde_json::to_string_pretty(output)?);
        }
        OutputMode::Markdown => print_whoami_pretty(output),
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
    let fields = map
        .iter()
        .filter(|(key, _)| key.as_str() != skip_key)
        .collect::<Vec<_>>();

    if !fields.is_empty() {
        println!("\n## Metadata");
        for (key, value) in fields {
            print_markdown_field(key, value);
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

    for (key, value) in map {
        print_markdown_field(key, value);
    }
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

fn print_whoami_pretty(output: &WhoamiOutput) {
    const GREEN: &str = "\x1b[32m";
    const CYAN: &str = "\x1b[36m";
    const GREY: &str = "\x1b[90m";
    const RESET: &str = "\x1b[0m";

    println!("{GREEN}✔{RESET} You are logged in to Indices");

    let rows = vec![
        ("Email", output.email.clone()),
        ("User ID", output.user_id.clone()),
    ];

    let width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
    for (label, value) in rows {
        println!("{label:<width$}  {value}", width = width);
    }

    println!("{GREY}Run {CYAN}indices logout{GREY} to log out{RESET}");
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    fn object_keys(value: &Value) -> Vec<String> {
        value.as_object().expect("object").keys().cloned().collect()
    }

    #[test]
    fn pretty_json_preserves_backend_key_order() {
        let value: Value = serde_json::from_str(
            r#"{
                "id": "run_1",
                "connector_id": "conn_1",
                "arguments": {"listing_id": "1", "adults": 2},
                "status": "success",
                "error": null
            }"#,
        )
        .expect("valid json");

        assert_eq!(
            object_keys(&value),
            ["id", "connector_id", "arguments", "status", "error"]
        );
        assert_eq!(object_keys(&value["arguments"]), ["listing_id", "adults"]);

        let pretty = serde_json::to_string_pretty(&value).expect("pretty json");
        let id = pretty.find("\"id\"").expect("id");
        let connector_id = pretty.find("\"connector_id\"").expect("connector_id");
        let arguments = pretty.find("\"arguments\"").expect("arguments");
        let listing_id = pretty.find("\"listing_id\"").expect("listing_id");
        let adults = pretty.find("\"adults\"").expect("adults");
        let status = pretty.find("\"status\"").expect("status");

        assert!(id < connector_id);
        assert!(connector_id < arguments);
        assert!(arguments < listing_id);
        assert!(listing_id < adults);
        assert!(adults < status);
    }
}
