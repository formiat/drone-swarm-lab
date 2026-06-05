use std::collections::HashMap;

use derive_more::{AsRef, Deref, DerefMut, From, Into};
use serde::{Deserialize, Serialize};

use crate::mavlink_capability_profile::MavlinkCapabilityProfileId;

/// Opaque FC parameter identifier.
#[derive(
    AsRef, Deref, DerefMut, From, Into, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct FcParamId(String);

/// MAVLink parameter value type.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FcParamValue {
    Int32(i32),
    Float(f32),
}

/// Requirement range for a single FC parameter.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FcParamRange {
    ExactInt(i32),
    ExactFloat(f32),
    IntBounds { min: i32, max: i32 },
    FloatBounds { min: f32, max: f32 },
}

/// One required parameter with validation range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamRequirement {
    pub param_id: FcParamId,
    pub required_range: FcParamRange,
    pub reason: String,
}

/// Point-in-time parameter snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamSnapshot {
    /// key: `param_id`
    pub params: HashMap<FcParamId, FcParamValue>,
    pub description: String,
}

/// Metadata for a known FC parameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FcKnownParam {
    pub id: &'static str,
    pub stack: MavlinkCapabilityProfileId,
    pub units: &'static str,
    pub range: Option<FcParamRange>,
    pub default_value: Option<FcParamValue>,
    pub caveats: &'static [&'static str],
}

/// Plan to read a set of parameters before mission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamReadPlan {
    pub param_ids: Vec<FcParamId>,
    pub rationale: String,
}

/// Plan to write/verify parameters before mission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamWritePlan {
    /// value: `(param_id, required_value)`
    pub writes: Vec<(FcParamId, FcParamValue)>,
    pub rationale: String,
}

/// Aggregate result of validating requirements against a snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcParamValidationResult {
    pub violations: Vec<FcParamViolation>,
    pub checked_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FcParamViolation {
    RequiredParamMissing {
        param_id: FcParamId,
    },
    ParamOutOfRange {
        param_id: FcParamId,
        actual: FcParamValue,
        range_description: String,
    },
}

pub static FC_KNOWN_PARAMS_PX4: &[FcKnownParam] = &[
    FcKnownParam {
        id: "GF_ACTION",
        stack: MavlinkCapabilityProfileId::Px4,
        units: "enum",
        range: Some(FcParamRange::IntBounds { min: 0, max: 5 }),
        default_value: Some(FcParamValue::Int32(1)),
        caveats: &["0=None, 1=Warning, 2=Hold, 3=RTL, 4=Terminate, 5=Land"],
    },
    FcKnownParam {
        id: "GF_MAX_HOR_DIST",
        stack: MavlinkCapabilityProfileId::Px4,
        units: "m",
        range: Some(FcParamRange::FloatBounds {
            min: 0.0,
            max: 10_000.0,
        }),
        default_value: Some(FcParamValue::Float(0.0)),
        caveats: &["0=disabled"],
    },
    FcKnownParam {
        id: "COM_ARM_WO_GPS",
        stack: MavlinkCapabilityProfileId::Px4,
        units: "bool",
        range: Some(FcParamRange::IntBounds { min: 0, max: 1 }),
        default_value: Some(FcParamValue::Int32(0)),
        caveats: &["Allows arming without GPS; use with caution in geofenced missions"],
    },
    FcKnownParam {
        id: "EKF2_AID_MASK",
        stack: MavlinkCapabilityProfileId::Px4,
        units: "bitmask",
        range: None,
        default_value: None,
        caveats: &["Controls EKF2 sensor fusion; required bits depend on mission"],
    },
];

pub static FC_KNOWN_PARAMS_ARDUPILOT: &[FcKnownParam] = &[
    FcKnownParam {
        id: "FENCE_ACTION",
        stack: MavlinkCapabilityProfileId::ArduPilot,
        units: "enum",
        range: Some(FcParamRange::IntBounds { min: 0, max: 4 }),
        default_value: Some(FcParamValue::Int32(0)),
        caveats: &["0=Report, 1=RTL, 2=Hold, 3=SmartRTL, 4=Brake"],
    },
    FcKnownParam {
        id: "FENCE_ALT_MAX",
        stack: MavlinkCapabilityProfileId::ArduPilot,
        units: "m",
        range: Some(FcParamRange::FloatBounds {
            min: 10.0,
            max: 1000.0,
        }),
        default_value: Some(FcParamValue::Float(100.0)),
        caveats: &["Maximum altitude for ArduPilot altitude fence"],
    },
    FcKnownParam {
        id: "FENCE_RADIUS",
        stack: MavlinkCapabilityProfileId::ArduPilot,
        units: "m",
        range: Some(FcParamRange::FloatBounds {
            min: 30.0,
            max: 10_000.0,
        }),
        default_value: Some(FcParamValue::Float(300.0)),
        caveats: &["Circular radius fence; 0=disabled"],
    },
];

/// Validate one parameter requirement against a snapshot.
pub fn check_param_requirement(
    snapshot: &FcParamSnapshot,
    req: &FcParamRequirement,
) -> Result<(), FcParamViolation> {
    let Some(actual) = snapshot.params.get(&req.param_id).copied() else {
        return Err(FcParamViolation::RequiredParamMissing {
            param_id: req.param_id.clone(),
        });
    };
    if value_matches_range(actual, req.required_range) {
        Ok(())
    } else {
        Err(FcParamViolation::ParamOutOfRange {
            param_id: req.param_id.clone(),
            actual,
            range_description: range_description(req.required_range),
        })
    }
}

/// Validate all requirements; returns aggregate result.
pub fn validate_param_requirements(
    snapshot: &FcParamSnapshot,
    requirements: &[FcParamRequirement],
) -> FcParamValidationResult {
    let violations = requirements
        .iter()
        .filter_map(|req| check_param_requirement(snapshot, req).err())
        .collect();
    FcParamValidationResult {
        violations,
        checked_count: requirements.len(),
    }
}

/// Build a read plan covering all required param IDs.
pub fn read_plan_from_requirements(
    requirements: &[FcParamRequirement],
    rationale: impl Into<String>,
) -> FcParamReadPlan {
    let mut param_ids = Vec::new();
    for requirement in requirements {
        if !param_ids.contains(&requirement.param_id) {
            param_ids.push(requirement.param_id.clone());
        }
    }
    FcParamReadPlan {
        param_ids,
        rationale: rationale.into(),
    }
}

pub fn range_description(range: FcParamRange) -> String {
    match range {
        FcParamRange::ExactInt(value) => format!("exact int {value}"),
        FcParamRange::ExactFloat(value) => format!("exact float {value}"),
        FcParamRange::IntBounds { min, max } => format!("int bounds {min}..={max}"),
        FcParamRange::FloatBounds { min, max } => format!("float bounds {min}..={max}"),
    }
}

fn value_matches_range(value: FcParamValue, range: FcParamRange) -> bool {
    match (value, range) {
        (FcParamValue::Int32(actual), FcParamRange::ExactInt(expected)) => actual == expected,
        (FcParamValue::Float(actual), FcParamRange::ExactFloat(expected)) => actual == expected,
        (FcParamValue::Int32(actual), FcParamRange::IntBounds { min, max }) => {
            (min..=max).contains(&actual)
        }
        (FcParamValue::Float(actual), FcParamRange::FloatBounds { min, max }) => {
            (min..=max).contains(&actual)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param_id(value: &str) -> FcParamId {
        FcParamId::from(value.to_owned())
    }

    fn snapshot(param_id: FcParamId, value: FcParamValue) -> FcParamSnapshot {
        FcParamSnapshot {
            params: [(param_id, value)].into(),
            description: "test snapshot".to_owned(),
        }
    }

    #[test]
    fn param_requirement_passes_within_int_bounds() {
        let param_id = param_id("GF_ACTION");
        let req = FcParamRequirement {
            param_id: param_id.clone(),
            required_range: FcParamRange::IntBounds { min: 0, max: 5 },
            reason: "geofence action must be configured".to_owned(),
        };

        assert_eq!(
            check_param_requirement(&snapshot(param_id, FcParamValue::Int32(3)), &req),
            Ok(())
        );
    }

    #[test]
    fn param_requirement_fails_outside_int_bounds() {
        let param_id = param_id("GF_ACTION");
        let req = FcParamRequirement {
            param_id: param_id.clone(),
            required_range: FcParamRange::IntBounds { min: 0, max: 5 },
            reason: "geofence action must be configured".to_owned(),
        };

        assert!(matches!(
            check_param_requirement(&snapshot(param_id, FcParamValue::Int32(10)), &req),
            Err(FcParamViolation::ParamOutOfRange { .. })
        ));
    }

    #[test]
    fn param_requirement_missing_returns_error() {
        let req = FcParamRequirement {
            param_id: param_id("GF_ACTION"),
            required_range: FcParamRange::ExactInt(1),
            reason: "required".to_owned(),
        };

        assert!(matches!(
            check_param_requirement(
                &FcParamSnapshot {
                    params: HashMap::new(),
                    description: "empty".to_owned(),
                },
                &req,
            ),
            Err(FcParamViolation::RequiredParamMissing { .. })
        ));
    }

    #[test]
    fn param_snapshot_roundtrip_json() {
        let value = snapshot(param_id("GF_ACTION"), FcParamValue::Int32(1));

        let json = serde_json::to_string(&value).unwrap();
        let parsed: FcParamSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, value);
    }

    #[test]
    fn exact_int_requirement_passes() {
        let param_id = param_id("GF_ACTION");
        let req = FcParamRequirement {
            param_id: param_id.clone(),
            required_range: FcParamRange::ExactInt(2),
            reason: "required".to_owned(),
        };

        assert_eq!(
            check_param_requirement(&snapshot(param_id, FcParamValue::Int32(2)), &req),
            Ok(())
        );
    }

    #[test]
    fn exact_int_requirement_fails() {
        let param_id = param_id("GF_ACTION");
        let req = FcParamRequirement {
            param_id: param_id.clone(),
            required_range: FcParamRange::ExactInt(2),
            reason: "required".to_owned(),
        };

        assert!(matches!(
            check_param_requirement(&snapshot(param_id, FcParamValue::Int32(3)), &req),
            Err(FcParamViolation::ParamOutOfRange { .. })
        ));
    }

    #[test]
    fn float_bounds_include_edges() {
        let param_id = param_id("GF_MAX_HOR_DIST");
        let req = FcParamRequirement {
            param_id: param_id.clone(),
            required_range: FcParamRange::FloatBounds {
                min: 0.0,
                max: 100.0,
            },
            reason: "range".to_owned(),
        };

        assert_eq!(
            check_param_requirement(&snapshot(param_id, FcParamValue::Float(100.0)), &req),
            Ok(())
        );
    }
}
