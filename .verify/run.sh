#!/usr/bin/env bash
set -euo pipefail

# agent-heart verification master entrypoint
# Executed by automated verification runners or developer testing

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$DIR/.." && pwd)"

echo "==> [agent-heart] Starting verification suite..."
cd "$REPO_ROOT"

echo "==> Checking cargo clippy with -D warnings..."
cargo clippy -- -D warnings

echo "==> Checking cargo check..."
cargo check

echo "==> Running Playbook: Usage Engine..."
"$DIR/playbooks/verify_usage_engine.sh"

echo "==> Running Playbook: Budget Limits..."
"$DIR/playbooks/verify_budget_limits.sh"

echo "==> Running Playbook: Full Suite..."
"$DIR/playbooks/verify_full_suite.sh"

echo "==> [agent-heart] All verification playbooks passed cleanly (exit 0)."
