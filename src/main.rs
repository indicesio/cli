mod cli;
mod client;
mod commands;
mod config;
mod errors;
mod output;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::client::{ApiClient, ClientOptions};
use crate::config::{ConfigStore, RuntimeOverrides};
use crate::errors::CliError;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(error.exit_code());
    }
}

async fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    let mut config_store = ConfigStore::load()?;

    let overrides = RuntimeOverrides {
        api_base: cli.api_base.as_deref(),
        timeout_seconds: cli.timeout,
        output: cli.output,
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
    let api_key = runtime.api_key.clone().ok_or(CliError::NotAuthenticated)?;

    let client = ApiClient::new(ClientOptions {
        api_base: runtime.api_base.clone(),
        api_key,
        timeout_seconds: runtime.timeout_seconds,
    })?;

    let response = match &cli.command {
        Command::Whoami => commands::auth::whoami(&client).await?,
        Command::Tasks { command } => {
            commands::tasks::handle_tasks_command(&client, command).await?
        }
        Command::Runs { command } => commands::runs::handle_runs_command(&client, command).await?,
        Command::Secrets { command } => {
            commands::secrets::handle_secrets_command(&client, command).await?
        }
        Command::Login(_) | Command::Logout => {
            return Err(CliError::Message("unexpected command routing".to_string()));
        }
    };

    output::print_response(&response, runtime.output)?;
    Ok(())
}
