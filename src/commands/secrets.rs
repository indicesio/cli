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
        SecretsCommand::List => Ok(client.list_secrets().await?),
        SecretsCommand::Delete(DeleteSecretArgs { id, yes }) => {
            delete_secret(client, id, *yes).await
        }
        SecretsCommand::Totp(args) => Ok(client.generate_totp(&args.id).await?),
    }
}

async fn create_secret(client: &ApiClient, args: &CreateSecretArgs) -> Result<Value, CliError> {
    let secret_type = resolve_secret_type(args)?;

    match secret_type.as_str() {
        "string" => {
            let value = string_secret_value(args)?;
            if value.is_empty() {
                return Err(CliError::Message(
                    "secret value cannot be empty".to_string(),
                ));
            }
            Ok(client
                .create_string_secret(&args.name, &value, args.website.as_deref())
                .await?)
        }
        "login" => {
            let username = args.username.clone().ok_or_else(|| {
                CliError::Message("`secrets create --type login` requires `--username`".to_string())
            })?;
            let password = login_password(args)?;
            if username.is_empty() || password.is_empty() {
                return Err(CliError::Message(
                    "login username and password cannot be empty".to_string(),
                ));
            }
            Ok(client
                .create_login_secret(
                    &args.name,
                    &username,
                    &password,
                    args.totp_secret.as_deref(),
                    args.website.as_deref(),
                )
                .await?)
        }
        other => Err(CliError::Message(format!(
            "invalid `--type` `{other}`; expected `string` or `login`"
        ))),
    }
}

async fn delete_secret(client: &ApiClient, id: &str, yes: bool) -> Result<Value, CliError> {
    if !yes {
        let confirmed = prompt_confirm(&format!("Delete secret `{id}`?"))?;
        if !confirmed {
            return Ok(json!({
                "deleted": false,
                "id": id,
                "message": "aborted"
            }));
        }
    }

    Ok(client.delete_secret(id).await?)
}

fn resolve_secret_type(args: &CreateSecretArgs) -> Result<String, CliError> {
    if let Some(secret_type) = &args.secret_type {
        return Ok(secret_type.to_ascii_lowercase());
    }

    if args.username.is_some() || args.password.is_some() || args.totp_secret.is_some() {
        return Ok("login".to_string());
    }

    Ok("string".to_string())
}

fn string_secret_value(args: &CreateSecretArgs) -> Result<String, CliError> {
    if args.username.is_some() || args.password.is_some() || args.totp_secret.is_some() {
        return Err(CliError::Message(
            "do not pass `--username`, `--password`, or `--totp-secret` for string secrets"
                .to_string(),
        ));
    }

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

fn login_password(args: &CreateSecretArgs) -> Result<String, CliError> {
    if args.stdin || args.value.is_some() {
        return Err(CliError::Message(
            "login secrets use `--password`, not `--value` or `--stdin`".to_string(),
        ));
    }

    if let Some(password) = &args.password {
        return Ok(password.clone());
    }

    print!("Enter password for secret `{}`: ", args.name);
    io::stdout().flush()?;
    Ok(rpassword::read_password()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CreateSecretArgs;

    fn base_args() -> CreateSecretArgs {
        CreateSecretArgs {
            name: "demo".to_string(),
            secret_type: None,
            value: None,
            username: None,
            password: None,
            totp_secret: None,
            website: None,
            stdin: false,
        }
    }

    #[test]
    fn defaults_to_string_type() {
        assert_eq!(resolve_secret_type(&base_args()).unwrap(), "string");
    }

    #[test]
    fn infers_login_type_from_username() {
        let mut args = base_args();
        args.username = Some("user".to_string());
        assert_eq!(resolve_secret_type(&args).unwrap(), "login");
    }
}
