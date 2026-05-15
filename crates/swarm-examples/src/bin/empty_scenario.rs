use swarm_sim::{Clock, Scenario};

fn main() {
    let scenario = Scenario::empty("empty", 42);
    let mut clock = Clock::new(100);

    for _ in 0..10 {
        clock.advance();
    }

    println!(
        "Scenario '{}' finished: {} ticks ({} ms elapsed)",
        scenario.name,
        u64::from(clock.now()),
        clock.elapsed_ms(),
    );
}
