# DRONE_C.25 - итоговые milestones: Urban, MAVLink, PX4/ArduPilot, swarm

Дата фиксации: 2026-06-03

Источник: сравнение `DRONE_A.24.md`, `DRONE_B.24.md`,
`DRONE_C.24.md` и последующее обсуждение.

## Executive Summary

Лучший итоговый план - не один из документов A/B/C.24 целиком, а синтез:

- `DRONE_C.24` дает лучший порядок зависимостей: сначала command IR, затем
  MAVLink compiler, затем compatibility profiles, затем missions/swarm/evidence.
- `DRONE_B.24` дает важную конкретику, которую нельзя потерять:
  geo-referenced Urban graph, multi-agent deconfliction, MAVLink geofence
  upload, FC parameter management, synchronized GCS swarm commands, mothership,
  transport abstraction.
- `DRONE_A.24` хорошо формулирует компактную hardware-facing границу, но его
  milestone-линейка слишком сжата для следующей фазы.

Итоговая стратегия:

```text
Build a real pre-hardware mission/supervisor platform:
Mission Command IR -> MAVLink Common plan -> PX4/ArduPilot compatibility
-> Urban real mission pack -> multi-agent deconfliction -> safety/FC config
-> swarm command plane -> topologies -> dual SITL/transport -> evidence pack.
```

Главное ограничение остается прежним:

```text
Do not implement a flight controller.
Do implement mission intent, command planning, safety gates, artifacts,
Urban mission semantics, MAVLink compatibility and swarm coordination.
```

## Architectural Boundary

### Autopilot owns

PX4, ArduPilot or another flight controller owns:

- stabilization;
- attitude/rate control;
- motor output;
- low-level waypoint following;
- estimator/EKF/local position;
- onboard failsafes;
- airframe-specific tuning;
- real flight mode implementation.

### This project owns

This workspace owns or should own:

- high-level mission commands;
- mission sequencing;
- Urban route and task semantics;
- task allocation/reallocation;
- command planning and upload planning;
- preflight validation;
- geofence/parameter intent and validation;
- supervisor decisions;
- swarm ownership and command fanout;
- replay, metrics, reports and evidence.

### Important distinction

`MAVLink Common` is a protocol-level starting point, not proof of identical
behavior across autopilots. PX4 and ArduPilot both use MAVLink, but differ in:

- mode switching;
- takeoff behavior;
- mission start behavior;
- accepted frames;
- parameter names;
- failsafe semantics;
- ACK and timeout behavior;
- geofence support details.

Therefore the project should compile to a generic MAVLink Common plan first,
then pass that plan through explicit PX4/ArduPilot capability profiles.

## Non-Goals

The following are not the main path for this phase:

- MCU/driver code for unknown hardware;
- motor control or low-level offboard control loops;
- real lidar/CV/SLAM;
- certified obstacle avoidance;
- RF mesh firmware or real RF propagation model;
- vendor SDK as the central abstraction;
- production safety certification;
- public semver API promise;
- hardware-readiness claims from dry-run/SITL alone.

## Final Milestone Chain

```text
M80 Mission Command IR
  -> M81 MAVLink Common Compiler
    -> M82 PX4 / ArduPilot Capability Profiles
      -> M83 Primitive Real Mission Pack
        -> M84 Urban Geo + Real Mission Pack
          -> M85 Urban Multi-Agent Deconfliction
            -> M86 MAVLink Safety + FC Config
              -> M87 Swarm Command Plane
                -> M88 Swarm Topologies: GCS, P2P, Mothership, Relay/Mesh
                  -> M89 Dual SITL + Transport Abstraction
                    -> M90 Hardware-Entry Evidence Pack
```

Why this order:

1. Without `Mission Command IR`, every backend will leak into mission logic.
2. Without `MAVLink Common Compiler`, missions remain simulation-only.
3. Without capability profiles, PX4/ArduPilot support becomes a hidden claim.
4. Without primitive real missions, Urban has no simple command lifecycle proof.
5. Without Urban geo/mission pack, the project lacks a realistic applied domain.
6. Without Urban deconfliction, "swarm in a city" is only allocation, not motion
   coordination.
7. Without geofence/param handling, hardware-facing safety remains too shallow.
8. Without swarm command plane, the project becomes single-drone automation.
9. Without topology modeling, P2P/mothership/relay/mesh stays conceptual.
10. Without dual SITL/transport abstraction, integration becomes PX4-local.
11. Without evidence pack, hardware entry remains ad-hoc.

---

## M80 - Mission Command IR

### Goal

Create a backend-neutral command representation for real drone mission actions.

This is not MAVLink yet and not a simulator API. It is the stable intermediate
representation:

```text
Mission intent -> Mission Command IR -> backend compiler/profile/executor
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

2. Explicit command metadata:
   - command id;
   - mission id;
   - agent id where relevant;
   - coordinate frame;
   - altitude reference;
   - units;
   - timeout policy;
   - completion tolerance;
   - expected terminal state.

3. Validation:
   - reject non-finite coordinates;
   - reject invalid altitude/duration/radius;
   - reject route without waypoints;
   - reject ambiguous coordinate frames;
   - reject duplicate command ids;
   - reject impossible orbit parameters.

4. Integration points:
   - Scenario DSL can reference command sequences;
   - Urban route export can produce `follow_route`;
   - preflight can validate command route/altitude;
   - replay can record command lifecycle events;
   - artifacts can preserve IR before backend compilation.

### Non-Goals

- No MAVLink serialization.
- No PX4/ArduPilot mode behavior.
- No hardware connection.
- No vendor SDK integration.

### Done Criteria

- Command IR exists as typed, serializable data.
- Validation covers all initial primitives.
- Urban route can be represented as `follow_route` without MAVLink fields.
- Dry-run artifact can include command IR summary.
- Docs state that IR is mission intent, not hardware execution.

### Automated Tests

Tests that need no refactoring:

- serialization roundtrip for every command primitive;
- invalid altitude/duration/radius/coordinate validation;
- command ordering is stable;
- route waypoint order is preserved;
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

Compile `Mission Command IR` into typed MAVLink Common command/mission plans
without binding mission logic to one autopilot.

Target shape:

```text
Mission Command IR -> MavlinkCommonPlan
```

### Scope

1. Supported command mapping:
   - `arm` / `disarm` -> `MAV_CMD_COMPONENT_ARM_DISARM`;
   - `takeoff` -> `MAV_CMD_NAV_TAKEOFF`;
   - `land` -> `MAV_CMD_NAV_LAND`;
   - `return_to_launch` -> `MAV_CMD_NAV_RETURN_TO_LAUNCH`;
   - `go_to` / `follow_route` -> `MISSION_ITEM_INT`
     with `MAV_CMD_NAV_WAYPOINT`;
   - `loiter_time` -> `MAV_CMD_NAV_LOITER_TIME`;
   - `pause` / `resume` / `abort` -> explicit command plan plus backend policy.

2. Plan phases:
   - command prelude;
   - mission upload items;
   - mission start command;
   - expected ACKs;
   - expected telemetry milestones;
   - retry policy;
   - timeout policy;
   - abort/fallback plan.

3. Artifact output:
   - source mission id;
   - command IR hash;
   - MAVLink command list;
   - mission item list;
   - expected ACK sequence;
   - backend profile name;
   - unsupported/degraded features;
   - validation result.

4. Unsupported behavior:
   - return structured compiler errors;
   - never silently invent stack-specific behavior;
   - record direct/fallback/unsupported decisions in artifact.

### Non-Goals

- No real upload.
- No serial/UDP transport.
- No complete MAVLink dialect implementation.
- No claim that Common semantics are identical across PX4/ArduPilot.

### Done Criteria

- Supported primitives compile to deterministic typed MAVLink plans.
- Unsupported primitives produce structured errors.
- Plans include expected ACK/telemetry contract.
- Artifact validator can inspect command and mission item structure.
- Docs list supported Common commands.

### Automated Tests

Tests that need no refactoring:

- `takeoff` compiles to `MAV_CMD_NAV_TAKEOFF`;
- `land` compiles to `MAV_CMD_NAV_LAND`;
- route compiles to ordered `MISSION_ITEM_INT`;
- unsupported command returns structured error;
- expected ACK list is deterministic.

Tests that need light refactoring:

- artifact validator checks MAVLink plan fields;
- dry-run CLI emits MAVLink plan artifact;
- preflight report links violations to command ids.

Tests that need heavy refactoring:

- backend-neutral MAVLink message model if current SITL structures are too
  narrow;
- streaming mission upload state machine tests;
- golden artifact schema versioning.

---

## M82 - PX4 / ArduPilot Capability Profiles

### Goal

Make stack compatibility explicit.

The project should answer:

```text
Is this command supported by this autopilot, in this mode, with this frame,
with these parameters and fallback rules?
```

### Scope

1. Initial profiles:
   - `mavlink_common_generic`;
   - `px4_multicopter`;
   - `ardupilot_copter`.

2. Profile data:
   - supported commands;
   - supported coordinate frames;
   - required mode transitions;
   - mission start semantics;
   - takeoff/landing constraints;
   - loiter/orbit support;
   - geofence support;
   - parameter support;
   - known caveats.

3. Compatibility classification:
   - `supported`;
   - `supported_with_caveats`;
   - `supported_via_fallback`;
   - `requires_stack_specific_mapping`;
   - `unsupported`;
   - `unknown_until_sitl_or_hardware`.

4. Compiler behavior:
   - generic Common plan first;
   - profile pass annotates or rejects;
   - profile may add stack-specific annotations;
   - profile cannot silently change mission semantics.

### Non-Goals

- No exhaustive autopilot certification.
- No vendor SDK.
- No unsupported command shim that pretends to work.
- No version-specific profile registry yet unless cheap.

### Done Criteria

- PX4 and ArduPilot profiles exist as data/config.
- Compiler output includes compatibility classification.
- Docs expose Common/PX4/ArduPilot differences.
- Unsupported/unknown behavior blocks hardware-facing artifact success unless
  explicitly accepted as dry-run-only.

### Automated Tests

Tests that need no refactoring:

- profile marks supported primitive commands correctly;
- unknown command is not treated as supported;
- unsupported frame fails compatibility pass;
- caveat text appears in artifact;
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

These missions are intentionally simple. Their value is command lifecycle
discipline, not visual simulation.

### Scope

1. `takeoff-hold-land`:

```text
arm -> takeoff(3m) -> hold(10s) -> land
```

2. `takeoff-orbit-land`:

```text
arm -> takeoff(3m) -> orbit(center=current, radius=1m, turns=3) -> land
```

3. `takeoff-square-land`:

```text
arm -> takeoff(3m) -> follow_route(square) -> land
```

4. Each mission defines:
   - command sequence;
   - expected ACKs;
   - expected telemetry milestones;
   - timeout policy;
   - abort policy;
   - safety/preflight checks;
   - artifact output.

5. Portability handling:
   - orbit may be direct, waypoint fallback or unsupported;
   - local frame behavior may be unknown without SITL/hardware;
   - landing completion may be profile-specific.

### Non-Goals

- No real flight.
- No assumption that orbit works identically on PX4 and ArduPilot.
- No external dependency on connected vehicle.

### Done Criteria

- Three primitive missions compile to MAVLink Common plans.
- PX4/ArduPilot profiles classify each mission.
- Artifact validator validates each mission artifact.
- Docs explain what can be validated without hardware and what cannot.

### Automated Tests

Tests that need no refactoring:

- takeoff-hold-land command order;
- takeoff-orbit-land command order;
- square route command order;
- timeout/abort policy present for every mission;
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

## M84 - Urban Geo + Real Mission Pack

### Goal

Make Urban the primary realistic mission setting and connect it to real command
plans.

This milestone merges two necessary tracks:

```text
geo-referenced Urban graph + Urban mission templates -> command IR -> MAVLink plan
```

### Scope

1. Geo-referenced Urban graph:
   - optional `geo` point on Urban nodes;
   - if `geo` exists, export WGS84 waypoints directly;
   - if no `geo`, keep current local-with-origin behavior;
   - reject mixed geo/non-geo maps unless explicitly normalized.

2. GeoJSON utility:
   - parse small stable LineString/Point fixtures;
   - store `geo` in `UrbanNode`;
   - compute local pose approximation for backward-compatible simulation;
   - document that this is a utility, not full GIS/OSM engine.

3. Urban mission templates:
   - `urban-block-patrol`;
   - `urban-perimeter-loop`;
   - `urban-search-target`;
   - `urban-blocked-route-response`;
   - `urban-inspection-corridor`.

4. Mock perception:
   - target/bus detection event is explicit scenario input;
   - temporary blocked route is explicit scenario input;
   - independent judge can validate mission-level outcome;
   - no real CV/lidar claim.

5. MAVLink output:
   - command IR;
   - MAVLink Common plan;
   - route id and metadata;
   - profile compatibility warnings;
   - dry-run artifact.

### Non-Goals

- No full OSM parser.
- No polygon/navmesh engine.
- No real obstacle avoidance.
- No terrain/elevation model.
- No real perception implementation.

### Done Criteria

- Geo-referenced Urban graph exports WGS84 waypoints.
- Non-geo Urban graph behavior remains backward compatible.
- At least one real city fixture loads and exports.
- Urban patrol/search/block missions compile to command IR and MAVLink plans.
- Docs separate mission-level reactivity from physical obstacle avoidance.

### Automated Tests

Tests that need no refactoring:

- geo node export uses node lat/lon directly;
- mixed geo/non-geo map fails validation;
- local node export remains unchanged;
- Urban patrol produces deterministic waypoint order;
- mocked bus/target detection changes mission outcome.

Tests that need light refactoring:

- GeoJSON fixture builder;
- scenario DSL fixtures for Urban command missions;
- artifact validator checks Urban route metadata.

Tests that need heavy refactoring:

- property tests for local/geo projection roundtrip;
- full dry-run through SITL-adjacent pipeline;
- richer map import pipeline for real city fragments.

---

## M85 - Urban Multi-Agent Deconfliction

### Goal

Add mission-level movement coordination for multiple agents on the same Urban
graph.

This is not certified collision avoidance. It is deterministic segment/route
deconfliction for known routes:

```text
two agents want same segment -> one owns it -> one waits/replans -> replay explains
```

### Scope

1. Segment reservation:
   - edge/segment lock;
   - owner agent;
   - acquired tick;
   - release on segment completion;
   - timeout or stale-lock policy.

2. Right-of-way policies:
   - first come;
   - priority;
   - round robin;
   - emergency/abort priority if practical.

3. Replay events:
   - lock acquired;
   - lock released;
   - conflict detected;
   - agent waits;
   - agent replans;
   - deadlock/timeout if any.

4. Metrics:
   - conflict count;
   - wait ticks;
   - segment utilization;
   - deadlock/timeout count;
   - mission delay caused by deconfliction.

5. Scenario fixtures:
   - two agents crossing one segment;
   - priority conflict;
   - repeated conflict;
   - single-agent backward compatibility.

### Non-Goals

- No physical collision avoidance.
- No multi-altitude airspace model.
- No real-time distributed consensus.
- No RF-dependent right-of-way policy.

### Done Criteria

- Two agents cannot occupy the same locked Urban segment simultaneously.
- Replay contains conflict/wait/lock events.
- Metrics are non-zero on conflict fixtures.
- Single-agent scenarios remain unchanged.
- Deconfliction policy is configured, not hardcoded.

### Automated Tests

Tests that need no refactoring:

- segment lock is exclusive;
- first-come policy respects arrival order;
- priority policy selects higher-priority agent;
- lock releases after segment completion;
- conflict replay event is emitted.

Tests that need light refactoring:

- multi-agent Urban scenario builder;
- segment-lock assertion helper;
- replay fixture for deconfliction events.

Tests that need heavy refactoring:

- property tests over random small graphs;
- stress test with 8 agents;
- temporal route reservation model.

---

## M86 - MAVLink Safety + FC Config

### Goal

Move from software-only preflight to a stronger hardware-facing safety/config
contract.

This milestone combines:

- MAVLink geofence upload;
- FC parameter read/write;
- param/fence snapshots in artifacts.

### Scope

1. Geofence upload planning:
   - circle inclusion/exclusion;
   - polygon inclusion/exclusion;
   - MAVLink fence mission items where supported;
   - `MAV_CMD_DO_FENCE_ENABLE` after successful upload;
   - dry-run fence summary.

2. Software-side safety remains:
   - route inside geofence;
   - no forbidden edge/zone;
   - altitude bounds;
   - finite coordinates;
   - no duplicate ownership;
   - blocked edge policy.

3. FC parameter management:
   - read one param;
   - write one param;
   - read all params if practical;
   - timeout/retry policy;
   - known-param registry for PX4 and ArduPilot names.

4. Param requirements:
   - declare expected bounds;
   - dry-run skips hardware reads with warning;
   - execute/SITL mode validates when connection exists;
   - artifact records param snapshot.

5. Profile integration:
   - geofence support marked per profile;
   - param names marked per profile;
   - unsupported stack behavior is explicit.

### Non-Goals

- No certified geofence enforcement claim.
- No runtime geofence breach handling beyond supervisor policy.
- No full FC configuration management system.
- No param backup/restore/migration system.

### Done Criteria

- Geofence plan appears in MAVLink artifact.
- Fence upload failure aborts mission in execute/SITL path.
- Param read/write mocks pass.
- Dry-run handles param requirements without hardware.
- Artifacts include fence and param snapshots where applicable.

### Automated Tests

Tests that need no refactoring:

- circle fence compiles to expected fence item;
- polygon fence compiles to expected vertex items;
- fence enable command appears after fence plan;
- param requirement passes within bounds;
- param requirement fails outside bounds;
- param snapshot roundtrips JSON.

Tests that need light refactoring:

- mock transport fence capture helper;
- mock transport param fixture helper;
- artifact validator checks fence and param sections.

Tests that need heavy refactoring:

- full fence upload state machine;
- local PX4/SITL fence acceptance check;
- ArduPilot legacy fence/parameter compatibility tests.

---

## M87 - Swarm Command Plane

### Goal

Move from single-drone command plans to coordinated multi-agent command plans.

This is where the project keeps its value as a swarm project:

```text
one supervisor mission -> N per-agent command plans -> ownership/replacement/abort
```

### Scope

1. Agent roles:
   - scout;
   - observer;
   - relay;
   - mothership/coordinator;
   - reserve;
   - recovery/return agent.

2. Command fanout:
   - per-agent command sequence;
   - per-agent expected ACKs;
   - per-agent telemetry milestones;
   - per-agent abort policy;
   - global abort policy.

3. Ownership:
   - task ownership;
   - route/segment ownership;
   - target ownership;
   - replacement mission ownership;
   - handoff/reassignment events.

4. Swarm command types:
   - arm all;
   - takeoff all;
   - start all;
   - abort all;
   - replace one agent mission;
   - release unfinished work.

5. Reports:
   - per-agent status;
   - all/partial/failed aggregate status;
   - command latency;
   - replacement cause;
   - replay events.

### Non-Goals

- No drone-to-drone RF protocol.
- No certified synchronization.
- No real-time formation control.
- No distributed consensus guarantee.

### Done Criteria

- Swarm mission produces one command plan per assigned agent.
- Duplicate ownership fails validation.
- Lost-agent recovery releases unfinished commands/tasks.
- Replacement mission compiles through M81/M82.
- Abort-all emits per-agent abort commands.

### Automated Tests

Tests that need no refactoring:

- command fanout creates one plan per assigned agent;
- duplicate ownership fails validation;
- failed agent triggers replacement policy;
- abort-all emits per-agent abort commands;
- replay records ownership handoff.

Tests that need light refactoring:

- multi-agent command fixture builder;
- artifact validator checks per-agent command sections;
- metrics include per-agent command success/failure.

Tests that need heavy refactoring:

- transport-independent swarm executor;
- concurrent supervisor actor model;
- CBBA/gossip integration through command-plane events.

---

## M88 - Swarm Topologies: GCS, P2P, Mothership, Relay/Mesh

### Goal

Represent practical swarm topologies at the coordination layer without claiming
to implement radio hardware.

Topology should affect mission decisions:

```text
topology -> allowed command routing -> supervisor policy -> artifacts/replay
```

### Scope

1. Centralized GCS:
   - ground supervisor dispatches commands;
   - all agents report to GCS;
   - lost link degrades or aborts according to policy.

2. P2P logical topology:
   - peer coordination events are allowed;
   - useful for CBBA/gossip research;
   - transport remains abstract.

3. Mothership/carrier:
   - deploy phase;
   - child mission activation;
   - recovery/collect phase;
   - abort policy for carrier failure, child failure and unrecovered child.

4. Relay/mesh coordination:
   - relay role influences command availability/routing;
   - link state is modeled input;
   - no RF propagation claim;
   - no mission-planner claim for real RF mesh placement.

5. Topology artifacts:
   - topology type;
   - link assumptions;
   - command route;
   - degraded transitions;
   - topology-specific caveats.

### Non-Goals

- No RF mesh stack.
- No antenna/RSSI/interference model.
- No physical docking/deployment mechanism.
- No guaranteed lost-link safety beyond modeled policy.

### Done Criteria

- Topology config exists and affects supervisor decisions.
- Centralized, P2P, mothership and relay/mesh fixtures exist.
- Mothership deploy/recover appears in replay.
- Mesh/relay documented as coordination abstraction, not RF truth.
- Artifacts record topology and caveats.

### Automated Tests

Tests that need no refactoring:

- centralized topology dispatches through GCS;
- P2P topology permits peer coordination event;
- mothership child mission waits for deploy phase;
- relay link loss changes degraded state;
- docs smoke test for "not RF mesh implementation".

Tests that need light refactoring:

- topology fixture builders;
- replay assertions for topology caveats;
- metrics for link/topology degradation.

Tests that need heavy refactoring:

- topology-aware CBBA/gossip benchmark harness;
- transport adapter split for InMem/UDP/Serial/MAVLink routing;
- multi-agent temporal deconfliction tied to topology.

---

## M89 - Dual SITL + Transport Abstraction

### Goal

Prepare the project for both PX4 and ArduPilot SITL and prevent the command
stack from becoming accidentally tied to one transport or one autopilot.

PX4 can remain first-class because the project already has PX4/SIH history.
ArduPilot should become an explicit experimental target.

### Scope

1. PX4 path:
   - route primitive/Urban missions through M81 compiler;
   - preserve existing PX4/SIH workflows;
   - profile caveats visible in artifacts.

2. ArduPilot path:
   - ArduPilot profile;
   - ArduPilot lifecycle mapping;
   - optional auto-detect from heartbeat;
   - experimental SITL/runbook path.

3. Transport abstraction:
   - InMemory for tests;
   - UDP for local/SITL where useful;
   - serial placeholder/interface for future hardware;
   - MAVLink routing as typed transport usage, not mission logic.

4. CLI/runbook:
   - select profile;
   - select transport;
   - dry-run without transport;
   - SITL path with explicit environment assumptions;
   - no machine-specific automated test dependencies.

### Non-Goals

- No hardware serial run.
- No stable public transport API.
- No deep ArduPilot validation before actual SITL evidence.
- No real P2P radio.

### Done Criteria

- PX4 profile remains compatible with existing SITL-adjacent tests.
- ArduPilot profile/lifecycle exists and is marked experimental.
- Transport interface can be mocked in tests.
- CLI/report records selected profile and transport.
- Docs separate dry-run, SITL and hardware.

### Automated Tests

Tests that need no refactoring:

- PX4 lifecycle command sequence;
- ArduPilot lifecycle command sequence;
- heartbeat detection for PX4/ArduPilot;
- transport selection serializes in artifact;
- dry-run works without transport.

Tests that need light refactoring:

- mock transport fixture;
- CLI profile/transport parser tests;
- report validator checks profile/transport fields.

Tests that need heavy refactoring:

- local PX4 SITL command upload run;
- local ArduPilot SITL command upload run;
- transport conformance test suite.

---

## M90 - Hardware-Entry Evidence Pack

### Goal

Produce the evidence required before any future real hardware experiment.

This milestone still does not require hardware. It creates a disciplined entry
gate so that the first hardware attempt starts from documented facts rather than
ad-hoc experiments.

### Scope

1. Evidence pack schema:
   - source scenario;
   - command IR;
   - MAVLink plan;
   - capability profile;
   - preflight report;
   - fence plan;
   - param requirements/snapshot;
   - expected ACK/telemetry contract;
   - artifact validation result;
   - replay summary;
   - run command;
   - git commit;
   - known caveats.

2. Evidence pack generation:
   - primitive missions;
   - Urban missions;
   - swarm missions;
   - topology-aware missions.

3. Hardware-entry checklist:
   - selected autopilot;
   - selected airframe;
   - selected link;
   - coordinate frame/local origin policy;
   - altitude reference;
   - geofence/failsafe parameters;
   - manual abort/kill procedure;
   - first allowed mission.

4. Result classification:
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
- Primitive, Urban and swarm missions can produce packs.
- Profile caveats are visible in reports.
- Hardware-entry checklist is documented.
- The project can answer what is ready for hardware and what is not.

### Automated Tests

Tests that need no refactoring:

- evidence pack validates for primitive mission;
- evidence pack validates for Urban perimeter mission;
- evidence pack validates for swarm command mission;
- missing preflight report fails validation;
- missing caveat fails validation for caveated command;
- docs smoke test for hardware-entry checklist.

Tests that need light refactoring:

- artifact validator subcommand for evidence packs;
- report exporter for evidence pack summaries;
- replay validator integration with evidence pack.

Tests that need heavy refactoring:

- dual PX4/ArduPilot SITL evidence runs;
- mocked ACK/telemetry upload state machine;
- versioned evidence schema across all mission families.

## Recommended First Implementation Slice

Do not start with Urban geo import, geofence upload or swarm topology. The first
slice should prove the command architecture:

```text
takeoff-hold-land
  -> Mission Command IR
  -> MAVLink Common plan
  -> PX4/ArduPilot compatibility classification
  -> dry-run artifact
  -> artifact validator checks
```

This slice is small enough to implement cleanly and large enough to validate the
main architectural decision. After it works, Urban block/perimeter missions,
swarm fanout, geofence/params and topology work can reuse the same command
layer instead of creating one-off simulation behavior.

## What Changed From A/B/C.24

### Compared to DRONE_A.24

Kept:

- primitive intent first;
- MAVLink Common first;
- PX4/ArduPilot capability profiles;
- Urban as main applied setting;
- swarm command plane;
- SITL readiness.

Changed:

- expanded the chain from M80-M85 to M80-M90;
- split Urban mission pack from Urban deconfliction;
- added explicit geofence/FC param milestone;
- added explicit topology and evidence milestones.

### Compared to DRONE_B.24

Kept:

- geo-referenced Urban graph;
- Urban multi-agent deconfliction;
- MAVLink geofence upload;
- FC parameter management;
- synchronized GCS swarm commands;
- mothership/carrier;
- transport abstraction.

Changed:

- do not start with Urban geo graph;
- put command IR/compiler/profiles before Urban-specific work;
- combine geofence and params into one safety/config milestone;
- combine topology concepts into one coordination milestone.

### Compared to DRONE_C.24

Kept:

- dependency order;
- mission command IR;
- MAVLink compiler;
- capability profiles;
- primitive mission pack;
- Urban real mission pack;
- swarm command plane;
- topology abstraction;
- evidence pack.

Changed:

- add Urban geo as part of M84;
- add Urban deconfliction as M85;
- add geofence/FC config as M86;
- add dual SITL/transport as M89;
- shift evidence pack to M90.
