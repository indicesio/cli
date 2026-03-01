# Releasing Indices CLI

This repository uses a manual GitHub Actions release workflow (`workflow_dispatch`) with a version input.

## Requirements

- Merge release changes to `main`.
- `Cargo.toml` package version must match the release version.
- GitHub Actions must be enabled with `contents: write` permission for workflow token.

## Recommended release steps (manual dispatch)

1. Update `version` in `Cargo.toml`.
2. Commit and merge the version bump to `main`.
3. Run the release workflow manually:

```bash
gh workflow run release.yml --ref main -f version=X.Y.Z
```

4. Wait for `.github/workflows/release.yml` to complete.
   - On manual trigger, the workflow creates/pushes tag `vX.Y.Z` if it does not already exist.
5. Verify the release has all expected assets:
   - `indices_<version>_darwin_arm64.tar.gz`
   - `indices_<version>_darwin_x86_64.tar.gz`
   - `indices_<version>_linux_arm64.tar.gz`
   - `indices_<version>_linux_x86_64.tar.gz`
   - `indices_<version>_windows_x86_64.zip`
   - `indices_<version>_checksums.txt`
6. Smoke test install on macOS/Linux:

```bash
curl -fsSL https://indices.io/install.sh | bash -s -- --version X.Y.Z
indices --version
```

## Rollback and hotfix

- Do not mutate or re-push an existing tag.
- For fixes, bump patch version and publish a new tag.

## install.sh hosting contract

- Canonical installer source lives in this repo at `install.sh`.
- Host `https://indices.io/install.sh` from your website infrastructure or redirect it to the canonical raw file in this repository.
- Keep the URL target updated if repository path or branch changes.
