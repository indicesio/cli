use std::path::PathBuf;

use clap::builder::Styles;
use clap::builder::styling::{AnsiColor, Effects};
use clap::{Args, Parser, Subcommand};

pub const TASKS_REMOVED_MESSAGE: &str = "\
The `tasks` command has been removed. Indices now uses connectors.

  indices tasks list          →  indices connectors list
  indices tasks get <id>      →  indices connectors get <id>
  indices tasks delete <id>   →  indices connectors delete <id>
  indices runs create --task-id <id>  →  indices runs create --connector-id <id>

Create and revise connectors in the dashboard: https://platform.indices.io";

const RUNS_CREATE_AFTER_HELP: &str = "\
\x1b[1;97mModes:\x1b[0m
Parameters can be supplied in one of three different ways:
  Command args: pass `--connector-id` and optionally `--arguments` / `--secret-bindings`
  Raw JSON as parameter: pass one of `--body`, `--file`, `--stdin` (do not mix with argument mode)
  Piped JSON: if no mode flags are provided and stdin has data, JSON is read from stdin

\x1b[1;97mAsync runs:\x1b[0m
By default `runs create` waits until the run finishes (up to `--max-timeout-s`, default 300).
Pass `--async` to return immediately, then poll `indices runs get <run-id>`.

\x1b[1;97mExamples:\x1b[0m
  indices runs create --connector-id con_0A1b2C3d4E5f6G7h8I9j0K
  indices runs create --connector-id con_0A1b2C3d4E5f6G7h8I9j0K --arguments '{\"job_id\":\"A1\"}'
  indices runs create --connector-id con_0A1b2C3d4E5f6G7h8I9j0K --arguments '{\"job_id\":\"A1\"}' --secret-bindings '{\"GOOGLE_LOGIN\":\"sec_0A1b2C3d4E5f6G7h8I9j0K\"}'
  indices runs create --connector-id con_0A1b2C3d4E5f6G7h8I9j0K --async
  indices runs create --file run-payload.json
  cat run-payload.json | indices runs create";

const CAPTURES_START_AFTER_HELP: &str = "\
\x1b[1;97mWorkflow:\x1b[0m
  1. Start a capture to get an `iframe_url`
  2. Perform the website workflow in that browser
  3. Run `indices captures complete <id>`
  4. Poll `indices captures get <id>` until `state` is `completed`
  5. Use the recording in the dashboard to build or revise a connector

\x1b[1;97mExamples:\x1b[0m
  indices captures start
  indices captures start --use-proxy
  indices captures start --cookies '[{\"name\":\"sid\",\"value\":\"abc\",\"domain\":\"example.com\"}]'";

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
        default_value_t = false,
        help = "Emit JSON instead of markdown",
        help_heading = "Global Options"
    )]
    pub json: bool,

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
    #[command(about = "Authenticate and store credentials")]
    Login(LoginArgs),
    #[command(about = "Remove stored credentials")]
    Logout,
    #[command(about = "Show the current authenticated user")]
    Whoami,
    #[command(about = "Manage connectors")]
    Connectors {
        #[command(subcommand)]
        command: ConnectorsCommand,
    },
    #[command(about = "Run connectors and inspect previous runs")]
    Runs {
        #[command(subcommand)]
        command: RunsCommand,
    },
    #[command(about = "Upload, list, and download files")]
    Files {
        #[command(subcommand)]
        command: FilesCommand,
    },
    #[command(about = "Record browser capture sessions")]
    Captures {
        #[command(subcommand)]
        command: CapturesCommand,
    },
    #[command(about = "Manage secrets")]
    Secrets {
        #[command(subcommand)]
        command: SecretsCommand,
    },
    #[command(about = "Removed. Use `connectors` instead.")]
    Tasks {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        _rest: Vec<String>,
    },
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "",
        value_name = "API_KEY",
        help = "Use API-key auth instead of the default browser-based OAuth flow"
    )]
    pub api_key: Option<String>,

    #[arg(
        long,
        default_value_t = false,
        help = "Skip authentication verification"
    )]
    pub no_verify: bool,
}

#[derive(Debug, Subcommand)]
pub enum ConnectorsCommand {
    #[command(about = "List connectors")]
    List(ListConnectorsArgs),
    #[command(about = "Get a connector by ID")]
    Get(ConnectorIdArgs),
    #[command(about = "Rename a connector")]
    Rename(RenameConnectorArgs),
    #[command(about = "Delete a connector")]
    Delete(DeleteConnectorArgs),
    #[command(about = "List a connector's revision lineage")]
    Revisions(ConnectorIdArgs),
}

#[derive(Debug, Args)]
pub struct ListConnectorsArgs {
    #[arg(long, help = "Maximum number of connectors to return (1-100)")]
    pub limit: Option<u32>,

    #[arg(long, help = "Cursor from a previous response's next_cursor")]
    pub cursor: Option<String>,

    #[arg(
        long,
        help = "Only connectors whose website is this domain or a subdomain"
    )]
    pub domain: Option<String>,
}

#[derive(Debug, Args)]
pub struct ConnectorIdArgs {
    pub connector_id: String,
}

#[derive(Debug, Args)]
pub struct RenameConnectorArgs {
    pub connector_id: String,

    #[arg(long, help = "New short human-readable name")]
    pub display_name: String,
}

#[derive(Debug, Args)]
pub struct DeleteConnectorArgs {
    pub connector_id: String,

    #[arg(long, default_value_t = false)]
    pub yes: bool,
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

#[derive(Debug, Subcommand)]
pub enum RunsCommand {
    #[command(
        about = "Create a run for a connector",
        long_about = None,
        after_help = RUNS_CREATE_AFTER_HELP
    )]
    Create(CreateRunArgs),
    #[command(about = "List runs for a connector")]
    List(ListRunsArgs),
    #[command(about = "Get a run by ID")]
    Get(RunIdArgs),
    #[command(about = "Get logs for a run")]
    Logs(RunIdArgs),
}

#[derive(Debug, Args)]
pub struct CreateRunArgs {
    #[command(flatten)]
    pub payload: CreatePayloadSourceArgs,

    #[arg(
        long,
        value_name = "CONNECTOR_ID",
        help = "Connector ID to execute (required in argument mode)",
        help_heading = "Argument Mode"
    )]
    pub connector_id: Option<String>,

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
        help = "JSON object mapping secret slots to secret IDs",
        help_heading = "Argument Mode"
    )]
    pub secret_bindings: Option<String>,

    #[arg(
        long = "async",
        default_value_t = false,
        help = "Return immediately with a pending run",
        help_heading = "Argument Mode"
    )]
    pub run_async: bool,

    #[arg(
        long,
        value_name = "SECONDS",
        help = "Maximum execution time in seconds (1-3600, default 300)",
        help_heading = "Argument Mode"
    )]
    pub max_timeout_s: Option<u64>,
}

#[derive(Debug, Args)]
pub struct ListRunsArgs {
    #[arg(long)]
    pub connector_id: String,

    #[arg(long, help = "Maximum number of runs to return (1-100)")]
    pub limit: Option<u32>,

    #[arg(long, help = "Cursor from a previous response's next_cursor")]
    pub cursor: Option<String>,
}

#[derive(Debug, Args)]
pub struct RunIdArgs {
    pub run_id: String,
}

#[derive(Debug, Subcommand)]
pub enum FilesCommand {
    #[command(about = "List files")]
    List(ListFilesArgs),
    #[command(about = "Get file metadata by ID")]
    Get(FileIdArgs),
    #[command(about = "Upload a local file")]
    Upload(UploadFileArgs),
    #[command(about = "Finalize a pending file upload")]
    Finalize(FileIdArgs),
    #[command(about = "Delete a file")]
    Delete(DeleteFileArgs),
    #[command(about = "Get a short-lived download URL")]
    DownloadUrl(FileIdArgs),
    #[command(about = "Download a file to disk")]
    Download(DownloadFileArgs),
}

#[derive(Debug, Args)]
pub struct ListFilesArgs {
    #[arg(long, help = "Only files produced by this run")]
    pub run_id: Option<String>,

    #[arg(long, help = "Only files produced by runs of this connector")]
    pub connector_id: Option<String>,

    #[arg(long, help = "Only files whose name contains this text")]
    pub filename: Option<String>,

    #[arg(long, help = "Only files from this source: UPLOAD or RUN_OUTPUT")]
    pub source: Option<String>,

    #[arg(
        long,
        help = "Column to sort by: name, created_at, size_bytes, or source"
    )]
    pub sort: Option<String>,

    #[arg(long, help = "Sort direction: asc or desc")]
    pub order: Option<String>,

    #[arg(long, help = "Maximum number of files to return (1-100)")]
    pub limit: Option<u32>,

    #[arg(long, help = "Cursor from a previous response's next_cursor")]
    pub cursor: Option<String>,
}

#[derive(Debug, Args)]
pub struct FileIdArgs {
    pub file_id: String,
}

#[derive(Debug, Args)]
pub struct UploadFileArgs {
    pub path: PathBuf,

    #[arg(long, help = "User-facing filename (defaults to the local file name)")]
    pub name: Option<String>,

    #[arg(
        long,
        help = "MIME type (guessed from the file extension when omitted)"
    )]
    pub content_type: Option<String>,
}

#[derive(Debug, Args)]
pub struct DeleteFileArgs {
    pub file_id: String,

    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct DownloadFileArgs {
    pub file_id: String,

    #[arg(short, long, help = "Destination path (defaults to the file's name)")]
    pub output: Option<PathBuf>,

    #[arg(
        long,
        default_value_t = false,
        help = "Overwrite the destination if it exists"
    )]
    pub yes: bool,
}

#[derive(Debug, Subcommand)]
pub enum CapturesCommand {
    #[command(
        about = "Start a capture session",
        long_about = None,
        after_help = CAPTURES_START_AFTER_HELP
    )]
    Start(StartCaptureArgs),
    #[command(about = "List capture sessions")]
    List,
    #[command(about = "Get a capture session by ID")]
    Get(CaptureIdArgs),
    #[command(about = "Complete a capture session")]
    Complete(CaptureIdArgs),
    #[command(about = "Abandon a capture session")]
    Abandon(CaptureIdArgs),
}

#[derive(Debug, Args)]
pub struct StartCaptureArgs {
    #[arg(
        long,
        default_value_t = false,
        help = "Spawn the browser session using a proxy"
    )]
    pub use_proxy: bool,

    #[arg(
        long,
        value_name = "JSON",
        help = "JSON array of cookies to set, each with name and value"
    )]
    pub cookies: Option<String>,
}

#[derive(Debug, Args)]
pub struct CaptureIdArgs {
    pub capture_session_id: String,
}

#[derive(Debug, Subcommand)]
pub enum SecretsCommand {
    #[command(about = "Create a string or login secret")]
    Create(CreateSecretArgs),
    #[command(about = "List secrets (metadata only)")]
    List,
    #[command(about = "Delete a secret")]
    Delete(DeleteSecretArgs),
    #[command(about = "Generate a TOTP code for a login secret")]
    Totp(SecretIdArgs),
}

#[derive(Debug, Args)]
pub struct CreateSecretArgs {
    pub name: String,

    #[arg(
        long = "type",
        value_name = "TYPE",
        help = "Secret type: string or login"
    )]
    pub secret_type: Option<String>,

    #[arg(long, help = "Secret value (string type)")]
    pub value: Option<String>,

    #[arg(long, help = "Login username (login type)")]
    pub username: Option<String>,

    #[arg(long, help = "Login password (login type)")]
    pub password: Option<String>,

    #[arg(long, help = "Optional TOTP secret, base32 encoded (login type)")]
    pub totp_secret: Option<String>,

    #[arg(long, help = "Optional website URL for context")]
    pub website: Option<String>,

    #[arg(
        long,
        default_value_t = false,
        help = "Read the string secret value from stdin"
    )]
    pub stdin: bool,
}

#[derive(Debug, Args)]
pub struct DeleteSecretArgs {
    pub id: String,

    #[arg(long, default_value_t = false)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct SecretIdArgs {
    pub id: String,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{
        Cli, Command, ConnectorsCommand, LoginArgs, RunIdArgs, RunsCommand, SecretsCommand,
    };

    #[test]
    fn parses_json_flag_as_global_option() {
        let cli = Cli::parse_from(["indices", "--json", "connectors", "list"]);

        assert!(cli.json);
        assert!(matches!(cli.command, Command::Connectors { .. }));
    }

    #[test]
    fn parses_runs_logs_command() {
        let cli = Cli::parse_from(["indices", "runs", "logs", "run_0A1b2C3d4E5f6G7h8I9j0K"]);

        assert!(matches!(
            cli.command,
            Command::Runs {
                command: RunsCommand::Logs(RunIdArgs { ref run_id })
            } if run_id == "run_0A1b2C3d4E5f6G7h8I9j0K"
        ));
    }

    #[test]
    fn parses_login_api_key_flag_without_value() {
        let cli = Cli::parse_from(["indices", "login", "--api-key"]);

        assert!(matches!(
            cli.command,
            Command::Login(LoginArgs {
                api_key: Some(ref api_key),
                no_verify: false,
            }) if api_key.is_empty()
        ));
    }

    #[test]
    fn parses_whoami_command() {
        let cli = Cli::parse_from(["indices", "whoami"]);

        assert!(matches!(cli.command, Command::Whoami));
    }

    #[test]
    fn parses_connectors_get() {
        let cli = Cli::parse_from(["indices", "connectors", "get", "con_123"]);

        assert!(matches!(
            cli.command,
            Command::Connectors {
                command: ConnectorsCommand::Get(ref args)
            } if args.connector_id == "con_123"
        ));
    }

    #[test]
    fn parses_runs_create_async_flag() {
        let cli = Cli::parse_from([
            "indices",
            "runs",
            "create",
            "--connector-id",
            "con_123",
            "--async",
        ]);

        assert!(matches!(
            cli.command,
            Command::Runs {
                command: RunsCommand::Create(ref args)
            } if args.connector_id.as_deref() == Some("con_123") && args.run_async
        ));
    }

    #[test]
    fn parses_secrets_totp() {
        let cli = Cli::parse_from(["indices", "secrets", "totp", "sec_123"]);

        assert!(matches!(
            cli.command,
            Command::Secrets {
                command: SecretsCommand::Totp(ref args)
            } if args.id == "sec_123"
        ));
    }

    #[test]
    fn parses_removed_tasks_command() {
        let cli = Cli::parse_from(["indices", "tasks", "list"]);

        assert!(matches!(cli.command, Command::Tasks { .. }));
    }
}
