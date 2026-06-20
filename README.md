# agent-heart

**Background distillation daemon for agent-brain — cron-based memory GC and MCP tools for scheduled maintenance.**

agent-heart is the autonomic pulse of the agent-brain ecosystem. It runs as a lightweight daemon that periodically calls `agent-brain gc` to deduplicate, prune, and compact the memory store, plus serves MCP tools for on-demand GC triggers and status checks.

Rust is the heart; agent-brain is the brain.

```bash
# Install (coming soon)
# agent-heart serve
```

---

## Why agent-heart?

agent-brain's memory and index store grows over time. Running GC manually is easy to forget, and automating it through IDE hooks is fragile. agent-heart solves this with:

| Problem | agent-heart answer |
|---------|-------------------|
| "I forget to run agent-brain gc" | **Cron scheduler** — runs `agent-brain gc` on configurable schedule (default: daily at 3 AM) |
| "I need on-demand GC from my IDE" | **MCP tools** — `heart_trigger_gc` and `heart_status` in any MCP host |
| "I want to check GC status" | **CLI** — `agent-heart status` shows last GC time and schedule |

agent-heart does NOT reimplement memory management. It is a thin scheduler that delegates all GC logic to agent-brain via subprocess CLI calls.

---

## Features

| Feature | What it does |
|---------|-------------|
| **Cron scheduling** | Runs `agent-brain gc` on configurable cron expression (default: `0 3 * * *`) |
| **MCP server** | Exposes `heart_trigger_gc` (run GC) and `heart_status` (daemon info) via stdio MCP |
| **One-shot GC** | `agent-heart gc` runs a single GC pass and prints JSON stats |
| **Config file** | `~/.config/agent-heart/config.yaml` — auto-created with defaults |
| **State tracking** | Writes `last_gc.txt` timestamp after each successful GC run |

---

## Commands

| Command | Description |
|---------|-------------|
| `agent-heart serve` | Start daemon: MCP server + cron scheduler |
| `agent-heart gc` | Run one-shot GC pass and print stats |
| `agent-heart status` | Show daemon config, schedule, and last GC timestamp |

---

## Configuration (`~/.config/agent-heart/config.yaml`)

```yaml
schedule:
  cron: "0 3 * * *"
  enabled: true

brain:
  binary_path: ""   # optional, auto-discovered

logging:
  level: info
  file: ~/.local/share/agent-heart/agent-heart.log
```

The `brain.binary_path` is optional — agent-heart finds agent-brain via config, well-known paths, `which`, and `$HOME/.cargo/bin/`.

---

## Development

```bash
cargo build --release -p agent-heart
cargo test --release -p agent-heart
```

## License

MIT
