mod cli;
mod client;
mod commands;
mod config;
mod errors;
mod oauth;
mod output;

use clap::Parser;
use serde_json::Value;

use crate::cli::{Cli, Command};
use crate::client::{ApiClient, ClientOptions};
use crate::config::{ConfigStore, OutputMode, RuntimeConfig, RuntimeOverrides, StoredAuth};
use crate::errors::CliError;

#[tokio::main]
async fn main() {
    match run().await {
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

async fn run() -> Result<(), CliError> {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() == 2 && argv[1] == "--version" {
        println!("Indices CLI v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let cli = Cli::parse();

    let mut config_store = ConfigStore::load()?;

    let overrides = RuntimeOverrides {
        api_base: cli.api_base.as_deref(),
        timeout_seconds: cli.timeout,
    };

    match &cli.command {
        Command::Login(args) => {
            let runtime = config_store.resolve_runtime(&overrides)?;
            commands::auth::login(&mut config_store, runtime, args).await?;
            return Ok(());
        }
        Command::Logout => {
            commands::auth::logout(&mut config_store)?;
            return Ok(());
        }
        _ => {}
    }

    let runtime = config_store.resolve_runtime(&overrides)?;
    let mut auth = runtime.auth.clone().ok_or(CliError::NotAuthenticated)?;
    refresh_auth_if_needed(&mut config_store, &runtime, &mut auth, false).await?;

    let mut client = build_client(&runtime, &auth)?;
    let response = match execute_authenticated_command(&cli.command, &client).await {
        Ok(response) => response,
        Err(CliError::Api(api_error)) if api_error.is_unauthorized() && auth.is_oauth() => {
            refresh_auth_if_needed(&mut config_store, &runtime, &mut auth, true).await?;
            client = build_client(&runtime, &auth)?;
            execute_authenticated_command(&cli.command, &client).await?
        }
        Err(error) => return Err(error),
    };

    let output_mode = if cli.json {
        OutputMode::Json
    } else {
        OutputMode::Markdown
    };
    output::print_response(&response, output_mode)?;
    Ok(())
}

fn build_client(runtime: &RuntimeConfig, auth: &StoredAuth) -> Result<ApiClient, CliError> {
    Ok(ApiClient::new(ClientOptions {
        api_base: runtime.api_base.clone(),
        bearer_token: auth.bearer_token().to_string(),
        timeout_seconds: runtime.timeout_seconds,
    })?)
}

async fn refresh_auth_if_needed(
    config_store: &mut ConfigStore,
    runtime: &RuntimeConfig,
    auth: &mut StoredAuth,
    force: bool,
) -> Result<(), CliError> {
    if let Some(refreshed) = oauth::refresh_auth(auth, runtime.timeout_seconds, force).await? {
        config_store.set_auth(
            refreshed.clone(),
            Some(runtime.api_base.as_str()),
            Some(runtime.timeout_seconds),
        )?;
        *auth = refreshed;
    }

    Ok(())
}

async fn execute_authenticated_command(
    command: &Command,
    client: &ApiClient,
) -> Result<Value, CliError> {
    match command {
        Command::AuthTest => commands::auth::auth_test(client).await,
        Command::Tasks { command } => commands::tasks::handle_tasks_command(client, command).await,
        Command::Runs { command } => commands::runs::handle_runs_command(client, command).await,
        Command::Secrets { command } => {
            commands::secrets::handle_secrets_command(client, command).await
        }
        Command::Login(_) | Command::Logout => {
            Err(CliError::Message("unexpected command routing".to_string()))
        }
    }
}
