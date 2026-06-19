# agent-heart

**Deterministic background daemon for AI coding agents — API budget enforcement, AST-level safety constraints, and memory distillation cron jobs.**

agent-heart is the autonomic pulse of the ecosystem. It manages global safety rules, dynamically allocates token budgets across models, and runs background maintenance tasks so that the orchestrator (`agent-spine`) never has to block for cleanup operations. 

Rust is the heart; Claude/Cursor are the hands.

```bash
curl -fsSL https://raw.githubusercontent.com/autonomic-ai-dev/agent-heart/master/scripts/install.sh | bash -s -- --global
agent-heart serve --daemon
```

**MCP is live immediately** — it runs silently in the background, listening to event buses and hooking into CLI execution paths.

---

## Why agent-heart?

When dealing with autonomous coding agents, three critical risks emerge at scale:

1. **Destructive Execution** — Prompting an agent not to delete files is a "soft constraint." Agents will occasionally hallucinate `rm -rf` or destructive database drops.
2. **Runaway Costs** — A single infinite loop in an agent workflow can burn $50 of Anthropic credits overnight.
3. **Context Degradation** — Over days of use, the memory vector index (`agent-brain`) gets bloated with overlapping facts, slowing down retrieval.

**agent-heart fixes this with a local background daemon:**

| Problem | agent-heart answer |
|---------|-------------------|
| "The agent ran `rm -rf /` by mistake" | **AST-Level Blocking** — intercepts generated shell commands and code to enforce hard safety constraints *before* execution. |
| "I burned $50 on API calls overnight" | **API Budget Allocation** — dynamically monitors and caps token usage for local and remote models via leaky bucket algorithms. |
| "Context gets bloated over time" | **Memory Distillation** — runs background cron jobs to analyze and summarize `agent-brain` vectors into denser embeddings. |

---

## Architectural Deep Dive

Unlike static analysis tools that run on save, `agent-heart` operates as a persistent daemon. 

### 1. The Global Safety Gate
`agent-heart` intercepts all execution commands dispatched by `agent-spine` or Cursor's MCP terminal.
- **Shell AST Parsing:** It does not use regex to block commands. It uses full bash AST parsing (via `tree-sitter`) to detect destructive operations, even if obfuscated (e.g., `rm "-r" "-f"`).
- **Hard Stops:** If a constraint is violated, the execution is instantly rejected and an error payload is returned to the agent, forcing it to rethink.

### 2. Algorithmic Budgeting
It maintains a local SQLite ledger of token usage.
- **Leaky Bucket Rate Limiting:** Enforces maximum tokens per minute (TPM) and maximum tokens per day (TPD).
- **Dynamic Routing:** If Claude 3.5 Sonnet hits its daily budget, `agent-heart` dynamically signals `agent-spine` to failover to local Qwen2.5 for remaining tasks.

### 3. Asynchronous Distillation
At 3:00 AM local time, the daemon spins up:
- Pulls cluster data from `agent-brain`.
- Identifies semantically overlapping facts (e.g., "Use React" and "Use functional components").
- Triggers a local LLM to rewrite them into a single, highly dense rule.

---

## Complete Setup (Copy & Paste)

### 1. Install the binary

```bash
curl -fsSL https://raw.githubusercontent.com/autonomic-ai-dev/agent-heart/master/scripts/install.sh | bash -s -- --global
```

### 2. Configuration (`~/.agent_heart/config.yaml`)

```yaml
daemon:
  pulse_interval_secs: 60
  auto_start: true

budgeting:
  anthropic_daily_usd_limit: 5.00
  local_fallback: true

safety:
  block_destructive_bash: true
  block_network_egress: false
  allowed_directories:
    - ~/workspace/
```

### 3. Verify

```bash
agent-heart version
agent-heart status      # Shows daemon uptime and current budget
```

---

## Commands

| Command | Description |
|---------|-------------|
| `agent-heart serve` | Start the background daemon (blocking) |
| `agent-heart limits` | View current token budgets and usage |
| `agent-heart rules` | List active AST safety constraints |
| `agent-heart distill`| Manually trigger a memory compression run |

---

## Development

```bash
cargo test --release -p agent-heart
cargo build --release -p agent-heart
```

## License
MIT
