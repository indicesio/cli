use std::env;
use std::process::Command as ProcessCommand;
use std::time::Instant;

use chrono::{DateTime, SecondsFormat, Utc};
use posthog_rs::Event;
use serde::Serialize;
use serde_json::{Map, Value, json};
use tracing::warn;
use uuid::Uuid;

use crate::cli::{Cli, Command, RunsCommand, SecretsCommand, TasksCommand};
use crate::config::StoredSession;

const POSTHOG_API_KEY: &str = "phc_jVpGaCd1oWZEsWxv5KHfmNacUjOp5VT4yhNJAnpBiok";
const POSTHOG_HOST: &str = "https://eu.i.posthog.com";
const TELEMETRY_DISABLED_ENV: &str = "INDICES_TELEMETRY_DISABLED";

/// Context included in every event
#[derive(Debug, Clone, Serialize)]
pub struct CommandTelemetryContext {
    pub distinct_id: String,
    pub is_authenticated: bool,

    // Command properties
    pub route: &'static str,
    pub command: &'static str,
    pub args: Vec<String>,

    // Skipped by Serde, because we manually set the `timestamp` field in rfc3339_millis format,
    // as needed by PostHog, later in the program. We record this as a DateTime for more accurate
    // "wall clock" time.
    #[serde(skip)]
    pub started_at: DateTime<Utc>,
    // Separately track the start instant, for accurate _monotonic_ time to then calculate an
    // elapsed duration for the command's execution.
    #[serde(skip)]
    start_instant: Instant,

    // Environment properties
    arch: &'static str,
    os_type: String,
    os_release: String,
    version: String,
    exec_path: String,
    install_method: String,
    pwd: String,

    // Workspace properties
    git_repo: Option<String>,
}

pub struct Analytics {
    client: Option<posthog_rs::Client>,
}

impl Analytics {
    pub async fn new() -> Self {
        let client = if telemetry_disabled() || POSTHOG_API_KEY.is_empty() {
            None
        } else {
            Some(posthog_rs::client((POSTHOG_API_KEY, POSTHOG_HOST)).await)
        };

        Self { client }
    }

    pub fn build_context(&self, cli: &Cli, argv: &[String]) -> CommandTelemetryContext {
        let os = os_info::get();
        // TODO: replace with a slightly more persistent identifier
        let run_id = Uuid::now_v7().to_string();
        let started_at = Utc::now();
        let distinct_id = format!("anon-cli:{run_id}");

        CommandTelemetryContext {
            distinct_id,
            is_authenticated: false,
            route: route_for_command(&cli.command),
            command: command_name(&cli.command),
            args: sanitize_args(argv),
            started_at,
            start_instant: Instant::now(),
            arch: env::consts::ARCH,
            os_type: os.os_type().to_string(),
            os_release: os.version().to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            exec_path: current_exec_path(),
            install_method: infer_install_method(),
            pwd: current_working_directory(),
            git_repo: current_git_repo(),
        }
    }

    pub async fn identify_authenticated_user(
        &self,
        context: &mut CommandTelemetryContext,
        session: &StoredSession,
    ) {
        let Some(identity) = session.cached_identity() else {
            return;
        };

        context.distinct_id = identity.user_id.clone();
    }

    pub async fn capture_command_start(&self, context: &CommandTelemetryContext) {
        self.capture_event(
            "cli:command:start",
            &context.distinct_id,
            context.started_at,
            start_properties(context),
        )
        .await;
    }

    pub async fn capture_command_end(
        &self,
        context: &CommandTelemetryContext,
        success: bool,
        exit_code: i32,
    ) {
        let finished_at = Utc::now();
        let mut properties = start_properties(context);
        add_end_properties(
            &mut properties,
            finished_at,
            success,
            exit_code,
            context.start_instant.elapsed().as_millis() as u64,
        );

        self.capture_event(
            "cli:command:end",
            &context.distinct_id,
            finished_at,
            properties,
        )
        .await;
    }

    async fn capture_event(
        &self,
        event_name: &str,
        distinct_id: &str,
        timestamp: DateTime<Utc>,
        properties: Map<String, Value>,
    ) {
        let Some(client) = &self.client else {
            return;
        };

        let mut event = Event::new(event_name.to_string(), distinct_id.to_string());
        for (key, value) in properties {
            let _ = event.insert_prop(&key, value);
        }
        let _ = event.set_timestamp(timestamp);
        if let Err(error) = client.capture(event).await {
            warn!(%error, event_name, distinct_id, "failed to capture telemetry event");
        }
    }
}

fn start_properties(context: &CommandTelemetryContext) -> Map<String, Value> {
    let mut properties: Map<String, Value> = match serde_json::to_value(context) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    };
    properties.insert(
        "timestamp".to_string(),
        Value::String(rfc3339_millis(context.started_at)),
    );
    properties.insert(
        "invoked_via".to_string(),
        Value::String(String::from("cli")),
    );
    properties
}

fn add_end_properties(
    properties: &mut Map<String, Value>,
    finished_at: DateTime<Utc>,
    success: bool,
    exit_code: i32,
    duration_ms: u64,
) {
    properties.extend([
        ("success".to_string(), Value::Bool(success)),
        ("exit_code".to_string(), json!(exit_code)),
        ("duration_ms".to_string(), json!(duration_ms)),
        (
            "timestamp".to_string(),
            Value::String(rfc3339_millis(finished_at)),
        ),
    ]);
}

pub fn route_for_command(command: &Command) -> &'static str {
    match command {
        Command::Login(_) => "auth.login",
        Command::Logout => "auth.logout",
        Command::Whoami => "auth.whoami",
        Command::Tasks { command } => match command {
            TasksCommand::Create(_) => "tasks.create",
            TasksCommand::Get(_) => "tasks.get",
            TasksCommand::List(_) => "tasks.list",
            TasksCommand::Delete(_) => "tasks.delete",
            TasksCommand::Retry(_) => "tasks.retry",
            TasksCommand::RegenerateApi(_) => "tasks.regenerate_api",
        },
        Command::Runs { command } => match command {
            RunsCommand::Create(_) => "runs.create",
            RunsCommand::List(_) => "runs.list",
            RunsCommand::Get(_) => "runs.get",
            RunsCommand::Logs(_) => "runs.logs",
        },
        Command::Secrets { command } => match command {
            SecretsCommand::Create(_) => "secrets.create",
            SecretsCommand::List => "secrets.list",
            SecretsCommand::Delete(_) => "secrets.delete",
        },
    }
}

fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Login(_) => "login",
        Command::Logout => "logout",
        Command::Whoami => "whoami",
        Command::Tasks { command } => match command {
            TasksCommand::Create(_) => "tasks create",
            TasksCommand::Get(_) => "tasks get",
            TasksCommand::List(_) => "tasks list",
            TasksCommand::Delete(_) => "tasks delete",
            TasksCommand::Retry(_) => "tasks retry",
            TasksCommand::RegenerateApi(_) => "tasks regenerate-api",
        },
        Command::Runs { command } => match command {
            RunsCommand::Create(_) => "runs create",
            RunsCommand::List(_) => "runs list",
            RunsCommand::Get(_) => "runs get",
            RunsCommand::Logs(_) => "runs logs",
        },
        Command::Secrets { command } => match command {
            SecretsCommand::Create(_) => "secrets create",
            SecretsCommand::List => "secrets list",
            SecretsCommand::Delete(_) => "secrets delete",
        },
    }
}

fn sanitize_args(argv: &[String]) -> Vec<String> {
    let mut sanitized = Vec::new();
    let mut redact_next = false;

    for arg in argv.iter().skip(1) {
        if redact_next {
            sanitized.push("[REDACTED]".to_string());
            redact_next = false;
            continue;
        }

        if let Some((flag, _)) = arg.split_once('=') {
            if should_redact_flag(flag) {
                sanitized.push(format!("{flag}=[REDACTED]"));
                continue;
            }
        }

        if should_redact_flag(arg) {
            sanitized.push(arg.clone());
            redact_next = true;
            continue;
        }

        sanitized.push(arg.clone());
    }

    sanitized
}

fn should_redact_flag(flag: &str) -> bool {
    matches!(
        flag,
        "--api-key"
            | "--value"
            | "--body"
            | "--arguments"
            | "--secret-bindings"
            | "--creation-params"
            | "--input-schema"
            | "--output-schema"
    )
}

fn infer_install_method() -> String {
    let exec_path = current_exec_path();

    if exec_path.contains(".cargo/bin") || exec_path.contains("/cargo/") {
        return "cargo".to_string();
    }

    if exec_path.contains("/.local/bin/")
        || exec_path.ends_with("/indices")
        || exec_path.contains("/bin/indices")
    {
        return "native".to_string();
    }

    "unknown".to_string()
}

fn current_exec_path() -> String {
    env::current_exe()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

fn current_working_directory() -> String {
    env::current_dir()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

fn current_git_repo() -> Option<String> {
    let output = ProcessCommand::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let remote = String::from_utf8(output.stdout).ok()?;
    let remote = remote.trim();
    if remote.is_empty() {
        None
    } else {
        Some(remote.to_string())
    }
}

fn rfc3339_millis(timestamp: DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn telemetry_disabled() -> bool {
    env::var(TELEMETRY_DISABLED_ENV)
        .ok()
        .map(|value| is_truthy_env_value(&value))
        .unwrap_or(false)
}

fn is_truthy_env_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, LoginArgs};
    use clap::Parser;

    #[test]
    fn route_mapping_covers_nested_commands() {
        let cli = Cli::parse_from(["indices", "runs", "logs", "123"]);
        assert_eq!(route_for_command(&cli.command), "runs.logs");
    }

    #[test]
    fn sanitize_args_redacts_sensitive_values() {
        let args = vec![
            "indices".to_string(),
            "login".to_string(),
            "--api-key".to_string(),
            "secret".to_string(),
            "--body={\"token\":\"secret\"}".to_string(),
        ];

        assert_eq!(
            sanitize_args(&args),
            vec![
                "login".to_string(),
                "--api-key".to_string(),
                "[REDACTED]".to_string(),
                "--body=[REDACTED]".to_string(),
            ]
        );
    }

    #[test]
    fn build_context_marks_login_route() {
        let cli = Cli {
            json: false,
            api_base: None,
            timeout: None,
            command: Command::Login(LoginArgs {
                api_key: None,
                no_verify: false,
            }),
        };
        let analytics = tokio::runtime::Runtime::new()
            .expect("runtime should build")
            .block_on(Analytics::new());
        let argv = vec!["indices".to_string(), "login".to_string()];
        let context = analytics.build_context(&cli, &argv);

        assert_eq!(context.route, "auth.login");
        assert_eq!(context.command, "login");
        assert_eq!(context.args, vec!["login".to_string()]);
    }

    #[test]
    fn can_disable_telemetry_at_runtime() {
        let _guard = EnvVarGuard::set(TELEMETRY_DISABLED_ENV, Some("1"));
        assert!(telemetry_disabled());

        let _guard = EnvVarGuard::set(TELEMETRY_DISABLED_ENV, Some("true"));
        assert!(telemetry_disabled());

        let _guard = EnvVarGuard::set(TELEMETRY_DISABLED_ENV, Some("YES"));
        assert!(telemetry_disabled());

        let _guard = EnvVarGuard::set(TELEMETRY_DISABLED_ENV, Some(" on "));
        assert!(telemetry_disabled());

        let _guard = EnvVarGuard::set(TELEMETRY_DISABLED_ENV, Some("0"));
        assert!(!telemetry_disabled());

        let _guard = EnvVarGuard::set(TELEMETRY_DISABLED_ENV, Some("false"));
        assert!(!telemetry_disabled());

        let _guard = EnvVarGuard::set(TELEMETRY_DISABLED_ENV, None);
        assert!(!telemetry_disabled());
    }

    /// Revert to previous env state when the guard is dropped.
    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let previous = env::var(key).ok();
            match value {
                Some(value) => unsafe { env::set_var(key, value) },
                None => unsafe { env::remove_var(key) },
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe { env::set_var(self.key, value) },
                None => unsafe { env::remove_var(self.key) },
            }
        }
    }
}
