#!/usr/bin/env bash
# Build agent-heart release and adhoc-sign on macOS.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build --release -p agent-heart
"$ROOT/scripts/sign-macos.sh"
