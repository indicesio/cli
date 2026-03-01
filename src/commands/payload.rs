use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::Path;

use serde_json::{Map, Value};

use crate::errors::CliError;

pub struct ExplicitJsonSource<'a> {
    pub body: Option<&'a str>,
    pub file: Option<&'a Path>,
    pub stdin: bool,
    pub command: &'static str,
}

pub fn load_explicit_json_payload(
    source: ExplicitJsonSource<'_>,
) -> Result<Option<Value>, CliError> {
    let selected = [source.body.is_some(), source.file.is_some(), source.stdin]
        .into_iter()
        .filter(|is_selected| *is_selected)
        .count();

    if selected == 0 {
        return Ok(None);
    }

    if selected > 1 {
        return Err(CliError::Message(format!(
            "provide at most one of `--body`, `--file`, or `--stdin` for `{}`",
            source.command
        )));
    }

    let parsed = if let Some(raw) = source.body {
        parse_json_value(raw, "--body")?
    } else if let Some(path) = source.file {
        let raw = fs::read_to_string(path)?;
        parse_json_value(&raw, &format!("file `{}`", path.display()))?
    } else {
        load_json_stdin(source.command)?
    };

    Ok(Some(parsed))
}

pub fn load_json_stdin(command: &str) -> Result<Value, CliError> {
    let mut raw = String::new();
    io::stdin().read_to_string(&mut raw)?;

    if raw.trim().is_empty() {
        return Err(CliError::Message(format!(
            "`{command}` expected JSON on stdin, but stdin was empty"
        )));
    }

    parse_json_value(&raw, "stdin")
}

pub fn stdin_has_data() -> bool {
    !io::stdin().is_terminal()
}

pub fn parse_json_value(raw: &str, source: &str) -> Result<Value, CliError> {
    serde_json::from_str::<Value>(raw)
        .map_err(|error| CliError::Message(format!("{source} must be valid JSON: {error}")))
}

pub fn parse_json_object_arg(raw: &str, flag: &str) -> Result<Map<String, Value>, CliError> {
    let value = parse_json_value(raw, flag)?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| CliError::Message(format!("{flag} must be a JSON object")))
}
