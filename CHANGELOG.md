# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [1.0.0-beta.3.1] - 2026-10-17

### Added
- **Core Model Usage Data Models** — Introduced `ModelUsageRecord`, `TokenUsage`, `UsageQueryFilter`, and `AggregatedUsageSummary` to model granular LLM token telemetry.
- **Provider Pricing Catalog** — Added built-in pricing engine covering major LLM providers (OpenAI GPT-4o, Anthropic Claude 3.5 Sonnet / Haiku, Google Gemini 2.5 Pro / Flash, DeepSeek, and local Ollama / vLLM at $0.00).
- **SQLite Usage Persistence Store** — Migrated `heart.db` with indexed `model_usage` table for sub-millisecond aggregation across time windows.
- **MCP Usage Governance Tools** — Exposed `heart_record_usage`, `heart_get_usage_summary`, and `heart_check_budget` tools for connected agent hosts.
- **Verification Playbook Suite** — Added comprehensive `.verify/run.sh` playbooks validating pricing math, SQL insertion, and budget limits.

### Changed
- Bumped version to `v1.0.0-beta.3.1`.

## [1.0.0-beta.3] - 2026-10-17

### Added
- **Offline Self-Learning Critic** — Added `heart_self_learn` MCP tool and background critic loop for self-improvement without human intervention.

### Changed
- Bumped version to `v1.0.0-beta.3` and updated `agent-body-core` dependency to `v1.0.0-beta.3`.
- Relocated comprehensive `CONTEXT.md` to repository root and aligned documentation hub links.

## [1.0.0-beta.2] - 2026-10-16

### Added
- Official Homebrew tap formula support (`brew install agent-heart`).

### Changed
- Consolidated architecture documentation into centralized ecosystem hub.

## [1.0.0-beta.1] - 2026-10-16

### Changed
- Unified ecosystem versioning to 1.0.0-beta.1
- Pinned `agent-body-core` dependency to `v1.0.0-beta.1`
- Preserved CI Developer ID signatures in `scripts/install.sh` (clears quarantine without re-signing)
- Dynamic GitHub pre-release flag in release workflows for beta/rc/alpha tags
- CI strictly arm64 macOS native runners with Developer ID code signing
- Aligned documentation and CI path filters with agent-brain

## [0.8.2] - 2026-07-18

### Added

- **/health** returns `ok`/`service`/`version` per product feature spec

## [0.7.11] - 2026-06-27

### Changed

- **agent-body-core dependency** — updated tag to `v0.5.14` for ecosystem alignment

## [0.7.10] - 2026-06-27

### Added

- **`serve-mcp` subcommand** — starts the MCP stdio server only (no HTTP daemon, no cron scheduler). Used by `agent-body serve-mcp` gateway for persistent tool aggregation.
- **Integration tests** — `serve_mcp` CLI parsing, `HeartMcp` tool handler response validation (budget status, GC triggers)

## [0.7.9] - 2026-06-27

### Added

- Heart status tool (`heart_status`) — returns daemon state from MCP
- Dual-mode MCP server with organ tool naming convention (`heart_budget_status`, `heart_gc`, `heart_memory_distill`)

## [0.7.8] - 2026-06-23

### Added

- Global `--progress` CLI flag for structured ProgressTree output (also `AGENT_PROGRESS=1`)

## [0.7.7] - 2026-06-21

### Changed

- **Budget gate defaults** — `anomaly_multiplier` raised from 2.5→4.0, `min_samples` raised from 5→20, reducing false-positive freezes for legitimate workflows
- **Env overrides** — `AUTONOMIC_ANOMALY_MULTIPLIER`, `AUTONOMIC_BUDGET_MIN_SAMPLES`, `AUTONOMIC_BUDGET_CEILING` allow runtime tuning without config file changes

## [0.7.6] - 2026-06-21

### Fixed

- **Cron panic** — wrapped `tokio-cron-scheduler` initialization block in `match` to prevent unhandled scheduler errors from crashing Tokio channels, and added `normalize_cron` to safely pad 5-field expressions to 6 fields automatically

## [0.7.5] - 2026-06-21

### Added

- `agent-heart update [--force]` — self-update subcommand that checks GitHub releases, compares versions, and downloads the latest binary

## [0.7.4] - 2026-06-21

### Added

- `agent-heart log <name> [--follow] [--list]` — read daemon logs from the supervisor log directory

## [0.7.3] - 2026-06-21

### Fixed

- Default cron expressions updated from 5-field to 6-field format (`"0 0 3 * * *"`) for compatibility with `cron` v0.15 parser that rejects 5-field expressions
- Scheduler start is now non-fatal — daemon continues running even if cron expression is invalid

## [0.7.2] - 2026-06-21

### Fixed

- agent-spine registration is now non-fatal — daemon starts even without spine available

## [0.7.1] - 2026-06-20

### Added

- `--version` CLI flag (`5b41f90`)
- Mermaid architecture charts in README (`7ebc351`)

### Changed

- Professional README with standalone and integrated usage (`d278f1a`)

## [0.7.0] - 2026-06-20

### Added

- **Predictive token budgeting** — reads `~/.autonomic/memory/brain.db` retrieval_log history (p95 + anomaly multiplier)
- **`agent-heart budget stats|check`** CLI and HTTP `/budget/stats`, `/budget/check`
- **Spine gate event** — frozen routes publish `heart.budget.frozen` via agent-spine events API

## [0.6.1] - 2026-06-20

### Added

- **`agent-heart finetune`** one-shot CLI — runs gated dataset pipeline then MLX training
- **`agent-heart distill`** now ingests spine execution graphs before cluster distillation

### Changed

- Finetune config supports UI/memory gates and `min_merged_entries`

## [0.6.0] - 2026-06-20

### Added

- **Nightly finetune cron** — 04:00 job runs `agent-brain dataset pipeline` then `agent-muscle train` on merged JSONL

## [0.5.0] - 2026-06-20

### Added

- **Unified config** — loads from `~/.autonomic/config.toml` via `agent-body-core::organ_config::load("heart")`
- **Global state dir** — GC timestamps and organ state under `~/.autonomic/state/heart/`

### Changed

- Version bumped from `0.4.0` to `0.5.0`

## [0.4.0] - 2026-06-20

### Added

- **Static Bash linter** — `agent-heart lint script.sh` parses bash scripts with tree-sitter-bash and checks for unsafe patterns (rm -rf, eval, hardcoded secrets, long lines)

### Changed

- Version bumped from `0.3.0` to `0.4.0`

## [0.3.0] - 2026-06-20

### Added

- **HTTP health endpoint** — axum HTTP server with `/health` and `/gc/status` endpoints on configurable port (default 3101)
- **Agent-spine client** — `SpineClient` module for registration, heartbeat (30s interval), and event publishing
- **Concurrent serve** — `agent-heart serve` now starts HTTP server concurrently with MCP server and cron scheduler
- **Config extended** — `server.port` (default 3101) and `spine.url` (default `http://localhost:3100`) settings

### Changed

- Version bumped from `0.2.0` to `0.3.0`

## [0.2.0] - 2026-06-20

### Added

- **Distill CLI** — `agent-heart distill` runs `agent-brain distill clusters` as a subprocess; delegates cluster distillation to agent-brain
- **`BrainHandle::call_distill()`** — subprocess call for distill command with output capture
- **`run_distill_once()`** — library function for one-shot distill execution
- **Agent registry integration** — heart registers with agent-spine event bus on startup (subject: `agent.heart.beat`)

### Changed

- Version bumped from `0.1.0` to `0.2.0`

## [0.1.0] - 2026-06-20

### Added

- **Initial project scaffold** — workspace, crate, config module with auto-created `~/.config/agent-heart/config.yaml`
- **Brain client** — discovers `agent-brain` binary via config path, well-known locations, `which`, and `$HOME/.cargo/bin/`; runs `agent-brain gc` as a subprocess
- **Cron scheduler** — uses `tokio-cron-scheduler` to run GC on configurable schedule (default: daily at 3 AM)
- **MCP server** — exposes `heart_trigger_gc` (run GC with optional params) and `heart_status` (daemon info) tools via stdio
- **CLI** — `agent-heart serve` (daemon with MCP + cron), `gc` (one-shot GC), `status` (config, schedule, last GC timestamp)
