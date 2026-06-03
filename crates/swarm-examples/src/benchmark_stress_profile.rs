/// # Benchmark Stress Profile (formerly "realism")
///
/// Injects synthetic noise into benchmark runs to stress-test allocator
/// robustness: Gaussian pose noise, simulated wind drift, comms jitter, and a
/// simplified battery drain model.
///
/// These are **made-up numbers**, not calibrated to any real airframe or sensor.
/// They are useful for producing varied benchmark conditions and catching
/// regressions under non-ideal simulation, but they do **not** represent:
/// - real aerodynamic behavior or flight physics;
/// - real battery discharge curves or voltage sag;
/// - real RF propagation, link budget, or interference;
/// - real sensor noise characteristics.
///
/// When real hardware appears, replace this module with parameters derived from
/// actual airframe measurements rather than extending these synthetic values.
use swarm_types::BatteryModel;

/// Synthetic noise and stress parameters for benchmark runs.
///
/// Previously called `RealismProfile`; renamed to reflect that these are
/// benchmark stress knobs, not a physics model.
#[derive(Clone, Debug, PartialEq)]
pub enum RealismProfile {
    /// Minimal realism — small noise, low wind, conservative battery drain.
    Light,
    /// Moderate realism — medium noise, moderate wind, realistic battery drain.
    Medium,
    /// High realism — large noise, strong wind, aggressive battery drain.
    Heavy,
}

impl RealismProfile {
    /// Parse a profile name (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "light" => Some(Self::Light),
            "medium" => Some(Self::Medium),
            "heavy" => Some(Self::Heavy),
            _ => None,
        }
    }

    /// Return the human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Medium => "medium",
            Self::Heavy => "heavy",
        }
    }

    /// Simulation parameters for this profile.
    pub fn params(&self) -> RealismParams {
        match self {
            Self::Light => RealismParams {
                pose_noise_m: 0.2,
                wind: Some((0.05, 0.05, 0.0)),
                comms_jitter_ticks: 1,
                battery: BatteryModel {
                    hover_drain_per_tick: 0.005,
                    climb_drain_per_meter: 0.03,
                    cruise_drain_per_meter: 0.01,
                    reserve_fraction: 0.1,
                },
            },
            Self::Medium => RealismParams {
                pose_noise_m: 0.5,
                wind: Some((0.1, 0.1, 0.0)),
                comms_jitter_ticks: 1,
                battery: BatteryModel {
                    hover_drain_per_tick: 0.01,
                    climb_drain_per_meter: 0.05,
                    cruise_drain_per_meter: 0.02,
                    reserve_fraction: 0.15,
                },
            },
            Self::Heavy => RealismParams {
                pose_noise_m: 1.0,
                wind: Some((0.2, 0.2, 0.0)),
                comms_jitter_ticks: 2,
                battery: BatteryModel {
                    hover_drain_per_tick: 0.02,
                    climb_drain_per_meter: 0.08,
                    cruise_drain_per_meter: 0.03,
                    reserve_fraction: 0.2,
                },
            },
        }
    }
}

/// Concrete simulation parameters for a realism profile.
#[derive(Clone, Debug, PartialEq)]
pub struct RealismParams {
    pub pose_noise_m: f64,
    pub wind: Option<(f64, f64, f64)>,
    pub comms_jitter_ticks: u64,
    pub battery: BatteryModel,
}

/// Apply realism parameters to a `(Scenario, RunConfig)` pair.
pub fn apply_realism_preset(
    mut scenario: swarm_sim::Scenario,
    mut run_config: swarm_sim::RunConfig,
    profile: RealismProfile,
) -> (swarm_sim::Scenario, swarm_sim::RunConfig) {
    let params = profile.params();
    run_config.pose_noise_m = params.pose_noise_m;
    run_config.wind = params.wind;
    run_config.comms_jitter_ticks = params.comms_jitter_ticks;
    for agent in &mut scenario.agents {
        if agent.battery_model.is_none() {
            agent.battery_model = Some(params.battery.clone());
        }
    }
    (scenario, run_config)
}
