//! Multi-dimensional budget vector: tokens, WASM fuel, latency (Phase 9 P1).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BudgetVector {
    pub tokens: f64,
    pub wasm_fuel: f64,
    pub latency_ms: f64,
}

impl Default for BudgetVector {
    fn default() -> Self {
        Self {
            tokens: 1.0,
            wasm_fuel: 1.0,
            latency_ms: 1.0,
        }
    }
}

impl BudgetVector {
    #[must_use]
    pub fn from_usage(tokens: u64, wasm_fuel: u64, latency_ms: u64, ceilings: Self) -> Self {
        Self {
            tokens: ratio(tokens as f64, ceilings.tokens),
            wasm_fuel: ratio(wasm_fuel as f64, ceilings.wasm_fuel),
            latency_ms: ratio(latency_ms as f64, ceilings.latency_ms),
        }
    }

    #[must_use]
    pub fn max_dimension(&self) -> f64 {
        self.tokens.max(self.wasm_fuel).max(self.latency_ms)
    }

    #[must_use]
    pub fn allows(&self, threshold: f64) -> bool {
        self.max_dimension() <= threshold
    }
}

fn ratio(used: f64, ceiling: f64) -> f64 {
    if ceiling <= 0.0 {
        return 0.0;
    }
    (used / ceiling).clamp(0.0, 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_when_any_dimension_exceeds_threshold() {
        let v = BudgetVector::from_usage(900, 100, 50, BudgetVector { tokens: 1000.0, wasm_fuel: 2000.0, latency_ms: 1000.0 });
        assert!(v.allows(1.0));
        let hot = BudgetVector::from_usage(1200, 100, 50, BudgetVector { tokens: 1000.0, wasm_fuel: 2000.0, latency_ms: 1000.0 });
        assert!(!hot.allows(1.0));
    }
}
