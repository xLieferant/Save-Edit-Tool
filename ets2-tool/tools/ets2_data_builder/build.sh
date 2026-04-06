#!/usr/bin/env bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cargo run --manifest-path "$REPO_ROOT/src-tauri/Cargo.toml" --bin ets2_data_builder -- "$REPO_ROOT"

