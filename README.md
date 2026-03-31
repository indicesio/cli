# indices-cli

Rust CLI for the Indices API.

## Install

### macOS and Linux

```bash
curl -fsSL https://indices.io/install.sh | bash
```

Install a specific version:

```bash
curl -fsSL https://indices.io/install.sh | bash -s -- --version 0.1.0
```

Install to a custom directory:

```bash
curl -fsSL https://indices.io/install.sh | bash -s -- --install-dir /usr/local/bin --yes
```

### Windows

`install.sh` does not run on Windows. Download `indices_<version>_windows_x86_64.zip` from GitHub Releases, extract `indices.exe`, and add its folder to `PATH`.

### Local development install

```bash
cargo install --path .
```

## Commands

```bash
indices login
indices login --api-key
indices login --api-key "idx_..."
indices auth-test
indices tasks list
indices tasks create --display-name "Apply Job" --website "https://example.com" --task "Fill form"
indices runs create --task-id "<task-uuid>" --arguments '{"job_id":"123"}'
indices runs list --task-id <task-uuid>
indices runs get <run-uuid>
indices runs logs <run-uuid>
indices secrets create OPENAI_API_KEY --value "sk-..."
indices secrets list
indices secrets delete <secret-uuid>
```

Commands render Markdown by default. Use `--json` on any command for JSON output.

Create methods support:
- Argument mode by default (for example, `--task-id`, `--display-name`, `--website`, `--task`)
- Piped JSON from stdin:
  - `cat payload.json | indices runs create`
  - `cat payload.json | indices tasks create`
- Explicit JSON payload sources: `--body`, `--file`, or `--stdin`

## Config

Config is stored at:

- macOS/Linux: `~/.config/indices/config.toml`
- Windows: platform-specific config directory via `directories`

`indices login` uses browser-based OAuth by default. An API-key flow is available with `indices login --api-key`.

## Release assets

Each GitHub Release contains:
- `indices_<version>_darwin_arm64.tar.gz`
- `indices_<version>_darwin_x86_64.tar.gz`
- `indices_<version>_linux_arm64.tar.gz`
- `indices_<version>_linux_x86_64.tar.gz`
- `indices_<version>_windows_x86_64.zip`
- `indices_<version>_checksums.txt`

The installer verifies downloaded binaries against the checksum file.

## Releasing

See [RELEASING.md](RELEASING.md) for the release process.

## Regenerate OpenAPI schema snapshot

```bash
make generate-client
cargo check
```
