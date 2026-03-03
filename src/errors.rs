use std::io;

use thiserror::Error;

use crate::client::ApiError;
use crate::config::ConfigError;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Clap(#[from] clap::Error),
    #[error("not authenticated. Run `indices login` first")]
    NotAuthenticated,
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Message(_) => 2,
            CliError::Clap(error) => error.exit_code(),
            CliError::NotAuthenticated => 3,
            CliError::Config(_) => 2,
            CliError::Api(api_error) if api_error.is_unauthorized() => 3,
            CliError::Api(api_error) if api_error.is_timeout_or_network() => 4,
            CliError::Api(_) => 5,
            CliError::Io(_) | CliError::Json(_) => 5,
        }
    }
}
