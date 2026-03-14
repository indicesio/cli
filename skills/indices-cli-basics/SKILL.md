---
name: indices-cli-basics
description: Use this skill for foundational Indices CLI usage: installation, global flags, output formats, create-command JSON input behavior, and config path behavior.
---

# Indices CLI Basics

## When To Use

Use this skill when a request is about:
- Installing or invoking `indices`
- Choosing between default Markdown output and `--json`
- Understanding global flags (`--api-base`, `--timeout`)
- Understanding create-command input precedence (`--body`, `--file`, `--stdin`, args, piped stdin)
- Config path and environment overrides

## Install

```bash
cargo install --path .
indices --help
```

## Global Flags

Available on all commands:
- `--json` to emit JSON instead of the default Markdown output
- `--api-base <url>` (default: `https://api.indices.io`)
- `--timeout <seconds>` (default: `30`)

## Output Modes

### `markdown` (default)
- Human/LLM-friendly CLI output.
- Nested arrays/objects are shown as JSON blocks.

### `json`
- Pretty JSON output for scripting.

## Create Input Precedence

For `tasks create` and `runs create`, precedence is:

1. Explicit JSON source if set: exactly one of `--body`, `--file`, `--stdin`
2. Argument mode if create args are present
3. Auto-read piped stdin JSON when no explicit source/args are present
4. Error if nothing is provided

Rules:
- Do not mix `--body/--file/--stdin` with create argument flags.
