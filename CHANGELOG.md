# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-06-20

### Added

- **Initial project scaffold** — workspace, crate, config module with auto-created `~/.config/agent-heart/config.yaml`
- **Brain client** — discovers `agent-brain` binary via config path, well-known locations, `which`, and `$HOME/.cargo/bin/`; runs `agent-brain gc` as a subprocess
- **Cron scheduler** — uses `tokio-cron-scheduler` to run GC on configurable schedule (default: daily at 3 AM)
- **MCP server** — exposes `heart_trigger_gc` (run GC with optional params) and `heart_status` (daemon info) tools via stdio
- **CLI** — `agent-heart serve` (daemon with MCP + cron), `gc` (one-shot GC), `status` (config, schedule, last GC timestamp)
