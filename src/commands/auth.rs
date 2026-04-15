use std::io::{self, Write};

use serde::Serialize;
use serde_json::Value;

use crate::cli::LoginArgs;
use crate::client::{ApiClient, ClientOptions, IdentityResponse};
use crate::config::{ConfigStore, RuntimeConfig, StoredAuth};
use crate::errors::CliError;
use crate::oauth;

#[derive(Debug, Clone, Serialize)]
pub struct WhoamiOutput {
    pub user_id: Value,
    pub email: Option<String>,
}

pub async fn login(
    config_store: &mut ConfigStore,
    runtime: RuntimeConfig,
    args: &LoginArgs,
) -> Result<(), CliError> {
    let auth = match &args.api_key {
        Some(key) => {
            let api_key = if key.trim().is_empty() {
                read_api_key_from_prompt()?
            } else {
                key.trim().to_string()
            };

            if api_key.is_empty() {
                return Err(CliError::Message("API key cannot be empty".to_string()));
            }

            StoredAuth::ApiKey { api_key }
        }
        None => oauth::login_with_oauth(runtime.timeout_seconds).await?,
    };

    if !args.no_verify {
        let client = ApiClient::new(ClientOptions {
            api_base: runtime.api_base.clone(),
            bearer_token: auth.bearer_token().to_string(),
            timeout_seconds: runtime.timeout_seconds,
        })?;

        client.get_identity().await.map_err(|error| {
            CliError::Message(format!("authentication verification failed: {error}"))
        })?;
    }

    let stored_message = match &auth {
        StoredAuth::ApiKey { .. } => "Stored API key in local config.",
        StoredAuth::OAuth { .. } => "Stored OAuth credentials in local config.",
    };

    config_store.set_auth(
        auth,
        Some(runtime.api_base.as_str()),
        Some(runtime.timeout_seconds),
    )?;
    println!("{stored_message}");

    Ok(())
}

pub fn logout(config_store: &mut ConfigStore) -> Result<(), CliError> {
    let removed = config_store.clear_auth()?;

    if removed {
        println!("Logged out.");
    } else {
        println!("Already logged out.");
    }

    Ok(())
}

pub async fn whoami(client: &ApiClient) -> Result<WhoamiOutput, CliError> {
    let IdentityResponse { user_id, email } = client.get_identity().await?;

    Ok(WhoamiOutput { user_id, email })
}

fn read_api_key_from_prompt() -> Result<String, CliError> {
    print!("Enter API key: ");
    io::stdout().flush()?;

    let key = rpassword::read_password()?;
    Ok(key.trim().to_string())
}
