# agent-heart architecture documentation

## Design goals

agent-heart exists to solve two maintenance problems that every autonomous system accumulates over time:

1. **Index bloat.** agent-brain's knowledge store grows indefinitely without GC. Old facts, stale embeddings, and deduplication debt degrade retrieval quality.
2. **Unbounded token spend.** agent-spine workflows can execute arbitrarily many LLM calls. Without a budget gate, a runaway workflow can burn through API quota in minutes.

### Key design decisions

| Decision | Rationale |
|----------|-----------|
| **Separate daemon, not embedded in brain** | Maintenance should never block the IDE. agent-heart runs on `:3101`, separate from brain's MCP stdio. |
| **CRON-driven, not continuous** | GC is expensive (index rebuild). Running it once daily at 3 AM is sufficient for most workloads. |
| **HTTP budget gate, not proxy** | agent-spine calls `POST /budget/check` before LLM work. Heart responds with allow/deny. No traffic proxying, no latency added to the LLM call itself. |
| **Token ceiling, not rate limit** | Hard ceiling prevents spike costs. Anomaly detection alerts on unusual patterns without blocking legitimate usage. |

### Budget gate algorithm

```
budget_check(tokens):
  ceiling = config.token_budget.ceiling  # default: 8000
  rolling_avg = historical_usage.last_24h().avg()
  anomaly_z = (tokens - rolling_avg) / std_dev
  
  if tokens > ceiling:
    return Deny("exceeds ceiling")
  if anomaly_z > config.anomaly_x:  # default: 2.5
    return Deny("statistical anomaly")
  return Allow
```

### Alternatives considered

| Option | Why rejected |
|--------|-------------|
| **Embed in agent-brain** | GC during routing would block MCP responses |
| **Systemd timers / cron** | Works but requires separate setup; `serve` is self-contained |
| **Proxy all LLM calls** | Adds latency to every call; gate before call is better |
| **Running total, no ceiling** | Doesn't protect against spike costs from a single large run |
