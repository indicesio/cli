use serde_json::Value;

use crate::cli::CapturesCommand;
use crate::client::ApiClient;
use crate::commands::payload::parse_json_value;
use crate::errors::CliError;

pub async fn handle_captures_command(
    client: &ApiClient,
    command: &CapturesCommand,
) -> Result<Value, CliError> {
    match command {
        CapturesCommand::Start(args) => {
            let cookies = if let Some(raw) = args.cookies.as_deref() {
                Some(parse_cookies(raw)?)
            } else {
                None
            };
            Ok(client
                .start_capture_session(args.use_proxy, cookies)
                .await?)
        }
        CapturesCommand::List => Ok(client.list_capture_sessions().await?),
        CapturesCommand::Get(args) => {
            Ok(client.get_capture_session(&args.capture_session_id).await?)
        }
        CapturesCommand::Complete(args) => Ok(client
            .complete_capture_session(&args.capture_session_id)
            .await?),
        CapturesCommand::Abandon(args) => Ok(client
            .abandon_capture_session(&args.capture_session_id)
            .await?),
    }
}

fn parse_cookies(raw: &str) -> Result<Value, CliError> {
    let value = parse_json_value(raw, "--cookies")?;
    if !value.is_array() {
        return Err(CliError::Message(
            "--cookies must be a JSON array of cookie objects".to_string(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::parse_cookies;
    use serde_json::json;

    #[test]
    fn parses_cookie_array() {
        let cookies = parse_cookies(r#"[{"name":"sid","value":"abc"}]"#).expect("cookies");
        assert_eq!(cookies, json!([{"name":"sid","value":"abc"}]));
    }

    #[test]
    fn rejects_non_array_cookies() {
        let error = parse_cookies(r#"{"name":"sid"}"#).expect_err("object should fail");
        assert!(error.to_string().contains("JSON array"));
    }
}
