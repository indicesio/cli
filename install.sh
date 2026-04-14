#!/bin/sh
# shellcheck shell=sh

set -eu

INDICES_REPO="indicesio/cli"
DEFAULT_INSTALL_DIR="${INDICES_INSTALL_DIR:-$HOME/.local/bin}"
VERSION_INPUT="${INDICES_INSTALL_VERSION:-}"
INSTALL_DIR="${DEFAULT_INSTALL_DIR}"
ASSUME_YES=0
INSTALL_TMPDIR=""

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

say() {
    printf '%s\n' "$1" >&2
}

err() {
    printf 'error: %s\n' "$1" >&2
}

check_cmd() {
    command -v "$1" >/dev/null 2>&1
}

need_cmd() {
    if ! check_cmd "$1"; then
        err "missing required command: $1"
        exit 1
    fi
}

ensure() {
    if ! "$@"; then
        err "command failed: $*"
        exit 1
    fi
}

cleanup_install_tmpdir() {
    if [ -n "${INSTALL_TMPDIR}" ] && [ -d "${INSTALL_TMPDIR}" ]; then
        rm -rf "${INSTALL_TMPDIR}"
    fi
}

normalize_version() {
    version="$1"

    if [ -z "$version" ]; then
        printf '\n'
    elif [ "${version#v}" != "$version" ]; then
        printf '%s\n' "$version"
    else
        printf 'v%s\n' "$version"
    fi
}

validate_version_tag() {
    version_tag="$1"

    case "$version_tag" in
        v[0-9]*.[0-9]*.[0-9]*)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

latest_version_tag() {
    latest_url="$(
        curl -fsSLI -o /dev/null -w '%{url_effective}' \
            "https://github.com/${INDICES_REPO}/releases/latest"
    )"
    tag="${latest_url##*/}"

    if validate_version_tag "$tag"; then
        printf '%s\n' "$tag"
        return 0
    fi

    err "failed to resolve latest release tag for ${INDICES_REPO}"
    exit 1
}

parse_args() {
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --version)
                if [ "$#" -lt 2 ]; then
                    err "--version requires a value"
                    exit 1
                fi
                VERSION_INPUT="$2"
                shift 2
                ;;
            --install-dir)
                if [ "$#" -lt 2 ]; then
                    err "--install-dir requires a value"
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
                err "unknown argument: $1"
                usage >&2
                exit 1
                ;;
        esac
    done
}

pick_platform() {
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os" in
        darwin|linux)
            ;;
        *)
            err "unsupported operating system: $os"
            say "For Windows, download the ZIP asset from GitHub Releases."
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64|amd64)
            arch="x86_64"
            ;;
        arm64|aarch64)
            arch="arm64"
            ;;
        *)
            err "unsupported architecture: $arch"
            exit 1
            ;;
    esac

    printf '%s_%s\n' "$os" "$arch"
}

compute_sha256() {
    asset_path="$1"

    if check_cmd sha256sum; then
        sha256sum "$asset_path" | awk '{ print $1 }'
    elif check_cmd shasum; then
        shasum -a 256 "$asset_path" | awk '{ print $1 }'
    else
        err "missing required checksum tool: sha256sum or shasum"
        exit 1
    fi
}

verify_checksum() {
    asset_path="$1"
    checksums_path="$2"
    asset_name="$(basename "$asset_path")"
    expected="$(
        awk -v file="$asset_name" '$2 == file { print $1 }' "$checksums_path"
    )"

    if [ -z "$expected" ]; then
        err "no checksum entry found for ${asset_name}"
        exit 1
    fi

    actual="$(compute_sha256 "$asset_path")"
    if [ "$actual" != "$expected" ]; then
        err "checksum verification failed for ${asset_name}"
        say "Expected: ${expected}"
        say "Actual:   ${actual}"
        exit 1
    fi
}

confirm_overwrite() {
    destination="$1"

    if [ ! -f "$destination" ] || [ "$ASSUME_YES" -eq 1 ] || [ ! -t 0 ]; then
        return 0
    fi

    printf '%s exists. Overwrite? [y/N] ' "$destination" >&2
    read -r reply

    case "$reply" in
        y|Y)
            return 0
            ;;
        *)
            say "Installation cancelled."
            exit 1
            ;;
    esac
}

install_binary() {
    binary_path="$1"
    destination="${INSTALL_DIR}/indices"

    ensure mkdir -p "$INSTALL_DIR"
    confirm_overwrite "$destination"
    ensure cp "$binary_path" "$destination"
    ensure chmod 0755 "$destination"
}

main() {
    parse_args "$@"
    trap cleanup_install_tmpdir EXIT INT TERM HUP

    need_cmd curl
    need_cmd tar
    need_cmd mktemp
    need_cmd uname
    need_cmd awk
    need_cmd basename
    need_cmd cp
    need_cmd chmod
    need_cmd mkdir

    platform="$(pick_platform)"
    tag="$(normalize_version "$VERSION_INPUT")"

    if [ -z "$tag" ]; then
        tag="$(latest_version_tag)"
    fi

    version="${tag#v}"
    asset_name="indices_${version}_${platform}.tar.gz"
    checksums_name="indices_${version}_checksums.txt"
    base_url="https://github.com/${INDICES_REPO}/releases/download/${tag}"

    INSTALL_TMPDIR="$(mktemp -d)"
    asset_path="${INSTALL_TMPDIR}/${asset_name}"
    checksums_path="${INSTALL_TMPDIR}/${checksums_name}"

    say "Installing Indices CLI ${tag} for ${platform} from ${INDICES_REPO}"

    ensure curl -fsSL "${base_url}/${asset_name}" -o "$asset_path"
    ensure curl -fsSL "${base_url}/${checksums_name}" -o "$checksums_path"
    verify_checksum "$asset_path" "$checksums_path"
    ensure tar -xzf "$asset_path" -C "$INSTALL_TMPDIR"
    install_binary "${INSTALL_TMPDIR}/indices"

    say "Installed to ${INSTALL_DIR}/indices"
    case ":${PATH:-}:" in
        *:"${INSTALL_DIR}":*)
            ;;
        *)
            say "Add ${INSTALL_DIR} to your PATH to run 'indices' directly."
            ;;
    esac
}

main "$@"
