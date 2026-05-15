use derive_more::{AsRef, Deref, DerefMut, From, Into};

/// A discrete simulation time unit.
#[derive(
    AsRef, Deref, DerefMut, From, Into, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct Tick(u64);

/// Deterministic simulation clock driven by discrete ticks.
///
/// Tick duration is fixed at construction; no wall-clock time is used.
#[derive(Debug)]
pub struct Clock {
    current: Tick,
    tick_duration_ms: u64,
}

impl Clock {
    /// Create a new clock; `tick_duration_ms` sets how many milliseconds each tick represents.
    pub fn new(tick_duration_ms: u64) -> Self {
        Self {
            current: Tick(0),
            tick_duration_ms,
        }
    }

    /// Return the current tick counter.
    pub fn now(&self) -> Tick {
        self.current
    }

    /// Advance the clock by one tick.
    pub fn advance(&mut self) {
        self.current = Tick(self.current.0 + 1);
    }

    /// Return total elapsed simulation time in milliseconds.
    pub fn elapsed_ms(&self) -> u64 {
        self.current.0 * self.tick_duration_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_starts_at_zero() {
        let clock = Clock::new(100);
        assert_eq!(clock.now(), Tick(0));
    }

    #[test]
    fn clock_advance_increments() {
        let mut clock = Clock::new(100);
        clock.advance();
        assert_eq!(clock.now(), Tick(1));
    }

    #[test]
    fn clock_elapsed_ms() {
        let mut clock = Clock::new(100);
        clock.advance();
        clock.advance();
        clock.advance();
        assert_eq!(clock.elapsed_ms(), 300);
    }
}
