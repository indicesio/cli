use std::io::{self, Write};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Serialize;
use serde_json::Value;
use tracing::warn;

use crate::cli::LoginArgs;
use crate::client::{ApiClient, ClientOptions, IdentityResponse};
use crate::config::{CachedIdentity, ConfigStore, RuntimeConfig, StoredAuth, StoredSession};
use crate::errors::CliError;
use crate::oauth;

#[derive(Debug, Clone, Serialize)]
pub struct WhoamiOutput {
    pub user_id: String,
    pub email: String,
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

    let mut identity = identity_from_jwt(&auth);

    if !args.no_verify {
        let client = ApiClient::new(ClientOptions {
            api_base: runtime.api_base.clone(),
            bearer_token: auth.bearer_token().to_string(),
            timeout_seconds: runtime.timeout_seconds,
        })?;

        // Override identity from JWT only if the identity from API is non-empty
        // Note: it may be empty if internet issues contacting the Indices API
        if let Some(api_identity) = identity_from_api(&client).await {
            identity = Some(api_identity);
        }
    }

    let stored_message = match &auth {
        StoredAuth::ApiKey { .. } => "Stored API key in local config.",
        StoredAuth::OAuth { .. } => "Stored OAuth credentials in local config.",
    };

    config_store.set_session(
        StoredSession { auth, identity },
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

/// Get the identity from the OAuth ID (JWT) token
fn identity_from_jwt(auth: &StoredAuth) -> Option<CachedIdentity> {
    let StoredAuth::OAuth { access_token, .. } = auth else {
        return None;
    };

    let payload = match access_token.split('.').nth(1) {
        Some(payload) => payload,
        None => {
            warn!("failed to extract identity from oauth token: missing jwt payload segment");
            return None;
        }
    };
    let bytes = match URL_SAFE_NO_PAD.decode(payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            warn!(%error, "failed to extract identity from oauth token: invalid base64 payload");
            return None;
        }
    };
    let value = match serde_json::from_slice::<Value>(&bytes) {
        Ok(value) => value,
        Err(error) => {
            warn!(%error, "failed to extract identity from oauth token: invalid json payload");
            return None;
        }
    };

    let object = value.as_object()?;
    let user_id = object.get("sub").and_then(Value::as_str)?.trim();
    let email = object.get("email").and_then(Value::as_str)?.trim();
    if user_id.is_empty() || email.is_empty() {
        warn!("failed to extract identity from oauth token: empty sub or email claim");
        return None;
    }

    Some(CachedIdentity::new(user_id.to_string(), email.to_string()))
}

/// Get the identity from the backend
async fn identity_from_api(client: &ApiClient) -> Option<CachedIdentity> {
    let identity_response = match client.get_identity().await {
        Ok(identity_response) => identity_response,
        Err(error) => {
            warn!(%error, "failed to fetch identity from backend");
            return None;
        }
    };
    let IdentityResponse { user_id, email } = identity_response;

    Some(CachedIdentity::new(user_id, email))
}

// TODO: improve test coverage
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn extracts_cached_identity_from_oauth_claims() {
        let payload = URL_SAFE_NO_PAD.encode(r#"{"sub":"user_123","email":"user@example.com"}"#);
        let auth = StoredAuth::OAuth {
            access_token: format!("header.{payload}.signature"),
            refresh_token: "refresh".to_string(),
            expires_at: Utc
                .with_ymd_and_hms(2026, 4, 15, 12, 0, 0)
                .single()
                .expect("timestamp should be valid"),
        };

        let identity = identity_from_jwt(&auth).expect("identity should be extracted");

        assert_eq!(
            identity,
            CachedIdentity {
                user_id: "user_123".to_string(),
                email: "user@example.com".to_string(),
            }
        );
    }

    #[test]
    fn ignores_non_oauth_auth_when_extracting_identity() {
        let auth = StoredAuth::ApiKey {
            api_key: "idx_test".to_string(),
        };

        assert_eq!(identity_from_jwt(&auth), None);
    }
}
