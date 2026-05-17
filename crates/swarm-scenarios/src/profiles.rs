/// Network conditions for a benchmark profile.
#[derive(Clone, Debug)]
pub struct NetworkProfile {
    pub name: &'static str,
    pub packet_loss_rate: f64,
    pub latency_ticks: u64,
    pub latency_per_hop: u64,
}

/// Failure injection pattern for a benchmark profile.
#[derive(Clone, Debug)]
pub struct FailureProfile {
    pub name: &'static str,
    pub failure_count: usize,
    pub failure_tick_range: (u64, u64),
}

/// Pre-defined network and failure profiles for strategy comparison.
pub struct StandardProfiles;

impl StandardProfiles {
    pub fn network_profiles() -> Vec<NetworkProfile> {
        vec![
            NetworkProfile {
                name: "ideal",
                packet_loss_rate: 0.0,
                latency_ticks: 0,
                latency_per_hop: 0,
            },
            NetworkProfile {
                name: "light-loss",
                packet_loss_rate: 0.05,
                latency_ticks: 1,
                latency_per_hop: 0,
            },
            NetworkProfile {
                name: "medium-loss",
                packet_loss_rate: 0.15,
                latency_ticks: 1,
                latency_per_hop: 1,
            },
            NetworkProfile {
                name: "heavy-loss",
                packet_loss_rate: 0.30,
                latency_ticks: 2,
                latency_per_hop: 2,
            },
            NetworkProfile {
                name: "high-latency",
                packet_loss_rate: 0.0,
                latency_ticks: 3,
                latency_per_hop: 1,
            },
            NetworkProfile {
                name: "partition-prone",
                packet_loss_rate: 0.10,
                latency_ticks: 1,
                latency_per_hop: 1,
            },
        ]
    }

    pub fn failure_profiles() -> Vec<FailureProfile> {
        vec![
            FailureProfile {
                name: "no-failures",
                failure_count: 0,
                failure_tick_range: (0, 0),
            },
            FailureProfile {
                name: "single-failure",
                failure_count: 1,
                failure_tick_range: (5, 15),
            },
            FailureProfile {
                name: "multiple-failures",
                failure_count: 2,
                failure_tick_range: (5, 20),
            },
            FailureProfile {
                name: "cascade-failure",
                failure_count: 3,
                failure_tick_range: (5, 30),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_network_profiles_exist() {
        let nets = StandardProfiles::network_profiles();
        assert_eq!(nets.len(), 6);
        assert!(nets.iter().any(|p| p.name == "ideal"));
        assert!(nets.iter().any(|p| p.name == "heavy-loss"));
    }

    #[test]
    fn standard_failure_profiles_exist() {
        let fails = StandardProfiles::failure_profiles();
        assert_eq!(fails.len(), 4);
        assert!(fails.iter().any(|p| p.name == "no-failures"));
        assert!(fails.iter().any(|p| p.name == "cascade-failure"));
    }
}
