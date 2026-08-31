use serde_json::Value;

use crate::commands::auth::WhoamiOutput;
use crate::errors::CliError;

pub fn print_response(value: &Value) -> Result<(), CliError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn print_whoami(output: &WhoamiOutput) -> Result<(), CliError> {
    print!("{}", format_whoami(output));
    Ok(())
}

fn format_whoami(output: &WhoamiOutput) -> String {
    const GREEN: &str = "\x1b[32m";
    const CYAN: &str = "\x1b[36m";
    const GREY: &str = "\x1b[90m";
    const RESET: &str = "\x1b[0m";

    let rows = [
        ("Email", output.email.as_str()),
        ("User ID", output.user_id.as_str()),
    ];
    let width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);

    let mut rendered = String::new();
    rendered.push_str(&format!("{GREEN}✔{RESET} You are logged in to Indices\n"));
    for (label, value) in rows {
        rendered.push_str(&format!("{label:<width$}  {value}\n", width = width));
    }
    rendered.push_str(&format!(
        "{GREY}Run {CYAN}indices logout{GREY} to log out{RESET}\n"
    ));
    rendered
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::format_whoami;
    use crate::commands::auth::WhoamiOutput;

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

    #[test]
    fn whoami_renders_human_status() {
        let rendered = format_whoami(&WhoamiOutput {
            user_id: "user_123".to_string(),
            email: "user@example.com".to_string(),
        });

        assert!(rendered.contains("You are logged in to Indices"));
        assert!(rendered.contains("user@example.com"));
        assert!(rendered.contains("user_123"));
        assert!(!rendered.contains('{'));
        assert!(!rendered.contains('}'));
    }
}
