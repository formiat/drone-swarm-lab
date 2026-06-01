impl ScenarioRunner {
    pub fn run(scenario: &Scenario, config: RunConfig) -> RunMetrics {
        use swarm_alloc::GreedyAllocator;
        Self::run_with(scenario, config, GreedyAllocator)
    }

    pub fn run_with<A: Allocator>(
        scenario: &Scenario,
        config: RunConfig,
        allocator: A,
    ) -> RunMetrics {
        Self::run_internal(scenario, config, allocator, None).0
    }

    /// Run a scenario with optional event logging.
    ///
    /// Returns `(RunMetrics, Option<EventLog>)`. The `EventLog` is `Some` when
    /// `enable_log` is `true`. Existing callers of `run_with` are unaffected.
    pub fn run_with_log<A: Allocator>(
        scenario: &Scenario,
        config: RunConfig,
        allocator: A,
    ) -> (RunMetrics, Option<swarm_replay::EventLog>) {
        let run_id = format!("{}_{}", scenario.name, scenario.seed);
        let builder = swarm_replay::EventLogBuilder::new(run_id, scenario.seed, &scenario.name);
        Self::run_internal(scenario, config, allocator, Some(builder))
    }

    /// Build a `RunState` from the current runtime state for adapter-driven checks.
    fn build_run_state(
        grid_state: &Option<swarm_runtime::GridState>,
        inspection_state: &Option<InspectionState>,
        wildfire_state: &Option<WildfireState>,
        tasks: &[Task],
    ) -> RunState {
        let mut state = RunState::default();
        if let Some(ref gs) = grid_state {
            for (idx, cell) in gs.cells.iter().enumerate() {
                if matches!(
                    cell,
                    swarm_types::CellState::Visited { .. }
                        | swarm_types::CellState::TargetFound { .. }
                ) {
                    let x = (idx % gs.grid.width as usize) as u32;
                    let y = (idx / gs.grid.width as usize) as u32;
                    state.scanned_cells.insert((x, y));
                }
            }
        }
        if let Some(ref is) = inspection_state {
            for edge_id in &is.covered {
                state.covered_edges.insert(edge_id.clone());
            }
        }
        if let Some(ref ws) = wildfire_state {
            for zone in &ws.mapped_zone_ids {
                state.mapped_zones.insert(zone.clone());
            }
        }
        // A task is "complete" for adapter purposes when it has been assigned or explicitly
        // completed. SAR/Inspection/Wildfire adapters use scanned_cells/covered_edges/
        // mapped_zones; only coverage-type adapters rely on completed_tasks. Treating
        // assigned tasks as complete here enables CoverageAdapter to report early-exit.
        for task in tasks {
            if task.assigned_to.is_some()
                || matches!(task.status, swarm_types::TaskStatus::Completed)
            {
                state.completed_tasks.insert(task.id.clone());
            }
        }
        state
    }

    /// Check adapter-driven mission completion.
    /// Returns true if all tasks with a known kind are completed according to their adapter.
    fn adapter_driven_complete(
        tasks: &[Task],
        run_state: &RunState,
        registry: &AdapterRegistry,
    ) -> bool {
        tasks.iter().filter(|t| t.kind.is_some()).all(|task| {
            if let Some(adapter) = registry.for_task(task) {
                adapter.is_completed(task, run_state)
            } else {
                true // no adapter for this kind → assume complete (or skip)
            }
        })
    }

}
