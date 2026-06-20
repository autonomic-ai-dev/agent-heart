# agent-heart

**Background maintenance daemon — scheduled agent-brain GC and token budgeting.**

`agent-heart` runs periodic `agent-brain gc`, exposes predictive token budgeting for agent-spine, and registers on the spine event bus. It does not duplicate brain logic; it orchestrates it on a schedule.

Standalone: `agent-heart gc` · Integrated: supervised by `autonomic start` on port **3101**.

---

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/autonomic-ai-dev/agent-heart/master/scripts/install.sh | bash
```

---

## Quick start

```bash
agent-heart status
agent-heart gc                    # one-shot GC via agent-brain
agent-heart serve                 # cron scheduler + HTTP :3101
agent-heart budget check --tokens 4000
```

---

## Commands

| Command | Description |
|---------|-------------|
| `serve` | Daemon with cron GC and HTTP API |
| `gc` | Run one agent-brain GC pass |
| `status` | Schedule and last-run info |
| `budget check` | Token budget gate (used by agent-spine) |

---

## HTTP API

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Daemon health |
| `POST /budget/check` | Approve or deny token spend |

---

## Configuration

Section `[heart]` in `~/.autonomic/config.toml` · State under `~/.autonomic/state/heart/`

---

## Development

```bash
cargo test --release -p agent-heart
```

---

## License

MIT
