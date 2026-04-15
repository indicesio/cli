pub mod generated;

use std::str::FromStr;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

const REQUEST_SOURCE_HEADER: &str = "x-indices-request-source";
const REQUEST_SOURCE_CLI: &str = "cli";

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub api_base: String,
    pub bearer_token: String,
    pub timeout_seconds: u64,
}

#[derive(Debug)]
pub struct ApiClient {
    inner: generated::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityResponse {
    pub user_id: Value,
    pub email: Option<String>,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("invalid API base URL: {0}")]
    InvalidBaseUrl(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("failed to serialize or parse response: {0}")]
    Serialization(String),
    #[error("API error ({status}): {message}")]
    HttpStatus {
        status: u16,
        message: String,
        body: Option<String>,
    },
}

impl ApiError {
    pub fn is_unauthorized(&self) -> bool {
        matches!(
            self,
            ApiError::HttpStatus {
                status: 401 | 403,
                ..
            }
        )
    }

    pub fn is_timeout_or_network(&self) -> bool {
        match self {
            ApiError::Transport(err) => err.is_timeout() || err.is_connect() || err.is_request(),
            _ => false,
        }
    }
}

impl ApiClient {
    pub fn new(options: ClientOptions) -> Result<Self, ApiError> {
        let _ = reqwest::Url::parse(&options.api_base)
            .map_err(|_| ApiError::InvalidBaseUrl(options.api_base.clone()))?;

        let mut headers = HeaderMap::new();
        let mut auth_value = HeaderValue::from_str(&format!("Bearer {}", options.bearer_token))
            .map_err(|_| {
                ApiError::InvalidRequest(
                    "bearer token contains invalid header characters".to_string(),
                )
            })?;
        auth_value.set_sensitive(true);
        headers.insert(AUTHORIZATION, auth_value);
        headers.insert(
            HeaderName::from_static(REQUEST_SOURCE_HEADER),
            HeaderValue::from_static(REQUEST_SOURCE_CLI),
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(options.timeout_seconds))
            .build()?;

        Ok(Self {
            inner: generated::Client::new_with_client(&options.api_base, http),
        })
    }

    pub async fn get_identity(&self) -> Result<IdentityResponse, ApiError> {
        let response = self
            .inner
            .client
            .get(format!("{}/v1beta/me", self.inner.baseurl))
            .header(ACCEPT, "application/json")
            .header(
                "api-version",
                <generated::Client as progenitor_client::ClientInfo<()>>::api_version(),
            )
            .send()
            .await?;

        let status = response.status();
        let bytes = response.bytes().await?;

        if !status.is_success() {
            return Err(http_error_from_bytes(status, &bytes));
        }

        serde_json::from_slice::<IdentityResponse>(&bytes).map_err(|error| {
            ApiError::Serialization(format!("invalid response payload: {error}"))
        })
    }

    pub async fn create_task(&self, body: Value) -> Result<Value, ApiError> {
        let request = serde_json::from_value::<generated::types::CreateTaskRequest>(body).map_err(
            |error| ApiError::InvalidArgument(format!("invalid create-task payload: {error}")),
        )?;

        let response = self
            .inner
            .create_task(&request)
            .await
            .map_err(map_generated_error)?
            .into_inner();

        to_json_value(response)
    }

    pub async fn get_task(&self, task_id: &str) -> Result<Value, ApiError> {
        let task_id = parse_uuid(task_id, "task_id")?;
        let response = self
            .inner
            .retrieve_task(&task_id)
            .await
            .map_err(map_generated_error)?
            .into_inner();

        to_json_value(response)
    }

    pub async fn list_tasks(
        &self,
        status: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<Value, ApiError> {
        if cursor.is_some() {
            return Err(ApiError::InvalidArgument(
                "`--cursor` is not supported by the current Tasks API".to_string(),
            ));
        }

        let mut tasks = self
            .inner
            .list_tasks()
            .await
            .map_err(map_generated_error)?
            .into_inner();

        if let Some(status) = status {
            let desired = generated::types::TaskState::from_str(status).map_err(|_| {
                ApiError::InvalidArgument(format!(
                    "invalid --status `{status}`; expected one of: not_ready, waiting_for_manual_completion, ready, failed"
                ))
            })?;

            tasks.retain(|task| task.current_state == desired);
        }

        if let Some(limit) = limit {
            tasks.truncate(limit as usize);
        }

        to_json_value(tasks)
    }

    pub async fn delete_task(&self, task_id: &str) -> Result<Value, ApiError> {
        let task_id = parse_uuid(task_id, "task_id")?;
        let response = self
            .inner
            .delete_task(&task_id)
            .await
            .map_err(map_generated_error)?
            .into_inner();

        to_json_value(response)
    }

    pub async fn retry_task(&self, task_id: &str) -> Result<Value, ApiError> {
        let task_id = parse_uuid(task_id, "task_id")?;
        let response = self
            .inner
            .retry_task(&task_id)
            .await
            .map_err(map_generated_error)?
            .into_inner();

        to_json_value(response)
    }

    pub async fn regenerate_task_api(&self, task_id: &str) -> Result<Value, ApiError> {
        let task_id = parse_uuid(task_id, "task_id")?;
        let response = self
            .inner
            .regenerate_task(&task_id)
            .await
            .map_err(map_generated_error)?
            .into_inner();

        to_json_value(response)
    }

    pub async fn list_runs(
        &self,
        task_id: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<Value, ApiError> {
        if cursor.is_some() {
            return Err(ApiError::InvalidArgument(
                "`--cursor` is not supported by the current Runs API".to_string(),
            ));
        }

        let task_id = task_id.ok_or_else(|| {
            ApiError::InvalidArgument(
                "`runs list` requires `--task-id` in the current API".to_string(),
            )
        })?;

        let task_uuid = parse_uuid(task_id, "task_id")?;

        let mut runs = self
            .inner
            .list_task_runs(&task_uuid)
            .await
            .map_err(map_generated_error)?
            .into_inner();

        if let Some(limit) = limit {
            runs.truncate(limit as usize);
        }

        to_json_value(runs)
    }

    pub async fn create_run(&self, body: Value) -> Result<Value, ApiError> {
        let request = serde_json::from_value::<generated::types::CreateRunRequest>(body).map_err(
            |error| ApiError::InvalidArgument(format!("invalid create-run payload: {error}")),
        )?;

        // Handle runs.create manually so undocumented 4xx responses and schema-drift
        // in error payloads still surface the backend's actual message.
        let response = self
            .inner
            .client
            .post(format!("{}/v1beta/runs", self.inner.baseurl))
            .header(reqwest::header::ACCEPT, "application/json")
            .header(
                "api-version",
                <generated::Client as progenitor_client::ClientInfo<()>>::api_version(),
            )
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let bytes = response.bytes().await?;

        if !status.is_success() {
            return Err(http_error_from_bytes(status, &bytes));
        }

        let run = serde_json::from_slice::<generated::types::Run>(&bytes).map_err(|error| {
            ApiError::Serialization(format!("invalid response payload: {error}"))
        })?;

        to_json_value(run)
    }

    pub async fn get_run(&self, run_id: &str) -> Result<Value, ApiError> {
        let run_id = parse_uuid(run_id, "run_id")?;
        let response = self
            .inner
            .retrieve_run(&run_id)
            .await
            .map_err(map_generated_error)?
            .into_inner();

        to_json_value(response)
    }

    pub async fn get_run_logs(&self, run_id: &str) -> Result<Value, ApiError> {
        let run_id = parse_uuid(run_id, "run_id")?;
        let response = self
            .inner
            .get_run_logs(&run_id)
            .await
            .map_err(map_generated_error)?
            .into_inner();

        to_json_value(response)
    }

    pub async fn create_secret(&self, name: &str, value: &str) -> Result<Value, ApiError> {
        let request = generated::types::CreateSecretRequest {
            name: name.to_string(),
            password: None,
            secret_type: generated::types::SecretType::String,
            totp_secret: None,
            username: None,
            value: Some(value.to_string()),
            website: None,
        };

        let response = self
            .inner
            .create_secret_v1beta_secrets_post(&request)
            .await
            .map_err(map_generated_error)?
            .into_inner();

        to_json_value(response)
    }

    pub async fn list_secrets(&self) -> Result<Value, ApiError> {
        let response = self
            .inner
            .list_user_secrets_v1beta_secrets_get()
            .await
            .map_err(map_generated_error)?
            .into_inner();

        to_json_value(response)
    }

    pub async fn delete_secret(&self, uuid: &str) -> Result<Value, ApiError> {
        let uuid = parse_uuid(uuid, "uuid")?;
        let response = self
            .inner
            .delete_user_secret_v1beta_secrets_uuid_delete(&uuid)
            .await
            .map_err(map_generated_error)?
            .into_inner();

        to_json_value(response)
    }
}

fn to_json_value<T: Serialize>(value: T) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|error| ApiError::Serialization(error.to_string()))
}

fn parse_uuid(raw: &str, field: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw).map_err(|error| {
        ApiError::InvalidArgument(format!("invalid {field} UUID `{raw}`: {error}"))
    })
}

fn map_generated_error<E: Serialize>(error: generated::Error<E>) -> ApiError {
    match error {
        generated::Error::InvalidRequest(message) | generated::Error::Custom(message) => {
            ApiError::InvalidRequest(message)
        }
        generated::Error::CommunicationError(error)
        | generated::Error::InvalidUpgrade(error)
        | generated::Error::ResponseBodyError(error) => ApiError::Transport(error),
        generated::Error::ErrorResponse(response) => {
            let status = response.status().as_u16();
            let body = serde_json::to_string(&response.into_inner()).ok();
            let message = body
                .as_deref()
                .and_then(summarize_error_payload)
                .unwrap_or_else(|| body.clone().unwrap_or_else(|| "request failed".to_string()));

            ApiError::HttpStatus {
                status,
                message,
                body,
            }
        }
        generated::Error::InvalidResponsePayload(bytes, error) => {
            let payload = String::from_utf8_lossy(&bytes).trim().to_string();
            let detail = if payload.is_empty() {
                String::new()
            } else {
                format!("; response body: {payload}")
            };

            ApiError::Serialization(format!("invalid response payload: {error}{detail}"))
        }
        generated::Error::UnexpectedResponse(response) => ApiError::HttpStatus {
            status: response.status().as_u16(),
            message: "unexpected API response".to_string(),
            body: None,
        },
    }
}

fn http_error_from_bytes(status: StatusCode, bytes: &[u8]) -> ApiError {
    let body = String::from_utf8_lossy(bytes).trim().to_string();
    let message = summarize_error_payload(&body).unwrap_or_else(|| {
        if body.is_empty() {
            "request failed".to_string()
        } else {
            body.clone()
        }
    });

    ApiError::HttpStatus {
        status: status.as_u16(),
        message,
        body: if body.is_empty() { None } else { Some(body) },
    }
}

fn summarize_error_payload(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let value = serde_json::from_str::<Value>(trimmed).ok()?;

    match value.get("detail") {
        Some(Value::String(detail)) => Some(detail.clone()),
        Some(Value::Object(detail)) => {
            let error = detail.get("error").and_then(Value::as_str);
            let details = detail.get("details").and_then(Value::as_str);

            match (error, details) {
                (Some(error), Some(details)) => Some(format!("{error} {details}")),
                (Some(error), None) => Some(error.to_string()),
                (None, Some(details)) => Some(details.to_string()),
                _ => serde_json::to_string(detail).ok(),
            }
        }
        Some(Value::Array(items)) => {
            let messages: Vec<String> = items
                .iter()
                .filter_map(|item| {
                    item.get("msg")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
                .collect();

            if messages.is_empty() {
                serde_json::to_string(items).ok()
            } else {
                Some(messages.join("; "))
            }
        }
        Some(other) => serde_json::to_string(other).ok(),
        None => serde_json::to_string(&value).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiError, http_error_from_bytes, summarize_error_payload};
    use reqwest::StatusCode;

    #[test]
    fn summarize_error_payload_prefers_detail_string() {
        let raw = r#"{"detail":"invalid api key"}"#;
        assert_eq!(
            summarize_error_payload(raw).as_deref(),
            Some("invalid api key")
        );
    }

    #[test]
    fn summarize_error_payload_combines_detail_object_fields() {
        let raw = r#"{"detail":{"error":"forbidden","details":"workspace access required"}}"#;
        assert_eq!(
            summarize_error_payload(raw).as_deref(),
            Some("forbidden workspace access required")
        );
    }

    #[test]
    fn http_error_from_bytes_preserves_backend_message() {
        let error = http_error_from_bytes(
            StatusCode::FORBIDDEN,
            br#"{"detail":"api key missing required scope"}"#,
        );

        match error {
            ApiError::HttpStatus {
                status,
                message,
                body,
            } => {
                assert_eq!(status, 403);
                assert_eq!(message, "api key missing required scope");
                assert_eq!(
                    body.as_deref(),
                    Some(r#"{"detail":"api key missing required scope"}"#)
                );
            }
            other => panic!("expected http status error, got {other:?}"),
        }
    }
}
