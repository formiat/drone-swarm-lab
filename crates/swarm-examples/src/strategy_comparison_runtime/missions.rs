use swarm_scenarios::{
    build_emergency_mesh_scenario, build_inspection_scenario, build_sar_scenario,
    build_urban_patrol_scenario, build_urban_perimeter_scenario, build_urban_search_scenario,
    build_wildfire_scenario, EmergencyMeshProfile, EmergencyMeshStandardProfiles,
    InspectionProfile, InspectionStandardProfiles, SarProfile, StandardProfiles, UrbanProfile,
    UrbanStandardProfiles, WildfireProfile,
};

use crate::regression_lib::{build_coverage_profile, ScenarioBuilder};

use super::cli::{CliArgs, Mission};

pub(super) struct MissionDescriptor {
    pub(super) mission: Mission,
    pub(super) name: &'static str,
    profiles: fn(&CliArgs) -> Vec<String>,
    builder: fn() -> ScenarioBuilder,
}

impl MissionDescriptor {
    pub(super) fn profile_names(&self, cli: &CliArgs) -> Vec<String> {
        if let Some(profiles) = &cli.profiles_filter {
            return profiles.clone();
        }
        (self.profiles)(cli)
    }

    pub(super) fn scenario_builder(&self) -> ScenarioBuilder {
        (self.builder)()
    }
}

const MISSION_DESCRIPTORS: &[MissionDescriptor] = &[
    MissionDescriptor {
        mission: Mission::Coverage,
        name: "coverage",
        profiles: coverage_profiles,
        builder: coverage_builder,
    },
    MissionDescriptor {
        mission: Mission::EmergencyMesh,
        name: "emergency-mesh",
        profiles: emergency_mesh_profiles,
        builder: emergency_mesh_builder,
    },
    MissionDescriptor {
        mission: Mission::Sar,
        name: "sar",
        profiles: sar_profiles,
        builder: sar_builder,
    },
    MissionDescriptor {
        mission: Mission::Inspection,
        name: "inspection",
        profiles: inspection_profiles,
        builder: inspection_builder,
    },
    MissionDescriptor {
        mission: Mission::Wildfire,
        name: "wildfire",
        profiles: wildfire_profiles,
        builder: wildfire_builder,
    },
    MissionDescriptor {
        mission: Mission::UrbanPatrol,
        name: "urban-patrol",
        profiles: urban_patrol_profiles,
        builder: urban_patrol_builder,
    },
    MissionDescriptor {
        mission: Mission::UrbanSearch,
        name: "urban-search",
        profiles: urban_search_profiles,
        builder: urban_search_builder,
    },
];

pub(super) fn mission_descriptor(mission: Mission) -> &'static MissionDescriptor {
    MISSION_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.mission == mission)
        .expect("mission descriptor must exist for every CLI mission")
}

fn coverage_profiles(cli: &CliArgs) -> Vec<String> {
    if cli.mode.uses_full_profile_matrix() {
        let nets = StandardProfiles::network_profiles();
        let fails = StandardProfiles::failure_profiles();
        let mut combos = Vec::new();
        for net in &nets {
            for fail in &fails {
                combos.push(format!("{}-{}", net.name, fail.name));
            }
        }
        combos
    } else {
        vec![
            "ideal-no-failures".to_owned(),
            "ideal-single-failure".to_owned(),
            "medium-loss-no-failures".to_owned(),
            "medium-loss-single-failure".to_owned(),
        ]
    }
}

fn coverage_builder() -> ScenarioBuilder {
    Box::new(|seed: u64, profile_name: &str| {
        let (scenario, mut run_config) = match profile_name {
            "m77-comms-heavy-loss" => build_coverage_profile(seed, "heavy-loss-single-failure"),
            "m77-comms-partition-prone" => {
                build_coverage_profile(seed, "partition-prone-single-failure")
            }
            "m77-cbba-heavy-loss" => build_coverage_profile(seed, "heavy-loss-single-failure"),
            other => build_coverage_profile(seed, other),
        };
        if matches!(
            profile_name,
            "m77-comms-heavy-loss" | "m77-comms-partition-prone"
        ) {
            run_config.comms_penalty_weight = 50.0;
        }
        if profile_name == "m77-cbba-heavy-loss" {
            run_config.enable_cbba = true;
        }
        (scenario, run_config)
    })
}

fn emergency_mesh_profiles(_cli: &CliArgs) -> Vec<String> {
    EmergencyMeshStandardProfiles::profile_names()
        .iter()
        .map(|name| name.to_string())
        .collect()
}

fn emergency_mesh_builder() -> ScenarioBuilder {
    Box::new(|seed: u64, profile_name: &str| {
        let profile =
            EmergencyMeshProfile::from_str(profile_name).unwrap_or(EmergencyMeshProfile::Ideal);
        build_emergency_mesh_scenario(&profile.config(seed))
    })
}

fn sar_profiles(_cli: &CliArgs) -> Vec<String> {
    vec!["ideal".to_owned(), "standard".to_owned()]
}

fn sar_builder() -> ScenarioBuilder {
    Box::new(|seed: u64, profile_name: &str| {
        let profile = if profile_name == "m77-dynamic-belief" {
            SarProfile::Standard
        } else {
            SarProfile::from_str(profile_name).unwrap_or(SarProfile::Ideal)
        };
        let (scenario, mut run_config) = build_sar_scenario(&profile.config(seed));
        if profile_name == "m77-dynamic-belief" {
            run_config.dynamic_belief_updates = true;
        }
        (scenario, run_config)
    })
}

fn inspection_profiles(_cli: &CliArgs) -> Vec<String> {
    InspectionStandardProfiles::profile_names()
        .iter()
        .map(|name| name.to_string())
        .collect()
}

fn inspection_builder() -> ScenarioBuilder {
    Box::new(|seed: u64, profile_name: &str| {
        let profile =
            InspectionProfile::from_str(profile_name).unwrap_or(InspectionProfile::Linear);
        build_inspection_scenario(&profile.config(seed))
    })
}

fn wildfire_profiles(_cli: &CliArgs) -> Vec<String> {
    vec![
        "small-static".to_owned(),
        "medium-dynamic".to_owned(),
        "large-static".to_owned(),
        "high-threat-dynamic".to_owned(),
    ]
}

fn wildfire_builder() -> ScenarioBuilder {
    Box::new(|seed: u64, profile_name: &str| {
        let profile = if profile_name == "m77-priority-realloc" {
            WildfireProfile::MediumDynamic
        } else {
            WildfireProfile::from_str(profile_name).unwrap_or(WildfireProfile::SmallStatic)
        };
        let (scenario, mut run_config) = build_wildfire_scenario(&profile.config(seed));
        if profile_name == "m77-priority-realloc" {
            run_config.wildfire_priority_realloc_threshold = Some(8);
        }
        (scenario, run_config)
    })
}

fn urban_patrol_profiles(_cli: &CliArgs) -> Vec<String> {
    UrbanStandardProfiles::patrol_profile_names()
        .iter()
        .map(|name| name.to_string())
        .collect()
}

fn urban_patrol_builder() -> ScenarioBuilder {
    Box::new(|seed: u64, profile_name: &str| {
        let profile =
            UrbanProfile::from_str(profile_name).unwrap_or(UrbanProfile::PatrolSmallBlock);
        if matches!(profile, UrbanProfile::PerimeterSquare) {
            build_urban_perimeter_scenario(&profile.config(seed))
        } else {
            build_urban_patrol_scenario(&profile.config(seed))
        }
    })
}

fn urban_search_profiles(_cli: &CliArgs) -> Vec<String> {
    UrbanStandardProfiles::search_profile_names()
        .iter()
        .map(|name| name.to_string())
        .collect()
}

fn urban_search_builder() -> ScenarioBuilder {
    Box::new(|seed: u64, profile_name: &str| {
        let profile = UrbanProfile::from_str(profile_name).unwrap_or(UrbanProfile::SearchStaticBus);
        build_urban_search_scenario(&profile.config(seed), profile)
    })
}

#[cfg(test)]
mod tests {
    use super::super::cli::RunMode;
    use super::super::strategies::PlannerChoice;
    use super::*;

    fn cli_args(mode: RunMode) -> CliArgs {
        CliArgs {
            mode,
            missions: vec![Mission::Coverage],
            json_path: None,
            csv_path: None,
            replay_log_dir: None,
            run_id_prefix: None,
            scenario_suite_path: None,
            output_dir: None,
            report_path: None,
            jobs: None,
            planner: PlannerChoice::NearestNeighbour,
            regression: false,
            compare_baseline: None,
            update_baseline: None,
            realism: false,
            realism_profile: None,
            profiles_filter: None,
            degradation: None,
        }
    }

    #[test]
    fn descriptors_cover_all_cli_missions() {
        let cli = cli_args(RunMode::Smoke);
        for mission in [
            Mission::Coverage,
            Mission::EmergencyMesh,
            Mission::Sar,
            Mission::Inspection,
            Mission::Wildfire,
            Mission::UrbanPatrol,
            Mission::UrbanSearch,
        ] {
            let descriptor = mission_descriptor(mission);
            let profiles = descriptor.profile_names(&cli);
            assert!(!descriptor.name.is_empty());
            assert!(
                !profiles.is_empty(),
                "mission {:?} must expose at least one profile",
                mission
            );
            let builder = descriptor.scenario_builder();
            let (scenario, run_config) = builder(0, &profiles[0]);
            assert!(!scenario.name.is_empty());
            assert!(run_config.max_ticks > 0);
        }
    }

    #[test]
    fn coverage_profiles_use_full_matrix_only_for_large_runs() {
        let smoke_profiles =
            mission_descriptor(Mission::Coverage).profile_names(&cli_args(RunMode::Smoke));
        let custom_profiles =
            mission_descriptor(Mission::Coverage).profile_names(&cli_args(RunMode::Custom(11)));

        assert_eq!(smoke_profiles.len(), 4);
        assert!(custom_profiles.len() > smoke_profiles.len());
    }

    #[test]
    fn explicit_profiles_filter_overrides_default_profiles() {
        let mut cli = cli_args(RunMode::Smoke);
        cli.profiles_filter = Some(vec![
            "m77-comms-heavy-loss".to_owned(),
            "m77-comms-partition-prone".to_owned(),
        ]);

        let profiles = mission_descriptor(Mission::Coverage).profile_names(&cli);

        assert_eq!(
            profiles,
            vec!["m77-comms-heavy-loss", "m77-comms-partition-prone"]
        );
    }

    #[test]
    fn m77_profile_aliases_enable_algorithm_flags() {
        let coverage = mission_descriptor(Mission::Coverage).scenario_builder();
        let (_scenario, coverage_config) = coverage(0, "m77-comms-heavy-loss");
        assert!(coverage_config.comms_penalty_weight > 0.0);

        let sar = mission_descriptor(Mission::Sar).scenario_builder();
        let (_scenario, sar_config) = sar(0, "m77-dynamic-belief");
        assert!(sar_config.dynamic_belief_updates);

        let wildfire = mission_descriptor(Mission::Wildfire).scenario_builder();
        let (_scenario, wildfire_config) = wildfire(0, "m77-priority-realloc");
        assert_eq!(wildfire_config.wildfire_priority_realloc_threshold, Some(8));
    }
}
