#!/usr/bin/env bash
set -euo pipefail

# Playbook: Verify Token Budget, Fuel & PID Limits
# Tests anomaly spike detection, WASM fuel limits, and PID throttling.

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$DIR/../.." && pwd)"

echo "==> [Playbook: Budget Limits] Running token budget anomaly tests..."
cd "$REPO_ROOT"
cargo test token_budget::tests

echo "==> [Playbook: Budget Limits] Running multi-dimensional budget vector tests..."
cargo test budget_vector::tests

echo "==> [Playbook: Budget Limits] Running PID throttle tests..."
cargo test pid_throttle::tests

echo "==> [Playbook: Budget Limits] Passed cleanly (exit 0)."
