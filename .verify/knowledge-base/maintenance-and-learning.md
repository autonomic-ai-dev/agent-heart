# Maintenance and learning

## What it does

agent-heart runs background upkeep for the agent stack: scheduled memory garbage collection, memory distillation, an offline self-learning critic over verification outcomes, and a fine-tune schedule. One status command shows the state of each.

## How it works

1. **Scheduled GC.** With `[heart.schedule]` enabled, `serve` runs `agent-brain gc` on its cron expression (default `0 3 * * *`). `agent-heart gc` runs the same pass once.
2. **Distillation.** `heart_memory_distill` groups related facts into concepts. Facts above the `threshold` in `[heart.distillation]` are clustered.
3. **Self-learning critic.** `agent-heart learn` reads verification outcomes from the last `lookback_days`. It writes anti-patterns and reinforcements into brain memory once at least `min_outcomes` exist. `serve` runs this on `[heart.self_learning].cron`. The daemon only does this when `AUTONOMIC_SELF_LEARNING=1` is set.
4. **Fine-tune schedule.** `[heart.finetune]` runs on a cron. It starts `muscle train` only when `min_merged_entries`, `ui_threshold`, and `memory_threshold_kb` are all met.
5. **Usage engine.** Pricing, usage rows, and quota headers are recorded here. The budget gate and its stats depend on these numbers.
6. **Status.** `agent-heart status` reads state under `~/.autonomic/state/heart/` and prints each subsystem.

## Where the code is

- Routes: `agent-heart/agent-heart/src/serve.rs` lines 28 to 33 (`/health`, `/gc/status`, `/budget/stats`, `/budget/check`, `/budget/wasm/record`, `/outcomes/record`)
- MCP tools: `heart_gc`, `heart_memory_distill`, `heart_self_learn`, `heart_status`

## How to use it

- `agent-heart serve` runs the daemon. `autonomic start` runs it under supervision.
- `agent-heart gc` and `agent-heart learn` run one pass each.
- `agent-heart status` shows the last GC time and learning state.

## How it is verified

- `cargo test` for the whole crate (35 tests in the recorded baseline)
- `.verify/playbooks/verify_usage_engine.sh` runs the pricing, usage store, quota parser, and MCP parameter tests together

## Known limits

- Whether cron jobs actually fire inside `serve` is not tested. Cron timing is unproven until a run shows it.
- `GET /gc/status` and `POST /outcomes/record` have no test.
- The self-learning critic writes to brain memory. Running `learn` against the real `~/.autonomic/memory` changes shared state. Tests must use a temporary brain path.
- `[heart.finetune]` can start a GPU-heavy training run. Keep it disabled in any environment that runs the functional suite.
- The health spec in `docs/features/health-endpoint.md` and `CONSTITUTION.md` contain placeholders. The code is the source of truth for the routes.

## Related

- Feature map: `agent-heart/.verify/features/maintenance-and-learning.md`
- Budget gate: [budget-gate](budget-gate.md)
