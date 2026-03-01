use std::io::{self, Write};

use serde_json::{Value, json};

use crate::cli::LoginArgs;
use crate::client::{ApiClient, ClientOptions};
use crate::config::{ConfigStore, RuntimeConfig};
use crate::errors::CliError;

pub async fn login(
    config_store: &mut ConfigStore,
    runtime: RuntimeConfig,
    args: &LoginArgs,
) -> Result<(), CliError> {
    let api_key = match &args.api_key {
        Some(key) => key.trim().to_string(),
        None => read_api_key_from_prompt()?,
    };

    if api_key.is_empty() {
        return Err(CliError::Message("API key cannot be empty".to_string()));
    }

    if !args.no_verify {
        let client = ApiClient::new(ClientOptions {
            api_base: runtime.api_base.clone(),
            api_key: api_key.clone(),
            timeout_seconds: runtime.timeout_seconds,
        })?;

        client
            .whoami_probe()
            .await
            .map_err(|error| CliError::Message(format!("API key verification failed: {error}")))?;
    }

    config_store.set_api_key(
        api_key,
        Some(runtime.api_base.as_str()),
        Some(runtime.timeout_seconds),
    )?;
    println!("Stored API key in local config.");

    Ok(())
}

pub fn logout(config_store: &mut ConfigStore) -> Result<(), CliError> {
    let removed = config_store.clear_api_key()?;

    if removed {
        println!("Logged out.");
    } else {
        println!("Already logged out.");
    }

    Ok(())
}

pub async fn whoami(client: &ApiClient) -> Result<Value, CliError> {
    let probe = client.whoami_probe().await?;

    Ok(json!({
        "authenticated": true,
        "probe": probe,
    }))
}

fn read_api_key_from_prompt() -> Result<String, CliError> {
    print!("Enter API key: ");
    io::stdout().flush()?;

    let key = rpassword::read_password()?;
    Ok(key.trim().to_string())
}
