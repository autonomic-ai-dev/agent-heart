# agent-heart — Context for agents

Background maintenance and quota controller for Autonomic. Part of [agent-body](https://github.com/autonomic-ai-dev/agent-body).

| Field | Value |
|-------|-------|
| Role | Controller / GC |
| Binary | `agent-heart` |
| HTTP | `:3101` |
| Version | v1.0.0-beta.2 |
| Docs Hub | [https://autonomic-ai.dev/docs/maintenance](https://autonomic-ai.dev/docs/maintenance) · [Architecture](https://autonomic-ai.dev/docs/architecture) |

## When to use

Token/WASM budget checks before expensive spine nodes; scheduled GC/distillation pulses against brain. Never embed maintenance in the IDE MCP path.

## Key APIs

- `POST /budget/check` — allow/deny spend
- Cron GC → `agent-brain gc`
- Health JSON for mesh probes

## Verification

Budget denials are operational signals, not verify outcomes. Record test/build success with `agent-brain verify --` from the project under test.

## CI

Path filter on `agent-heart/**` + Cargo/Makefile/scripts/`.github`. Docs-only PRs skip compose.

## Related

Operator usage: [https://autonomic-ai.dev/docs/maintenance](https://autonomic-ai.dev/docs/maintenance) · Architecture: [https://autonomic-ai.dev/docs/architecture](https://autonomic-ai.dev/docs/architecture) · Spine (calls budget) · brain (GC target) · body (supervisor)
