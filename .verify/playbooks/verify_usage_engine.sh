#!/usr/bin/env bash
set -euo pipefail

# Playbook: Verify Native Model Usage & Pricing Engine
# Tests model pricing catalog formulas, SQLite schema migrations, atomic rollups, and month-end burn rate projections.

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$DIR/../.." && pwd)"

echo "==> [Playbook: Usage Engine] Verifying pricing catalog formulas..."
cd "$REPO_ROOT"
cargo test pricing_catalog::tests

echo "==> [Playbook: Usage Engine] Verifying SQLite persistence and rolling rollups..."
cargo test usage_store::tests

echo "==> [Playbook: Usage Engine] Verifying Heart MCP usage tools..."
cargo test mcp_server::tests::default_lookback_days
cargo test mcp_server::tests::default_billing_type
cargo test mcp_server::tests::test_record_quota_params
cargo test mcp_server::tests::test_parse_headers_quota_params

echo "==> [Playbook: Usage Engine] Verifying Provider Quota Header Parser..."
cargo test quota_parser::tests

echo "==> [Playbook: Usage Engine] Passed cleanly (exit 0)."
