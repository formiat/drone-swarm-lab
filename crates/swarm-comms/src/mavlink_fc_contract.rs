use serde::{Deserialize, Serialize};

use crate::mavlink_capability_profile::{fence_item_support_rule, MavlinkCapabilityProfileId};
use crate::mavlink_geofence::{FcGeofenceItemKind, MavlinkFencePlan};
use crate::mavlink_parameters::{
    validate_param_requirements, FcParamId, FcParamRequirement, FcParamSnapshot, FcParamValue,
    FcParamViolation,
};

/// Combined FC contract: fence upload plan + parameter requirements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcContract {
    pub profile: MavlinkCapabilityProfileId,
    pub fence_plan: Option<MavlinkFencePlan>,
    pub param_requirements: Vec<FcParamRequirement>,
}

/// Result of validating a FcContract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FcContractValidationResult {
    pub violations: Vec<FcContractViolation>,
    pub blocks_mission_start: bool,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FcContractViolation {
    UnsupportedFenceType {
        profile_id: MavlinkCapabilityProfileId,
        fence_kind: FcGeofenceItemKind,
        reason: String,
    },
    ParamOutOfRange {
        param_id: FcParamId,
        actual: FcParamValue,
        range_description: String,
    },
    RequiredParamMissing {
        param_id: FcParamId,
    },
}

/// Validate an FC contract.
pub fn validate_fc_contract(
    contract: &FcContract,
    param_snapshot: Option<&FcParamSnapshot>,
) -> FcContractValidationResult {
    let profile = contract.profile.profile();
    let mut violations = Vec::new();
    if let Some(fence_plan) = &contract.fence_plan {
        for item in &fence_plan.items {
            match fence_item_support_rule(profile, item.kind) {
                Some(rule) if !rule.classification.blocks_hardware_facing_success() => {}
                Some(rule) => violations.push(FcContractViolation::UnsupportedFenceType {
                    profile_id: contract.profile,
                    fence_kind: item.kind,
                    reason: format!(
                        "fence item is classified as {} for profile {}",
                        rule.classification.as_str(),
                        contract.profile.as_str()
                    ),
                }),
                None => violations.push(FcContractViolation::UnsupportedFenceType {
                    profile_id: contract.profile,
                    fence_kind: item.kind,
                    reason: "profile has no fence support rule for this kind".to_owned(),
                }),
            }
        }
    }

    if let Some(snapshot) = param_snapshot {
        for violation in
            validate_param_requirements(snapshot, &contract.param_requirements).violations
        {
            violations.push(match violation {
                FcParamViolation::RequiredParamMissing { param_id } => {
                    FcContractViolation::RequiredParamMissing { param_id }
                }
                FcParamViolation::ParamOutOfRange {
                    param_id,
                    actual,
                    range_description,
                } => FcContractViolation::ParamOutOfRange {
                    param_id,
                    actual,
                    range_description,
                },
            });
        }
    }

    let blocks_mission_start = !violations.is_empty();
    let summary = if blocks_mission_start {
        format!(
            "{} FC contract violation(s) block mission start",
            violations.len()
        )
    } else if param_snapshot.is_none() && !contract.param_requirements.is_empty() {
        "FC contract passed dry-run fence checks; parameter checks require a snapshot".to_owned()
    } else {
        "FC contract passed".to_owned()
    };
    FcContractValidationResult {
        violations,
        blocks_mission_start,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mavlink_geofence::{FcGeofenceItem, FcGeofenceShape};
    use crate::mavlink_parameters::{FcParamRange, FcParamValue};
    use std::collections::HashMap;

    fn polygon_contract(profile: MavlinkCapabilityProfileId) -> FcContract {
        FcContract {
            profile,
            fence_plan: Some(MavlinkFencePlan {
                items: vec![FcGeofenceItem {
                    id: "poly".to_owned(),
                    kind: FcGeofenceItemKind::PolygonInclusion,
                    shape: FcGeofenceShape::Polygon {
                        vertices: vec![(1, 2), (3, 4), (5, 6)],
                    },
                }],
                enable_fence: true,
            }),
            param_requirements: vec![FcParamRequirement {
                param_id: FcParamId::from("GF_ACTION".to_owned()),
                required_range: FcParamRange::IntBounds { min: 0, max: 5 },
                reason: "geofence action".to_owned(),
            }],
        }
    }

    fn snapshot(value: FcParamValue) -> FcParamSnapshot {
        FcParamSnapshot {
            params: [(FcParamId::from("GF_ACTION".to_owned()), value)].into(),
            description: "test".to_owned(),
        }
    }

    #[test]
    fn fc_contract_no_violations_does_not_block() {
        let result = validate_fc_contract(
            &polygon_contract(MavlinkCapabilityProfileId::Px4),
            Some(&snapshot(FcParamValue::Int32(2))),
        );

        assert!(!result.blocks_mission_start);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn unsupported_fence_type_blocks_mission() {
        let mut contract = polygon_contract(MavlinkCapabilityProfileId::ArduPilot);
        contract.fence_plan.as_mut().unwrap().items[0].kind = FcGeofenceItemKind::CircleInclusion;

        let result = validate_fc_contract(&contract, None);

        assert!(result.blocks_mission_start);
        assert!(matches!(
            result.violations[0],
            FcContractViolation::UnsupportedFenceType { .. }
        ));
    }

    #[test]
    fn param_out_of_range_blocks_mission() {
        let result = validate_fc_contract(
            &polygon_contract(MavlinkCapabilityProfileId::Px4),
            Some(&snapshot(FcParamValue::Int32(99))),
        );

        assert!(result.blocks_mission_start);
        assert!(matches!(
            result.violations[0],
            FcContractViolation::ParamOutOfRange { .. }
        ));
    }

    #[test]
    fn no_snapshot_skips_param_violations() {
        let mut contract = polygon_contract(MavlinkCapabilityProfileId::Px4);
        contract.param_requirements.push(FcParamRequirement {
            param_id: FcParamId::from("MISSING".to_owned()),
            required_range: FcParamRange::ExactInt(1),
            reason: "requires snapshot".to_owned(),
        });

        let result = validate_fc_contract(&contract, None);

        assert!(!result.blocks_mission_start);
    }

    #[test]
    fn missing_param_blocks_when_snapshot_exists() {
        let result = validate_fc_contract(
            &polygon_contract(MavlinkCapabilityProfileId::Px4),
            Some(&FcParamSnapshot {
                params: HashMap::new(),
                description: "empty".to_owned(),
            }),
        );

        assert!(result.blocks_mission_start);
        assert!(matches!(
            result.violations[0],
            FcContractViolation::RequiredParamMissing { .. }
        ));
    }
}
