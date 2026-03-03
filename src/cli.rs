use std::path::PathBuf;

use clap::builder::styling::{AnsiColor, Effects};
use clap::builder::Styles;
use clap::{Args, Parser, Subcommand};

use crate::config::OutputMode;

const RUNS_CREATE_AFTER_HELP: &str = "\
\x1b[1;97mModes:\x1b[0m

Parameters can be supplied in one of three different ways:

  Command args: pass `--task-id` and optionally `--arguments` / `--secret-bindings`
  Raw JSON as parameter: pass one of `--body`, `--file`, `--stdin` (do not mix with argument mode)
  Piped JSON: if no mode flags are provided and stdin has data, JSON is read from stdin

\x1b[1;97mExamples:\x1b[0m
  indices runs create --task-id 11111111-1111-1111-1111-111111111111
  indices runs create --task-id 11111111-1111-1111-1111-111111111111 --arguments '{\"job_id\":\"A1\"}'
  indices runs create --task-id 11111111-1111-1111-1111-111111111111 --arguments '{\"job_id\":\"A1\"}' --secret-bindings '{\"GOOGLE_LOGIN\":\"22222222-2222-2222-2222-222222222222\"}'
  indices runs create --file run-payload.json
  cat run-payload.json | indices runs create";

fn cli_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::BrightWhite.on_default().effects(Effects::BOLD))
        .usage(AnsiColor::BrightWhite.on_default().effects(Effects::BOLD))
        .literal(AnsiColor::BrightCyan.on_default())
        .placeholder(AnsiColor::BrightYellow.on_default())
}

#[derive(Debug, Parser)]
#[command(name = "indices")]
#[command(about = "Indices API CLI", long_about = None, styles = cli_styles())]
pub struct Cli {
    #[arg(
        long,
        global = true,
        value_enum,
        help = "Output format",
        help_heading = "Global Options"
    )]
    pub output: Option<OutputMode>,

    #[arg(
        long,
        global = true,
        help = "Override API base URL",
        help_heading = "Global Options"
    )]
    pub api_base: Option<String>,

    #[arg(
        long,
        global = true,
        help = "HTTP timeout in seconds",
        help_heading = "Global Options"
    )]
    pub timeout: Option<u64>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Authenticate and store API credentials")]
    Login(LoginArgs),
    #[command(about = "Remove stored API credentials")]
    Logout,
    #[command(about = "Verify current authentication")]
    AuthTest,
    #[command(about = "Manage tasks")]
    Tasks {
        #[command(subcommand)]
        command: TasksCommand,
    },
    #[command(about = "Run tasks and inspect previous runs")]
    Runs {
        #[command(subcommand)]
        command: RunsCommand,
    },
    #[command(about = "Manage secrets")]
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
    #[command(about = "Create a task")]
    Create(CreateTaskArgs),
    #[command(about = "Get a task by ID")]
    Get(TaskIdArgs),
    #[command(about = "List tasks")]
    List(ListTasksArgs),
    #[command(about = "Delete a task")]
    Delete(DeleteTaskArgs),
    #[command(about = "Retry a task")]
    Retry(TaskIdArgs),
    #[command(about = "Regenerate connector logic for a task")]
    RegenerateApi(TaskIdArgs),
}

#[derive(Debug, Args)]
pub struct CreatePayloadSourceArgs {
    #[arg(
        long,
        help = "Raw JSON payload string",
        help_heading = "Payload Sources"
    )]
    pub body: Option<String>,

    #[arg(
        long,
        help = "Path to a JSON payload file",
        help_heading = "Payload Sources"
    )]
    pub file: Option<PathBuf>,

    #[arg(
        long,
        default_value_t = false,
        help = "Read JSON payload from stdin",
        help_heading = "Payload Sources"
    )]
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
    #[command(
        about = "Create a run for a task",
        long_about = None,
        after_help = RUNS_CREATE_AFTER_HELP
    )]
    Create(CreateRunArgs),
    #[command(about = "List runs for a task")]
    List(ListRunsArgs),
    #[command(about = "Get a run by ID")]
    Get(RunIdArgs),
}

#[derive(Debug, Args)]
pub struct CreateRunArgs {
    #[command(flatten)]
    pub payload: CreatePayloadSourceArgs,

    #[arg(
        long,
        value_name = "TASK_ID",
        help = "Task UUID to execute (required in argument mode)",
        help_heading = "Argument Mode"
    )]
    pub task_id: Option<String>,

    #[arg(
        long,
        value_name = "JSON",
        help = "JSON object for run arguments",
        help_heading = "Argument Mode"
    )]
    pub arguments: Option<String>,

    #[arg(
        long,
        value_name = "JSON",
        help = "JSON object mapping secret slots to secret UUIDs",
        help_heading = "Argument Mode"
    )]
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
