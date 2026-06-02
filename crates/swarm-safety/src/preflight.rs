use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationSeverity {
    Error,
    Warning,
}

/// A single static preflight rule violation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyViolation {
    pub rule_id: String,
    pub severity: ViolationSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affected_id: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyValidationReport {
    pub passed: bool,
    pub violations: Vec<SafetyViolation>,
}

impl SafetyValidationReport {
    pub fn ok() -> Self {
        Self {
            passed: true,
            violations: vec![],
        }
    }

    pub fn from_violations(violations: Vec<SafetyViolation>) -> Self {
        let passed = violations
            .iter()
            .all(|violation| violation.severity != ViolationSeverity::Error);
        Self { passed, violations }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violation(rule_id: &str, severity: ViolationSeverity) -> SafetyViolation {
        SafetyViolation {
            rule_id: rule_id.to_owned(),
            severity,
            affected_id: Some("wp-0".to_owned()),
            reason: "test violation".to_owned(),
        }
    }

    #[test]
    fn report_ok_when_no_violations() {
        assert!(SafetyValidationReport::ok().passed);
        assert!(SafetyValidationReport::ok().violations.is_empty());
    }

    #[test]
    fn report_fails_on_error_violation() {
        let report =
            SafetyValidationReport::from_violations(vec![violation("x", ViolationSeverity::Error)]);
        assert!(!report.passed);
    }

    #[test]
    fn report_passes_on_warning_only() {
        let report = SafetyValidationReport::from_violations(vec![violation(
            "x",
            ViolationSeverity::Warning,
        )]);
        assert!(report.passed);
    }

    #[test]
    fn violation_severity_serde_roundtrip() {
        assert_eq!(
            serde_json::to_string(&ViolationSeverity::Error).unwrap(),
            "\"error\""
        );
        assert_eq!(
            serde_json::to_string(&ViolationSeverity::Warning).unwrap(),
            "\"warning\""
        );
        assert_eq!(
            serde_json::from_str::<ViolationSeverity>("\"error\"").unwrap(),
            ViolationSeverity::Error
        );
    }
}
