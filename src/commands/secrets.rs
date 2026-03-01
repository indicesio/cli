use std::io::{self, Read, Write};

use serde_json::{Value, json};

use crate::cli::{CreateSecretArgs, DeleteSecretArgs, SecretsCommand};
use crate::client::ApiClient;
use crate::commands::prompt_confirm;
use crate::errors::CliError;

pub async fn handle_secrets_command(
    client: &ApiClient,
    command: &SecretsCommand,
) -> Result<Value, CliError> {
    match command {
        SecretsCommand::Create(args) => create_secret(client, args).await,
        SecretsCommand::List => client.list_secrets().await.map_err(Into::into),
        SecretsCommand::Delete(DeleteSecretArgs { uuid, yes }) => {
            delete_secret(client, uuid, *yes).await
        }
    }
}

async fn create_secret(client: &ApiClient, args: &CreateSecretArgs) -> Result<Value, CliError> {
    let value = secret_value(args)?;

    if value.is_empty() {
        return Err(CliError::Message(
            "secret value cannot be empty".to_string(),
        ));
    }

    let response = client.create_secret(&args.name, &value).await?;
    Ok(response)
}

async fn delete_secret(client: &ApiClient, uuid: &str, yes: bool) -> Result<Value, CliError> {
    if !yes {
        let confirmed = prompt_confirm(&format!("Delete secret `{uuid}`?"))?;
        if !confirmed {
            return Ok(json!({
                "deleted": false,
                "uuid": uuid,
                "message": "aborted"
            }));
        }
    }

    client.delete_secret(uuid).await.map_err(Into::into)
}

fn secret_value(args: &CreateSecretArgs) -> Result<String, CliError> {
    if args.stdin {
        let mut raw = String::new();
        io::stdin().read_to_string(&mut raw)?;
        return Ok(raw.trim_end_matches(['\n', '\r']).to_string());
    }

    if let Some(value) = &args.value {
        return Ok(value.clone());
    }

    print!("Enter value for secret `{}`: ", args.name);
    io::stdout().flush()?;
    let value = rpassword::read_password()?;
    Ok(value)
}
