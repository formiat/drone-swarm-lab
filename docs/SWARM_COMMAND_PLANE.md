# Swarm Command Plane

M87 adds a mission-level command plane for coordinated multi-agent missions.
M88 extends that command plane with logical topology contracts and deterministic
command-route decisions.
It connects existing assignment outputs to per-agent command plans and replayable
ownership/state transitions.

This is not drone-to-drone RF firmware, distributed consensus, low-level
collision avoidance, or a simultaneous hardware takeoff guarantee. PX4 and
ArduPilot can still interpret MAVLink commands differently; M87 only records
the command intent and policy decisions before transport-specific execution.
For topology details, see [`SWARM_TOPOLOGIES.md`](SWARM_TOPOLOGIES.md).

## Schema

Command-plane artifacts use:

```text
swarm_command_plane.v1
```

The reusable implementation lives in `swarm-command-plane`:

- `SwarmCommandPlan` is the top-level plan.
- `SwarmAgentCommandPlan` stores one per-agent `MissionCommandPlan`, the
  compiled `MavlinkCommonPlan`, expected ACKs, telemetry milestones, abort
  policy, and ownership references.
- `SwarmCommandArtifactSummary` is the compact manifest/report section used by
  `swarm-examples` artifacts.
- `SwarmTopologyConfig` and `SwarmCommandRoute` record M88 logical topology
  assumptions and route decisions inside the same additive
  `swarm_command_plane.v1` artifact.

## Roles

M87 roles are command-plane roles, not necessarily physical payload roles:

- `scout`;
- `observer`;
- `relay`;
- `leader`;
- `coordinator`;
- `mothership`;
- `carrier`;
- `reserve`;
- `recovery`.

Existing `swarm-types::Role` values are preserved. The command plane maps
existing scenario roles into M87 roles instead of changing scenario semantics.

## Fanout

`build_swarm_command_plan` accepts per-agent assignments and produces one
`SwarmAgentCommandPlan` per assigned agent. Each agent plan contains:

- hardware-agnostic M80 `MissionCommandPlan`;
- M81/M82/M86 `MavlinkCommonPlan`;
- per-agent ACK expectations;
- per-agent telemetry milestones;
- per-agent abort policy;
- ownership references.

The compiler reuses `compile_mavlink_common_plan`; it does not duplicate
MAVLink Common mapping, capability-profile checks, geofence prelude, or FC
contract handling.

## Ownership

M87 ownership is explicit and replayable:

- task ownership;
- route/segment ownership;
- target ownership;
- replacement mission ownership.

Active duplicate ownership of the same `(kind, resource_id)` is invalid unless
the transition is represented as a release/acquire handoff. This is a
coordination invariant only: it is not physical separation, RF arbitration, or
collision avoidance.

## Failure Policy

`apply_agent_failure` returns deterministic decisions:

- `abort_agent_only`;
- `abort_mission`;
- `continue_degraded`;
- `replace_from_reserve`.

`replace_from_reserve` requires a reserve or recovery agent. Missing replacement
capacity is a structured validation/failure-policy error, not an implicit
best-effort reassignment.

## Synchronized GCS Operations

M87 represents these synchronized operations:

- `arm_all`;
- `takeoff_all`;
- `start_all`;
- `abort_all`.

The current implementation includes deterministic fake evaluation for success,
failure, timeout, and partial-success policies. It does not send real
synchronized MAVLink commands and does not prove simultaneous hardware behavior.

## Replay

Generic M87 replay events explain command fanout and ownership over time:

- `swarm_command_plan_dispatched`;
- `swarm_agent_command_dispatched`;
- `swarm_ownership_acquired`;
- `swarm_ownership_released`;
- `swarm_ownership_handoff`;
- `swarm_supervisor_state_changed`;
- `swarm_sync_command_issued`;
- `swarm_sync_command_result`.

`ReplaySummary` counts command-plan dispatches, per-agent dispatches,
ownership handoffs, sync partial failures, and supervisor state changes.
M88 adds topology configured, route selected, route blocked, topology degraded,
and mothership dependency events/counters.

## Artifact Validation

`artifact_validator` knows the M87 rule ids and checks full command-plane
manifest artifacts for strict current supervisor runs. It validates the
summary/artifact identity, duplicate active ownership, missing handoff evidence
for released-to-active ownership moves, per-agent ACK consistency with compiled
MAVLink plans, and partial synchronized command results without matching command
windows. Historical supervisor artifacts remain readable without M87 sections
when validated in historical mode.

M88 strict validation additionally checks topology nodes/links, per-agent GCS
route decisions, P2P peer route decisions, allowed route paths, blocked-route
reasons, mothership dependency acyclicity, mothership child route parent
dependency, and explicit transport hardware-boundary text.

## Test Boundary

M87 is covered by portable Rust tests. No PX4, ArduPilot, Gazebo, HIL, real
hardware, long benchmark, or 1000-seed run is required for the command-plane
foundation. M88 topology checks are also portable and do not require PX4,
ArduPilot, Gazebo, HIL, real hardware, long benchmarks, or a 1000-seed run.
