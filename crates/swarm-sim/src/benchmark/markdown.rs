use super::ComparisonReport;

impl std::fmt::Display for ComparisonReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let seeds = format!("{}-{}", self.seed_range_start, self.seed_range_end);
        writeln!(
            f,
            "| Mission | Scenario | Strategy | Profile | Seeds | Success | Completion | Detection | Realloc | Coverage | Messages | Bytes | Conflicts | Stale | BatteryMin | BatteryAvg | Availability | TimeToFind | PoD | Targets | BeliefEntropy | FalsePosRate | ConfirmationScans | ConvP50 | ConvP95 | BundleDist | EdgeCoverage | MissedEdges | Revisits | RouteEfficiency | UrbanRouteLength | UrbanRisk | UrbanPlanned | UrbanViolations | UrbanCompleted | PatrolCompleted | TimeToLoop | UrbanDistance | UrbanEfficiency | UrbanReplans | BusDetected | TimeToBus | BusFalsePos | DistanceBeforeBus | SearchSuccessNoViolation | PerimeterCompletion | PerimeterLength | TimeToPerimeter | PerimeterViolations |"
        )?;
        writeln!(
            f,
            "|---------|----------|----------|---------|-------|---------|------------|-----------|---------|----------|----------|-------|-----------|-------|------------|------------|--------------|-----------|-----|---------|---------------|--------------|-------------------|---------|---------|------------|--------------|-------------|----------|-----------------|------------------|-----------|--------------|-----------------|----------------|-----------------|------------|---------------|-----------------|--------------|-------------|-----------|-------------|-------------------|--------------------------|---------------------|-----------------|-----------------|---------------------|"
        )?;
        for strategy_name in &self.strategy_names {
            for profile_name in &self.profile_names {
                let key = (strategy_name.clone(), profile_name.clone());
                if let Some(metrics) = self.results.get(&key) {
                    let ttf = if metrics.avg_time_to_find > 0.0 {
                        format!("{:.1}", metrics.avg_time_to_find)
                    } else {
                        "-".to_owned()
                    };
                    writeln!(
                        f,
                        "| {:7} | {:8} | {:8} | {:7} | {:5} | {:7.3} | {:10.3} | {:9.3} | {:7.3} | {:8.3} | {:8.3} | {:5.0} | {:9.3} | {:5.0} | {:10.3} | {:10.3} | {:12.3} | {:>10} | {:3.3} | {:7.1} | {:13.3} | {:12.3} | {:17.3} | {:7.3} | {:7.3} | {:10.3} | {:12.3} | {:11.3} | {:8.3} | {:15.3} | {:16.3} | {:9.3} | {:12.3} | {:15.3} | {:14.3} | {:15.3} | {:10.3} | {:13.3} | {:15.3} | {:12.3} | {:11.3} | {:9.3} | {:11.3} | {:17.3} | {:24.3} | {:19.3} | {:15.3} | {:15.3} | {:19.3} |",
                        metrics.mission.as_str(),
                        metrics.scenario.as_str(),
                        strategy_name,
                        profile_name,
                        seeds,
                        metrics.success_rate,
                        metrics.avg_task_completion_rate,
                        metrics.avg_detection_ticks,
                        metrics.avg_reallocation_ticks,
                        metrics.avg_coverage_progress,
                        metrics.avg_messages_attempted,
                        metrics.avg_bytes_sent,
                        metrics.avg_conflicting_assignments,
                        metrics.avg_stale_state_age_ticks,
                        metrics.avg_battery_margin_min,
                        metrics.avg_battery_margin_avg,
                        metrics.avg_network_availability,
                        ttf,
                        metrics.avg_probability_of_detection,
                        metrics.avg_targets_found,
                        metrics.avg_belief_entropy_final,
                        metrics.avg_false_positive_rate,
                        metrics.avg_confirmation_scans,
                        metrics.convergence_ticks_p50,
                        metrics.convergence_ticks_p95,
                        metrics.avg_bundle_travel_distance,
                        // v0.16 Inspection metrics
                        metrics.avg_edge_coverage_rate,
                        metrics.avg_missed_edges,
                        metrics.avg_revisit_count,
                        metrics.avg_route_efficiency,
                        // v0.64 Urban Foundations metrics
                        metrics.avg_urban_route_length_m,
                        metrics.avg_urban_route_risk_score,
                        metrics.urban_route_planned_rate,
                        metrics.avg_urban_violation_count,
                        metrics.urban_route_completed_rate,
                        // v0.65 Urban Patrol v0 metrics
                        metrics.urban_patrol_completed_rate,
                        metrics.avg_urban_time_to_complete_loop,
                        metrics.avg_urban_distance_travelled_m,
                        metrics.avg_urban_route_efficiency,
                        metrics.avg_urban_replan_count,
                        // v0.66 Urban Search v1 metrics
                        metrics.bus_detection_rate,
                        metrics.avg_time_to_detect_bus,
                        metrics.avg_false_positive_count,
                        metrics.avg_distance_before_detection,
                        metrics.search_success_without_violation_rate,
                        // v0.75 Urban Mission Realism Follow-up metrics
                        metrics.avg_perimeter_completion_rate,
                        metrics.avg_perimeter_length_m,
                        metrics.avg_time_to_complete_perimeter,
                        metrics.avg_perimeter_violations,
                    )?;
                }
            }
        }
        Ok(())
    }
}
