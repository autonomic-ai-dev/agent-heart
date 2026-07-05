//! PID-style gradual throttle instead of binary budget freeze (Phase 6.1).

#[derive(Debug, Clone, Copy)]
pub struct PidState {
    pub integral: f64,
    pub last_error: f64,
}

impl Default for PidState {
    fn default() -> Self {
        Self {
            integral: 0.0,
            last_error: 0.0,
        }
    }
}

/// Returns throttle factor in `(0, 1]` — lower means slow down gradually.
#[must_use]
pub fn throttle_factor(state: &mut PidState, estimated: u64, predicted_p95: u64) -> f64 {
    if predicted_p95 == 0 {
        return 1.0;
    }
    let error = (estimated as f64 - predicted_p95 as f64) / predicted_p95 as f64;
    state.integral = (state.integral + error).clamp(-5.0, 5.0);
    let derivative = error - state.last_error;
    state.last_error = error;
    let kp = 0.35;
    let ki = 0.08;
    let kd = 0.05;
    let control = kp * error + ki * state.integral + kd * derivative;
    (1.0 / (1.0 + control.max(0.0))).clamp(0.05, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_overrun_reduces_throttle() {
        let mut state = PidState::default();
        let low = throttle_factor(&mut state, 500, 100);
        assert!(low < 0.5);
    }
}
