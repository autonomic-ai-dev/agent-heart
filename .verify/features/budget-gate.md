# budget-gate

Summary: agent-heart answers whether a requested token spend fits the configured budget and returns allow or deny, so that callers such as agent-spine halt LLM-heavy work before it runs.

## Sub-features

- `budget-check-cli`: Decides one token request from the command line.
  - Intent: Let operators and scripts test a spend before running it.
  - Behaviour: `agent-heart budget check --tokens N` compares N against the per-route ceiling in `[heart.token_budget]`.
  - Outcome: Exit status and printed decision reflect allow or deny.
- `budget-check-http`: Approves or denies a spend over HTTP.
  - Intent: Give spine a synchronous gate before each LLM node.
  - Behaviour: `POST /budget/check` (`agent-heart/src/serve.rs` line 31) takes a `BudgetCheckRequest` with `phase` and `estimated_tokens`.
  - Outcome: Always HTTP 200 with `{"ok": true, "decision": {...}}`. A denial is `decision.frozen == true`, with the reason in `decision.reason`. A freeze also publishes `heart.budget.frozen` on the event bus. On an internal error the body is `{"ok": false, "error": "..."}`, still HTTP 200.
- `budget-stats-http`: Returns the stored budget statistics.
  - Behaviour: `GET /budget/stats` returns `{"ok": true, "stats": ...}` or `{"ok": false, "error": ...}`.
  - Outcome: Read-only. Not verified by a test in this pass.
- `budget-wasm-record`: Records one WASM execution for fuel accounting.
  - Behaviour: `POST /budget/wasm/record` returns `{"ok": true}` or `{"ok": false, "error": ...}`.
  - Outcome: The record is written to the budget store. Not verified by a test in this pass.
- `budget-vector-limits`: Tracks several budget dimensions at once.
  - Intent: Enforce tokens and WASM fuel together, so one dimension cannot be spent silently.
  - Behaviour: Covered by `budget_vector::tests`.
  - Outcome: A request over any single dimension is denied.
- `budget-pid-throttle`: Slows spend that drifts above target.
  - Intent: Smooth bursts instead of cutting off abruptly.
  - Behaviour: Covered by `pid_throttle::tests`.
  - Outcome: Throttle output moves toward the target over repeated samples.
- `budget-stats`: Reports historical usage, anomalies, and predictions.
  - Intent: Show whether spend is abnormal before it becomes a bill.
  - Behaviour: `agent-heart budget stats` reads recorded usage and flags statistical outliers.
  - Outcome: A printed summary with history, anomaly flags, and predictions.

## How to get to it (user POV)

- CLI: `agent-heart budget check --tokens 5000`
- CLI: `agent-heart budget stats`
- HTTP: `POST /budget/check` with body `{"phase": "build", "estimated_tokens": 5000}` on the daemon at `http://localhost:3101` (started by `agent-heart serve`). Read `decision.frozen` for the verdict.
- HTTP: `GET /budget/stats`, `POST /budget/wasm/record`
- MCP: `heart_budget_status` through `agent-heart serve-mcp`
- Config: `[heart.token_budget]` in `~/.autonomic/config.toml`

## Verification Suites (Targeted Test Commands)

- **Unit Tests:** `cargo test token_budget::tests`, `cargo test budget_vector::tests`, `cargo test pid_throttle::tests`
- **Functional API Tests (Hurl):** not yet written. The HTTP contract `POST /budget/check` has no Hurl file. See Gotchas.
- **Allowlisted Verification:** `.verify/playbooks/verify_budget_limits.sh`, listed in `.verify/verify.toml`
- **Preconditions:** Rust toolchain and locked dependencies. No daemon is needed for the unit tests. The HTTP check needs `agent-heart serve` on port 3101.

### Verification Steps

1. **budget-unit-tests** (sub-features: budget-vector-limits, budget-pid-throttle, budget-check-http)
   - Request: `sh .verify/checks/require_tests.sh token_budget::tests` then `sh .verify/checks/require_tests.sh budget_vector::tests` then `sh .verify/checks/require_tests.sh pid_throttle::tests`. The guard fails when a filter matches zero tests.
   - Why: Proves the limit, vector, and throttle logic in isolation.
   - Expected: All tests in the three modules pass, 0 failed.
   - Impact: A failure means spend control changed. Fix the cause. Do not change the expected ceiling.

## Gotchas

- Documentation drift: `agent-heart/README.md` says a deny returns HTTP 429 and that the body is `{"tokens": N}`. The code does neither (see `budget-check-http`). The contract follows the code. Fix the README.
- The HTTP endpoint `POST /budget/check` and the `budget check` CLI have no functional Hurl suite. A passing unit run does not prove the HTTP contract or the `decision.frozen` result.
- The budget ceiling is read from `~/.autonomic/config.toml`. A local config with a different `max_tokens_per_route` changes results, so the test must not depend on the user's file.
- Baseline on this repository: `cargo test` for the whole crate passed 35 tests, 0 failed (see `.verify/artifacts/feature-map-baseline/cargo-test.log`).
