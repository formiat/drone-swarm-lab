# DRONE_C.24 - milestones for Urban, MAVLink and swarm without hardware

Дата фиксации: 2026-06-03

Источник: обсуждение направления после `BEFORE_HARDWARE_A/B/C.23`,
`DRONE_A.24`, `DRONE_B.24` and current project direction.

## Executive Summary

Железа сейчас нет, конкретная аппаратная платформа неизвестна. Это не мешает
писать реальный hardware-facing код, если держать правильную границу:

```text
Do not implement a flight controller.
Implement mission intent, MAVLink-compatible command planning, safety gates,
artifacts, Urban mission semantics and swarm coordination contracts.
```

Рекомендуемое направление:

```text
Urban-first mission/supervisor platform
  + MAVLink Common command compiler
  + explicit PX4/ArduPilot capability profiles
  + swarm coordination layer
  + strict safety/artifact/evidence discipline
```

Urban стоит сделать основным прикладным полигоном, потому что он ближе всего к
реальным задачам: маршруты по известной карте, запретные зоны, временные
блокировки, периметры, патруль, поиск объекта, перераспределение задач и
supervisor decisions. Но фундамент должен быть reusable: SAR, inspection,
wildfire и другие миссии должны использовать те же mission primitives,
MAVLink compiler, safety gate and artifact format.

## Architectural Boundary

### Autopilot owns

PX4, ArduPilot or another flight controller owns:

- stabilization;
- attitude/rate control;
- motor output;
- low-level waypoint following;
- estimator/EKF/local position source;
- onboard failsafes;
- vehicle-specific mode implementation;
- real airframe-specific tuning.

### This project owns

This workspace should own:

- mission intent and mission sequencing;
- task allocation and reallocation;
- Urban route planning and mission-level decisions;
- no-fly/geofence/ownership/preflight checks;
- MAVLink command/mission/geofence/parameter planning;
- supervisor lifecycle and abort/replacement logic;
- swarm roles and command coordination;
- replay, metrics, artifacts and benchmark evidence.

### Why MAVLink Common first

MAVLink Common is the safest first hardware-facing contract because both PX4 and
ArduPilot live in the MAVLink ecosystem. But "MAVLink-compatible" does not mean
"identical autopilot behavior":

- mode switching differs;
- takeoff semantics differ;
- local frame assumptions differ;
- mission start behavior differs;
- supported frames and command parameters differ;
- ACK/failure behavior can differ;
- parameters and failsafe policies differ.

Therefore the project should use MAVLink Common as the default command
representation, then model PX4/ArduPilot differences as explicit capability
profiles instead of hidden assumptions.

## Non-Goals

Do not make these the main workstream now:

- MCU/driver code for an unknown board;
- direct motor control or control-loop logic;
- vendor SDK as the central abstraction;
- real lidar/raycast/SLAM/CV implementation;
- certified obstacle avoidance;
- real RF mesh implementation without chosen radio hardware;
- hardware readiness claims from dry-run or simulation artifacts;
- PX4-only or ArduPilot-only behavior hidden in generic mission primitives;
- production API/semver promises before the command/backend boundary stabilizes.

## Target State After This Plan

After M80-M87 the project should be able to:

- represent primitive real drone missions independent of hardware;
- compile those missions into MAVLink Common command/mission plans;
- say which parts are common, PX4-specific, ArduPilot-specific or unsupported;
- express Urban patrol/search/perimeter missions as real command plans;
- coordinate multiple drones through roles, ownership and supervisor decisions;
- model P2P/mothership/relay/mesh at the coordination layer without pretending
  to implement RF hardware;
- produce reproducible dry-run/SITL-ready artifacts with command intent,
  expected ACKs, telemetry assumptions, abort policy and validation results.

This still does not make the project production-ready. It makes it a serious
pre-hardware mission/supervisor platform, ready for controlled SITL and later
hardware integration.

## Milestone Chain

```text
M80 Mission Command IR
  -> M81 MAVLink Common Compiler
    -> M82 PX4 / ArduPilot Capability Profiles
      -> M83 Primitive Real Mission Pack
        -> M84 Urban Real Mission Pack
          -> M85 Swarm Command Plane
            -> M86 Swarm Topologies: P2P, Mothership, Relay, Mesh
              -> M87 SITL / Hardware-Entry Evidence Pack
```

The order matters:

1. Without an IR, MAVLink code will leak autopilot quirks into mission logic.
2. Without a compiler, primitive missions remain simulation-only descriptions.
3. Without capability profiles, PX4/ArduPilot compatibility will become a
   hidden claim.
4. Without primitive missions, Urban work cannot become real command work.
5. Without Urban mission pack, the project lacks a realistic applied domain.
6. Without swarm command plane, the project becomes single-drone automation.
7. Without topology modeling, "mesh/mothership/P2P" remains hand-waving.
8. Without evidence pack, there is no disciplined path to future hardware.

---

## M80 - Mission Command IR

### Goal

Create a hardware-agnostic command representation for real drone mission
actions.

This is not a simulator API and not MAVLink yet. It is a stable intermediate
representation:

```text
MissionIntent -> MissionCommand IR -> backend compiler
```

### Scope

1. Core command primitives:
   - `arm`;
   - `disarm`;
   - `takeoff(altitude_m)`;
   - `hold(duration_s)`;
   - `land`;
   - `return_to_launch`;
   - `go_to(position)`;
   - `follow_route(route_id, waypoints)`;
   - `loiter_time(duration_s)`;
   - `orbit(center, radius_m, turns, direction)`;
   - `pause`;
   - `resume`;
   - `abort`.

2. Explicit semantics:
   - coordinate frame;
   - altitude reference;
   - units;
   - timeout policy;
   - expected terminal state;
   - acceptable completion tolerance;
   - command id and mission id.

3. Validation:
   - no negative altitude where invalid;
   - no zero/negative hold duration;
   - no non-finite coordinates;
   - no ambiguous coordinate frame;
   - no impossible orbit radius/turn count;
   - no route without waypoints;
   - no duplicate command ids.

4. Integration:
   - Scenario DSL may reference command sequences;
   - Urban route export may produce `follow_route`;
   - preflight safety can validate command-level route/altitude data;
   - replay can record command lifecycle;
   - artifacts can preserve IR before backend compilation.

### Non-Goals

- No MAVLink byte/message serialization yet.
- No PX4/ArduPilot-specific mode behavior.
- No hardware execution.
- No raw vendor SDK abstraction.

### Done Criteria

- Mission command IR types exist and serialize deterministically.
- Primitive command validation is covered by tests.
- Urban route can be represented as `follow_route` without MAVLink fields.
- Dry-run artifact can include command IR summary.
- Docs explain that this is mission intent, not hardware execution.

### Automated Tests

Tests that need no refactoring:

- serialization roundtrip for all primitive commands;
- invalid altitude/duration/radius/coordinate validation tests;
- command ordering is stable;
- route waypoint ordering is preserved;
- docs smoke test for "mission intent, not hardware execution".

Tests that need light refactoring:

- Scenario DSL fixture with command sequence;
- dry-run artifact includes command IR summary;
- preflight validation consumes route and altitude from command IR.

Tests that need heavy refactoring:

- shared mission schema versioning across DSL, replay and SITL artifacts;
- typed mission/command id registry;
- reusable backend executor trait.

---

## M81 - MAVLink Common Compiler

### Goal

Compile mission command IR into MAVLink Common command/mission plans without
binding the mission layer to PX4-only or ArduPilot-only assumptions.

Target shape:

```text
MissionCommand IR -> MavlinkCommonPlan
```

### Scope

1. Compile supported primitives to MAVLink Common:
   - `arm` / `disarm` -> `MAV_CMD_COMPONENT_ARM_DISARM`;
   - `takeoff` -> `MAV_CMD_NAV_TAKEOFF`;
   - `land` -> `MAV_CMD_NAV_LAND`;
   - `return_to_launch` -> `MAV_CMD_NAV_RETURN_TO_LAUNCH`;
   - `go_to` / route waypoints -> `MISSION_ITEM_INT`
     with `MAV_CMD_NAV_WAYPOINT`;
   - `loiter_time` -> `MAV_CMD_NAV_LOITER_TIME`;
   - selected abort/pause behavior -> command plan plus backend policy.

2. Represent plan phases:
   - command prelude;
   - mission upload items;
   - mission start command;
   - expected ACKs;
   - expected telemetry milestones;
   - timeout policy;
   - abort/fallback plan.

3. Artifact shape:
   - source mission id;
   - command IR hash;
   - MAVLink command list;
   - mission item list;
   - expected ACK sequence;
   - backend profile name;
   - unsupported/degraded features;
   - validation result.

4. No byte transport requirement yet:
   - M81 may produce typed MAVLink plan data;
   - actual serial/UDP/TCP transport can stay outside this milestone.

### Non-Goals

- No actual hardware upload.
- No real serial link.
- No claim that PX4/ArduPilot semantics are identical.
- No complete MAVLink dialect implementation.

### Done Criteria

- Supported primitives compile to typed MAVLink Common plans.
- Unsupported primitives produce structured errors, not silent fallbacks.
- Plans include expected ACK/telemetry contract.
- Artifact validator can inspect command and mission item structure.
- Docs list exactly which Common commands are currently supported.

### Automated Tests

Tests that need no refactoring:

- `takeoff` compiles to `MAV_CMD_NAV_TAKEOFF`;
- `land` compiles to `MAV_CMD_NAV_LAND`;
- route compiles to ordered `MISSION_ITEM_INT` entries;
- unsupported command returns structured compiler error;
- expected ACK list is deterministic.

Tests that need light refactoring:

- artifact validator checks MAVLink plan fields;
- dry-run CLI can emit MAVLink plan artifact;
- preflight report links violations to command ids.

Tests that need heavy refactoring:

- backend-neutral MAVLink message model if current structures are too SITL-only;
- streaming mission upload state machine tests;
- golden artifact schema versioning.

---

## M82 - PX4 / ArduPilot Capability Profiles

### Goal

Make hardware-stack compatibility explicit.

The project should never say "MAVLink supports it" when the real question is:

```text
Is this command supported by this autopilot, in this mode, with this frame,
with these parameters and fallbacks?
```

### Scope

1. Capability profile model:
   - stack name: `px4`, `ardupilot`, `mavlink_common_generic`;
   - supported commands;
   - supported coordinate frames;
   - required mode transitions;
   - mission start semantics;
   - takeoff/landing constraints;
   - loiter/orbit support;
   - geofence support;
   - parameter support;
   - known caveats.

2. Compatibility classification:
   - `supported`;
   - `supported_with_caveats`;
   - `requires_stack_specific_mapping`;
   - `unsupported`;
   - `unknown_until_sitl_or_hardware`.

3. Compiler behavior:
   - generic Common plan first;
   - profile pass annotates or rejects;
   - profile may produce stack-specific command annotations;
   - profile cannot silently change mission semantics.

4. Documentation:
   - compatibility matrix;
   - exact caveats;
   - no hardware-readiness claims.

### Non-Goals

- No exhaustive autopilot certification.
- No vendor-specific SDK integration.
- No unsupported command shims that fake success.

### Done Criteria

- PX4 and ArduPilot profiles exist as data/config, not comments only.
- Compiler output includes compatibility classification.
- Docs expose Common/PX4/ArduPilot differences.
- Unsupported/unknown behavior blocks hardware-facing artifact success unless
  explicitly accepted as dry-run-only.

### Automated Tests

Tests that need no refactoring:

- profile marks supported primitive commands correctly;
- unknown command is not treated as supported;
- unsupported frame fails compatibility pass;
- caveat text appears in artifact for `supported_with_caveats`;
- docs smoke test for PX4/ArduPilot caveats.

Tests that need light refactoring:

- compatibility matrix rendered from profile data;
- artifact validator checks compatibility classification;
- dry-run CLI can select profile.

Tests that need heavy refactoring:

- SITL-backed profile conformance checks;
- autopilot-version-specific profile registry;
- parameter schema validation from real autopilot metadata.

---

## M83 - Primitive Real Mission Pack

### Goal

Implement a small set of real command missions that can compile to MAVLink
plans even without hardware.

These missions are intentionally simple. Their value is not visual simulation;
their value is command lifecycle discipline.

### Scope

1. Mission: takeoff, hold, land.

```text
arm -> takeoff(3m) -> hold(10s) -> land
```

2. Mission: takeoff, orbit, land.

```text
arm -> takeoff(3m) -> orbit(center=current, radius=1m, turns=3) -> land
```

3. Mission: takeoff, waypoint square, land.

```text
arm -> takeoff(3m) -> follow_route(square) -> land
```

4. Each mission must define:
   - command sequence;
   - expected ACKs;
   - expected telemetry milestones;
   - timeout policy;
   - abort policy;
   - safety/preflight checks;
   - artifact output.

5. If a primitive is not portable, the mission must say so:
   - orbit may be native, waypoint approximation or unsupported depending on
     profile;
   - local-frame behavior may be unknown without SITL/hardware;
   - landing completion may be profile-specific.

### Non-Goals

- No real flight.
- No claim that orbit works identically on PX4 and ArduPilot.
- No external dependency on a connected vehicle.

### Done Criteria

- Three primitive missions compile to MAVLink Common plans.
- PX4/ArduPilot profiles classify each mission.
- Artifact validator can validate each mission artifact.
- Docs explain what can be validated without hardware and what cannot.

### Automated Tests

Tests that need no refactoring:

- takeoff-hold-land command order;
- takeoff-orbit-land command order;
- square route command order;
- timeout/abort policy is present for every mission;
- profile classification exists for every mission.

Tests that need light refactoring:

- fixture-backed dry-run artifacts for all primitive missions;
- artifact validator checks expected ACK and telemetry sections;
- replay summary includes command lifecycle events.

Tests that need heavy refactoring:

- simulated ACK/telemetry state machine;
- backend executor integration tests;
- SITL execution harness for primitive missions.

---

## M84 - Urban Real Mission Pack

### Goal

Make Urban the primary realistic mission setting, using real command plans
instead of only simulation-local behavior.

Urban should become the place where project logic feels closest to a future
production use case.

### Scope

1. Urban perimeter patrol:
   - route around a block/perimeter;
   - deterministic waypoint export;
   - geofence/no-fly validation;
   - return/land policy.

2. Urban block loop:
   - "облети квартал" as a mission template;
   - polygon/perimeter converted to route over allowed graph;
   - no arbitrary physical obstacle avoidance claim;
   - blocked segment handled by mission-level wait/replan/abort policy.

3. Urban search until target:
   - "облетай квартал пока не встретишь автобус";
   - target detection is a mocked perception event;
   - command logic reacts to event by reporting, holding, returning or landing;
   - independent judge checks mission-level outcome in simulation artifacts.

4. Urban inspection corridor:
   - route follows line/corridor;
   - camera/perception remains mocked;
   - command output remains MAVLink-compatible waypoints/loiter/hold.

5. Mission result semantics:
   - success predicate;
   - partial completion;
   - target found/not found;
   - blocked route;
   - timeout;
   - abort cause.

### Non-Goals

- No real lidar.
- No real CV/bus detector.
- No certified collision avoidance.
- No GIS/navmesh engine.
- No claim that buildings/roads are physically accurate unless fixture says so.

### Done Criteria

- Urban perimeter/block/search missions are represented as command IR.
- Missions compile to MAVLink plans where supported.
- Mock perception events are explicit in scenario/artifact data.
- Route blocking produces deterministic supervisor decision.
- Replay explains route, command lifecycle and decision cause.

### Automated Tests

Tests that need no refactoring:

- perimeter mission produces deterministic waypoint order;
- block loop fails if no valid route exists;
- mocked bus detection changes mission outcome;
- blocked segment triggers configured policy;
- result semantics distinguish success, timeout and abort.

Tests that need light refactoring:

- generated Urban blocked-edge fixture feeds runtime route-decision tests;
- replay/artifact validator checks perception and route-decision events;
- scenario DSL fixtures for perimeter/search/corridor missions.

Tests that need heavy refactoring:

- multi-agent Urban deconfliction tests;
- temporal route reservation model;
- richer map import pipeline for real geo-referenced Urban graphs.

---

## M85 - Swarm Command Plane

### Goal

Move from single-drone command plans to coordinated multi-drone missions.

This is still mission-level coordination, not drone-to-drone RF firmware.

### Scope

1. Agent roles:
   - scout;
   - observer;
   - relay;
   - mothership/carrier;
   - reserve;
   - recovery/return agent.

2. Command fanout:
   - one supervisor mission;
   - per-agent command sequences;
   - per-agent ACK/telemetry expectations;
   - per-agent abort policy;
   - global mission abort policy.

3. Ownership:
   - task ownership;
   - route/segment ownership;
   - target ownership;
   - replacement mission ownership;
   - handoff/reassignment events.

4. Swarm supervisor state:
   - planned;
   - dispatched;
   - active;
   - degraded;
   - replacing;
   - aborting;
   - completed;
   - failed.

5. Communication abstraction:
   - link state as input to decisions;
   - no RF protocol implementation;
   - command delivery/retry semantics;
   - dropped/delayed command modeling for evidence runs.

### Non-Goals

- No real distributed consensus guarantee.
- No RF mesh implementation.
- No low-level collision avoidance.
- No simultaneous takeoff hardware claim.

### Done Criteria

- A swarm mission can produce N per-agent command plans.
- Per-agent failures trigger replacement or abort according to policy.
- Artifacts preserve command fanout and ownership transitions.
- Replay can explain which agent owned what and when.
- Existing allocation strategies can feed command plane assignments.

### Automated Tests

Tests that need no refactoring:

- command fanout creates one plan per assigned agent;
- duplicate ownership fails validation;
- failed agent triggers replacement policy;
- global abort emits per-agent abort commands;
- replay records ownership handoff.

Tests that need light refactoring:

- scenario fixture for scout/reserve replacement;
- artifact validator checks per-agent command sections;
- metrics include command success/failure per agent.

Tests that need heavy refactoring:

- transport-independent swarm executor;
- temporal route/segment reservation across agents;
- CBBA/gossip integration through command plane events.

---

## M86 - Swarm Topologies: P2P, Mothership, Relay, Mesh

### Goal

Represent practical swarm topologies at the coordination layer without claiming
to implement radio hardware.

The point is to make topology influence mission planning and failure handling:

```text
topology -> allowed command routing -> supervisor policy -> artifacts/replay
```

### Scope

1. Centralized GCS topology:
   - ground supervisor dispatches all commands;
   - all agents report to GCS;
   - degraded if agent link lost.

2. P2P logical topology:
   - agents can exchange coordination messages in the model;
   - transport is abstract;
   - no physical RF implementation;
   - useful for CBBA/gossip research.

3. Mothership/carrier topology:
   - carrier has dependent child agents;
   - deploy/recover phases;
   - child missions can depend on carrier position/state;
   - abort policy covers child not recovered, carrier failure, child failure.

4. Relay/mesh coordination topology:
   - relay role can influence command routing and mission availability;
   - link availability is modeled as input, not RF simulation truth;
   - no claim that mission planner solves real RF mesh placement.

5. Topology-aware artifacts:
   - topology type;
   - link assumptions;
   - command route;
   - degraded transitions;
   - topology-specific caveats.

### Non-Goals

- No radio protocol.
- No antenna/RSSI/interference model.
- No guarantee that relay placement works in physical RF.
- No certified lost-link behavior beyond modeled supervisor policy.

### Done Criteria

- Topology config exists and affects supervisor decisions.
- Centralized, P2P logical, mothership and relay/mesh fixtures exist.
- Artifacts record topology and caveats.
- Mesh/relay is clearly documented as coordination abstraction, not RF truth.
- Mothership mission can deploy/recover child agents in dry-run/sim evidence.

### Automated Tests

Tests that need no refactoring:

- centralized topology dispatches through GCS;
- P2P topology permits peer coordination event;
- mothership child mission waits for deploy phase;
- relay link loss changes availability/degraded state;
- docs smoke test for "not RF mesh implementation".

Tests that need light refactoring:

- topology fixture builders;
- replay assertions for command route and topology caveats;
- metrics for link/topology degradation.

Tests that need heavy refactoring:

- topology-aware CBBA/gossip benchmark harness;
- transport adapter split for InMem/UDP/Serial/MAVLink routing;
- multi-agent temporal deconfliction tied to topology.

---

## M87 - SITL / Hardware-Entry Evidence Pack

### Goal

Produce the evidence required before any future real hardware experiment.

This milestone does not require hardware. It creates the discipline that makes
hardware integration less ad-hoc when hardware appears.

### Scope

1. Evidence pack for every serious mission:
   - source scenario;
   - command IR;
   - MAVLink plan;
   - capability profile;
   - preflight report;
   - expected ACK/telemetry contract;
   - artifact validation result;
   - replay summary;
   - run command;
   - git commit;
   - known caveats.

2. SITL readiness:
   - PX4 SITL path remains supported where available;
   - ArduPilot SITL path can be planned or scaffolded if practical;
   - no local machine-specific paths in automated tests;
   - offline/dry-run tests remain primary.

3. Hardware-entry checklist:
   - selected autopilot;
   - selected airframe;
   - selected link;
   - selected coordinate frame/local origin policy;
   - selected altitude reference;
   - geofence/failsafe parameters;
   - manual kill/abort procedure;
   - first mission limited to primitive takeoff-hold-land or waypoint dry-run.

4. Result interpretation:
   - dry-run success;
   - SITL success;
   - unsupported command;
   - profile caveat;
   - telemetry mismatch;
   - ACK mismatch;
   - aborted run.

### Non-Goals

- No real hardware flight.
- No production certification.
- No operator training claim.
- No assumption that SITL equals real airframe behavior.

### Done Criteria

- Evidence pack schema exists and is validated.
- Primitive and Urban mission packs can produce evidence packs.
- Profile caveats are visible in reports.
- Hardware-entry checklist is documented.
- The project can answer: "what exactly is ready for hardware, and what is not?"

### Automated Tests

Tests that need no refactoring:

- evidence pack validates for primitive mission;
- evidence pack validates for Urban perimeter mission;
- missing preflight report fails validation;
- missing profile caveat fails validation when command is caveated;
- docs smoke test for hardware-entry checklist.

Tests that need light refactoring:

- artifact validator subcommand for evidence packs;
- report exporter for evidence pack summaries;
- replay validator integration with evidence pack.

Tests that need heavy refactoring:

- dual PX4/ArduPilot SITL evidence runs;
- end-to-end command upload state machine under mocked ACK/telemetry;
- versioned evidence schema across all mission families.

## Recommended Immediate Next Step

Start with M80 and M81 together as a narrow implementation slice:

```text
takeoff-hold-land mission
  -> MissionCommand IR
  -> MAVLink Common plan
  -> PX4/ArduPilot compatibility classification
  -> dry-run artifact
  -> artifact validator test
```

That slice proves the architecture without pretending to solve hardware,
perception or swarm networking too early. After that, Urban block/perimeter
missions and swarm command fanout can reuse the same command layer instead of
creating one-off simulation behavior.
