# agent-heart Verification Suite

Deterministic playbook suite testing memory GC, cluster distillation, PID throttle, token anomaly budgeting, and the native **Model Usage & Budget Engine**.

## Running Verification

```bash
# Run all playbooks through the master entrypoint:
.verify/run.sh

# Or run individual playbooks:
.verify/playbooks/verify_usage_engine.sh
.verify/playbooks/verify_budget_limits.sh
.verify/playbooks/verify_full_suite.sh
```

## Playbook Matrix

| Playbook | Target Subsystem | Key Invariants Tested |
| :--- | :--- | :--- |
| `verify_usage_engine.sh` | Model Usage & Pricing | Price calculations, SQLite transactions, rolling rollups, and month-end burn rate projections |
| `verify_budget_limits.sh` | Token Budget & WASM Fuel | Anomaly spikes, PID throttling, WASM fuel p95 tracking, and multi-dimensional budget vector |
| `verify_full_suite.sh` | All agent-heart modules | 100% unit, integration, and doc tests pass with 0 warnings |
