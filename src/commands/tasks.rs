use serde_json::{Map, Value, json};

use crate::cli::{CreateTaskArgs, DeleteTaskArgs, TaskIdArgs, TasksCommand};
use crate::client::ApiClient;
use crate::commands::payload::{
    ExplicitJsonSource, load_explicit_json_payload, load_json_stdin, parse_json_object_arg,
    stdin_has_data,
};
use crate::commands::prompt_confirm;
use crate::errors::CliError;

pub async fn handle_tasks_command(
    client: &ApiClient,
    command: &TasksCommand,
) -> Result<Value, CliError> {
    match command {
        TasksCommand::Create(args) => create_task(client, args).await,
        TasksCommand::Get(TaskIdArgs { task_id }) => {
            client.get_task(task_id).await.map_err(Into::into)
        }
        TasksCommand::List(args) => client
            .list_tasks(args.status.as_deref(), args.limit, args.cursor.as_deref())
            .await
            .map_err(Into::into),
        TasksCommand::Delete(DeleteTaskArgs { task_id, yes }) => {
            delete_task(client, task_id, *yes).await
        }
        TasksCommand::Retry(TaskIdArgs { task_id }) => {
            client.retry_task(task_id).await.map_err(Into::into)
        }
        TasksCommand::RegenerateApi(TaskIdArgs { task_id }) => client
            .regenerate_task_api(task_id)
            .await
            .map_err(Into::into),
    }
}

async fn create_task(client: &ApiClient, args: &CreateTaskArgs) -> Result<Value, CliError> {
    let body = load_create_task_payload(args)?;
    client.create_task(body).await.map_err(Into::into)
}

async fn delete_task(client: &ApiClient, task_id: &str, yes: bool) -> Result<Value, CliError> {
    if !yes {
        let confirmed = prompt_confirm(&format!("Delete task `{task_id}`?"))?;
        if !confirmed {
            return Ok(json!({
                "deleted": false,
                "task_id": task_id,
                "message": "aborted"
            }));
        }
    }

    client.delete_task(task_id).await.map_err(Into::into)
}

fn load_create_task_payload(args: &CreateTaskArgs) -> Result<Value, CliError> {
    let explicit = load_explicit_json_payload(ExplicitJsonSource {
        body: args.payload.body.as_deref(),
        file: args.payload.file.as_deref(),
        stdin: args.payload.stdin,
        command: "tasks create",
    })?;

    let has_argument_values = has_task_argument_values(args);

    if let Some(payload) = explicit {
        if has_argument_values {
            return Err(CliError::Message(
                "do not mix `--body/--file/--stdin` with task argument flags".to_string(),
            ));
        }
        return Ok(payload);
    }

    if has_argument_values {
        return build_task_payload_from_args(args);
    }

    if stdin_has_data() {
        return load_json_stdin("tasks create");
    }

    Err(CliError::Message(
        "provide task arguments (`--display-name`, `--website`, `--task`) or one of `--body`, `--file`, `--stdin`"
            .to_string(),
    ))
}

fn has_task_argument_values(args: &CreateTaskArgs) -> bool {
    args.display_name.is_some()
        || args.website.is_some()
        || args.task.is_some()
        || args.input_schema.is_some()
        || args.output_schema.is_some()
        || args.creation_params.is_some()
}

fn build_task_payload_from_args(args: &CreateTaskArgs) -> Result<Value, CliError> {
    let display_name = args.display_name.clone().ok_or_else(|| {
        CliError::Message("`tasks create` requires `--display-name` in argument mode".to_string())
    })?;
    let website = args.website.clone().ok_or_else(|| {
        CliError::Message("`tasks create` requires `--website` in argument mode".to_string())
    })?;
    let task = args.task.clone().ok_or_else(|| {
        CliError::Message("`tasks create` requires `--task` in argument mode".to_string())
    })?;

    let creation_params = if let Some(raw) = args.creation_params.as_deref() {
        parse_json_object_arg(raw, "--creation-params")?
    } else {
        Map::new()
    };

    let mut payload = Map::new();
    payload.insert(
        "creation_params".to_string(),
        Value::Object(creation_params),
    );
    payload.insert("display_name".to_string(), Value::String(display_name));
    payload.insert("website".to_string(), Value::String(website));
    payload.insert("task".to_string(), Value::String(task));

    if let Some(input_schema) = &args.input_schema {
        payload.insert(
            "input_schema".to_string(),
            Value::String(input_schema.clone()),
        );
    }

    if let Some(output_schema) = &args.output_schema {
        payload.insert(
            "output_schema".to_string(),
            Value::String(output_schema.clone()),
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
    fn builds_task_payload_from_argument_mode() {
        let args = CreateTaskArgs {
            payload: empty_payload_source(),
            display_name: Some("Apply Job".to_string()),
            website: Some("https://example.com".to_string()),
            task: Some("Fill out application".to_string()),
            input_schema: None,
            output_schema: None,
            creation_params: None,
        };

        let payload = build_task_payload_from_args(&args).expect("payload should build");

        assert_eq!(payload["display_name"], "Apply Job");
        assert_eq!(payload["website"], "https://example.com");
        assert_eq!(payload["task"], "Fill out application");
        assert_eq!(payload["creation_params"], json!({}));
    }

    #[test]
    fn creation_params_must_be_json_object() {
        let args = CreateTaskArgs {
            payload: empty_payload_source(),
            display_name: Some("Apply Job".to_string()),
            website: Some("https://example.com".to_string()),
            task: Some("Fill out application".to_string()),
            input_schema: None,
            output_schema: None,
            creation_params: Some("[]".to_string()),
        };

        let error = build_task_payload_from_args(&args).expect_err("array should fail");
        assert!(
            error
                .to_string()
                .contains("--creation-params must be a JSON object")
        );
    }
}
