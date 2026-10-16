#!/usr/bin/env bash
# Build agent-heart release and Developer ID–sign on macOS (Apple Silicon).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build --release -p agent-heart
"$ROOT/scripts/sign-macos.sh"
