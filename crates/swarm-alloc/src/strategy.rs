use crate::{
    Allocator, AuctionAllocator, CbbaAllocator, ConnectivityAwareAllocator, GreedyAllocator,
};

/// A named allocation strategy that can be compared against others.
pub trait Strategy: Allocator {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
}

impl Strategy for GreedyAllocator {
    fn name(&self) -> &'static str {
        "greedy"
    }

    fn description(&self) -> &'static str {
        "Round-robin over capable agents (fast, simple)"
    }
}

impl Strategy for AuctionAllocator {
    fn name(&self) -> &'static str {
        "auction"
    }

    fn description(&self) -> &'static str {
        "Cost-minimization over distance, battery and role preference"
    }
}

impl Strategy for ConnectivityAwareAllocator {
    fn name(&self) -> &'static str {
        "connectivity-aware"
    }

    fn description(&self) -> &'static str {
        "Auction with network-availability optimization for relay placement"
    }
}

impl Strategy for CbbaAllocator {
    fn name(&self) -> &'static str {
        "cbba"
    }

    fn description(&self) -> &'static str {
        "Consensus-Based Bundle Algorithm — distributed auction with bundle building"
    }
}

/// Registry of all available strategies for benchmark harnesses.
pub struct StrategyRegistry {
    strategies: Vec<Box<dyn Strategy>>,
}

impl StrategyRegistry {
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    pub fn register(&mut self, strategy: Box<dyn Strategy>) {
        self.strategies.push(strategy);
    }

    pub fn strategies(&self) -> &[Box<dyn Strategy>] {
        &self.strategies
    }

    pub fn iter(&self) -> impl Iterator<Item = &Box<dyn Strategy>> {
        self.strategies.iter()
    }
}

impl Default for StrategyRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(GreedyAllocator::default()));
        reg.register(Box::new(AuctionAllocator::default()));
        reg.register(Box::new(ConnectivityAwareAllocator {
            base_allocator: AuctionAllocator::default(),
        }));
        reg.register(Box::new(CbbaAllocator::default()));
        reg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AllocationAgent, AllocationTask};
    use swarm_types::{AgentId, TaskId};

    #[derive(Default)]
    struct TestExtensionStrategy;

    impl Allocator for TestExtensionStrategy {
        fn allocate(
            &mut self,
            tasks: &[AllocationTask<'_>],
            agents: &[AllocationAgent],
        ) -> Vec<(TaskId, AgentId)> {
            match (tasks.first(), agents.first()) {
                (Some(task), Some(agent)) => vec![(task.task.id.clone(), agent.id.clone())],
                _ => Vec::new(),
            }
        }
    }

    impl Strategy for TestExtensionStrategy {
        fn name(&self) -> &'static str {
            "test-extension"
        }

        fn description(&self) -> &'static str {
            "Test-only extension strategy"
        }
    }

    #[test]
    fn empty_strategy_registry_is_valid() {
        let registry = StrategyRegistry::new();

        assert!(registry.strategies().is_empty());
        assert_eq!(registry.iter().count(), 0);
    }

    #[test]
    fn custom_strategy_can_be_registered_without_mutating_default_registry() {
        let mut registry = StrategyRegistry::new();
        registry.register(Box::new(TestExtensionStrategy));

        let names: Vec<_> = registry.iter().map(|strategy| strategy.name()).collect();
        assert_eq!(names, vec!["test-extension"]);
        assert_eq!(
            registry.strategies()[0].description(),
            "Test-only extension strategy"
        );

        let default_names: Vec<_> = StrategyRegistry::default()
            .iter()
            .map(|strategy| strategy.name())
            .collect();
        assert!(default_names.contains(&"greedy"));
        assert!(!default_names.contains(&"test-extension"));
    }
}
