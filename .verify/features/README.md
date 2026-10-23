# agent-heart features

## Baseline preconditions

- Rust toolchain matching `Cargo.lock`. No daemon is needed for unit tests.
- The crate source is nested: `agent-heart/agent-heart/src/`. The workspace root `Cargo.toml` runs `cargo test` for it.
- HTTP checks need `agent-heart serve` on port 3101, started by the test and stopped by its recorded PID.
- Config comes from `~/.autonomic/config.toml`. Tests must not read the user's file for expected values.

## Driving conventions

- Unit tests run through `cargo test`, never a bare binary.
- Logs go to `.verify/artifacts/<RUN_ID>/`. Baseline logs are in `.verify/artifacts/feature-map-baseline/`.

## Proof standards

- A feature is proven when its unit filter passes and, for HTTP or CLI surfaces, its Hurl or shell check passes. Only the second part is currently missing for every feature here.

## Feature index

| Feature file | Sub-features | Primary surface |
|---|---|---|
| `budget-gate.md` | 5 | `POST /budget/check`, `budget check`, `budget stats` |
| `maintenance-and-learning.md` | 10 | `serve` cron jobs, `gc`, `learn`, `status`, MCP tools, HTTP `/health`, `/gc/status`, `/outcomes/record` |
