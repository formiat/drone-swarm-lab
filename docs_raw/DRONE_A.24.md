# DRONE_A.24 - план развития без железа: Urban, MAVLink Common, swarm

Дата: 2026-06-03

Этот документ фиксирует следующий линейный план после блока
`BEFORE_HARDWARE_A.23`. Условие: реального железа сейчас нет, конкретная
аппаратная платформа неизвестна, но проект всё равно должен двигаться в сторону
реального программно-аппаратного комплекса.

Главный вывод:

```text
Без железа можно писать не "симуляцию ради симуляции", а реальный
mission/control слой: типы миссий, компилятор в MAVLink Common, capability
profiles для PX4/ArduPilot, Urban mission pack и swarm command plane.
```

Проект не должен писать flight controller, motor control, EKF, SLAM, RF mesh
firmware или perception stack. Но он может уже сейчас строить слой выше
автопилота:

- mission intent;
- mission-to-MAVLink compilation;
- safety/preflight validation;
- command/report/artifact discipline;
- multi-agent ownership/reallocation;
- Urban mission semantics;
- swarm coordination contracts.

## Архитектурная позиция

### Что делает автопилот

PX4/ArduPilot or another flight controller owns:

- stabilization;
- attitude/rate control;
- low-level waypoint tracking;
- motor output;
- onboard failsafes;
- EKF/local position estimate;
- vehicle-specific mode implementation.

### Что делает этот проект

This project owns:

- high-level mission primitives;
- mission sequencing;
- task allocation and reallocation;
- route planning over known mission maps;
- no-fly/geofence/ownership/preflight checks;
- MAVLink command/mission upload planning;
- supervisor lifecycle;
- replay, metrics, artifacts and evidence;
- swarm-level coordination.

### Почему MAVLink Common first

MAVLink Common is the safest first hardware-facing contract because it is the
standard common message/command layer shared by MAVLink ecosystems. PX4 and
ArduPilot both use MAVLink, but they do not necessarily implement every command
with identical semantics. Therefore the project should avoid "one universal
autopilot behavior" assumptions.

The practical rule:

```text
Use MAVLink Common as the default representation.
Represent PX4/ArduPilot differences as explicit capability profiles.
Prefer fallback plans over stack-specific hidden behavior.
```

## Не делать сейчас

- No MCU/driver code for an unknown board.
- No direct motor/control-loop code.
- No vendor SDK as the main abstraction.
- No real obstacle-avoidance/safety claims.
- No real RF mesh implementation without chosen radio hardware.
- No hardware-ready claim from simulation or dry-run artifacts.
- No PX4-only or ArduPilot-only hidden semantics in mission primitives.

## Milestone Chain

```text
M80 Primitive Mission Intent Layer
  -> M81 MAVLink Common Command Compiler
    -> M82 PX4 / ArduPilot Capability Profiles
      -> M83 Urban Real Mission Pack
        -> M84 Swarm Command Plane
          -> M85 SITL Dual-Stack Readiness
```

This chain keeps Urban as the main applied setting, but does not make Urban the
only project direction. SAR, inspection, wildfire and future real missions can
reuse the same primitive mission layer, MAVLink compiler and swarm command plane.

---

## M80 - Primitive Mission Intent Layer

### Goal

Create a hardware-agnostic mission intent layer for primitive real-world drone
actions.

The goal is not to execute on hardware yet. The goal is to define stable mission
semantics that can later compile to MAVLink, dry-run artifacts, PX4 SITL,
ArduPilot SITL, or another backend.

### Core primitives

Initial primitive set:

- `arm`;
- `takeoff(altitude_m)`;
- `hold(duration)`;
- `land`;
- `rtl`;
- `go_to(local_or_global_position)`;
- `follow_route(route_id, waypoints)`;
- `orbit(center, radius_m, turns, direction)`;
- `pause`;
- `resume`;
- `abort`.

The first version should prefer conservative semantics:

- explicit units;
- explicit coordinate frame;
- explicit altitude reference;
- explicit timeout policy;
- explicit expected terminal state.

### Data model

Add a type family along these lines:

```rust
pub enum MissionPrimitive {
    Arm,
    Takeoff(TakeoffIntent),
    Hold(HoldIntent),
    Land(LandIntent),
    ReturnToLaunch(ReturnToLaunchIntent),
    GoTo(GoToIntent),
    FollowRoute(FollowRouteIntent),
    Orbit(OrbitIntent),
    Pause,
    Resume,
    Abort(AbortIntent),
}
```

Each intent should be serializable in snake_case and should avoid raw ambiguous
numbers:

- durations as `Duration` or explicit seconds wrappers;
- altitudes as named fields;
- coordinate frames as enums;
- orbit direction as enum;
- mission ids / route ids as typed ids where useful.

### Integration points

M80 should not bypass existing project concepts. It should connect to:

- `TaskKind` / mission semantics;
- Scenario DSL;
- preflight safety;
- dry-run artifacts;
- replay events;
- future MAVLink compiler.

### Urban relevance

Urban patrol/search should be expressible as primitives:

```text
takeoff -> follow_route(perimeter) -> hold/search condition -> land/rtl
```

This gives Urban a real mission form instead of only benchmark/simulation
behavior.

### Done criteria

- Primitive mission types exist and are serializable.
- Basic validation exists for impossible or ambiguous primitives.
- Urban route/follow-route can be represented without MAVLink-specific fields.
- Existing scenario/dry-run layers can reference primitive missions.
- Docs explain that primitives are mission intent, not hardware execution.

### Tests

Tests that need no refactoring:

- serialization roundtrip for every primitive;
- validation rejects negative/zero altitude where invalid;
- validation rejects zero/negative hold duration;
- validation rejects orbit radius/turn count outside allowed range;
- route primitive preserves waypoint ordering;
- docs smoke test for "intent layer, not hardware execution".

Tests that need light refactoring:

- Scenario DSL fixture with primitive mission sequence;
- dry-run artifact includes primitive mission summary;
- preflight validation consumes primitive route/altitude fields.

Tests that need heavy refactoring:

- shared mission schema versioning across scenario DSL and SITL artifacts;
- typed mission id registry;
- reusable primitive mission executor abstraction for all backends.

---

## M81 - MAVLink Common Command Compiler

### Goal

Compile primitive mission intents into a MAVLink Common compatible command or
mission plan.

This milestone should create a real hardware-facing representation without
requiring real hardware. The output is a deterministic command/mission upload
plan with expected acknowledgements, retries, timeouts and unsupported-command
warnings.

### Scope

1. Compiler input:
   - primitive mission sequence from M80;
   - vehicle/system/component ids;
   - coordinate origin;
   - altitude frame;
   - backend capability profile;
   - safety/preflight result.

2. Compiler output:
   - ordered command sequence;
   - ordered mission item sequence where appropriate;
   - expected ACKs;
   - timeout/retry policy;
   - fallback notes;
   - unsupported primitive warnings;
   - machine-readable dry-run artifact.

3. MAVLink Common first:
   - prefer common mission items and commands;
   - avoid dialect-specific messages in the first version;
   - if a primitive cannot be represented portably, output a structured
     unsupported result instead of silently inventing stack-specific behavior.

4. Orbit fallback:
   - `orbit` may compile directly only when the profile supports it;
   - otherwise approximate orbit as a deterministic waypoint loop;
   - record approximation error and segment count.

5. Command lifecycle:
   - compiler should describe what the supervisor expects:
     - upload;
     - start;
     - monitor progress;
     - abort/clear where applicable;
     - completion criteria.

### Non-goals

- No real connection required.
- No actual SITL run required.
- No custom dialect.
- No vendor SDK wrapper.
- No low-level offboard setpoint loop.

### Done criteria

- A primitive mission can compile to a MAVLink Common plan artifact.
- Unsupported primitives are explicit and testable.
- `takeoff -> hold -> land` has a deterministic compiled plan.
- `takeoff -> orbit -> land` has either direct support or waypoint fallback.
- Plan artifacts include profile name, target stack, command ids and warnings.
- Existing `sitl_agent --dry-run` or adjacent dry-run path can expose the plan.

### Tests

Tests that need no refactoring:

- compile `takeoff-hold-land` to deterministic command/mission plan;
- compile orbit fallback to waypoint loop with stable ordering;
- unsupported primitive returns structured warning/error;
- command plan JSON roundtrip;
- expected ACK/timeout policy appears in artifact.

Tests that need light refactoring:

- integrate command compiler with existing SITL dry-run artifact;
- compare compiler output against a small golden JSON fixture;
- preflight gate blocks compilation when mission violates safety rules.

Tests that need heavy refactoring:

- reusable backend trait shared by dry-run, mock, PX4 and ArduPilot paths;
- generated MAVLink command metadata table from upstream definitions;
- cross-stack compatibility validator.

---

## M82 - PX4 / ArduPilot Capability Profiles

### Goal

Make autopilot differences explicit. PX4 and ArduPilot both speak MAVLink, but
not every command has identical support or behavior. M82 should prevent hidden
"works on my stack" assumptions.

### Profiles

Initial profiles:

- `MavlinkCommonGeneric`;
- `Px4Multicopter`;
- `ArduPilotCopter`.

Each profile should classify primitives:

- `supported`;
- `supported_with_caveat`;
- `supported_via_fallback`;
- `unsupported`;
- `unknown_until_sitl`.

### Compatibility matrix

The matrix should cover at least:

- arm/disarm;
- takeoff;
- land;
- RTL;
- waypoint route;
- hold/loiter;
- orbit;
- pause/resume;
- abort/clear mission;
- geofence upload if in current scope;
- rally/safe points if in current scope.

Each row should have:

- primitive id;
- MAVLink command/mission representation;
- PX4 status;
- ArduPilot status;
- fallback;
- caveat;
- test coverage.

### Code structure

Profiles should be data-driven enough to avoid hardcoding hidden behavior in the
compiler. A profile should answer:

```text
Can this primitive be compiled?
If yes, direct or fallback?
If no, why?
What warning should appear in the artifact?
```

### Done criteria

- Compiler can run under all three profiles.
- Profile differences appear in artifacts.
- Unsupported behavior is not silently compiled.
- Docs describe the compatibility boundary.
- Urban mission pack can request a target profile and receive warnings.

### Tests

Tests that need no refactoring:

- profile lookup for every initial primitive;
- PX4 and ArduPilot profile snapshots contain no `unknown` for core primitives;
- orbit fallback behavior differs only through explicit profile rules;
- artifacts include profile and compatibility warnings.

Tests that need light refactoring:

- docs table generated or checked from profile data;
- CLI accepts target profile for dry-run compilation;
- existing SITL dry-run tests parameterized by profile.

Tests that need heavy refactoring:

- source profile data from machine-readable external tables;
- SITL-backed capability probing;
- versioned profiles for specific PX4/ArduPilot releases.

---

## M83 - Urban Real Mission Pack

### Goal

Turn Urban from mostly simulation/benchmark setting into a real mission pack
that compiles to primitive missions and MAVLink plans.

This is still pre-hardware. The key improvement is that Urban scenarios become
closer to real operator intent:

```text
Patrol this block.
Search this route until target is detected.
Avoid known forbidden zones.
React to blocked route at mission level.
Return/land safely.
```

### Mission types

Initial Urban mission pack:

1. `urban-block-patrol`
   - takeoff;
   - follow perimeter route;
   - optional hold at observation points;
   - land or RTL.

2. `urban-search-target`
   - takeoff;
   - follow search route;
   - evaluate mocked target detector events;
   - stop/hold/return after detection;
   - record target detection artifact.

3. `urban-blocked-route-response`
   - takeoff/follow route;
   - encounter temporary blocked edge from known map state;
   - choose wait/replan/abort policy;
   - produce updated primitive plan.

4. `urban-multi-agent-patrol`
   - split route segments between agents;
   - preserve ownership;
   - avoid duplicate route ownership;
   - recover unfinished route tasks after lost agent.

### Map and safety assumptions

Urban mission pack should keep assumptions explicit:

- known static map;
- buildings/no-fly polygons represented as constraints or forbidden cells/edges;
- one altitude band unless explicitly extended;
- no real lidar;
- no certified obstacle avoidance;
- mocked detector providers for bus/target/temporary blocked route;
- independent simulation judge remains a testing tool, not real safety.

### MAVLink output

Urban missions should compile to:

- primitive sequence;
- MAVLink Common plan;
- dry-run artifact;
- route identity;
- expected completion criteria;
- profile compatibility warnings.

### Done criteria

- At least one Urban patrol mission compiles to primitive sequence and MAVLink
  plan.
- At least one Urban search mission compiles with mocked detector semantics.
- Blocked-route response can produce a replacement mission plan.
- Multi-agent route ownership is explicit in artifacts.
- Docs clearly distinguish mission-level reactivity from real obstacle
  avoidance.

### Tests

Tests that need no refactoring:

- Urban patrol primitive sequence is deterministic;
- Urban search sequence stops on mocked target detection;
- blocked-route policy updates mission plan as expected;
- no duplicate route segment ownership in multi-agent patrol;
- exported MAVLink plan includes Urban route metadata.

Tests that need light refactoring:

- scenario DSL fixture for Urban primitive mission pack;
- replay summary includes primitive mission ids;
- artifact validator checks Urban mission plan metadata.

Tests that need heavy refactoring:

- richer map constraint model beyond current road graph;
- multi-altitude Urban airspace model;
- generalized route deconfliction for multiple active agents.

---

## M84 - Swarm Command Plane

### Goal

Develop swarm-level command and coordination semantics that work above
individual vehicle commands.

This is where the project keeps its core value: not just "send a command to one
drone", but coordinate a group.

### Swarm concepts

Add or formalize:

- agent role:
  - scout;
  - relay;
  - leader;
  - mothership/coordinator;
  - reserve;
- task/route ownership;
- swarm heartbeat;
- peer progress events;
- leader/coordinator election or configured leader;
- degraded mode;
- reassignment protocol;
- mission replacement protocol;
- P2P event envelope.

### Mothership / coordinator model

The "mothership" should be represented first as a coordination role, not as a
specific physical vehicle:

- may be ground station;
- may be companion computer;
- may be one drone;
- may later be a larger carrier platform.

The code should not assume physical properties of the mothership until hardware
exists.

### Mesh/P2P scope

Do not implement a real RF mesh stack. Instead:

- represent link availability/capability;
- represent P2P command/event exchange;
- route swarm-level decisions through current connectivity model;
- keep RF-specific behavior behind future adapter boundaries.

This keeps emergency/relay ideas useful without pretending that mission planner
implements radio networking.

### Integration with M80-M83

Swarm command plane should operate on primitive missions:

```text
agent A owns route segment 1
agent B owns route segment 2
agent C is reserve
agent A lost -> release unfinished primitives -> compile replacement for B/C
```

### Done criteria

- Swarm event envelope exists for command/progress/failure/reassignment events.
- Multi-agent primitive mission ownership is represented.
- Lost-agent recovery can release unfinished primitive missions.
- Replacement mission plans compile through M81.
- Mothership/coordinator role is explicit and does not imply hardware.
- P2P/Mesh language is documented as coordination/link model, not RF stack.

### Tests

Tests that need no refactoring:

- ownership prevents duplicate primitive assignment;
- lost agent releases unfinished primitive missions;
- reserve agent receives replacement route;
- swarm event envelope serializes deterministically;
- coordinator/mothership role can be configured without physical assumptions.

Tests that need light refactoring:

- integrate primitive mission ownership into current runtime task registry;
- artifact validator checks swarm command events;
- replay summary shows reassignment at primitive mission level.

Tests that need heavy refactoring:

- actor-style supervisor for concurrent agents;
- leader election/failover beyond configured coordinator;
- network partition recovery with partial command logs.

---

## M85 - SITL Dual-Stack Readiness

### Goal

Prepare the project for both PX4 and ArduPilot SITL without requiring hardware.

PX4 can remain the first-class path because the project already has PX4/SIH
history. ArduPilot should be added as a second target profile/workflow so the
code does not become accidentally PX4-only.

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

3. Shared behavior:
   - same primitive mission input;
   - same artifact schema where possible;
   - backend/profile-specific warnings;
   - no hidden stack-specific behavior.

4. Evidence:
   - no hardware evidence required;
   - local SITL evidence optional;
   - dry-run artifacts required;
   - tests must not depend on locally installed PX4/ArduPilot.

### Done criteria

- Primitive mission compiler can target PX4 and ArduPilot profiles.
- Dry-run artifacts expose target stack/profile.
- Docs explain how to run PX4 SITL and ArduPilot SITL when available.
- Automated tests remain portable and do not require SITL installation.
- Existing PX4 path is not regressed.

### Tests

Tests that need no refactoring:

- PX4 profile dry-run compiles core primitive mission;
- ArduPilot profile dry-run compiles core primitive mission;
- unsupported/caveat differences are visible in artifacts;
- existing PX4 dry-run tests still pass.

Tests that need light refactoring:

- parameterize existing SITL dry-run tests over target profile;
- docs smoke tests for ArduPilot runbook commands;
- shared artifact schema test for PX4/ArduPilot dry-runs.

Tests that need heavy refactoring:

- automated ArduPilot SITL harness;
- dual-stack SITL comparison runner;
- backend abstraction shared with real connection code.

---

## Priority Recommendation

The most pragmatic next step is:

```text
M80 -> M81 -> M82
```

These three milestones create a reusable real mission/control foundation before
adding more Urban behavior. After that, `M83` can make Urban the main applied
mission pack, and `M84` can lift it from single-agent command sequencing into
swarm coordination.

Do not start with ArduPilot SITL integration before M80-M82. Without primitive
mission semantics and capability profiles, ArduPilot support would likely become
another backend-specific branch instead of a clean extension.

## Module Status Implications

This plan also clarifies older module status:

- UDP/multiprocess demo path is not part of the future product direction.
- Playground binaries should be audited for removal or legacy marking.
- `coverage` remains benchmark baseline, not real area-survey mission.
- `realism` remains synthetic disturbance/stress layer, not calibrated physics.
- `emergency_mesh` remains research/coordination experiment, not RF mesh stack.
- `inspection` should be treated as a candidate for real mission development,
  not merely benchmark-only.
- Urban becomes the primary applied mission setting for the next phase.

## Expected Result After M85

After M85 the project should still not claim hardware readiness. But it should
have a much stronger pre-hardware position:

- real mission primitive model;
- deterministic compiler to MAVLink Common plans;
- explicit PX4/ArduPilot capability profiles;
- Urban missions that look like operator intent;
- swarm command/ownership/reallocation semantics over primitive missions;
- portable dry-run artifacts that can be reviewed before any real vehicle run.

That is the right foundation for future hardware work because it minimizes the
amount of code that depends on unknown airframe, firmware version, controller
board, radio hardware or sensor stack.
