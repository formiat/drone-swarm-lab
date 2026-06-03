use super::strategies::PlannerChoice;

#[derive(Clone, Copy)]
pub(super) enum RunMode {
    Smoke,       // 1 seed
    Quick,       // 10 seeds (default)
    Full,        // 1000 seeds
    Custom(u64), // User-specified seed count
}

impl RunMode {
    fn seed_count(self) -> u64 {
        match self {
            Self::Smoke => 1,
            Self::Quick => 10,
            Self::Full => 1000,
            Self::Custom(seed_count) => seed_count,
        }
    }

    pub(super) fn uses_full_profile_matrix(self) -> bool {
        matches!(self, Self::Full) || self.seed_count() > 10
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mission {
    Coverage,
    EmergencyMesh,
    Sar,
    Inspection,
    Wildfire,
    UrbanPatrol,
    UrbanSearch,
}

fn parse_mission(arg: &str) -> Vec<Mission> {
    match arg {
        "coverage" => vec![Mission::Coverage],
        "emergency-mesh" => vec![Mission::EmergencyMesh],
        "sar" => vec![Mission::Sar],
        "inspection" => vec![Mission::Inspection],
        "wildfire" => vec![Mission::Wildfire],
        "urban-patrol" => vec![Mission::UrbanPatrol],
        "urban-search" => vec![Mission::UrbanSearch],
        "urban" => vec![Mission::UrbanPatrol, Mission::UrbanSearch],
        "all" => vec![
            Mission::Coverage,
            Mission::EmergencyMesh,
            Mission::Sar,
            Mission::Inspection,
            Mission::Wildfire,
        ],
        _ => {
            panic!("unknown mission: {arg}. Valid: coverage, emergency-mesh, sar, inspection, wildfire, urban-patrol, urban-search, urban, all")
        }
    }
}

pub(super) struct CliArgs {
    pub(super) mode: RunMode,
    pub(super) missions: Vec<Mission>,
    pub(super) json_path: Option<String>,
    pub(super) csv_path: Option<String>,
    pub(super) replay_log_dir: Option<String>,
    pub(super) run_id_prefix: Option<String>,
    pub(super) scenario_suite_path: Option<String>,
    pub(super) output_dir: Option<String>,
    pub(super) report_path: Option<String>,
    /// Limit rayon parallelism; `None` uses all available CPUs.
    pub(super) jobs: Option<usize>,
    /// Route planner for bundle ordering (CBBA only).
    pub(super) planner: PlannerChoice,
    /// Run regression suites instead of normal benchmark.
    pub(super) regression: bool,
    /// Compare current run against a baseline file.
    pub(super) compare_baseline: Option<String>,
    /// Write current run as new baseline.
    pub(super) update_baseline: Option<String>,
    /// Enable M31 realism preset (wind, pose noise, comms jitter, battery model v2).
    pub(super) realism: bool,
    /// Realism profile: light, medium, or heavy (default: medium).
    pub(super) realism_profile: Option<String>,
    /// Optional comma-separated profile filter for targeted benchmark runs.
    pub(super) profiles_filter: Option<Vec<String>>,
    /// Optional degradation sweep preset name.
    pub(super) degradation: Option<String>,
}

pub(super) fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut cli = CliArgs {
        mode: RunMode::Quick,
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
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--smoke" => cli.mode = RunMode::Smoke,
            "--quick" => cli.mode = RunMode::Quick,
            "--full" => cli.mode = RunMode::Full,
            "--seeds" => {
                i += 1;
                if i < args.len() {
                    let seed_count = args[i].parse::<u64>().unwrap_or_else(|_| {
                        eprintln!(
                            "Invalid --seeds value '{}'. Expected positive integer.",
                            args[i]
                        );
                        std::process::exit(1);
                    });
                    if seed_count == 0 {
                        eprintln!("Invalid --seeds value '0'. Expected positive integer.");
                        std::process::exit(1);
                    }
                    cli.mode = RunMode::Custom(seed_count);
                }
            }
            "--mission" => {
                i += 1;
                if i < args.len() {
                    cli.missions = parse_mission(&args[i]);
                }
            }
            "--json" => {
                i += 1;
                if i < args.len() {
                    cli.json_path = Some(args[i].clone());
                }
            }
            "--csv" => {
                i += 1;
                if i < args.len() {
                    cli.csv_path = Some(args[i].clone());
                }
            }
            "--replay-log" => {
                i += 1;
                if i < args.len() {
                    cli.replay_log_dir = Some(args[i].clone());
                }
            }
            "--run-id-prefix" => {
                i += 1;
                if i < args.len() {
                    cli.run_id_prefix = Some(args[i].clone());
                }
            }
            "--scenario-suite" => {
                i += 1;
                if i < args.len() {
                    cli.scenario_suite_path = Some(args[i].clone());
                }
            }
            "--output-dir" => {
                i += 1;
                if i < args.len() {
                    cli.output_dir = Some(args[i].clone());
                }
            }
            "--report" => {
                i += 1;
                if i < args.len() {
                    cli.report_path = Some(args[i].clone());
                }
            }
            "--jobs" => {
                i += 1;
                if i < args.len() {
                    cli.jobs = args[i].parse::<usize>().ok();
                }
            }
            "--planner" => {
                i += 1;
                if i < args.len() {
                    cli.planner = match args[i].as_str() {
                        "nn" | "nearest-neighbour" => PlannerChoice::NearestNeighbour,
                        "two-opt" | "2opt" => PlannerChoice::TwoOpt,
                        "battery-aware" | "battery" => PlannerChoice::BatteryAware,
                        _ => {
                            eprintln!(
                                "Unknown planner '{}'. Valid: nn, two-opt, battery-aware",
                                args[i]
                            );
                            std::process::exit(1);
                        }
                    };
                }
            }
            "--regression" => cli.regression = true,
            "--realism" => cli.realism = true,
            "--realism-profile" => {
                i += 1;
                if i < args.len() {
                    cli.realism_profile = Some(args[i].clone());
                }
            }
            "--profiles" => {
                i += 1;
                if i < args.len() {
                    let profiles = args[i]
                        .split(',')
                        .map(str::trim)
                        .filter(|profile| !profile.is_empty())
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>();
                    if profiles.is_empty() {
                        eprintln!("Invalid --profiles value. Expected comma-separated names.");
                        std::process::exit(1);
                    }
                    cli.profiles_filter = Some(profiles);
                }
            }
            "--degradation" => {
                i += 1;
                if i < args.len() {
                    cli.degradation = Some(args[i].clone());
                }
            }
            "--compare-baseline" => {
                i += 1;
                if i < args.len() {
                    cli.compare_baseline = Some(args[i].clone());
                }
            }
            "--update-baseline" => {
                i += 1;
                if i < args.len() {
                    cli.update_baseline = Some(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    cli
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_urban_expands_to_urban_patrol_and_search() {
        assert_eq!(
            parse_mission("urban"),
            vec![Mission::UrbanPatrol, Mission::UrbanSearch]
        );
    }

    #[test]
    fn mission_all_remains_legacy_non_urban_suite() {
        let missions = parse_mission("all");

        assert!(missions.contains(&Mission::Coverage));
        assert!(missions.contains(&Mission::EmergencyMesh));
        assert!(missions.contains(&Mission::Sar));
        assert!(missions.contains(&Mission::Inspection));
        assert!(missions.contains(&Mission::Wildfire));
        assert!(!missions.contains(&Mission::UrbanPatrol));
        assert!(!missions.contains(&Mission::UrbanSearch));
    }
}
