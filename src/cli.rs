use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::config::OutputMode;

#[derive(Debug, Parser)]
#[command(name = "indices")]
#[command(about = "Indices API CLI", long_about = None)]
pub struct Cli {
    #[arg(long, global = true, value_enum)]
    pub output: Option<OutputMode>,

    #[arg(long, global = true)]
    pub api_base: Option<String>,

    #[arg(long, global = true)]
    pub timeout: Option<u64>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Login(LoginArgs),
    Logout,
    Whoami,
    Tasks {
        #[command(subcommand)]
        command: TasksCommand,
    },
    Runs {
        #[command(subcommand)]
        command: RunsCommand,
    },
    Secrets {
        #[command(subcommand)]
        command: SecretsCommand,
    },
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    #[arg(long)]
    pub api_key: Option<String>,

    #[arg(long, default_value_t = false)]
    pub no_verify: bool,
}

#[derive(Debug, Subcommand)]
pub enum TasksCommand {
    Create(CreateTaskArgs),
    Get(TaskIdArgs),
    List(ListTasksArgs),
    Delete(DeleteTaskArgs),
    Retry(TaskIdArgs),
    RegenerateApi(TaskIdArgs),
}

#[derive(Debug, Args)]
pub struct CreatePayloadSourceArgs {
    #[arg(long, help = "Raw JSON payload")]
    pub body: Option<String>,

    #[arg(long, help = "Path to JSON payload file")]
    pub file: Option<PathBuf>,

    #[arg(long, default_value_t = false, help = "Read JSON payload from stdin")]
    pub stdin: bool,
}

#[derive(Debug, Args)]
pub struct CreateTaskArgs {
    #[command(flatten)]
    pub payload: CreatePayloadSourceArgs,

    #[arg(long, help = "Task title shown in dashboard")]
    pub display_name: Option<String>,

    #[arg(long, help = "Website URL for the task")]
    pub website: Option<String>,

    #[arg(long, help = "Detailed instructions for the task")]
    pub task: Option<String>,

    #[arg(long, help = "Input schema JSON string")]
    pub input_schema: Option<String>,

    #[arg(long, help = "Output schema JSON string")]
    pub output_schema: Option<String>,

    #[arg(long, help = "JSON object for creation_params")]
    pub creation_params: Option<String>,
}

#[derive(Debug, Args)]
pub struct TaskIdArgs {
    pub task_id: String,
}

#[derive(Debug, Args)]
pub struct DeleteTaskArgs {
    pub task_id: String,

    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct ListTasksArgs {
    #[arg(long)]
    pub status: Option<String>,

    #[arg(long)]
    pub limit: Option<u32>,

    #[arg(long)]
    pub cursor: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum RunsCommand {
    Create(CreateRunArgs),
    List(ListRunsArgs),
    Get(RunIdArgs),
}

#[derive(Debug, Args)]
pub struct CreateRunArgs {
    #[command(flatten)]
    pub payload: CreatePayloadSourceArgs,

    #[arg(long, help = "Task UUID to execute")]
    pub task_id: Option<String>,

    #[arg(long, help = "JSON object for run arguments")]
    pub arguments: Option<String>,

    #[arg(long, help = "JSON object mapping secret slots to UUIDs")]
    pub secret_bindings: Option<String>,
}

#[derive(Debug, Args)]
pub struct ListRunsArgs {
    #[arg(long)]
    pub task_id: String,

    #[arg(long)]
    pub limit: Option<u32>,

    #[arg(long)]
    pub cursor: Option<String>,
}

#[derive(Debug, Args)]
pub struct RunIdArgs {
    pub run_id: String,
}

#[derive(Debug, Subcommand)]
pub enum SecretsCommand {
    Create(CreateSecretArgs),
    List,
    Delete(DeleteSecretArgs),
}

#[derive(Debug, Args)]
pub struct CreateSecretArgs {
    pub name: String,

    #[arg(long)]
    pub value: Option<String>,

    #[arg(long, default_value_t = false)]
    pub stdin: bool,
}

#[derive(Debug, Args)]
pub struct DeleteSecretArgs {
    pub uuid: String,

    #[arg(long, default_value_t = false)]
    pub yes: bool,
}
