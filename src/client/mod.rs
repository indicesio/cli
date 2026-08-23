pub mod generated;

use std::num::NonZeroU64;
use std::str::FromStr;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use tracing::instrument;

use crate::telemetry;

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
    options: ClientOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityResponse {
    pub user_id: String,
    pub email: String,
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

        let headers = default_api_headers(&options)?;
        let http = build_http_client(&headers, options.timeout_seconds)?;

        Ok(Self {
            inner: generated::Client::new_with_client(&options.api_base, http),
            options,
        })
    }

    #[instrument(name = "cli.api.get_identity", skip_all, err)]
    pub async fn get_identity(&self) -> Result<IdentityResponse, ApiError> {
        let mut headers = HeaderMap::new();
        telemetry::inject_trace_context(&mut headers);

        let response = self
            .inner
            .client
            .get(format!("{}/v1beta/identity", self.inner.baseurl))
            .headers(headers)
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

        serde_json::from_slice::<IdentityResponse>(&bytes)
            .map_err(|error| ApiError::Serialization(format!("invalid response payload: {error}")))
    }

    #[instrument(name = "cli.api.list_connectors", skip_all, fields(limit, domain), err)]
    pub async fn list_connectors(
        &self,
        limit: Option<u32>,
        cursor: Option<&str>,
        domain: Option<&str>,
    ) -> Result<Value, ApiError> {
        let response = map_generated_result(
            self.inner
                .list_connectors(cursor, domain, parse_limit(limit)?)
                .await,
        )
        .await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.get_connector", skip_all, fields(connector_id), err)]
    pub async fn get_connector(&self, connector_id: &str) -> Result<Value, ApiError> {
        let response =
            map_generated_result(self.inner.retrieve_connector(connector_id).await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.rename_connector", skip_all, fields(connector_id), err)]
    pub async fn rename_connector(
        &self,
        connector_id: &str,
        display_name: &str,
    ) -> Result<Value, ApiError> {
        let request = serde_json::from_value::<generated::types::RenameConnectorRequest>(json!({
            "display_name": display_name,
        }))
        .map_err(|error| {
            ApiError::InvalidArgument(format!("invalid connector display name: {error}"))
        })?;

        let response =
            map_generated_result(self.inner.update_connector(connector_id, &request).await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.delete_connector", skip_all, fields(connector_id), err)]
    pub async fn delete_connector(&self, connector_id: &str) -> Result<Value, ApiError> {
        let response =
            map_generated_result(self.inner.delete_connector(connector_id).await).await?;

        to_json_value(response)
    }

    #[instrument(
        name = "cli.api.list_connector_revisions",
        skip_all,
        fields(connector_id),
        err
    )]
    pub async fn list_connector_revisions(&self, connector_id: &str) -> Result<Value, ApiError> {
        let response =
            map_generated_result(self.inner.list_connector_revisions(connector_id).await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.list_runs", skip_all, fields(connector_id, limit), err)]
    pub async fn list_runs(
        &self,
        connector_id: &str,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<Value, ApiError> {
        let response = map_generated_result(
            self.inner
                .list_runs(connector_id, cursor, parse_limit(limit)?)
                .await,
        )
        .await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.create_run", skip_all, err)]
    pub async fn create_run(&self, body: Value) -> Result<Value, ApiError> {
        let request = serde_json::from_value::<generated::types::CreateRunRequest>(body).map_err(
            |error| ApiError::InvalidArgument(format!("invalid create-run payload: {error}")),
        )?;

        // Sync runs can block until max_timeout_s. The shared client timeout is
        // typically much shorter, so this request uses a dedicated HTTP client.
        let request_timeout = if request.async_ {
            self.options.timeout_seconds
        } else {
            self.options
                .timeout_seconds
                .max(request.max_timeout_s.get().saturating_add(30))
        };
        let http = self.http_client_with_timeout(request_timeout)?;

        // Handle runs.create manually so undocumented 4xx responses and schema-drift
        // in error payloads still surface the backend's actual message.
        let mut trace_headers = HeaderMap::new();
        telemetry::inject_trace_context(&mut trace_headers);

        let response = http
            .post(format!("{}/v1beta/runs", self.inner.baseurl))
            .headers(trace_headers)
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

    #[instrument(name = "cli.api.get_run", skip_all, fields(run_id), err)]
    pub async fn get_run(&self, run_id: &str) -> Result<Value, ApiError> {
        let response = map_generated_result(self.inner.retrieve_run(run_id).await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.get_run_logs", skip_all, fields(run_id), err)]
    pub async fn get_run_logs(&self, run_id: &str) -> Result<Value, ApiError> {
        let response = map_generated_result(self.inner.get_run_logs(run_id).await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.list_files", skip_all, err)]
    #[allow(clippy::too_many_arguments)]
    pub async fn list_files(
        &self,
        run_id: Option<&str>,
        connector_id: Option<&str>,
        filename: Option<&str>,
        source: Option<&str>,
        sort: Option<&str>,
        order: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<Value, ApiError> {
        let source = parse_optional_enum::<generated::types::FileSource>(
            source,
            "--source",
            "UPLOAD, RUN_OUTPUT",
        )?;
        let sort = parse_optional_enum::<generated::types::FileListSort>(
            sort,
            "--sort",
            "name, created_at, size_bytes, source",
        )?;
        let order =
            parse_optional_enum::<generated::types::SortOrder>(order, "--order", "asc, desc")?;

        let response = map_generated_result(
            self.inner
                .list_files(
                    connector_id,
                    cursor,
                    filename,
                    parse_limit(limit)?,
                    order,
                    run_id,
                    sort,
                    source,
                )
                .await,
        )
        .await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.get_file", skip_all, fields(file_id), err)]
    pub async fn get_file(&self, file_id: &str) -> Result<Value, ApiError> {
        let response = map_generated_result(self.inner.retrieve_file(file_id).await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.upload_file", skip_all, fields(name, size_bytes), err)]
    pub async fn upload_file(
        &self,
        name: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<Value, ApiError> {
        let request =
            serde_json::from_value::<generated::types::InitiateFileUploadRequest>(json!({
                "name": name,
                "size_bytes": bytes.len() as u64,
                "content_type": content_type,
            }))
            .map_err(|error| ApiError::InvalidArgument(format!("invalid file upload: {error}")))?;

        let initiated =
            map_generated_result(self.inner.initiate_file_upload(&request).await).await?;

        let storage = storage_http_client()?;
        let mut put = storage.put(&initiated.upload_url).body(bytes);
        for (header_name, header_value) in &initiated.upload_headers {
            put = put.header(header_name.as_str(), header_value.as_str());
        }

        let upload_response = put.send().await?;
        let upload_status = upload_response.status();
        if !upload_status.is_success() {
            let upload_bytes = upload_response.bytes().await.unwrap_or_default();
            return Err(http_error_from_bytes(upload_status, &upload_bytes));
        }

        match self.finalize_file(&initiated.file_id).await {
            Ok(value) => Ok(value),
            Err(error) => Err(ApiError::InvalidRequest(format!(
                "uploaded bytes for `{}` but finalize failed: {error}",
                initiated.file_id
            ))),
        }
    }

    #[instrument(name = "cli.api.finalize_file", skip_all, fields(file_id), err)]
    pub async fn finalize_file(&self, file_id: &str) -> Result<Value, ApiError> {
        let response = map_generated_result(self.inner.complete_file_upload(file_id).await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.delete_file", skip_all, fields(file_id), err)]
    pub async fn delete_file(&self, file_id: &str) -> Result<Value, ApiError> {
        let response = map_generated_result(self.inner.delete_file(file_id).await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.get_file_download_url", skip_all, fields(file_id), err)]
    pub async fn get_file_download_url(&self, file_id: &str) -> Result<Value, ApiError> {
        let response =
            map_generated_result(self.inner.get_file_download_url(file_id).await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.download_file_bytes", skip_all, fields(file_id), err)]
    pub async fn download_file_bytes(&self, file_id: &str) -> Result<Vec<u8>, ApiError> {
        let download =
            map_generated_result(self.inner.get_file_download_url(file_id).await).await?;
        let storage = storage_http_client()?;
        let response = storage.get(&download.url).send().await?;
        let status = response.status();
        let bytes = response.bytes().await?;

        if !status.is_success() {
            return Err(http_error_from_bytes(status, &bytes));
        }

        Ok(bytes.to_vec())
    }

    #[instrument(name = "cli.api.list_capture_sessions", skip_all, err)]
    pub async fn list_capture_sessions(&self) -> Result<Value, ApiError> {
        let response = map_generated_result(self.inner.list_capture_sessions().await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.start_capture_session", skip_all, err)]
    pub async fn start_capture_session(
        &self,
        use_proxy: bool,
        cookies: Option<Value>,
    ) -> Result<Value, ApiError> {
        let cookies = if let Some(cookies) = cookies {
            serde_json::from_value::<Vec<generated::types::SessionCookie>>(cookies).map_err(
                |error| ApiError::InvalidArgument(format!("invalid --cookies payload: {error}")),
            )?
        } else {
            Vec::new()
        };

        let request = generated::types::StartCaptureSessionRequest { cookies, use_proxy };
        let response =
            map_generated_result(self.inner.start_capture_session(&request).await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.get_capture_session", skip_all, fields(id), err)]
    pub async fn get_capture_session(&self, capture_session_id: &str) -> Result<Value, ApiError> {
        let response = map_generated_result(
            self.inner
                .retrieve_capture_session(capture_session_id)
                .await,
        )
        .await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.complete_capture_session", skip_all, fields(id), err)]
    pub async fn complete_capture_session(
        &self,
        capture_session_id: &str,
    ) -> Result<Value, ApiError> {
        let response = map_generated_result(
            self.inner
                .complete_capture_session(capture_session_id)
                .await,
        )
        .await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.abandon_capture_session", skip_all, fields(id), err)]
    pub async fn abandon_capture_session(
        &self,
        capture_session_id: &str,
    ) -> Result<Value, ApiError> {
        let response =
            map_generated_result(self.inner.abandon_capture_session(capture_session_id).await)
                .await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.create_string_secret", skip_all, fields(name), err)]
    pub async fn create_string_secret(
        &self,
        name: &str,
        value: &str,
        website: Option<&str>,
    ) -> Result<Value, ApiError> {
        let request = generated::types::CreateSecretRequest {
            name: name.to_string(),
            password: None,
            secret_type: generated::types::SecretType::String,
            totp_secret: None,
            username: None,
            value: Some(value.to_string()),
            website: website.map(ToOwned::to_owned),
        };

        self.create_secret_request(request).await
    }

    #[instrument(name = "cli.api.create_login_secret", skip_all, fields(name), err)]
    pub async fn create_login_secret(
        &self,
        name: &str,
        username: &str,
        password: &str,
        totp_secret: Option<&str>,
        website: Option<&str>,
    ) -> Result<Value, ApiError> {
        let request = generated::types::CreateSecretRequest {
            name: name.to_string(),
            password: Some(password.to_string()),
            secret_type: generated::types::SecretType::Login,
            totp_secret: totp_secret.map(ToOwned::to_owned),
            username: Some(username.to_string()),
            value: None,
            website: website.map(ToOwned::to_owned),
        };

        self.create_secret_request(request).await
    }

    async fn create_secret_request(
        &self,
        request: generated::types::CreateSecretRequest,
    ) -> Result<Value, ApiError> {
        let response =
            map_generated_result(self.inner.create_secret_v1beta_secrets_post(&request).await)
                .await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.list_secrets", skip_all, err)]
    pub async fn list_secrets(&self) -> Result<Value, ApiError> {
        let response =
            map_generated_result(self.inner.list_user_secrets_v1beta_secrets_get().await).await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.delete_secret", skip_all, fields(id), err)]
    pub async fn delete_secret(&self, id: &str) -> Result<Value, ApiError> {
        let response = map_generated_result(
            self.inner
                .delete_user_secret_v1beta_secrets_id_delete(id)
                .await,
        )
        .await?;

        to_json_value(response)
    }

    #[instrument(name = "cli.api.generate_totp", skip_all, fields(id), err)]
    pub async fn generate_totp(&self, id: &str) -> Result<Value, ApiError> {
        let response = map_generated_result(
            self.inner
                .generate_totp_code_v1beta_secrets_id_totp_post(id)
                .await,
        )
        .await?;

        to_json_value(response)
    }

    fn http_client_with_timeout(&self, timeout_seconds: u64) -> Result<reqwest::Client, ApiError> {
        build_http_client(&default_api_headers(&self.options)?, timeout_seconds)
    }
}

fn to_json_value<T: Serialize>(value: T) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(|error| ApiError::Serialization(error.to_string()))
}

fn default_api_headers(options: &ClientOptions) -> Result<HeaderMap, ApiError> {
    let mut headers = HeaderMap::new();
    let mut auth_value = HeaderValue::from_str(&format!("Bearer {}", options.bearer_token))
        .map_err(|_| {
            ApiError::InvalidRequest("bearer token contains invalid header characters".to_string())
        })?;
    auth_value.set_sensitive(true);
    headers.insert(AUTHORIZATION, auth_value);
    headers.insert(
        HeaderName::from_static(REQUEST_SOURCE_HEADER),
        HeaderValue::from_static(REQUEST_SOURCE_CLI),
    );
    Ok(headers)
}

fn build_http_client(
    headers: &HeaderMap,
    timeout_seconds: u64,
) -> Result<reqwest::Client, ApiError> {
    Ok(reqwest::Client::builder()
        .default_headers(headers.clone())
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?)
}

fn storage_http_client() -> Result<reqwest::Client, ApiError> {
    // Signed storage URLs must be fetched without the Indices API key.
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(3600))
        .build()?)
}

fn parse_limit(limit: Option<u32>) -> Result<Option<NonZeroU64>, ApiError> {
    match limit {
        None => Ok(None),
        Some(0) => Err(ApiError::InvalidArgument(
            "`--limit` must be at least 1".to_string(),
        )),
        Some(value) => Ok(NonZeroU64::new(u64::from(value))),
    }
}

fn parse_optional_enum<T: FromStr>(
    value: Option<&str>,
    flag: &str,
    expected: &str,
) -> Result<Option<T>, ApiError> {
    match value {
        None => Ok(None),
        Some(raw) => raw.parse::<T>().map(Some).map_err(|_| {
            ApiError::InvalidArgument(format!(
                "invalid {flag} `{raw}`; expected one of: {expected}"
            ))
        }),
    }
}

/// Unwraps a generated-client result, mapping any error through
/// [`map_generated_error`] so undocumented status codes still surface a useful
/// message instead of a bare "unexpected API response".
async fn map_generated_result<T, E>(
    result: Result<generated::ResponseValue<T>, generated::Error<E>>,
) -> Result<T, ApiError>
where
    E: Serialize,
{
    match result {
        Ok(response) => Ok(response.into_inner()),
        Err(error) => Err(map_generated_error(error).await),
    }
}

async fn map_generated_error<E: Serialize>(error: generated::Error<E>) -> ApiError {
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
        // Progenitor only matches the status codes documented in the OpenAPI spec
        // (typically 200/202 and 422); everything else — 404, 401/403, 409, 429,
        // 5xx — lands here. Read the body and route it through the same path as the
        // hand-rolled callers so the backend's `detail` message reaches the user.
        generated::Error::UnexpectedResponse(response) => {
            let status = response.status();
            let bytes = response.bytes().await.unwrap_or_default();
            http_error_from_bytes(status, &bytes)
        }
    }
}

fn http_error_from_bytes(status: StatusCode, bytes: &[u8]) -> ApiError {
    let body = String::from_utf8_lossy(bytes).trim().to_string();
    let message = summarize_error_payload(&body).unwrap_or_else(|| {
        if body.is_empty() {
            status_fallback_message(status)
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

/// Human-readable fallback for when the backend returns an error status with no
/// usable body (e.g. an empty 404), so the message is still descriptive.
fn status_fallback_message(status: StatusCode) -> String {
    let reason = match status {
        StatusCode::NOT_FOUND => "not found",
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => "not authorized",
        StatusCode::CONFLICT => "conflict",
        StatusCode::TOO_MANY_REQUESTS => "rate limited",
        _ if status.is_server_error() => "server error",
        _ if status.is_client_error() => "request failed",
        _ => "unexpected API response",
    };

    reason.to_string()
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
    fn http_error_from_bytes_surfaces_not_found_detail() {
        // Mirrors `indices runs logs <id>` 404: previously this came back as the
        // generic "unexpected API response"; now the backend's `detail` survives.
        let error = http_error_from_bytes(StatusCode::NOT_FOUND, br#"{"detail":"Run not found"}"#);

        match error {
            ApiError::HttpStatus {
                status, message, ..
            } => {
                assert_eq!(status, 404);
                assert_eq!(message, "Run not found");
            }
            other => panic!("expected http status error, got {other:?}"),
        }
    }

    #[test]
    fn http_error_from_bytes_falls_back_to_status_for_empty_body() {
        let error = http_error_from_bytes(StatusCode::NOT_FOUND, b"");

        match error {
            ApiError::HttpStatus {
                status,
                message,
                body,
            } => {
                assert_eq!(status, 404);
                assert_eq!(message, "not found");
                assert!(body.is_none());
            }
            other => panic!("expected http status error, got {other:?}"),
        }
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
