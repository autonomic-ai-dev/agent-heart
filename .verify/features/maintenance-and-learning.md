# maintenance-and-learning

Summary: agent-heart runs background upkeep for the agent stack: scheduled memory garbage collection, weekly cluster distillation, an offline self-learning critic over verification outcomes, and a fine-tune schedule, and it reports their state through one status command.

## Sub-features

- `scheduled-gc`: Runs agent-brain garbage collection on a cron schedule.
  - Intent: Keep the memory index free of stale facts without manual work.
  - Behaviour: With `[heart.schedule]` enabled, `serve` triggers `agent-brain gc` on its cron expression (default `0 3 * * *`).
  - Outcome: The brain index is pruned, and `status` shows the last GC time.
- `gc-one-shot`: Runs one garbage collection pass now.
  - Intent: Support CI and ad-hoc maintenance.
  - Behaviour: `agent-heart gc` invokes the same brain GC once and exits.
  - Outcome: Exit status 0 when the pass completes.
- `health-endpoint`: Reports that the process is live, for supervisor probes.
  - Intent: Give load balancers and supervisors a liveness probe that needs no auth or downstream service.
  - Behaviour: `GET /health` (`agent-heart/src/serve.rs` line 28) returns the service name, version, and whether token budget is enabled.
  - Outcome: HTTP 200 with a JSON body. The spec in `docs/features/health-endpoint.md` describes `{"ok":true,"service":...,"version":...}`. The live response may differ, so check the JSON before depending on the exact keys.
- `gc-status-http`: Reports when GC last ran.
  - Behaviour: `GET /gc/status` (line 29) reads `last_gc.txt` under the heart state directory, or returns "never".
  - Outcome: JSON with `last_gc` and `version`. Not verified by a test in this pass.
- `outcome-record-http`: Stores one execution outcome for the self-learning critic.
  - Behaviour: `POST /outcomes/record` (line 33) writes an `ExecutionOutcome` to the heart database.
  - Outcome: `{"ok": true}` or `{"ok": false, "error": ...}`. Not verified by a test in this pass.
- `memory-distillation`: Groups related memory facts into higher-level concepts.
  - Intent: Keep recall useful as the fact count grows.
  - Behaviour: `heart_memory_distill` runs cluster distillation with `threshold` from `[heart.distillation]`.
  - Outcome: Clustered concepts are produced from facts above the similarity threshold.
- `self-learning-critic`: Reads recent verification outcomes and records anti-patterns and reinforcements.
  - Intent: Turn verification history into durable guidance for later routes.
  - Behaviour: `agent-heart learn` runs one cycle. `serve` runs it on `[heart.self_learning].cron`, looking back `lookback_days`.
  - Outcome: Entries are written to brain memory when at least `min_outcomes` exist. `heart_self_learn` runs the same cycle over MCP.
- `finetune-schedule`: Triggers model training when enough trajectories exist.
  - Intent: Retrain on schedule instead of by hand.
  - Behaviour: `[heart.finetune]` cron checks `min_merged_entries`, `ui_threshold`, and `memory_threshold_kb`, then calls `muscle train`.
  - Outcome: A training run starts only when every threshold is met.
- `status-report`: Shows schedule, last GC, self-learning state, fine-tune state, and budget ceiling.
  - Intent: Let an operator confirm maintenance is alive.
  - Behaviour: `agent-heart status` reads state under `~/.autonomic/state/heart/`.
  - Outcome: A printed table of each subsystem.
- `usage-engine`: Records token usage, quotas, and pricing for the budget gate and stats.
  - Intent: Provide the measured basis for every budget decision.
  - Behaviour: Covered by `pricing_catalog::tests`, `usage_store::tests`, `quota_parser::tests`, and MCP parameter defaults.
  - Outcome: Usage rows persist and quota headers parse into quota records.

## Code

- `agent-heart/src/brain_client.rs`
- `agent-heart/src/config.rs`
- `agent-heart/src/distillation.rs`
- `agent-heart/src/finetune.rs`
- `agent-heart/src/lib.rs`
- `agent-heart/src/log.rs`
- `agent-heart/src/lint.rs`
- `agent-heart/src/main.rs`
- `agent-heart/src/mcp_server.rs`
- `agent-heart/src/outcomes.rs`
- `agent-heart/src/pricing_catalog.rs`
- `agent-heart/src/quota_parser.rs`
- `agent-heart/src/scheduler.rs`
- `agent-heart/src/self_learning.rs`
- `agent-heart/src/update.rs`
- `agent-heart/src/usage_schema.rs`
- `agent-heart/src/usage_store.rs`
- `agent-heart/Cargo.toml`

## How to get to it (user POV)

- CLI: `agent-heart serve`, `agent-heart serve-mcp`, `agent-heart gc`, `agent-heart learn`, `agent-heart status`
- HTTP: `GET /health`, `GET /gc/status`, `GET /budget/stats`, `POST /budget/check`, `POST /budget/wasm/record`, `POST /outcomes/record` on `http://localhost:3101`. All six routes are in `agent-heart/src/serve.rs` lines 28-33.
- MCP: `heart_gc`, `heart_memory_distill`, `heart_self_learn`, `heart_status`
- Integrated: `autonomic start` runs `serve` under agent-body supervision

## Verification Suites (Targeted Test Commands)

- **Unit Tests:** `cargo test` (whole crate), with the usage-engine subset `cargo test pricing_catalog::tests`, `cargo test usage_store::tests`, `cargo test quota_parser::tests`
- **Functional API Tests (Hurl):** not yet written. `GET /health` and the MCP tools have no Hurl file.
- **Allowlisted Verification:** `.verify/run.sh` (runs clippy, check, and the three playbooks), `.verify/playbooks/verify_usage_engine.sh`, `.verify/playbooks/verify_full_suite.sh`
- **Preconditions:** Rust toolchain. The scheduler tests need no network. Cron-driven runs need `agent-brain` on PATH.

### Verification Steps

1. **full-crate-tests** (sub-features: all)
   - Request: `cargo test`
   - Why: Proves the whole crate, including scheduler, usage, and distillation code.
   - Expected: 35 passed, 0 failed (baseline).
   - Impact: A failure in any module means maintenance or usage accounting changed. Fix the cause.
2. **usage-engine-subset** (sub-feature: usage-engine)
   - Request: `.verify/playbooks/verify_usage_engine.sh`
   - Why: Runs the pricing, usage store, quota parser, and MCP parameter tests as one group.
   - Expected: Exit 0.
   - Impact: A failure means spend data would be recorded wrongly.

## Gotchas

- Not verified in this pass: whether any test starts `agent-heart serve` and checks the cron jobs. Treat cron timing as unproven until a run shows it.
- The self-learning critic writes into brain memory. Running `learn` against a real `~/.autonomic/memory` changes shared state. Tests must use a temporary brain path.
- `[heart.finetune]` can start a GPU-heavy training run. Keep `enabled = false` in any environment that runs the functional suite.
- The health spec `docs/features/health-endpoint.md` has `Status | example` in its metadata, and `CONSTITUTION.md` still contains unfilled placeholder comments in its architecture, security, and non-goals sections. Neither document is a reliable source of project rules yet. The code is the source of truth for the routes above.
- A route test exists in `serve.rs` (around line 202) that calls `/health` in-process through the router. It does not start a daemon, so it does not prove the listening socket on port 3101.
- Distillation and self-learning are gated by config flags and by the `AUTONOMIC_SELF_LEARNING=1` variable. A passing test with the flag unset does not prove the critic runs in the daemon.
