# M68 Urban Corridor Planner Delta

This directory captures the M68 small algorithm-delta run for commit
`87e51a9331b65278f0f1fe5503958ca2ab35a998`.

## Scope

This is a deterministic Urban mission-level planning fixture. It compares the
baseline shortest-path planner with the experimental `corridor-aware` planner
on the same road graph.

It is not a full M69 benchmark refresh, not PX4/SITL evidence, and not a
physical avoidance or hardware-readiness claim.

## Command

```bash
/home/formi/.local/bin/runlim cargo run -p swarm-examples --bin strategy_comparison -- \
  --scenario-suite scenarios/urban.corridor-delta.json \
  --output-dir results/m68_urban_corridor_delta \
  --replay-log results/m68_urban_corridor_delta/replay \
  --jobs 4
```

## Result

The run includes all default strategies because the CLI runs the full strategy
set for the suite. For this single-agent Urban Patrol fixture, the relevant
comparison is the profile/planner pair; all strategies see the same route-level
delta.

| Profile | Planner | Success | UrbanRouteLength | UrbanRisk | TimeToLoop |
|---|---|---:|---:|---:|---:|
| `urban-patrol/corridor-delta-dijkstra` | `dijkstra` | 1.000 | 40.000 | 190.000 | 10.000 |
| `urban-patrol/corridor-delta-corridor-aware` | `corridor-aware` | 1.000 | 80.000 | 70.000 | 20.000 |

The corridor-aware planner lowers `avg_urban_route_risk_score` from `190.000`
to `70.000` while keeping the run successful and violation-free. The tradeoff
is explicit: route length and completion time double on this fixture.

## Files

- `manifest.json` - run metadata, command line, commit, jobs, and build profile.
- `results.json` - machine-readable aggregate metrics.
- `results.csv` - CSV aggregate metrics.
- `table.md` - rendered aggregate table.
- `scenario_snapshot.json` - scenario suite copied at run time.
- `replay/` and `replay_logs/` - replay timelines for each run.
- `urban_analysis/` - route traces and judge reports.

## Limitations

- The route-risk metric is a deterministic proxy based on corridor width and
  static AABB obstacle clearance.
- Dynamic obstacles, lidar/CV, physical collision avoidance, multi-agent route
  deconfliction, PX4 behavior, and hardware behavior are outside this artifact.
- This evidence only supports the narrow M68 claim: on the synthetic
  `corridor-delta` map, `corridor-aware` chooses a longer lower-risk route than
  the Dijkstra baseline.
