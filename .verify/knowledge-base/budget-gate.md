# Budget gate

## What it does

agent-heart answers one question: does a requested token spend fit the configured budget? It returns allow or deny. agent-spine calls it before each LLM-heavy node, so work over budget halts before it spends tokens.

## How it works

1. **Limits.** `[heart.token_budget]` in `~/.autonomic/config.toml` sets the per-route ceiling.
2. **Vector check.** Several dimensions are tracked together: tokens and WASM fuel. A request over any one dimension is denied (`budget_vector::tests`).
3. **Throttle.** A PID controller slows spend that drifts above target, so bursts smooth out instead of cutting off abruptly (`pid_throttle::tests`).
4. **HTTP check.** `POST /budget/check` takes `{"phase": ..., "estimated_tokens": N}`. A decision, allow or deny, returns HTTP 200. A denial is `decision.frozen == true`, with the reason in `decision.reason`. A freeze also publishes `heart.budget.frozen` on the event bus. An internal error returns `{"ok": false, "error": ...}` with HTTP 500, so a caller can treat it as unavailable. Spine does this, and it denies the node.
5. **Stats.** `GET /budget/stats` returns stored usage. `agent-heart budget stats` prints history, anomaly flags, and predictions.
6. **WASM accounting.** `POST /budget/wasm/record` records one WASM run's fuel.

## Where the code is

- Routes: `agent-heart/agent-heart/src/serve.rs` lines 28 to 33
- Budget logic: `token_budget`, `budget_vector`, and `pid_throttle` modules

## How to use it

- `agent-heart budget check --tokens 5000` to test one spend from the shell
- `agent-heart serve` on port 3101, then `POST /budget/check` with a JSON body
- Read `decision.frozen` for the verdict. Do not read the HTTP status, which is always 200

## How it is verified

- `cargo test token_budget::tests`, `cargo test budget_vector::tests`, `cargo test pid_throttle::tests`
- `cargo test error_routes_answer_500_with_ok_false`: each route answers HTTP 500 with `ok: false` when its store is unavailable
- `.verify/playbooks/verify_budget_limits.sh`, listed in the allowlist

## Known limits

- The README says a denial returns HTTP 429 and the body is `{"tokens": N}`. The code does neither. The code is correct; the README needs fixing.
- No Hurl suite exists for `POST /budget/check`. A passing unit run does not prove the HTTP contract.
- The ceiling comes from the user's `config.toml`. Tests must not depend on that file.
- On the caller side, spine's per-node check fails open when heart is unreachable. See [workflow-engine](../../../agent-spine/.verify/knowledge-base/workflow-engine.md).

## Related

- Feature map: `agent-heart/.verify/features/budget-gate.md`
- Maintenance and self-learning: [maintenance-and-learning](maintenance-and-learning.md)
