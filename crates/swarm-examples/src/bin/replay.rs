use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut log_path: Option<String> = None;
    let mut summary = false;
    let mut tick: Option<u64> = None;
    let mut follow = false;
    let mut sitl_summary_path: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--log" => {
                i += 1;
                if i < args.len() {
                    log_path = Some(args[i].clone());
                }
            }
            "--summary" => summary = true,
            "--tick" => {
                i += 1;
                if i < args.len() {
                    tick = args[i].parse().ok();
                }
            }
            "--follow" => follow = true,
            "--sitl-summary" => {
                i += 1;
                if i < args.len() {
                    sitl_summary_path = Some(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(path) = sitl_summary_path {
        if log_path.is_some() || summary || tick.is_some() || follow {
            eprintln!(
                "--sitl-summary cannot be combined with --log, --summary, --tick, or --follow"
            );
            std::process::exit(1);
        }
        let log = match swarm_examples::sitl_observability::read_sitl_event_log(Path::new(&path)) {
            Ok(log) => log,
            Err(error) => {
                eprintln!("Failed to read SITL replay log: {error}");
                std::process::exit(1);
            }
        };
        let summary = swarm_examples::sitl_observability::summarize_sitl_event_log(&log);
        println!(
            "{}",
            swarm_examples::sitl_observability::format_sitl_summary(&summary)
        );
        return;
    }

    let path = match log_path {
        Some(p) => p,
        None => {
            eprintln!("Usage: replay --log <path> [--summary] [--tick N] [--follow] | replay --sitl-summary <path>");
            std::process::exit(1);
        }
    };

    let log = match swarm_replay::read_from_file(Path::new(&path)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to read replay log: {}", e);
            std::process::exit(1);
        }
    };

    println!(
        "Replay log: {} (schema: {})",
        log.run_id, log.schema_version
    );
    println!("Scenario: {} | Seed: {}", log.scenario_name, log.seed);
    println!("Events: {}", log.events.len());

    if summary {
        let s = swarm_replay::summarize(&log);
        println!("\n=== Summary ===");
        println!("Total ticks: {}", s.total_ticks);
        println!("Assignments: {}", s.assignments);
        println!("Completions: {}", s.completions);
        println!("Failures: {}", s.failures);
        println!("Messages sent: {}", s.messages_sent);
        println!("Messages dropped: {}", s.messages_dropped);
        println!("Safety violations: {}", s.safety_violations);
        println!("SAR scans: {}", s.sar_scans);
        println!("SAR detections: {}", s.sar_detections);
        println!("Edges visited: {}", s.edges_visited);
        println!("CBBA convergences: {}", s.cbba_convergence_ticks.len());
        if !s.cbba_convergence_ticks.is_empty() {
            println!("  Convergence ticks: {:?}", s.cbba_convergence_ticks);
        }
    }

    if let Some(t) = tick {
        let snap = swarm_replay::snapshot_at_tick(&log, t);
        println!("\n=== Snapshot at tick {} ===", t);
        println!("Active agents: {}", snap.active_agents.len());
        println!("Failed agents: {}", snap.failed_agents.len());
        println!("Assigned tasks: {}", snap.assigned_tasks.len());

        // Determine grid bounds from agent poses
        if !snap.agent_poses.is_empty() {
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for (_, pose) in &snap.agent_poses {
                min_x = min_x.min(pose.x);
                max_x = max_x.max(pose.x);
                min_y = min_y.min(pose.y);
                max_y = max_y.max(pose.y);
            }
            // Add padding
            let pad = ((max_x - min_x).max(max_y - min_y) * 0.2).max(1.0);
            let grid = swarm_replay::render_ascii_grid(
                &snap,
                (min_x - pad, max_x + pad, min_y - pad, max_y + pad),
                20,
            );
            println!("\n{}", grid);
        } else {
            println!("No agent poses available for grid rendering.");
        }
    }

    if follow {
        let max_tick = log
            .events
            .iter()
            .filter_map(|e| match e {
                swarm_replay::Event::TickStart { tick } => Some(*tick),
                _ => None,
            })
            .max()
            .unwrap_or(0);

        for t in 0..=max_tick {
            let snap = swarm_replay::snapshot_at_tick(&log, t);
            if !snap.agent_poses.is_empty() {
                let mut min_x = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for (_, pose) in &snap.agent_poses {
                    min_x = min_x.min(pose.x);
                    max_x = max_x.max(pose.x);
                    min_y = min_y.min(pose.y);
                    max_y = max_y.max(pose.y);
                }
                let pad = ((max_x - min_x).max(max_y - min_y) * 0.2).max(1.0);
                let grid = swarm_replay::render_ascii_grid(
                    &snap,
                    (min_x - pad, max_x + pad, min_y - pad, max_y + pad),
                    20,
                );
                println!("\n{}", grid);
            }
        }
    }

    if !summary && tick.is_none() && !follow {
        eprintln!("No action specified. Use --summary, --tick N, or --follow.");
        std::process::exit(1);
    }
}
