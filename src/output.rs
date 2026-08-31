use serde_json::Value;

use crate::commands::auth::WhoamiOutput;
use crate::errors::CliError;

pub fn print_response(value: &Value) -> Result<(), CliError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn print_whoami(output: &WhoamiOutput) -> Result<(), CliError> {
    println!("{}", serde_json::to_string_pretty(output)?);
    Ok(())
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
