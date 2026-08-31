# Indices CLI

A CLI interface for the Indices API.

## Install

### macOS and Linux

```bash
curl -fsSL https://get.indices.io | sh
```

<details>

<summary>Advanced install flags</summary>

You can customise the install command, if you wish:

```bash
# Install a specific version:
curl -fsSL https://get.indices.io | sh -s -- --version 0.2.0

# Install to a custom directory:
curl -fsSL https://get.indices.io | sh -s -- --install-dir /usr/local/bin --yes
```

</details>

### Windows

`install.sh` does not run on Windows. Download `indices_<version>_windows_x86_64.zip` from GitHub Releases, extract `indices.exe`, and add its folder to `PATH`.

### Local development install

```bash
cargo install --path .

# With copy
cargo install --path . && cp target/release/indices ~/.local/bin
```

## Commands

```bash
indices login
indices login --api-key
indices login --api-key "idx_..."
indices whoami
indices update
indices update --check

indices connectors list
indices connectors list --domain example.com
indices connectors get <connector-id>
indices connectors rename <connector-id> --display-name "Invoice retrieval"
indices connectors revisions <connector-id>
indices connectors delete <connector-id>

indices runs create --connector-id "<connector-id>" --arguments '{"job_id":"123"}'
indices runs create --connector-id "<connector-id>" --async
indices runs list --connector-id <connector-id>
indices runs get <run-id>
indices runs logs <run-id>

indices files list --run-id <run-id>
indices files upload ./report.pdf
indices files get <file-id>
indices files download <file-id> --output ./report.pdf
indices files download-url <file-id>
indices files delete <file-id>

indices captures start
indices captures get <capture-session-id>
indices captures complete <capture-session-id>
indices captures abandon <capture-session-id>
indices captures list

indices secrets create OPENAI_API_KEY --value "sk-..."
indices secrets create STORE_LOGIN --type login --username "user" --password "..."
indices secrets list
indices secrets totp <secret-id>
indices secrets delete <secret-id>
```

Commands emit pretty-printed JSON.

Create methods support:
- Argument mode by default (for example, `--connector-id`, `--arguments`)
- Piped JSON from stdin:
  - `cat payload.json | indices runs create`
- Explicit JSON payload sources: `--body`, `--file`, or `--stdin`

Connectors are created and revised in the [Indices dashboard](https://platform.indices.io). The CLI lists, inspects, runs, and manages them.

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

## Refresh OpenAPI schema snapshot

`openapi/openapi.json` is a committed snapshot of the production schema. CI verifies that this snapshot is up-to-date with production.

```bash
make generate-client
cargo check
```
