# indices-cli

Rust CLI for the Indices API.

## Install

```bash
cargo install --path .
```

## Commands

```bash
indices login
indices whoami
indices tasks list
indices tasks create --display-name "Apply Job" --website "https://example.com" --task "Fill form"
indices runs create --task-id "<task-uuid>" --arguments '{"job_id":"123"}'
indices runs list --task-id <task-uuid>
indices secrets create OPENAI_API_KEY --value "sk-..."
indices secrets list
indices secrets delete <secret-uuid>
```

Use `--output markdown|json` on any command.

Create methods support:
- Argument mode by default (for example, `--task-id`, `--display-name`, `--website`, `--task`)
- Piped JSON from stdin when no explicit source flags are provided:
  - `cat payload.json | indices runs create`
  - `cat payload.json | indices tasks create`
- Explicit JSON payload sources: `--body`, `--file`, or `--stdin`

## Config

Config is stored at:

- macOS/Linux: `~/.config/indices/config.toml`
- Windows: platform-specific config directory via `directories`

## Regenerate OpenAPI schema snapshot

```bash
make generate-client
cargo check
```
