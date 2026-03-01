#!/usr/bin/env bash
set -euo pipefail

INDICES_REPO="indicesio/cli"
DEFAULT_INSTALL_DIR="${INDICES_INSTALL_DIR:-$HOME/.local/bin}"
VERSION_INPUT="${INDICES_INSTALL_VERSION:-}"
INSTALL_DIR="${DEFAULT_INSTALL_DIR}"
ASSUME_YES=0

usage() {
  cat <<'EOF'
Usage: install.sh [options]

Install the Indices CLI from GitHub Releases.

Options:
  --version <X.Y.Z>   Install a specific version (default: latest)
  --install-dir <dir> Install directory (default: ~/.local/bin)
  --yes               Overwrite existing binary without prompting
  --help              Show this help message

Environment variables:
  INDICES_INSTALL_VERSION      Version to install (same as --version)
  INDICES_INSTALL_DIR          Install directory (same as --install-dir)
EOF
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

normalize_version() {
  local raw="$1"
  if [[ -z "$raw" ]]; then
    echo ""
    return
  fi

  if [[ "$raw" == v* ]]; then
    echo "$raw"
  else
    echo "v$raw"
  fi
}

latest_version_tag() {
  local latest_url tag
  latest_url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/${INDICES_REPO}/releases/latest")"
  tag="${latest_url##*/}"

  if [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z]+)*$ ]]; then
    echo "$tag"
    return
  fi

  echo "Failed to resolve latest release tag for ${INDICES_REPO}" >&2
  exit 1
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --version)
        if [[ $# -lt 2 ]]; then
          echo "--version requires a value" >&2
          exit 1
        fi
        VERSION_INPUT="$2"
        shift 2
        ;;
      --install-dir)
        if [[ $# -lt 2 ]]; then
          echo "--install-dir requires a value" >&2
          exit 1
        fi
        INSTALL_DIR="$2"
        shift 2
        ;;
      --yes)
        ASSUME_YES=1
        shift
        ;;
      --help)
        usage
        exit 0
        ;;
      *)
        echo "Unknown argument: $1" >&2
        usage
        exit 1
        ;;
    esac
  done
}

pick_platform() {
  local os arch
  os="$(uname -s | tr '[:upper:]' '[:lower:]')"
  arch="$(uname -m)"

  case "$os" in
    darwin) os="darwin" ;;
    linux) os="linux" ;;
    *)
      echo "Unsupported operating system: $os" >&2
      echo "For Windows, download the ZIP asset from GitHub Releases." >&2
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="arm64" ;;
    *)
      echo "Unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac

  printf "%s_%s" "$os" "$arch"
}

verify_checksum() {
  local asset_path checksums_path asset_name expected actual
  asset_path="$1"
  checksums_path="$2"
  asset_name="$(basename "$asset_path")"

  expected="$(awk -v file="$asset_name" '$2 == file { print $1 }' "$checksums_path")"
  if [[ -z "$expected" ]]; then
    echo "No checksum entry found for ${asset_name}" >&2
    exit 1
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$asset_path" | awk '{ print $1 }')"
  elif command -v shasum >/dev/null 2>&1; then
    actual="$(shasum -a 256 "$asset_path" | awk '{ print $1 }')"
  else
    echo "Missing required checksum tool: sha256sum or shasum" >&2
    exit 1
  fi

  if [[ "$actual" != "$expected" ]]; then
    echo "Checksum verification failed for ${asset_name}" >&2
    echo "Expected: ${expected}" >&2
    echo "Actual:   ${actual}" >&2
    exit 1
  fi
}

install_binary() {
  local binary_path destination
  binary_path="$1"
  destination="${INSTALL_DIR}/indices"

  mkdir -p "$INSTALL_DIR"

  if [[ -f "$destination" && "$ASSUME_YES" -ne 1 && -t 0 ]]; then
    read -r -p "${destination} exists. Overwrite? [y/N] " reply
    if [[ ! "$reply" =~ ^[Yy]$ ]]; then
      echo "Installation cancelled."
      exit 1
    fi
  fi

  cp "$binary_path" "$destination"
  chmod 0755 "$destination"
}

main() {
  parse_args "$@"

  require_cmd curl
  require_cmd tar

  local platform tag version asset_name checksums_name base_url tmpdir asset_path checksums_path
  platform="$(pick_platform)"
  tag="$(normalize_version "$VERSION_INPUT")"

  if [[ -z "$tag" ]]; then
    tag="$(latest_version_tag)"
  fi

  version="${tag#v}"
  asset_name="indices_${version}_${platform}.tar.gz"
  checksums_name="indices_${version}_checksums.txt"
  base_url="https://github.com/${INDICES_REPO}/releases/download/${tag}"

  tmpdir="$(mktemp -d)"
  trap 'rm -rf "$tmpdir"' EXIT

  asset_path="${tmpdir}/${asset_name}"
  checksums_path="${tmpdir}/${checksums_name}"

  echo "Installing indices ${tag} for ${platform} from ${INDICES_REPO}"

  curl -fsSL "${base_url}/${asset_name}" -o "$asset_path"
  curl -fsSL "${base_url}/${checksums_name}" -o "$checksums_path"

  verify_checksum "$asset_path" "$checksums_path"

  tar -xzf "$asset_path" -C "$tmpdir"
  install_binary "${tmpdir}/indices"

  echo "Installed to ${INSTALL_DIR}/indices"
  if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    echo "Add ${INSTALL_DIR} to your PATH to run 'indices' directly."
  fi
}

main "$@"
