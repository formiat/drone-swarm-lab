use swarm_scenarios::{
    build_emergency_mesh_scenario, build_inspection_scenario, build_sar_scenario,
    build_urban_patrol_scenario, build_urban_search_scenario, build_wildfire_scenario,
    EmergencyMeshProfile, EmergencyMeshStandardProfiles, InspectionProfile,
    InspectionStandardProfiles, SarProfile, StandardProfiles, UrbanProfile, UrbanStandardProfiles,
    WildfireProfile,
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
    Box::new(|seed: u64, profile_name: &str| build_coverage_profile(seed, profile_name))
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
        let profile = SarProfile::from_str(profile_name).unwrap_or(SarProfile::Ideal);
        build_sar_scenario(&profile.config(seed))
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
        let profile =
            WildfireProfile::from_str(profile_name).unwrap_or(WildfireProfile::SmallStatic);
        build_wildfire_scenario(&profile.config(seed))
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
        build_urban_patrol_scenario(&profile.config(seed))
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
}
