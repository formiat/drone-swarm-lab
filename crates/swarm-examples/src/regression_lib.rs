use std::collections::HashMap;

use swarm_alloc::Strategy;
use swarm_scenarios::{
    build_coverage_scenario, build_emergency_mesh_scenario, build_inspection_scenario,
    build_sar_scenario, build_urban_patrol_scenario, build_urban_perimeter_scenario,
    build_urban_search_scenario, build_wildfire_scenario, CoverageConfig, EmergencyMeshProfile,
    InspectionProfile, SarProfile, StandardProfiles, UrbanProfile, WildfireProfile,
};
use swarm_sim::{
    BenchmarkHarness, BenchmarkOptions, RegressionSuite, RunConfig, Scenario, SuiteMode,
};
use swarm_types::AgentId;

use crate::realism::{apply_realism_preset, RealismProfile};

pub type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig) + Send + Sync>;
pub type StrategyFactory = Box<dyn Fn(&Scenario, &RunConfig) -> Box<dyn Strategy> + Send + Sync>;

/// Build a coverage profile from a composite profile name like "ideal-no-failures".
pub fn build_coverage_profile(seed: u64, profile_name: &str) -> (Scenario, RunConfig) {
    let parts: Vec<&str> = profile_name.split('-').collect();
    let mut net_name_parts = Vec::new();
    let mut fail_name_parts = Vec::new();
    let fail_names: Vec<&str> = StandardProfiles::failure_profiles()
        .iter()
        .map(|f| f.name)
        .collect();

    let mut found_split = false;
    for i in 0..parts.len() {
        let candidate_fail: String = parts[i..].join("-");
        if fail_names.contains(&candidate_fail.as_str()) {
            net_name_parts = parts[..i].to_vec();
            fail_name_parts = parts[i..].to_vec();
            found_split = true;
            break;
        }
    }

    if !found_split {
        net_name_parts = parts.clone();
        fail_name_parts = vec!["no-failures"];
    }

    let net_name = net_name_parts.join("-");
    let fail_name = fail_name_parts.join("-");

    let net_profile = StandardProfiles::network_profiles()
        .into_iter()
        .find(|p| p.name == net_name)
        .unwrap_or_else(|| StandardProfiles::network_profiles()[0].clone());
    let fail_profile = StandardProfiles::failure_profiles()
        .into_iter()
        .find(|p| p.name == fail_name)
        .unwrap_or_else(|| StandardProfiles::failure_profiles()[0].clone());

    let failure_tick = if fail_profile.failure_count > 0 {
        let range = fail_profile.failure_tick_range;
        range.0 + (seed % (range.1.saturating_sub(range.0) + 1).max(1))
    } else {
        999
    };

    let (scenario, mut run_config) = build_coverage_scenario(&CoverageConfig {
        seed,
        agent_count: 10,
        task_count: 15,
        failure_tick,
        packet_loss_rate: net_profile.packet_loss_rate,
        latency_ticks: net_profile.latency_ticks,
        timeout_ticks: 3,
        max_unassigned_ticks: 10,
        max_ticks: 200,
    });

    run_config.latency_per_hop = net_profile.latency_per_hop;

    if fail_profile.failure_count == 0 {
        run_config.failures.clear();
    } else if fail_profile.failure_count > 1 {
        for i in 1..fail_profile.failure_count {
            let agent_id = AgentId::from(format!("agent-{}", i % 10));
            let range = fail_profile.failure_tick_range;
            let at_tick =
                range.0 + ((seed + i as u64) % (range.1.saturating_sub(range.0) + 1).max(1));
            if at_tick < run_config.max_ticks {
                run_config
                    .failures
                    .push(swarm_sim::FailureEvent { agent_id, at_tick });
            }
        }
    }

    if net_name == "partition-prone" {
        let group_a: Vec<AgentId> = (0..5)
            .map(|i| AgentId::from(format!("agent-{i}")))
            .collect();
        let group_b: Vec<AgentId> = (5..10)
            .map(|i| AgentId::from(format!("agent-{i}")))
            .collect();
        let partition_start = 10u64;
        let partition_end = 30u64;
        for a in &group_a {
            for b in &group_b {
                run_config.partition_events.push(swarm_sim::PartitionEvent {
                    at_tick: partition_start,
                    until_tick: Some(partition_end),
                    heal_at_tick: None,
                    agents: (a.clone(), b.clone()),
                });
            }
        }
    }

    (scenario, run_config)
}

/// Build a scenario builder for a given mission name.
pub fn build_mission_scenario_builder(mission: &str) -> Option<ScenarioBuilder> {
    match mission {
        "coverage" => Some(Box::new(|seed, profile| {
            build_coverage_profile(seed, profile)
        })),
        "emergency-mesh" => Some(Box::new(|seed, profile| {
            let profile =
                EmergencyMeshProfile::from_str(profile).unwrap_or(EmergencyMeshProfile::Ideal);
            build_emergency_mesh_scenario(&profile.config(seed))
        })),
        "sar" => Some(Box::new(|seed, profile| {
            let profile = SarProfile::from_str(profile).unwrap_or(SarProfile::Ideal);
            build_sar_scenario(&profile.config(seed))
        })),
        "inspection" => Some(Box::new(|seed, profile| {
            let profile = InspectionProfile::from_str(profile).unwrap_or(InspectionProfile::Linear);
            build_inspection_scenario(&profile.config(seed))
        })),
        "wildfire" => Some(Box::new(|seed, profile| {
            let profile =
                WildfireProfile::from_str(profile).unwrap_or(WildfireProfile::SmallStatic);
            build_wildfire_scenario(&profile.config(seed))
        })),
        "urban-patrol" => Some(Box::new(|seed, profile| {
            let profile = UrbanProfile::from_str(profile).unwrap_or(UrbanProfile::PatrolSmallBlock);
            if matches!(profile, UrbanProfile::PerimeterSquare) {
                build_urban_perimeter_scenario(&profile.config(seed))
            } else {
                build_urban_patrol_scenario(&profile.config(seed))
            }
        })),
        "urban-search" => Some(Box::new(|seed, profile| {
            let profile = UrbanProfile::from_str(profile).unwrap_or(UrbanProfile::SearchStaticBus);
            build_urban_search_scenario(&profile.config(seed), profile)
        })),
        _ => None,
    }
}

/// Apply realism preset if suite requests it.
pub fn with_realism_if_needed(
    builder: ScenarioBuilder,
    suite: &RegressionSuite,
) -> ScenarioBuilder {
    if suite.realism {
        let profile = RealismProfile::Medium;
        Box::new(move |seed, profile_name| {
            let (scenario, run_config) = builder(seed, profile_name);
            apply_realism_preset(scenario, run_config, profile.clone())
        })
    } else {
        builder
    }
}

/// Run a single regression suite and return metrics map.
pub fn run_regression_suite(
    suite: &RegressionSuite,
    factories: &[StrategyFactory],
    jobs: Option<usize>,
) -> Result<HashMap<String, swarm_metrics::AggregateMetrics>, Box<dyn std::error::Error>> {
    let builder = build_mission_scenario_builder(&suite.mission)
        .ok_or_else(|| format!("Unknown mission: {}", suite.mission))?;
    let builder = with_realism_if_needed(builder, suite);

    let profile_names = vec![suite.profile.clone()];
    let result = match suite.mode {
        SuiteMode::Smoke => BenchmarkHarness::run_smoke_with_options(
            factories,
            &profile_names,
            &builder,
            BenchmarkOptions {
                prefix: Some(&suite.name),
                enable_replay_log: false,
                mission_name: &suite.mission,
                jobs,
            },
        ),
        SuiteMode::Quick => BenchmarkHarness::run_quick_with_options(
            factories,
            &profile_names,
            &builder,
            BenchmarkOptions {
                prefix: Some(&suite.name),
                enable_replay_log: false,
                mission_name: &suite.mission,
                jobs,
            },
        ),
    };

    let mut metrics_map = HashMap::new();
    for (strategy_name, _profile_name) in result.report.results.keys() {
        let key = (strategy_name.clone(), suite.profile.clone());
        if let Some(metrics) = result.report.results.get(&key) {
            metrics_map.insert(strategy_name.clone(), metrics.clone());
        }
    }
    Ok(metrics_map)
}
