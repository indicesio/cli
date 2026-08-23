use serde_json::{Map, Value};

use clap::{CommandFactory, error::ErrorKind};

use crate::cli::{Cli, CreateRunArgs, RunIdArgs, RunsCommand};
use crate::client::ApiClient;
use crate::commands::payload::{
    ExplicitJsonSource, load_explicit_json_payload, load_json_stdin, parse_json_object_arg,
    stdin_has_data,
};
use crate::errors::CliError;

pub async fn handle_runs_command(
    client: &ApiClient,
    command: &RunsCommand,
) -> Result<Value, CliError> {
    match command {
        RunsCommand::Create(args) => create_run(client, args).await,
        RunsCommand::List(args) => Ok(client
            .list_runs(&args.connector_id, args.limit, args.cursor.as_deref())
            .await?),
        RunsCommand::Get(RunIdArgs { run_id }) => Ok(client.get_run(run_id).await?),
        RunsCommand::Logs(RunIdArgs { run_id }) => Ok(client.get_run_logs(run_id).await?),
    }
}

async fn create_run(client: &ApiClient, args: &CreateRunArgs) -> Result<Value, CliError> {
    let body = load_create_run_payload(args)?;
    Ok(client.create_run(body).await?)
}

fn load_create_run_payload(args: &CreateRunArgs) -> Result<Value, CliError> {
    let explicit = load_explicit_json_payload(ExplicitJsonSource {
        body: args.payload.body.as_deref(),
        file: args.payload.file.as_deref(),
        stdin: args.payload.stdin,
        command: "runs create",
    })?;

    let has_argument_values = has_run_argument_values(args);

    if let Some(payload) = explicit {
        if has_argument_values {
            return Err(CliError::Message(
                "do not mix `--body/--file/--stdin` with run argument flags".to_string(),
            ));
        }
        return Ok(payload);
    }

    if has_argument_values {
        return build_run_payload_from_args(args);
    }

    if stdin_has_data() {
        match load_json_stdin("runs create") {
            Ok(payload) => return Ok(payload),
            Err(CliError::Message(message))
                if message
                    .contains("`runs create` expected JSON on stdin, but stdin was empty") => {}
            Err(error) => return Err(error),
        }
    }

    Err(render_runs_create_help_error().into())
}

fn render_runs_create_help_error() -> clap::Error {
    let mut command = Cli::command();

    match command.try_get_matches_from_mut(["indices", "runs", "create", "--help"]) {
        Err(error) if error.kind() == ErrorKind::DisplayHelp => error,
        _ => command.error(
            ErrorKind::DisplayHelp,
            "Run `indices runs create --help` for usage.",
        ),
    }
}

fn has_run_argument_values(args: &CreateRunArgs) -> bool {
    args.connector_id.is_some()
        || args.arguments.is_some()
        || args.secret_bindings.is_some()
        || args.run_async
        || args.max_timeout_s.is_some()
}

fn build_run_payload_from_args(args: &CreateRunArgs) -> Result<Value, CliError> {
    let connector_id = args.connector_id.clone().ok_or_else(|| {
        CliError::Message("`runs create` requires `--connector-id` in argument mode".to_string())
    })?;

    let arguments = if let Some(raw) = args.arguments.as_deref() {
        parse_json_object_arg(raw, "--arguments")?
    } else {
        Map::new()
    };

    let secret_bindings = if let Some(raw) = args.secret_bindings.as_deref() {
        parse_json_object_arg(raw, "--secret-bindings")?
    } else {
        Map::new()
    };

    if let Some(max_timeout_s) = args.max_timeout_s
        && !(1..=3600).contains(&max_timeout_s)
    {
        return Err(CliError::Message(
            "`--max-timeout-s` must be between 1 and 3600".to_string(),
        ));
    }

    let mut payload = Map::new();
    payload.insert("connector_id".to_string(), Value::String(connector_id));
    payload.insert("arguments".to_string(), Value::Object(arguments));
    payload.insert(
        "secret_bindings".to_string(),
        Value::Object(secret_bindings),
    );
    if args.run_async {
        payload.insert("async".to_string(), Value::Bool(true));
    }
    if let Some(max_timeout_s) = args.max_timeout_s {
        payload.insert(
            "max_timeout_s".to_string(),
            Value::Number(max_timeout_s.into()),
        );
    }

    Ok(Value::Object(payload))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::cli::CreatePayloadSourceArgs;

    fn empty_payload_source() -> CreatePayloadSourceArgs {
        CreatePayloadSourceArgs {
            body: None,
            file: None,
            stdin: false,
        }
    }

    #[test]
    fn builds_run_payload_from_argument_mode() {
        let args = CreateRunArgs {
            payload: empty_payload_source(),
            connector_id: Some("con_0A1b2C3d4E5f6G7h8I9j0K".to_string()),
            arguments: Some(r#"{"job_id":"A1"}"#.to_string()),
            secret_bindings: Some(r#"{"login":"sec_0A1b2C3d4E5f6G7h8I9j0K"}"#.to_string()),
            run_async: true,
            max_timeout_s: Some(120),
        };

        let payload = build_run_payload_from_args(&args).expect("payload should build");

        assert_eq!(payload["connector_id"], "con_0A1b2C3d4E5f6G7h8I9j0K");
        assert_eq!(payload["arguments"], json!({"job_id":"A1"}));
        assert_eq!(
            payload["secret_bindings"],
            json!({"login":"sec_0A1b2C3d4E5f6G7h8I9j0K"})
        );
        assert_eq!(payload["async"], true);
        assert_eq!(payload["max_timeout_s"], 120);
    }

    #[test]
    fn run_arguments_require_connector_id() {
        let args = CreateRunArgs {
            payload: empty_payload_source(),
            connector_id: None,
            arguments: Some(r#"{"job_id":"A1"}"#.to_string()),
            secret_bindings: None,
            run_async: false,
            max_timeout_s: None,
        };

        let error =
            build_run_payload_from_args(&args).expect_err("missing connector id should fail");
        assert!(
            error
                .to_string()
                .contains("`runs create` requires `--connector-id` in argument mode")
        );
    }

    #[test]
    fn rejects_out_of_range_max_timeout() {
        let args = CreateRunArgs {
            payload: empty_payload_source(),
            connector_id: Some("con_123".to_string()),
            arguments: None,
            secret_bindings: None,
            run_async: false,
            max_timeout_s: Some(5000),
        };

        let error = build_run_payload_from_args(&args).expect_err("timeout should fail");
        assert!(
            error
                .to_string()
                .contains("`--max-timeout-s` must be between")
        );
    }
}
