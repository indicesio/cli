use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Duration as ChronoDuration, Utc};
use reqwest::StatusCode;
use serde::Deserialize;

use tracing::instrument;

use crate::config::StoredAuth;
use crate::errors::CliError;

const OAUTH_BASE_URL: &str = "https://mature-guitar-27.authkit.app";
const OAUTH_CLIENT_ID: &str = "client_01KN11DZ1XJ9BS3DG9XQJPZPXP";
const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
const REFRESH_GRANT_TYPE: &str = "refresh_token";
const REFRESH_SKEW_SECONDS: i64 = 30;
const OAUTH_SCOPES: &str = "openid profile email offline_access";

#[derive(Debug, Clone)]
struct OAuthClientConfig {
    base_url: String,
    client_id: String,
}

#[derive(Debug, Deserialize)]
struct DeviceAuthorizationResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    error: String,
    error_description: Option<String>,
}

pub async fn login_with_oauth(timeout_seconds: u64) -> Result<StoredAuth, CliError> {
    let config = auth_client_config()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;

    let device = request_device_code(&client, &config).await?;
    print_login_instructions(&device);

    let open_target = device
        .verification_uri_complete
        .as_deref()
        .unwrap_or(device.verification_uri.as_str());
    let opened = open_browser(open_target);
    if opened {
        println!("Opened browser for sign-in.");
    } else {
        println!("Open this URL to continue: {open_target}");
    }

    poll_for_device_tokens(&client, &config, &device).await
}

#[instrument(name = "cli.oauth_refresh", skip_all, fields(force), err)]
pub async fn refresh_auth(
    auth: &StoredAuth,
    timeout_seconds: u64,
    force: bool,
) -> Result<Option<StoredAuth>, CliError> {
    let StoredAuth::OAuth {
        refresh_token,
        expires_at,
        ..
    } = auth
    else {
        return Ok(None);
    };

    if !force && *expires_at > Utc::now() + ChronoDuration::seconds(REFRESH_SKEW_SECONDS) {
        return Ok(None);
    }

    let config = auth_client_config()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;

    let response = client
        .post(format!(
            "{}/oauth2/token",
            config.base_url.trim_end_matches('/')
        ))
        .form(&[
            ("grant_type", REFRESH_GRANT_TYPE),
            ("refresh_token", refresh_token.as_str()),
            ("client_id", config.client_id.as_str()),
            ("scope", OAUTH_SCOPES),
        ])
        .send()
        .await?;

    parse_token_response(response, "token refresh")
        .await
        .map(Some)
}

fn auth_client_config() -> Result<OAuthClientConfig, CliError> {
    Ok(OAuthClientConfig {
        base_url: OAUTH_BASE_URL.to_string(),
        client_id: OAUTH_CLIENT_ID.to_string(),
    })
}

async fn request_device_code(
    client: &reqwest::Client,
    config: &OAuthClientConfig,
) -> Result<DeviceAuthorizationResponse, CliError> {
    let response = client
        .post(format!(
            "{}/oauth2/device_authorization",
            config.base_url.trim_end_matches('/')
        ))
        .json(&serde_json::json!({
            "client_id": config.client_id,
            "scope": OAUTH_SCOPES,
        }))
        .send()
        .await?;

    if response.status().is_success() {
        return Ok(response.json().await?);
    }

    Err(auth_http_error(response, "device authorization").await)
}

async fn poll_for_device_tokens(
    client: &reqwest::Client,
    config: &OAuthClientConfig,
    device: &DeviceAuthorizationResponse,
) -> Result<StoredAuth, CliError> {
    let deadline = Instant::now() + Duration::from_secs(device.expires_in);
    let mut interval_seconds = device.interval.unwrap_or(5).max(1);

    loop {
        if Instant::now() >= deadline {
            return Err(CliError::Message(
                "OAuth login timed out before authorization completed.".to_string(),
            ));
        }

        tokio::time::sleep(Duration::from_secs(interval_seconds)).await;

        let response = client
            .post(format!(
                "{}/oauth2/token",
                config.base_url.trim_end_matches('/')
            ))
            .form(&[
                ("grant_type", DEVICE_GRANT_TYPE),
                ("device_code", device.device_code.as_str()),
                ("client_id", config.client_id.as_str()),
                ("scope", OAUTH_SCOPES),
            ])
            .send()
            .await?;

        if response.status().is_success() {
            return parse_token_response(response, "device authentication").await;
        }

        if response.status() == StatusCode::BAD_REQUEST {
            let error = response.json::<OAuthErrorResponse>().await?;
            match error.error.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval_seconds += 5;
                    continue;
                }
                "access_denied" => {
                    return Err(CliError::Message(
                        error
                            .error_description
                            .unwrap_or_else(|| "OAuth login was denied.".to_string()),
                    ));
                }
                "expired_token" => {
                    return Err(CliError::Message(
                        "OAuth login expired before it was completed. Run `indices login` again."
                            .to_string(),
                    ));
                }
                _ => {
                    let message = error
                        .error_description
                        .unwrap_or_else(|| error.error.clone());
                    return Err(CliError::Message(format!(
                        "device authentication failed: {message}"
                    )));
                }
            }
        }

        return Err(auth_http_error(response, "device authentication").await);
    }
}

async fn parse_token_response(
    response: reqwest::Response,
    context: &str,
) -> Result<StoredAuth, CliError> {
    if response.status().is_success() {
        let token = response.json::<OAuthTokenResponse>().await?;
        let expires_at = Utc::now() + ChronoDuration::seconds(token.expires_in.max(0));
        return Ok(StoredAuth::OAuth {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_at,
        });
    }

    Err(auth_http_error(response, context).await)
}

async fn auth_http_error(response: reqwest::Response, context: &str) -> CliError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<OAuthErrorResponse>(&body)
        .ok()
        .and_then(|error| error.error_description.or(Some(error.error)))
        .unwrap_or_else(|| {
            if body.trim().is_empty() {
                format!("{context} failed with status {}", status.as_u16())
            } else {
                format!("{context} failed: {body}")
            }
        });

    CliError::Message(message)
}

fn print_login_instructions(device: &DeviceAuthorizationResponse) {
    println!("Sign in with your browser to complete login.");
    println!("URL: {}", device.verification_uri);
    println!("One-time code: {}", device.user_code);
}

fn open_browser(url: &str) -> bool {
    if cfg!(target_os = "macos") {
        return Command::new("open").arg(url).spawn().is_ok();
    }

    if cfg!(target_os = "windows") {
        return Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .is_ok();
    }

    Command::new("xdg-open").arg(url).spawn().is_ok()
}
