# agent-heart

`agent-heart` is the background controller for Autonomic. It runs scheduled memory garbage collection, enforces the token budget that agent-spine checks before LLM-heavy nodes, and records execution outcomes for the self-learning critic.

Docs: [autonomic-ai-dev.vercel.app](https://autonomic-ai-dev.vercel.app)

## Quick start

```bash
brew tap autonomic-ai-dev/tap && brew install agent-heart
agent-heart status
agent-heart serve
```

## Commands

| Command | What it does |
|---------|--------------|
| `agent-heart serve` | Runs the daemon: scheduled GC, self-learning, fine-tune scheduler, and the HTTP API on port 3101 |
| `agent-heart serve-mcp` | Runs the MCP stdio server only |
| `agent-heart gc` | Runs one garbage collection pass now |
| `agent-heart learn` | Runs one self-learning cycle now |
| `agent-heart status` | Shows the schedule, last GC, and budget ceiling |
| `agent-heart budget check --tokens N` | Checks whether N tokens are within budget |
| `agent-heart budget stats` | Shows usage history and anomalies |

## HTTP API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Liveness probe |
| `POST` | `/budget/check` | Checks a token request. Body: `{"phase": "...", "estimated_tokens": N}`. Always HTTP 200 with `{"ok": true, "decision": {...}}`. A denial is `decision.frozen == true`. |
| `GET` | `/gc/status` | Time of the last GC pass |
| `GET` | `/budget/stats` | Stored budget statistics |
| `POST` | `/budget/wasm/record` | Records one WASM execution for fuel accounting |
| `POST` | `/outcomes/record` | Records one execution outcome for the self-learning critic |

Failures on these routes return HTTP 500 with `{"ok": false, "error": ...}`.

## Configuration

Settings live in the `[heart]` section of `~/.autonomic/config.toml`. Each feature has its own sub-section: `schedule`, `token_budget`, `distillation`, `self_learning`, and `finetune`. State is stored under `~/.autonomic/state/heart/`.

## Develop

```bash
cargo test -p agent-heart
cargo clippy -p agent-heart -- -D warnings
```

Feature map and knowledge base: `.verify/features/` and `.verify/knowledge-base/`.
