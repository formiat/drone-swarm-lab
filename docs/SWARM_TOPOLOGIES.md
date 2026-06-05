# Logical Swarm Topologies

M88 adds logical swarm topology contracts to the mission-level command plane.
It covers centralized GCS, P2P, relay, mesh, and mothership coordination
patterns at artifact level.
The topology layer answers one question:

```text
Can the supervisor route this command to this agent under the declared
coordination topology?
```

In short: no RF mesh and no production radio routing. It does not implement RF mesh networking, radio firmware, packet routing,
distributed consensus, physical collision avoidance, hardware readiness, or
physical mothership deployment/recovery.

## Schema

M88 is additive inside `swarm_command_plane.v1`.

`SwarmCommandPlan` now includes:

- `topology`: a logical `SwarmTopologyConfig`;
- `command_routes`: one deterministic `SwarmCommandRoute` per agent command
  target;
- summary counters for topology kind, node count, link count, route count,
  degraded route count, and mothership dependency count.

`multi_sitl.v1` configs may optionally include the same `topology` section. If
omitted, the command-plane builder creates a centralized GCS topology with a
logical route from `gcs` to every agent.

## Topology Kinds

| Kind | Meaning |
|---|---|
| `centralized_gcs` | The supervisor/GCS is the command source. Commands route from `gcs` to each agent node. |
| `p2p_logical` | Explicit logical peer links may carry command routes. This is still a coordination artifact, not RF behavior. |
| `relay` | Commands may route through declared relay nodes when a direct route is unavailable or undesirable. |
| `mesh` | Commands route over explicit logical links with deterministic BFS. Unavailable links produce blocked/degraded route evidence. |
| `mothership` | Parent/child command dependencies are recorded at mission level. This does not create physical deploy/recover commands. |

## Route Decisions

Routes are computed deterministically:

- neighbor expansion is stable by node id;
- missing agent nodes produce blocked routes;
- unavailable links are ignored;
- a route with no path is recorded as `allowed=false`, `degraded=true`, and
  a topology-specific reason such as `mesh_partition_or_blocked_link`;
- successful routes include `via_node_ids` so replay and artifact validators
  can explain the command path.

## Transport Assumptions

Every topology has explicit `transport` assumptions:

- `delivery_model`: for example `in_memory`, `logical`, `legacy_udp`, or
  `future_mavlink_router`;
- optional delay/drop annotations;
- `hardware_boundary`, which must state that the artifact is not a hardware RF
  guarantee.

The old UDP prototype remains a legacy/test transport. M88 does not promote it
to a production radio layer.

## Replay Events

Generic replay and SITL logs can now record:

- `SwarmTopologyConfigured`;
- `SwarmCommandRouteSelected`;
- `SwarmCommandRouteBlocked`;
- `SwarmTopologyDegraded`;
- `SwarmMothershipDependencyRecorded`.

These events are observational. They explain command-plane routing decisions;
they do not change simulation physics or prove vehicle behavior.

## Artifact Validation

Strict current supervisor artifacts validate:

- topology summary matches the full `command_plane_artifact`;
- every manifest agent has a route decision;
- blocked routes include a reason;
- topology nodes and link endpoints are known;
- mothership dependencies reference known agents and are acyclic;
- transport assumptions include an explicit hardware boundary.

Historical artifacts can still be validated in historical mode.

## Portable Fixtures

The repository includes small `multi_sitl.v1` config fixtures:

- `scenarios/sitl.multi-agent.topology.centralized.json`;
- `scenarios/sitl.multi-agent.topology.p2p.json`;
- `scenarios/sitl.multi-agent.topology.relay.json`;
- `scenarios/sitl.multi-agent.topology.mesh-partition.json`;
- `scenarios/sitl.multi-agent.topology.mothership.json`.

They are portable fixtures for dry-run/mock manifest and artifact checks. They
do not require PX4, ArduPilot, network sockets, serial ports, real vehicles, or
external simulators.

## Test Boundary

M88 is covered by unit/integration/doc tests around routing, serialization,
validation, replay summaries, SITL event summaries, and artifact validation.

No 500/1000-seed benchmark, PX4/SIH live run, Gazebo/HIL run, or hardware test
is required for M88.
