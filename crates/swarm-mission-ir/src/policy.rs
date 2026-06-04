use serde::{Deserialize, Serialize};

/// Action taken when a command or mission times out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutAction {
    Abort,
    ReturnToLaunch,
    Hold,
}

/// Timeout configuration for a mission command plan.
///
/// Durations are in seconds (f64) for deterministic serialization.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeoutPolicy {
    /// Maximum seconds to wait for a single command to be acknowledged.
    pub command_timeout_secs: f64,
    /// Maximum seconds to wait for mission completion after the last command.
    pub completion_timeout_secs: f64,
    /// Action taken when either timeout elapses.
    pub on_timeout: TimeoutAction,
}

/// Expected state of the vehicle after the mission plan completes successfully.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalState {
    Landed,
    Hovering,
    AtWaypoint,
    OrbitComplete,
    RouteComplete,
    Aborted,
}

/// Acceptable position and altitude deviation for declaring a command complete.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompletionTolerance {
    /// Acceptable horizontal position error in metres.
    pub position_m: f64,
    /// Acceptable altitude error in metres.
    pub altitude_m: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_policy_roundtrip() {
        let policy = TimeoutPolicy {
            command_timeout_secs: 5.0,
            completion_timeout_secs: 30.0,
            on_timeout: TimeoutAction::Abort,
        };
        let json = serde_json::to_string(&policy).unwrap();
        let back: TimeoutPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, back);
    }

    #[test]
    fn terminal_state_roundtrip() {
        for state in [
            TerminalState::Landed,
            TerminalState::Hovering,
            TerminalState::AtWaypoint,
            TerminalState::OrbitComplete,
            TerminalState::RouteComplete,
            TerminalState::Aborted,
        ] {
            let json = serde_json::to_string(&state).unwrap();
            let back: TerminalState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, back);
        }
    }

    #[test]
    fn completion_tolerance_roundtrip() {
        let tol = CompletionTolerance {
            position_m: 1.0,
            altitude_m: 0.5,
        };
        let json = serde_json::to_string(&tol).unwrap();
        let back: CompletionTolerance = serde_json::from_str(&json).unwrap();
        assert_eq!(tol, back);
    }
}
