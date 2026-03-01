#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SPEC_PATH="$ROOT_DIR/openapi/openapi.json"

mkdir -p "$ROOT_DIR/openapi"

echo "Fetching OpenAPI schema..."
curl -fsSL "https://api.indices.io/openapi.json" -o "$SPEC_PATH"

echo "Saved schema to $SPEC_PATH"
echo "Next step: run cargo check or cargo build to regenerate the typed client via build.rs."
