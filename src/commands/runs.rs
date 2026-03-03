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
        RunsCommand::List(args) => client
            .list_runs(Some(&args.task_id), args.limit, args.cursor.as_deref())
            .await
            .map_err(Into::into),
        RunsCommand::Get(RunIdArgs { run_id }) => client.get_run(run_id).await.map_err(Into::into),
    }
}

async fn create_run(client: &ApiClient, args: &CreateRunArgs) -> Result<Value, CliError> {
    let body = load_create_run_payload(args)?;
    client.create_run(body).await.map_err(Into::into)
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
                if message.contains("`runs create` expected JSON on stdin, but stdin was empty") => {}
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
    args.task_id.is_some() || args.arguments.is_some() || args.secret_bindings.is_some()
}

fn build_run_payload_from_args(args: &CreateRunArgs) -> Result<Value, CliError> {
    let task_id = args.task_id.clone().ok_or_else(|| {
        CliError::Message("`runs create` requires `--task-id` in argument mode".to_string())
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

    let mut payload = Map::new();
    payload.insert("task_id".to_string(), Value::String(task_id));
    payload.insert("arguments".to_string(), Value::Object(arguments));
    payload.insert(
        "secret_bindings".to_string(),
        Value::Object(secret_bindings),
    );

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
            task_id: Some("11111111-1111-1111-1111-111111111111".to_string()),
            arguments: Some(r#"{"job_id":"A1"}"#.to_string()),
            secret_bindings: Some(
                r#"{"login":"22222222-2222-2222-2222-222222222222"}"#.to_string(),
            ),
        };

        let payload = build_run_payload_from_args(&args).expect("payload should build");

        assert_eq!(payload["task_id"], "11111111-1111-1111-1111-111111111111");
        assert_eq!(payload["arguments"], json!({"job_id":"A1"}));
        assert_eq!(
            payload["secret_bindings"],
            json!({"login":"22222222-2222-2222-2222-222222222222"})
        );
    }

    #[test]
    fn run_arguments_require_task_id() {
        let args = CreateRunArgs {
            payload: empty_payload_source(),
            task_id: None,
            arguments: Some(r#"{"job_id":"A1"}"#.to_string()),
            secret_bindings: None,
        };

        let error = build_run_payload_from_args(&args).expect_err("missing task id should fail");
        assert!(
            error
                .to_string()
                .contains("`runs create` requires `--task-id` in argument mode")
        );
    }
}
