mod analytics;
mod cli;
mod client;
mod commands;
mod config;
mod errors;
mod oauth;
mod output;
mod telemetry;

use clap::Parser;
use serde_json::Value;
use tracing::instrument;

use crate::analytics::Analytics;
use crate::cli::{Cli, Command};
use crate::client::{ApiClient, ClientOptions};
use crate::commands::auth::WhoamiOutput;
use crate::config::{ConfigStore, OutputMode, RuntimeConfig, RuntimeOverrides, StoredSession};
use crate::errors::CliError;

enum CommandResponse {
    Value(Value),
    Whoami(WhoamiOutput),
}

#[tokio::main]
async fn main() {
    let _telemetry = telemetry::init();
    let result = run().await;
    // Force a flush of telemetry traces before we return
    drop(_telemetry);

    match result {
        Ok(()) => {}
        Err(CliError::Clap(error)) => {
            let code = error.exit_code();
            let _ = error.print();
            std::process::exit(code);
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(error.exit_code());
        }
    }
}

#[instrument(name = "cli.run", skip_all, err)]
async fn run() -> Result<(), CliError> {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() == 2 && argv[1] == "--version" {
        println!("Indices CLI v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let cli = Cli::try_parse_from(argv.clone())?;
    let analytics = Analytics::new().await;
    let mut telemetry = analytics.build_context(&cli, &argv);

    let mut config_store = ConfigStore::load()?;
    let overrides = RuntimeOverrides {
        api_base: cli.api_base.as_deref(),
        timeout_seconds: cli.timeout,
    };

    let result = execute_command(
        &analytics,
        &mut telemetry,
        &mut config_store,
        &overrides,
        &cli,
    )
    .await;
    analytics
        .capture_command_end(&telemetry, result.is_ok(), exit_code_for_result(&result))
        .await;
    result
}

#[instrument(name = "cli.command", skip_all, err)]
async fn execute_command(
    analytics: &Analytics,
    telemetry: &mut analytics::CommandTelemetryContext,
    config_store: &mut ConfigStore,
    overrides: &RuntimeOverrides<'_>,
    cli: &Cli,
) -> Result<(), CliError> {
    match &cli.command {
        Command::Login(args) => {
            let runtime = config_store.resolve_runtime(overrides)?;
            analytics.capture_command_start(telemetry).await;
            commands::auth::login(config_store, runtime, args).await?;
            Ok(())
        }
        Command::Logout => {
            analytics.capture_command_start(telemetry).await;
            commands::auth::logout(config_store)?;
            Ok(())
        }
        _ => {
            let runtime = config_store.resolve_runtime(overrides)?;
            let mut session = runtime.auth.clone().ok_or(CliError::NotAuthenticated)?;
            refresh_auth_if_needed(config_store, &runtime, &mut session, false).await?;

            let mut client = build_client(&runtime, &session)?;
            analytics
                .identify_authenticated_user(telemetry, &session)
                .await;
            telemetry.is_authenticated = true;
            analytics.capture_command_start(telemetry).await;

            let response = match execute_authenticated_command(&cli.command, &client).await {
                Ok(response) => response,
                Err(CliError::Api(api_error))
                    if api_error.is_unauthorized() && session.is_oauth() =>
                {
                    // TODO: Cleaner / recursive way of expressing this?
                    // as we're just repeating the above.
                    refresh_auth_if_needed(config_store, &runtime, &mut session, true).await?;
                    client = build_client(&runtime, &session)?;
                    analytics
                        .identify_authenticated_user(telemetry, &session)
                        .await;
                    execute_authenticated_command(&cli.command, &client).await?
                }
                Err(e) => return Err(e),
            };

            let output_mode = if cli.json {
                OutputMode::Json
            } else {
                OutputMode::Markdown
            };
            match response {
                CommandResponse::Value(value) => output::print_response(&value, output_mode)?,
                CommandResponse::Whoami(output) => output::print_whoami(&output, output_mode)?,
            }
            Ok(())
        }
    }
}

fn build_client(runtime: &RuntimeConfig, session: &StoredSession) -> Result<ApiClient, CliError> {
    Ok(ApiClient::new(ClientOptions {
        api_base: runtime.api_base.clone(),
        bearer_token: session.bearer_token().to_string(),
        timeout_seconds: runtime.timeout_seconds,
    })?)
}

#[instrument(name = "cli.refresh_auth", skip_all, fields(force), err)]
async fn refresh_auth_if_needed(
    config_store: &mut ConfigStore,
    runtime: &RuntimeConfig,
    session: &mut StoredSession,
    force: bool,
) -> Result<(), CliError> {
    if let Some(refreshed_auth) =
        oauth::refresh_auth(&session.auth, runtime.timeout_seconds, force).await?
    {
        let refreshed_session = StoredSession {
            auth: refreshed_auth,
            identity: session.cached_identity().cloned(),
        };
        config_store.set_session(
            refreshed_session.clone(),
            Some(runtime.api_base.as_str()),
            Some(runtime.timeout_seconds),
        )?;
        *session = refreshed_session;
    }

    Ok(())
}

async fn execute_authenticated_command(
    command: &Command,
    client: &ApiClient,
) -> Result<CommandResponse, CliError> {
    match command {
        Command::Whoami => Ok(CommandResponse::Whoami(
            commands::auth::whoami(client).await?,
        )),
        Command::Tasks { command } => Ok(CommandResponse::Value(
            commands::tasks::handle_tasks_command(client, command).await?,
        )),
        Command::Runs { command } => Ok(CommandResponse::Value(
            commands::runs::handle_runs_command(client, command).await?,
        )),
        Command::Secrets { command } => Ok(CommandResponse::Value(
            commands::secrets::handle_secrets_command(client, command).await?,
        )),
        Command::Login(_) | Command::Logout => {
            Err(CliError::Message("unexpected command routing".to_string()))
        }
    }
}

fn exit_code_for_result(result: &Result<(), CliError>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => error.exit_code(),
    }
}
