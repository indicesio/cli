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
    Http(#[from] reqwest::Error),
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
            CliError::Http(error)
                if error.is_timeout() || error.is_connect() || error.is_request() =>
            {
                4
            }
            CliError::Http(_) => 5,
            CliError::Io(_) | CliError::Json(_) => 5,
        }
    }

    /// Clap help and usage output is a normal CLI interaction, not a system
    /// failure. `#[instrument(err)]` would otherwise mark these as OTel ERROR
    /// spans and page on-call for commands like `indices` or `indices --help`.
    pub fn should_record_as_error(&self) -> bool {
        !matches!(self, CliError::Clap(_))
    }
}

/// Record unexpected failures on the current span without treating clap
/// help/usage as an OpenTelemetry ERROR.
pub fn record_cli_outcome(result: &Result<(), CliError>) {
    if let Err(error) = result {
        if error.should_record_as_error() {
            tracing::error!(error = %error);
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::Cli;

    fn parse_error(args: &[&str]) -> CliError {
        CliError::from(Cli::try_parse_from(args).expect_err("expected a clap usage or help error"))
    }

    #[test]
    fn help_and_missing_subcommand_are_not_otel_errors() {
        let cases: &[&[&str]] = &[
            &["indices"],
            &["indices", "--help"],
            &["indices", "-h"],
            &["indices", "runs"],
            &["indices", "runs", "--help"],
            &["indices", "runs", "create", "--help"],
            &["indices", "connectors"],
            &["indices", "connectors", "--help"],
            &["indices", "files"],
            &["indices", "captures"],
            &["indices", "secrets"],
        ];

        for args in cases {
            let error = parse_error(args);
            assert!(
                !error.should_record_as_error(),
                "{args:?} should not record an OTel ERROR span, got {error}"
            );
        }
    }

    #[test]
    fn unknown_command_is_not_otel_error() {
        let error = parse_error(&["indices", "not-a-command"]);
        assert!(
            !error.should_record_as_error(),
            "unknown commands are clap usage errors, not system failures"
        );
    }

    #[test]
    fn system_failures_are_otel_errors() {
        assert!(CliError::NotAuthenticated.should_record_as_error());
        assert!(CliError::Message("boom".into()).should_record_as_error());
        assert!(
            CliError::Api(ApiError::InvalidArgument("bad payload".into())).should_record_as_error()
        );
    }
}
