use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use swarm_types::{Pose, UrbanBus, UrbanBusId, UrbanDetectorConfig, UrbanSearchState};

#[derive(Clone, Debug, PartialEq)]
pub struct UrbanBusObservation {
    pub bus_id: UrbanBusId,
    pub pose: Pose,
    pub distance_m: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UrbanDetectionOutcome {
    pub observations: Vec<UrbanBusObservation>,
    pub detection: Option<UrbanBusObservation>,
    pub false_positive: bool,
}

/// Evaluate the mocked distance-based Urban Search detector for one tick.
pub fn detect_buses(
    agent_pose: Pose,
    tick: u64,
    scenario_seed: u64,
    search_state: &UrbanSearchState,
) -> UrbanDetectionOutcome {
    let mut observations: Vec<UrbanBusObservation> = search_state
        .buses
        .iter()
        .filter(|bus| bus_is_active(bus, tick))
        .filter_map(|bus| {
            let distance_m = agent_pose.distance_to(&bus.pose);
            (distance_m <= search_state.detector.detection_range_m).then(|| UrbanBusObservation {
                bus_id: bus.id.clone(),
                pose: bus.pose,
                distance_m,
            })
        })
        .collect();
    observations.sort_by(|left, right| left.bus_id.as_ref().cmp(right.bus_id.as_ref()));

    let detection = observations
        .iter()
        .enumerate()
        .find(|(index, _)| {
            deterministic_probability_draw(
                &search_state.detector,
                scenario_seed,
                tick,
                *index as u64,
                0xD37E_C710_0000_0001,
            ) < search_state.detector.detection_probability
        })
        .map(|(_, observation)| observation.clone());

    let false_positive = detection.is_none()
        && deterministic_probability_draw(
            &search_state.detector,
            scenario_seed,
            tick,
            observations.len() as u64,
            0xFA15_EF05_1717_0001,
        ) < search_state.detector.false_positive_rate;

    UrbanDetectionOutcome {
        observations,
        detection,
        false_positive,
    }
}

fn bus_is_active(bus: &UrbanBus, tick: u64) -> bool {
    bus.active_from_tick.is_none_or(|from| tick >= from)
        && bus.active_until_tick.is_none_or(|until| tick <= until)
}

fn deterministic_probability_draw(
    detector: &UrbanDetectorConfig,
    scenario_seed: u64,
    tick: u64,
    draw_index: u64,
    salt: u64,
) -> f64 {
    let seed = detector.seed
        ^ scenario_seed.rotate_left(13)
        ^ tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ draw_index.wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ salt;
    let mut rng = StdRng::seed_from_u64(seed);
    rng.gen()
}
