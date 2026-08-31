use std::io::{self, IsTerminal};
use std::sync::OnceLock;

use clap::ValueEnum;

/// Controls when the CLI emits ANSI color.
///
/// `--color always`/`on` and `--color never`/`off` win over environment
/// detection. `auto` (the default) colors only an interactive human terminal.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum ColorChoice {
    /// Color when stdout is a TTY and no agent/`NO_COLOR` override applies.
    #[default]
    Auto,
    /// Always emit color, including when piped (for `less -R`).
    #[value(alias = "on")]
    Always,
    /// Never emit color.
    #[value(alias = "off")]
    Never,
}

const RESET: &str = "\x1b[0m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const GREY: &str = "\x1b[90m";

// Stripe CLI TerminalStyle (tidwall/pretty defaults).
const JSON_KEY: &str = "\x1b[1m\x1b[94m";
const JSON_STRING: &str = "\x1b[32m";
const JSON_NUMBER: &str = "\x1b[33m";
const JSON_BOOL: &str = "\x1b[36m";
const JSON_NULL: &str = "\x1b[2m";
const JSON_PUNCT: &str = "\x1b[1m";

/// Env vars set by coding agents and CI. Presence of a non-empty value means
/// stdout is being captured for a machine, even when it is a PTY.
const AGENT_ENV_VARS: &[&str] = &[
    "ANTIGRAVITY_CLI_ALIAS",
    "CI",
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLINE_ACTIVE",
    "CODEX_CI",
    "CODEX_INTERNAL_ORIGINATOR_OVERRIDE",
    "CODEX_SANDBOX",
    "CODEX_SANDBOX_NETWORK_DISABLED",
    "CODEX_THREAD_ID",
    "CURSOR_AGENT",
    "GEMINI_CLI",
    "OPENCLAW_SHELL",
    "OPENCODE",
];

static CONTEXT: OnceLock<ColorContext> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
struct ColorContext {
    choice: ColorChoice,
    stdout_is_tty: bool,
    no_color: bool,
    clicolor: Option<String>,
    clicolor_force: Option<String>,
    term: Option<String>,
    is_agent: bool,
}

impl ColorContext {
    fn from_env(choice: ColorChoice) -> Self {
        Self {
            choice,
            stdout_is_tty: io::stdout().is_terminal(),
            no_color: env_nonempty("NO_COLOR"),
            clicolor: env_string("CLICOLOR"),
            clicolor_force: env_string("CLICOLOR_FORCE"),
            term: env_string("TERM"),
            is_agent: detect_agent(),
        }
    }

    fn should_colorize(&self) -> bool {
        match self.choice {
            ColorChoice::Never => return false,
            ColorChoice::Always => return true,
            ColorChoice::Auto => {}
        }

        // Agents often have a real PTY (Cursor tmux, Claude Code). Treat them
        // as non-color even if CLICOLOR_FORCE is set; `--color always` overrides.
        if self.no_color || self.is_agent {
            return false;
        }

        if let Some(force) = self.clicolor_force.as_deref() {
            return force != "0";
        }

        if self.clicolor.as_deref() == Some("0") {
            return false;
        }

        if self.term.as_deref() == Some("dumb") {
            return false;
        }

        self.stdout_is_tty
    }
}

/// Record the parsed `--color` flag and enable Windows VT processing.
pub fn init(choice: ColorChoice) {
    let _ = CONTEXT.set(ColorContext::from_env(choice));
    enable_windows_ansi();
}

pub fn should_colorize() -> bool {
    match CONTEXT.get() {
        Some(ctx) => ctx.should_colorize(),
        None => ColorContext::from_env(ColorChoice::Auto).should_colorize(),
    }
}

/// Prefix/reset codes for human status lines (`whoami`, `update`). Empty when
/// color is off so callers can keep the same format strings.
pub fn status_ansi() -> StatusAnsi {
    StatusAnsi::for_enabled(should_colorize())
}

#[derive(Debug, Clone, Copy)]
pub struct StatusAnsi {
    pub green: &'static str,
    pub cyan: &'static str,
    pub grey: &'static str,
    pub reset: &'static str,
}

impl StatusAnsi {
    pub fn for_enabled(enabled: bool) -> Self {
        if enabled {
            Self {
                green: GREEN,
                cyan: CYAN,
                grey: GREY,
                reset: RESET,
            }
        } else {
            Self {
                green: "",
                cyan: "",
                grey: "",
                reset: "",
            }
        }
    }
}

/// Colorize already-pretty JSON when [`should_colorize`] is true; otherwise
/// return `pretty` unchanged so the bytes stay valid JSON.
pub fn colorize_pretty_json(pretty: &str) -> String {
    if should_colorize() {
        colorize_pretty_json_forced(pretty)
    } else {
        pretty.to_string()
    }
}

fn detect_agent() -> bool {
    AGENT_ENV_VARS.iter().copied().any(env_nonempty)
}

fn env_nonempty(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn env_string(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn enable_windows_ansi() {
    #[cfg(windows)]
    {
        let _ = anstyle_query::windows::enable_ansi_colors();
    }
}

enum Container {
    Object { expect_key: bool },
    Array,
}

impl Container {
    fn expect_key(&self) -> bool {
        matches!(self, Self::Object { expect_key: true })
    }
}

/// Stripe-style SGR overlay on pretty-printed JSON. The structural text is
/// unchanged: stripping ANSI yields the original `pretty` bytes.
fn colorize_pretty_json_forced(pretty: &str) -> String {
    let bytes = pretty.as_bytes();
    let mut out = String::with_capacity(pretty.len().saturating_add(pretty.len() / 2));
    let mut stack: Vec<Container> = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let is_key = stack.last().is_some_and(Container::expect_key);
                i = push_json_string(&mut out, pretty, i, is_key);
            }
            b'{' => {
                push_styled(&mut out, JSON_PUNCT, "{");
                stack.push(Container::Object { expect_key: true });
                i += 1;
            }
            b'[' => {
                push_styled(&mut out, JSON_PUNCT, "[");
                stack.push(Container::Array);
                i += 1;
            }
            b'}' | b']' => {
                stack.pop();
                push_styled(&mut out, JSON_PUNCT, &pretty[i..i + 1]);
                i += 1;
            }
            b':' => {
                if let Some(Container::Object { expect_key }) = stack.last_mut() {
                    *expect_key = false;
                }
                push_styled(&mut out, JSON_PUNCT, ":");
                i += 1;
            }
            b',' => {
                if let Some(Container::Object { expect_key }) = stack.last_mut() {
                    *expect_key = true;
                }
                push_styled(&mut out, JSON_PUNCT, ",");
                i += 1;
            }
            b't' if pretty[i..].starts_with("true") => {
                push_styled(&mut out, JSON_BOOL, "true");
                i += 4;
            }
            b'f' if pretty[i..].starts_with("false") => {
                push_styled(&mut out, JSON_BOOL, "false");
                i += 5;
            }
            b'n' if pretty[i..].starts_with("null") => {
                push_styled(&mut out, JSON_NULL, "null");
                i += 4;
            }
            c if c == b'-' || c.is_ascii_digit() => {
                let start = i;
                i += 1;
                while i < bytes.len() && is_json_number_char(bytes[i]) {
                    i += 1;
                }
                push_styled(&mut out, JSON_NUMBER, &pretty[start..i]);
            }
            _ => {
                let ch = pretty[i..].chars().next().expect("index on char boundary");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    out
}

fn push_json_string(out: &mut String, src: &str, start: usize, is_key: bool) -> usize {
    let color = if is_key { JSON_KEY } else { JSON_STRING };
    let end = find_string_end(src.as_bytes(), start);
    push_styled(out, color, &src[start..end]);
    end
}

fn find_string_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 1;
            if i < bytes.len() {
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'"' {
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}

fn is_json_number_char(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-')
}

fn push_styled(out: &mut String, code: &str, text: &str) {
    out.push_str(code);
    out.push_str(text);
    out.push_str(RESET);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auto_ctx() -> ColorContext {
        ColorContext {
            choice: ColorChoice::Auto,
            stdout_is_tty: true,
            no_color: false,
            clicolor: None,
            clicolor_force: None,
            term: Some("xterm-256color".to_string()),
            is_agent: false,
        }
    }

    fn strip_ansi(input: &str) -> String {
        let mut out = String::new();
        let mut chars = input.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\u{1b}' {
                if chars.peek() == Some(&'[') {
                    chars.next();
                    for next in chars.by_ref() {
                        if next.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                continue;
            }
            out.push(ch);
        }
        out
    }

    #[test]
    fn never_and_always_override_environment() {
        let mut ctx = auto_ctx();
        ctx.choice = ColorChoice::Never;
        ctx.stdout_is_tty = true;
        ctx.clicolor_force = Some("1".to_string());
        assert!(!ctx.should_colorize());

        ctx.choice = ColorChoice::Always;
        ctx.stdout_is_tty = false;
        ctx.no_color = true;
        ctx.is_agent = true;
        assert!(ctx.should_colorize());
    }

    #[test]
    fn auto_disables_for_no_color_agents_and_pipes() {
        let mut ctx = auto_ctx();
        assert!(ctx.should_colorize());

        ctx.no_color = true;
        assert!(!ctx.should_colorize());
        ctx.no_color = false;

        ctx.is_agent = true;
        assert!(!ctx.should_colorize());
        ctx.is_agent = false;

        ctx.stdout_is_tty = false;
        assert!(!ctx.should_colorize());
    }

    #[test]
    fn auto_respects_clicolor_and_dumb_term() {
        let mut ctx = auto_ctx();
        ctx.clicolor = Some("0".to_string());
        assert!(!ctx.should_colorize());
        ctx.clicolor = None;

        ctx.term = Some("dumb".to_string());
        assert!(!ctx.should_colorize());
        ctx.term = Some("xterm".to_string());

        ctx.stdout_is_tty = false;
        ctx.clicolor_force = Some("1".to_string());
        assert!(ctx.should_colorize());

        ctx.clicolor_force = Some("0".to_string());
        ctx.stdout_is_tty = true;
        assert!(!ctx.should_colorize());
    }

    #[test]
    fn agent_wins_over_clicolor_force() {
        let mut ctx = auto_ctx();
        ctx.is_agent = true;
        ctx.clicolor_force = Some("1".to_string());
        assert!(!ctx.should_colorize());
    }

    #[test]
    fn colorize_wraps_tokens_and_round_trips() {
        let pretty = serde_json::to_string_pretty(&serde_json::json!({
            "id": "cus_1",
            "email": "jenny@example.com",
            "balance": 0,
            "livemode": false,
            "deleted": null,
            "ok": true,
            "tags": ["a", "b"],
            "nested": { "n": -1.5 }
        }))
        .expect("pretty json");

        let colored = colorize_pretty_json_forced(&pretty);
        assert_eq!(strip_ansi(&colored), pretty);

        assert!(colored.contains(&format!("{JSON_KEY}\"id\"{RESET}")));
        assert!(colored.contains(&format!("{JSON_STRING}\"jenny@example.com\"{RESET}")));
        assert!(colored.contains(&format!("{JSON_NUMBER}0{RESET}")));
        assert!(colored.contains(&format!("{JSON_BOOL}false{RESET}")));
        assert!(colored.contains(&format!("{JSON_BOOL}true{RESET}")));
        assert!(colored.contains(&format!("{JSON_NULL}null{RESET}")));
        assert!(colored.contains(&format!("{JSON_PUNCT}{{{RESET}")));
        // Array strings are values, not keys.
        assert!(colored.contains(&format!("{JSON_STRING}\"a\"{RESET}")));
        assert!(colored.contains(&format!("{JSON_KEY}\"tags\"{RESET}")));
    }

    #[test]
    fn colorize_handles_escaped_quotes_and_unicode() {
        let pretty = serde_json::to_string_pretty(&serde_json::json!({
            "note": "say \"hi\" — café",
            "path": "C:\\tmp"
        }))
        .expect("pretty json");

        let colored = colorize_pretty_json_forced(&pretty);
        assert_eq!(strip_ansi(&colored), pretty);
        assert!(colored.contains("say \\\"hi\\\""));
        assert!(colored.contains("café"));
    }

    #[test]
    fn colorize_empty_containers() {
        let pretty = "{\n  \"empty_obj\": {},\n  \"empty_arr\": []\n}";
        let colored = colorize_pretty_json_forced(pretty);
        assert_eq!(strip_ansi(&colored), pretty);
    }
}
