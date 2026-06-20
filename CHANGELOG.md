# Changelog

## [v] - 2026-06-20

### Added
- Added Mermaid charts to README
- Added `--version` flag to CLI parser


All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
