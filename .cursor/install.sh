#!/usr/bin/env bash
# Cloud Agent install script for the Indices CLI.
#
# Idempotent: safe to run repeatedly against cached or partially prepared state.
set -euo pipefail

# The crate targets Rust edition 2024, which requires rustc >= 1.85. The default
# base image may ship an older toolchain, so pin a recent stable toolchain here.
rustup toolchain install stable --profile minimal \
    --component rustfmt --component clippy
rustup default stable

# CI validates install.sh with shellcheck; make it available for local checks too.
if ! command -v shellcheck >/dev/null 2>&1; then
    sudo apt-get update -qq
    sudo apt-get install -y -qq shellcheck
fi

# Warm the dependency cache and generate the typed OpenAPI client via build.rs
# (from the committed openapi/openapi.json snapshot, so no network is required).
cargo build --locked --all-targets
