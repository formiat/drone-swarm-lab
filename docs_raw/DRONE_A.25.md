# DRONE_A.25 - итоговый план M80-M89: Urban, MAVLink Common, PX4/ArduPilot, swarm

Дата: 2026-06-03

Источник: сравнение `docs_raw/DRONE_A.24.md`,
`docs_raw/DRONE_B.24.md`, `docs_raw/DRONE_C.24.md` и текущего направления
проекта после M70-M79.

## Краткий вывод

Лучший итоговый план получается не из одной версии:

- `DRONE_C.24` даёт лучший архитектурный backbone: сначала command IR, потом
  MAVLink compiler, потом PX4/ArduPilot profiles, затем mission packs, swarm
  and evidence.
- `DRONE_B.24` даёт сильную прикладную конкретику: geo-referenced Urban graph,
  multi-agent route deconfliction, geofence upload, FC params, synchronized GCS
  commands, mothership/carrier and transport abstraction.
- `DRONE_A.24` правильно удерживает границы: no flight-controller code, no RF
  mesh implementation, no hardware-ready claims from dry-run artifacts.

Итоговая цепочка:

```text
M80 Mission Command IR
  -> M81 MAVLink Common Compiler
    -> M82 PX4 / ArduPilot Capability Profiles
      -> M83 Primitive Real Mission Pack
        -> M84 Urban Geo-Referenced Mission Pack
          -> M85 Urban Multi-Agent Deconfliction
            -> M86 MAVLink Safety / FC Contract
              -> M87 Swarm Command Plane
                -> M88 Swarm Topologies
                  -> M89 SITL Dual-Stack Evidence Pack
```

Это линейный план без выбора веток. Urban становится главным прикладным
сеттингом, но фундамент остаётся reusable для SAR, inspection, wildfire and
future missions.

## Архитектурная граница

PX4, ArduPilot or another flight controller owns:

- stabilization;
- attitude/rate control;
- motor output;
- low-level waypoint following;
- estimator/EKF/local position source;
- onboard failsafes;
- vehicle-specific mode implementation;
- airframe-specific tuning.

This project owns:

- mission command intent;
- task allocation and reallocation;
- Urban route planning and mission-level decisions;
- no-fly/geofence/ownership/preflight checks;
- MAVLink command/mission/geofence/parameter planning;
- supervisor lifecycle and abort/replacement logic;
- swarm command coordination;
- replay, metrics, artifacts and evidence.

## Non-Goals

Do not make these the main workstream:

- MCU/driver code for an unknown board;
- direct motor control or control-loop logic;
- vendor SDK as the central abstraction;
- real lidar/raycast/SLAM/CV implementation;
- certified obstacle avoidance;
- real RF mesh implementation without chosen radio hardware;
- hardware readiness claims from dry-run, benchmark or simulation artifacts;
- hidden PX4-only or ArduPilot-only behavior in generic mission primitives;
- production API/semver promises before backend boundaries stabilize.

## Target State After M89

After M80-M89 the project should be able to:

- represent primitive real drone missions independent of hardware;
- compile those missions into MAVLink Common command/mission plans;
- classify PX4/ArduPilot support explicitly;
- express Urban patrol/search/perimeter missions as real command plans;
- use real GPS coordinates in Urban graphs when provided;
- coordinate multiple drones through route ownership and supervisor decisions;
- model GCS/P2P/mothership/relay/mesh at coordination level;
- plan geofence and parameter interactions with FCs;
- produce dry-run/SITL-ready artifacts with command intent, ACK assumptions,
  telemetry milestones, abort policy and validation results.

This still does not make the project production-ready. It makes it a serious
pre-hardware mission/supervisor platform.

---

## M80 - Mission Command IR

### Goal

Create a hardware-agnostic command representation for real drone mission
actions.

This is not MAVLink yet and not a simulator-only API. It is an intermediate
representation:

```text
MissionIntent -> MissionCommand IR -> backend compiler
```

Without this step, MAVLink code will leak PX4/ArduPilot quirks into mission
logic.

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
   - command id;
   - mission id;
   - optional source task/route/agent ids.

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
- Docs explain that IR is mission intent, not hardware execution.

### Automated Tests

Tests that need no refactoring:

- serialization roundtrip for all primitive commands;
- invalid altitude/duration/radius/coordinate validation tests;
- command ordering is stable;
- route waypoint ordering is preserved;
- duplicate command ids fail validation;
- docs smoke test for "mission intent, not hardware execution".

Tests that need light refactoring:

- Scenario DSL fixture with command sequence;
- dry-run artifact includes command IR summary;
- preflight validation consumes route and altitude from command IR;
- replay event fixture for command lifecycle.

Tests that need heavy refactoring:

- shared mission schema versioning across DSL, replay and SITL artifacts;
- typed mission/command id registry;
- reusable backend executor trait.

---

## M81 - MAVLink Common Compiler

### Goal

Compile mission command IR into MAVLink Common command/mission plans without
binding mission logic to PX4-only or ArduPilot-only assumptions.

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
   - `go_to` / route waypoints -> mission items with
     `MAV_CMD_NAV_WAYPOINT`;
   - `loiter_time` -> `MAV_CMD_NAV_LOITER_TIME`;
   - `orbit` -> direct command only if profile permits, otherwise waypoint
     approximation or structured unsupported result;
   - selected abort/pause behavior -> command plan plus backend policy.

2. Represent plan phases:
   - command prelude;
   - optional geofence prelude placeholder;
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

4. No transport requirement yet:
   - M81 may produce typed MAVLink plan data;
   - actual serial/UDP/TCP transport can stay outside this milestone.

### Non-Goals

- No actual hardware upload.
- No real serial link.
- No claim that PX4/ArduPilot semantics are identical.
- No complete MAVLink dialect implementation.
- No vendor SDK wrapper.

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
- route compiles to ordered waypoint mission items;
- unsupported command returns structured compiler error;
- expected ACK list is deterministic;
- orbit fallback produces stable waypoint ordering when configured.

Tests that need light refactoring:

- artifact validator checks MAVLink plan fields;
- dry-run CLI can emit MAVLink plan artifact;
- preflight report links violations to command ids;
- golden artifact fixture for `takeoff -> hold -> land`.

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
   - stack name: `mavlink_common_generic`, `px4`, `ardupilot`;
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
   - `supported_via_fallback`;
   - `unsupported`;
   - `unknown_until_sitl_or_hardware`.

3. Compiler behavior:
   - generic Common plan first;
   - profile pass annotates or rejects;
   - profile may produce stack-specific command annotations;
   - profile cannot silently change mission semantics.

4. CLI/artifact behavior:
   - dry-run can select profile;
   - artifact records selected profile and compatibility classification;
   - unsupported/unknown behavior blocks hardware-facing success unless
     explicitly accepted as dry-run-only.

### Non-Goals

- No exhaustive autopilot certification.
- No vendor-specific SDK integration.
- No unsupported command shims that fake success.
- No version-specific profile registry in the first implementation.

### Done Criteria

- PX4 and ArduPilot profiles exist as data/config, not comments only.
- Compiler output includes compatibility classification.
- Docs expose Common/PX4/ArduPilot differences.
- Unsupported/unknown behavior is visible in artifacts.
- Existing PX4 paths remain backward compatible.

### Automated Tests

Tests that need no refactoring:

- profile marks supported primitive commands correctly;
- unknown command is not treated as supported;
- unsupported frame fails compatibility pass;
- caveat text appears in artifact for `supported_with_caveats`;
- PX4 and ArduPilot profiles classify core primitive missions;
- docs smoke test for PX4/ArduPilot caveats.

Tests that need light refactoring:

- compatibility matrix rendered or checked from profile data;
- artifact validator checks compatibility classification;
- dry-run CLI can select profile;
- existing SITL dry-run tests parameterized by profile.

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
- No Urban-specific behavior yet.

### Done Criteria

- Three primitive missions compile to MAVLink Common plans.
- PX4/ArduPilot profiles classify each mission.
- Artifact validator can validate each mission artifact.
- Docs explain what can be validated without hardware and what cannot.
- Existing dry-run behavior is not regressed.

### Automated Tests

Tests that need no refactoring:

- takeoff-hold-land command order;
- takeoff-orbit-land command order;
- square route command order;
- timeout/abort policy is present for every mission;
- profile classification exists for every mission;
- dry-run artifact roundtrip for every primitive mission.

Tests that need light refactoring:

- fixture-backed dry-run artifacts for all primitive missions;
- artifact validator checks expected ACK and telemetry sections;
- replay summary includes command lifecycle events.

Tests that need heavy refactoring:

- simulated ACK/telemetry state machine;
- backend executor integration tests;
- SITL execution harness for primitive missions.

---

## M84 - Urban Geo-Referenced Mission Pack

### Goal

Make Urban the primary realistic mission setting and connect it to real command
plans.

This milestone combines the Urban mission pack from A/C with B's
geo-referenced graph work.

### Scope

1. Geo-referenced Urban graph:
   - `UrbanNode` may carry GPS coordinates;
   - if geo is present, route export uses WGS84 directly;
   - if geo is absent, local pose + `geo_origin` behavior remains;
   - mixed geo/non-geo maps fail validation;
   - artifact records `coordinate_mode`.

2. GeoJSON import utility:
   - parse a small stable GeoJSON fixture into an Urban map;
   - support simple `Point` nodes and `LineString` edges;
   - compute local pose from geo for simulation compatibility;
   - keep this as utility/testbed, not production GIS engine.

3. Urban mission templates:
   - perimeter patrol;
   - block loop ("облети квартал");
   - search until target ("облетай квартал пока не встретишь автобус");
   - inspection corridor candidate.

4. Mission behavior:
   - known static map;
   - explicit no-fly/building/blocked-edge assumptions;
   - one altitude band unless explicitly extended;
   - mocked target detector events;
   - blocked route handled by wait/replan/abort policy;
   - independent simulation judge remains testing/evidence, not real safety.

5. MAVLink output:
   - Urban missions become command IR;
   - command IR compiles to MAVLink Common plan;
   - route/mission metadata appears in artifacts.

### Non-Goals

- No full OSM parser.
- No polygon/navmesh/geometry engine.
- No real lidar/CV/bus detector.
- No certified collision avoidance.
- No elevation model or terrain following.
- No claim that fixtures are physically accurate unless explicitly documented.

### Done Criteria

- Geo-referenced Urban graph exports correct WGS84 waypoints.
- Local-with-origin Urban graph still works unchanged.
- GeoJSON utility parses a portable fixture.
- Urban perimeter/block/search missions are represented as command IR.
- Missions compile to MAVLink plans where supported.
- Mock perception events are explicit in scenario/artifact data.
- Docs clearly distinguish mission-level reactivity from obstacle avoidance.

### Automated Tests

Tests that need no refactoring:

- geo-referenced node export uses node geo directly;
- mixed geo nodes fail validation;
- local node export remains unchanged;
- GeoJSON import roundtrip preserves coordinates within tolerance;
- perimeter mission produces deterministic waypoint order;
- mocked target detection changes mission outcome;
- blocked segment triggers configured policy.

Tests that need light refactoring:

- shared geo-node fixture builder;
- scenario DSL fixtures for perimeter/search/corridor missions;
- replay/artifact validator checks perception and route-decision events;
- artifact records `coordinate_mode`.

Tests that need heavy refactoring:

- richer map constraint model beyond current road graph;
- multi-altitude Urban airspace model;
- generalized map import pipeline for real OSM fragments;
- property tests for geo/pose roundtrip.

---

## M85 - Urban Multi-Agent Deconfliction

### Goal

Allow multiple drones to operate on the same Urban graph without simultaneous
ownership of the same route segment.

This is mission-level deconfliction, not physical collision avoidance.

### Scope

1. Segment ownership registry:
   - edge id;
   - holder agent id;
   - acquisition tick;
   - planned release condition;
   - conflict history.

2. Right-of-way policies:
   - `FirstCome`;
   - `Priority`;
   - `RoundRobin`;
   - future hook for mission-critical override.

3. Supervisor behavior:
   - reserve next segment before entering it;
   - wait/replan/abort on locked segment;
   - release segment after completion;
   - do not assign duplicate route ownership;
   - record reason for every wait/replan/abort.

4. Replay events:
   - segment lock acquired;
   - segment lock released;
   - segment conflict;
   - deconflict wait;
   - deconflict replan;
   - deconflict abort.

5. Metrics:
   - conflict count;
   - wait ticks;
   - replan count;
   - abort count;
   - segment utilization;
   - average delay per agent.

### Non-Goals

- No physical collision avoidance.
- No multi-height 3D deconfliction.
- No real-time RF coordination between drones.
- No yield policy based on external traffic or people.

### Done Criteria

- Two agents on an overlapping Urban route never own the same segment
  simultaneously.
- Replay contains lock/conflict/wait/replan events.
- All initial policies pass deterministic tests.
- Metrics are non-zero on conflict fixtures.
- Single-agent Urban scenarios remain unchanged.

### Automated Tests

Tests that need no refactoring:

- two agents requesting same segment produce exactly one lock holder;
- first-come policy respects arrival order;
- priority policy prefers higher-priority agent;
- lock releases after segment completion;
- replay contains conflict event;
- single-agent scenario has no deconfliction events.

Tests that need light refactoring:

- multi-agent Urban scenario builder;
- segment lock assertion helper;
- deconfliction replay event fixture;
- artifact validator checks route ownership records.

Tests that need heavy refactoring:

- property tests for N agents on random topology;
- stress test with 8 agents and all policy variants;
- temporal route reservation across command-plane execution.

---

## M86 - MAVLink Safety / FC Contract

### Goal

Add real hardware-facing safety and configuration planning around MAVLink
without needing a connected vehicle.

This milestone combines B's geofence upload and FC parameter management into one
coherent "FC contract" layer.

### Scope

1. Geofence upload plan:
   - circular inclusion/exclusion;
   - polygon inclusion/exclusion;
   - MAVLink Common mission/fence item representation where supported;
   - fence enable command plan;
   - dry-run fence artifact.

2. Software preflight remains authoritative before upload:
   - existing geofence/no-fly/route checks stay in project preflight;
   - FC geofence upload complements, not replaces, software validation.

3. FC parameter plan:
   - read param plan;
   - write param plan;
   - param requirement checks;
   - param snapshot artifact;
   - known param registry with stack, units, range and caveats.

4. Capability profile integration:
   - PX4/ArduPilot profiles classify geofence support;
   - PX4/ArduPilot profiles classify known params;
   - unsupported parameter/fence operations are explicit.

5. Execution boundary:
   - dry-run produces plan without FC;
   - execute mode may later call transport-specific functions;
   - failure to satisfy FC contract blocks mission start.

### Non-Goals

- No certified geofence enforcement claim.
- No full FC configuration management system.
- No param backup/restore.
- No runtime param changes during mission.
- No geofence breach handling beyond FC/project reports.

### Done Criteria

- Geofence plan compiles for supported profiles.
- Dry-run artifact contains fence summary.
- Param requirements serialize and validate.
- Param snapshot schema exists.
- Capability profiles describe fence/param support and caveats.
- Mission start can be blocked by failed FC contract validation.

### Automated Tests

Tests that need no refactoring:

- circular fence compiles to expected fence plan item;
- polygon fence compiles to N ordered vertex items;
- fence enable command appears after fence items;
- param requirement passes within bounds;
- param requirement fails outside bounds;
- param snapshot roundtrip JSON;
- unsupported fence/param operation returns structured profile error.

Tests that need light refactoring:

- shared fence item assertion helper;
- mock transport fence/param capture helper;
- dry-run artifact fence/param assertion helper;
- preflight-to-FC-contract integration test.

Tests that need heavy refactoring:

- local PX4/SIH manual test for fence acceptance;
- ArduPilot legacy fence path tests;
- read-all-params large fixture;
- version-specific param registry.

---

## M87 - Swarm Command Plane

### Goal

Move from single-drone command plans to coordinated multi-drone missions.

This is still mission-level coordination, not drone-to-drone RF firmware.

### Scope

1. Agent roles:
   - scout;
   - observer;
   - relay;
   - leader/coordinator;
   - mothership/carrier;
   - reserve;
   - recovery/return agent.

2. Command fanout:
   - one supervisor mission;
   - per-agent command sequences;
   - per-agent ACK expectations;
   - per-agent telemetry milestones;
   - per-agent abort policy;
   - global mission abort policy.

3. Ownership:
   - task ownership;
   - route/segment ownership;
   - target ownership;
   - replacement mission ownership;
   - handoff/reassignment events.

4. Swarm supervisor states:
   - planned;
   - dispatched;
   - active;
   - degraded;
   - replacing;
   - aborting;
   - completed;
   - failed.

5. Synchronized GCS operations:
   - arm all;
   - takeoff all;
   - start all;
   - abort all;
   - command timeout and partial-success handling.

### Non-Goals

- No real distributed consensus guarantee.
- No RF mesh implementation.
- No low-level collision avoidance.
- No simultaneous takeoff hardware claim.
- No guarantee that all FCs obey commands identically.

### Done Criteria

- A swarm mission can produce N per-agent command plans.
- Per-agent failures trigger replacement or abort according to policy.
- Artifacts preserve command fanout and ownership transitions.
- Replay can explain which agent owned what and when.
- Existing allocation strategies can feed command-plane assignments.
- Synchronized GCS commands are represented and fake-tested.

### Automated Tests

Tests that need no refactoring:

- command fanout creates one plan per assigned agent;
- duplicate ownership fails validation;
- failed agent triggers replacement policy;
- global abort emits per-agent abort commands;
- replay records ownership handoff;
- arm-all/takeoff-all reports partial failure deterministically.

Tests that need light refactoring:

- scenario fixture for scout/reserve replacement;
- artifact validator checks per-agent command sections;
- metrics include command success/failure per agent;
- fake controller for synchronized command windows.

Tests that need heavy refactoring:

- transport-independent swarm executor;
- temporal route/segment reservation across agents;
- CBBA/gossip integration through command plane events;
- concurrent agent command runner.

---

## M88 - Swarm Topologies

### Goal

Represent practical swarm topologies at the coordination layer without claiming
to implement radio hardware.

Topology should influence mission planning and failure handling:

```text
topology -> allowed command routing -> supervisor policy -> artifacts/replay
```

### Scope

1. Centralized GCS topology:
   - ground supervisor dispatches all commands;
   - all agents report to GCS;
   - degraded if agent link is lost.

2. P2P logical topology:
   - agents can exchange coordination messages in the model;
   - transport is abstract;
   - delivery/delay/drop assumptions are explicit.

3. Mothership/carrier topology:
   - mothership as coordination role first, not physical vehicle assumption;
   - deploy child drones;
   - assign sub-missions;
   - recover/return/abort child drones;
   - artifact records dependency graph.

4. Relay topology:
   - relay role may improve command reachability in model;
   - no RF-layer implementation;
   - relay availability affects mission decisions.

5. Mesh topology:
   - logical mesh connectivity;
   - route command/control messages over model links;
   - partition behavior visible in replay;
   - no real radio protocol.

6. Transport abstraction:
   - InMem for tests;
   - existing UDP only if retained as legacy/test transport;
   - future serial/MAVLink-router adapter boundary;
   - no dependency on concrete radio hardware.

### Non-Goals

- No RF mesh stack.
- No latency-bounded consensus guarantee.
- No physical mothership hardware model.
- No production radio routing.
- No hidden reuse of old UDP prototype as production transport.

### Done Criteria

- Topology config can represent GCS, P2P, mothership, relay and mesh.
- Command routing decisions use topology config.
- Partition/degraded cases are replayed and reported.
- Mothership mission dependency graph is represented.
- Transport abstraction is testable without network/hardware.

### Automated Tests

Tests that need no refactoring:

- centralized topology routes all commands through GCS;
- P2P topology permits peer command event in model;
- partition blocks command path and marks degraded;
- mothership deployment creates dependent child missions;
- relay node improves model reachability when available;
- transport abstraction serializes command envelope deterministically.

Tests that need light refactoring:

- topology scenario fixtures;
- replay summary for topology route decisions;
- artifact validator topology section checks.

Tests that need heavy refactoring:

- actor-style swarm executor;
- network partition recovery with partial command logs;
- CBBA/gossip over abstract transport;
- repeated stochastic topology degradation sweeps.

---

## M89 - SITL Dual-Stack Evidence Pack

### Goal

Prepare a disciplined evidence path for PX4 and ArduPilot without requiring
hardware.

PX4 remains first-class because the project already has PX4/SIH history.
ArduPilot becomes a second profile/workflow so the code does not accidentally
become PX4-only.

### Scope

1. PX4 path:
   - keep existing PX4/SIH workflow;
   - route primitive missions through M81 compiler;
   - keep hardware gates and runbooks conservative;
   - ensure old SITL mission upload path still works.

2. ArduPilot path:
   - add profile and docs first;
   - add dry-run compilation first;
   - add SITL runbook;
   - add optional local SITL harness only if dependencies are manageable.

3. Shared evidence:
   - same command IR input;
   - same artifact schema where possible;
   - profile-specific warnings;
   - command plan section;
   - expected ACK/telemetry section;
   - abort/replacement section;
   - safety/FC contract section.

4. Validation:
   - artifact validator checks command/MAVLink/profile sections;
   - docs explain current vs historical evidence;
   - tests do not require installed SITL.

5. Optional manual evidence:
   - local PX4 SITL primitive mission dry-run/execute;
   - local ArduPilot SITL dry-run/execute only if environment is available;
   - no hardware claim from either.

### Non-Goals

- No mandatory PX4/ArduPilot installation in automated tests.
- No hardware evidence.
- No publication benchmark.
- No claim that dual-stack SITL means production readiness.

### Done Criteria

- Primitive mission compiler can target PX4 and ArduPilot profiles.
- Dry-run artifacts expose target stack/profile.
- Docs explain how to run PX4 SITL and ArduPilot SITL when available.
- Automated tests remain portable.
- Existing PX4 path is not regressed.
- Artifact validator accepts new command/evidence sections.

### Automated Tests

Tests that need no refactoring:

- PX4 profile dry-run compiles core primitive mission;
- ArduPilot profile dry-run compiles core primitive mission;
- unsupported/caveat differences are visible in artifacts;
- existing PX4 dry-run tests still pass;
- artifact validator checks command/profile section.

Tests that need light refactoring:

- parameterize existing SITL dry-run tests over target profile;
- docs smoke tests for ArduPilot runbook commands;
- shared artifact schema test for PX4/ArduPilot dry-runs;
- validator helper for command ACK/telemetry section.

Tests that need heavy refactoring:

- automated ArduPilot SITL harness;
- dual-stack SITL comparison runner;
- backend abstraction shared with real connection code;
- manual evidence pack generator for optional local SITL.

---

## Почему не B-порядок

`DRONE_B.24` предлагает полезные задачи, но начинает с Urban geo graph and
MAVLink geofence/params before command IR/compiler/profiles. Это рискованно:
hardware-facing behavior начнёт расползаться по существующим Urban/SITL
функциям без общего командного контракта.

Geo graph, deconfliction, geofence, params, synchronized GCS and mothership
нужно сохранить, но поставить после M80-M82.

## Почему не A-порядок

`DRONE_A.24` правильно держит границы и даёт хороший M80-M85, но там меньше
конкретных hardware-facing задач. Для текущего направления важно не только
написать primitive mission and compiler, но и приблизиться к реальному FC
контракту:

- geofence upload planning;
- parameter management;
- dual-stack PX4/ArduPilot dry-run evidence;
- Urban geo-referenced graph;
- topology-aware swarm coordination.

## Почему C-база лучше

`DRONE_C.24` лучше всего задаёт последовательность:

```text
IR -> compiler -> profiles -> missions -> Urban -> swarm -> topology -> evidence
```

Итоговый `DRONE_A.25` сохраняет эту последовательность, но расширяет её до
M80-M89 и добавляет конкретику из B.

## Recommended First Slice

Практический первый delivery slice:

```text
M80 + минимальный M81 for takeoff-hold-land
```

Не надо начинать с ArduPilot SITL, geofence or mothership. Сначала нужен
стабильный command IR и один маленький MAVLink Common plan artifact. После
этого остальные этапы будут расширением одного контракта, а не набором
разрозненных backend hacks.
