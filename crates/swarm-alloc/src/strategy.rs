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
        reg.register(Box::new(GreedyAllocator));
        reg.register(Box::new(AuctionAllocator::default()));
        reg.register(Box::new(ConnectivityAwareAllocator {
            base_allocator: AuctionAllocator::default(),
        }));
        reg.register(Box::new(CbbaAllocator::default()));
        reg
    }
}
