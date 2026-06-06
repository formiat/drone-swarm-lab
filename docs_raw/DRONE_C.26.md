# DRONE_C.26 - следующий вектор: real command path, swarm protocol, urban operations

Дата: 2026-06-06

Источник: текущее состояние `HEAD` после M80-M89, `README.md`, `docs/STATUS.md`,
`docs/HARDWARE_READINESS.md`, `docs/SWARM_COMMAND_PLANE.md`,
`docs/SWARM_TOPOLOGIES.md` и обсуждение приоритетов:

- реальные дроновые команды и реальный дроновый код;
- несколько одновременно работающих дронов;
- рои дронов;
- протокол/архитектура общения в сети без привязки к одному каналу;
- приоритетный сеттинг `urban`.

## Executive Summary

Текущий проект уже умеет:

- описывать mission intent как command IR;
- компилировать его в transport-free MAVLink Common plan;
- явно фиксировать PX4/ArduPilot capability profiles;
- строить per-agent swarm command plans;
- моделировать логические topologies и degraded routing;
- выпускать честные dry-run evidence artifacts.

Следующий правильный шаг - не усложнять simulation ради simulation, а доводить
верхние слои до состояния реального pre-hardware control stack:

```text
real MAVLink-facing execution/config layer
  -> swarm communication protocol
    -> transport abstraction
      -> partition/degraded coordination
        -> urban multi-drone operations
          -> hardware-entry evidence discipline
```

Главная идея:

- физика, реальный radio link и конкретный carrier могут оставаться mock/abstract;
- mission commands, MAVLink execution semantics, swarm protocol,
  failure/degraded behavior и evidence discipline должны быть настоящими.

## Architectural Boundary

### Autopilot owns

PX4, ArduPilot or another FC owns:

- stabilization;
- attitude/rate control;
- motor output;
- low-level waypoint following;
- EKF/local position estimate;
- onboard failsafes;
- airframe-specific mode semantics;
- airframe tuning.

### This project should own

This project should own:

- mission intent and mission sequencing;
- route/segment/task ownership;
- MAVLink mission/command/geofence/param planning;
- MAVLink upload/execute state machine at application layer;
- swarm mission fanout;
- swarm coordination protocol;
- degraded and partition-aware supervisor policy;
- Urban mission semantics;
- replay, artifacts, validator rules and evidence packs.

### Design Principle

Do not bind mission logic to one autopilot or one network carrier.

Use these layers:

```text
mission logic
  -> command IR
    -> MAVLink Common plan
      -> stack profile (PX4 / ArduPilot)
        -> swarm protocol / supervisor policy
          -> transport adapter
```

This keeps the project hardware-adjacent without pretending to be a flight
controller or radio stack.

## Non-Goals

These are still not the main workstream:

- writing FC firmware;
- direct motor/offboard control loops;
- real CV/lidar/SLAM;
- real RF mesh implementation;
- vendor SDK as the central architecture;
- “beautiful simulation” that does not map to future hardware work;
- production certification claims;
- pretending dry-run or SITL equals real airframe behavior.

## Why This Direction

Current M80-M89 close the “planning and evidence” phase well, but three major
gaps remain before the project looks like a serious pre-production drone stack:

1. There is still much more planning than execution.
   `MavlinkCommonPlan`, capability reports, FC contract summaries and
   dual-stack evidence are strong, but live FC-facing operations are still
   shallow.

2. Swarm coordination is still mostly command-plane and topology policy.
   The project knows how a swarm should be structured, but not yet how drones
   should communicate in a degraded real network.

3. Urban is strong as mission semantics, but not yet as operational multi-drone
   behavior under imperfect communication.

Therefore the next phase should push execution, protocol and degraded swarm
behavior forward together.

## Milestone Chain

```text
M90 Live MAVLink Config + Execution Layer
  -> M91 Swarm Communication Protocol
    -> M92 Transport Abstraction
      -> M93 Partition / Degraded Swarm Supervisor
        -> M94 Urban Multi-Drone Operational Missions
          -> M95 Dual-Autopilot Execution Evidence
            -> M96 Hardware-Entry Evidence Pack
```

This is a linear plan. Each milestone feeds the next one.

---

## M90 - Live MAVLink Config + Execution Layer

### Goal

Move from transport-free planning to real FC-facing application logic while
keeping the mission layer stack-agnostic.

This milestone should turn existing planning artifacts into executable control
operations for:

- mission upload;
- mission start;
- abort/RTL/land handling;
- geofence upload;
- parameter verification and writes.

### Scope

1. Mission upload/execute state machine:
   - mission upload handshake;
   - mission start command handling;
   - ACK correlation;
   - bounded retry policy;
   - timeout policy;
   - structured failure reasons;
   - explicit abort path on partial execution failure.

2. FC config operations:
   - geofence upload execution path built from existing `MavlinkFencePlan`;
   - parameter read path;
   - parameter write path;
   - parameter requirement verification before mission start;
   - explicit “blocked by FC contract” execute-time failure path.

3. Stack-aware execution behavior:
   - PX4 path first-class;
   - ArduPilot path experimental but real at API/state-machine layer;
   - differences expressed in profile and executor policy, not hidden in mission
     logic.

4. Evidence/report integration:
   - upload lifecycle events;
   - per-command/per-phase timing;
   - execute-time ACK mismatch reporting;
   - param/fence execution summaries in artifacts.

### Non-Goals

- No real hardware run required.
- No production-grade MAVLink library rewrite.
- No guarantee that PX4 and ArduPilot accept every command identically.
- No attempt to implement autopilot failsafes in this repo.

### Done Criteria

- There is a real execute-time mission upload/start/abort state machine, not
  only dry-run plans.
- Existing FC contract intent can be applied through real transport-facing code.
- Fence and param operations fail structurally and visibly when unsupported or
  rejected.
- PX4 path remains the first verified live-local path.
- ArduPilot path exists at the same API boundary, even if evidence remains
  experimental.

### Automated Tests

Tests that need no refactoring:

- upload state machine happy path;
- upload timeout returns structured failure;
- mission start ACK mismatch returns structured failure;
- fence upload action sequence is emitted in expected order;
- param requirement blocks mission start when execute-time snapshot violates it.

Tests that need light refactoring:

- shared mock MAVLink connection fixture;
- execute-time ACK/timeout assertion helper;
- artifact validator support for execute-time fence/param sections.

Tests that need heavy refactoring:

- local PX4/SIH execute-and-config smoke path;
- experimental ArduPilot SITL execute path;
- long retry/recovery matrix with synthetic packet loss.

---

## M91 - Swarm Communication Protocol

### Goal

Define the real protocol and state machine by which drones, a mothership and a
GCS/mission control coordinate mission work.

This is the most important new architectural layer after M87/M88.

The key rule:

```text
Model the swarm protocol first.
Do not start from mesh vs LTE vs serial.
```

### Scope

1. Protocol entities:
   - GCS / mission control;
   - leader/coordinator;
   - ordinary drone;
   - reserve/recovery drone;
   - mothership/carrier node.

2. Core message families:
   - `heartbeat`;
   - `presence` / `capability_advertisement`;
   - `mission_offer`;
   - `mission_accept` / `mission_reject`;
   - `ownership_claim`;
   - `ownership_release`;
   - `lease_renew`;
   - `replacement_offer`;
   - `abort_notice`;
   - `degraded_notice`;
   - `topology_update`;
   - `mission_result`.

3. Protocol semantics:
   - idempotent message handling;
   - correlation ids;
   - delivery attempts and TTL;
   - lease-based ownership;
   - duplicate suppression;
   - partial knowledge is allowed;
   - visible degraded state transitions.

4. Failure-aware behavior:
   - GCS unavailable;
   - mothership unavailable;
   - isolated drone;
   - stale lease;
   - conflicting ownership claims;
   - delayed result after reassignment.

5. Replay/artifact integration:
   - protocol message traces;
   - lease/ownership decisions;
   - degraded reasons;
   - command-to-message causality where possible.

### Non-Goals

- No radio-specific implementation.
- No full consensus protocol.
- No Byzantine fault model.
- No cryptography PKI stack in this phase.

### Done Criteria

- There is an explicit swarm protocol schema and typed message model.
- Ownership and reassignment semantics are lease-based, not implicit.
- Protocol can represent loss of GCS or mothership without collapsing to
  “everything failed”.
- Replay can explain why a drone continued, waited, aborted or accepted
  reassignment.

### Automated Tests

Tests that need no refactoring:

- heartbeat timeout marks peer unavailable;
- stale lease expires ownership;
- duplicate message is ignored idempotently;
- late mission result after reassignment is rejected or downgraded deterministically;
- GCS unavailable transition produces degraded state.

Tests that need light refactoring:

- message fixture builder;
- lease clock/test-time helper;
- replay assertion helper for protocol events.

Tests that need heavy refactoring:

- many-node protocol simulation harness;
- eventual delivery under delayed/duplicated messages;
- protocol fuzzing for reorder/drop/retry cases.

---

## M92 - Transport Abstraction

### Goal

Run the same swarm protocol and supervisor logic over multiple transport
adapters without changing mission logic.

This is where “mesh vs modem vs serial vs local loopback” becomes an adapter
choice, not an architecture fork.

### Scope

1. Transport interface:
   - send one typed envelope;
   - receive one typed envelope;
   - local node identity;
   - transport error classification;
   - optional delivery metadata;
   - explicit connection class.

2. First adapters:
   - `in_memory`;
   - `udp_loopback`;
   - `internet_like_mock`;
   - `serial_placeholder`.

3. Transport properties:
   - latency model;
   - drop model;
   - duplication model;
   - connectivity partition model;
   - optional ordering guarantees.

4. CLI/runtime integration:
   - selected transport recorded in artifact/report;
   - runtime can pick transport without changing mission/topology/protocol code;
   - dry-run still works transport-free.

### Non-Goals

- No production network stack.
- No real mesh routing firmware.
- No stable public networking API promise.
- No hardware serial deployment yet.

### Done Criteria

- Same swarm protocol logic runs over at least `in_memory` and `udp_loopback`.
- Transport selection is explicit in runtime and artifacts.
- Serial is represented as a future-facing placeholder/interface, not hidden
  TODO code.
- Mission logic and protocol logic do not import transport-specific behavior.

### Automated Tests

Tests that need no refactoring:

- in-memory send/receive roundtrip;
- udp loopback roundtrip;
- transport selection persists into artifact/report;
- dry-run path works without transport.

Tests that need light refactoring:

- common transport conformance helper;
- fault-injection adapter wrapper;
- CLI parser tests for transport selection.

Tests that need heavy refactoring:

- multi-process UDP swarm harness;
- transport conformance matrix shared by every adapter;
- soak tests with latency/drop/duplication distributions.

---

## M93 - Partition / Degraded Swarm Supervisor

### Goal

Upgrade the supervisor from “agent failed” handling to true degraded-network
and partition-aware coordination.

This is the milestone that makes swarm logic much closer to a real field
deployment, even without real hardware.

### Scope

1. New degraded conditions:
   - GCS lost, drones still mutually reachable;
   - mothership lost, children still active;
   - one drone isolated from all others;
   - topology partition creates split mission knowledge;
   - delayed restoration after partition.

2. Policy decisions:
   - continue mission locally under lease;
   - hold position and wait for lease renew;
   - return to launch;
   - release ownership after timeout;
   - forbid conflicting reassignment until lease expiry;
   - elect fallback coordinator only if explicitly configured.

3. Supervisor invariants:
   - no duplicate active ownership across partitions after lease accounting;
   - no silent task disappearance;
   - every degraded decision has a recorded reason;
   - restored connectivity triggers reconciliation, not blind overwrite.

4. Artifact/report outputs:
   - partition summary;
   - lease expiry decisions;
   - reconciliation events;
   - command suppression due to stale authority;
   - topology and protocol cause chain.

### Non-Goals

- No consensus algorithm guarantee.
- No cryptographic trust model.
- No real radio validation.
- No autonomous tactical swarm AI beyond explicit policy.

### Done Criteria

- Supervisor distinguishes node death from connectivity loss.
- Ownership and reassignment remain deterministic under partition.
- Replay explains recovery/reconciliation after partition heal.
- Policies are configured and testable, not hidden in ad hoc runtime branches.

### Automated Tests

Tests that need no refactoring:

- GCS loss causes configured degraded transition;
- isolated agent continues only while lease remains valid;
- stale partitioned owner loses authority after lease expiry;
- reconciliation rejects stale ownership after reconnect;
- restored link emits reconciliation events.

Tests that need light refactoring:

- partition scenario fixture builder;
- lease/reconnect time helper;
- artifact validator rules for partition reports.

Tests that need heavy refactoring:

- large split-brain simulation harness;
- reconciliation stress tests with repeated partition/heal cycles;
- protocol+supervisor fuzz tests for delayed duplicates after partition.

---

## M94 - Urban Multi-Drone Operational Missions

### Goal

Turn Urban from a good mission semantic testbed into the main operational swarm
setting.

This milestone should answer:

```text
Can several drones execute a realistic urban mission family with ownership,
handoff, degraded comms and explicit supervisor policy?
```

### Scope

1. Mission families:
   - urban perimeter patrol;
   - urban corridor inspection;
   - urban search-until-detection;
   - urban blocked-route response;
   - urban sector handoff after agent loss or partition.

2. Multi-drone coordination:
   - area/segment ownership;
   - patrol sector split;
   - inspection corridor split;
   - handoff of unfinished route segments;
   - reserve drone activation for unfinished work.

3. Communication-aware behavior:
   - continue under local ownership when isolated;
   - wait at checkpoint if authority ambiguous;
   - replacement mission after route segment release;
   - explicit degraded outcome if no safe policy exists.

4. Urban-specific evidence:
   - route ownership timeline;
   - blocked route decisions;
   - handoff report;
   - segment conflict report;
   - mission delay caused by coordination/network issues.

### Non-Goals

- No physical collision avoidance.
- No real sensor fusion.
- No traffic model beyond explicit mocked inputs.
- No full GIS/navmesh planner.

### Done Criteria

- At least two meaningful multi-drone Urban mission families work end-to-end
  through command plans and supervisor policy.
- Handoff/release/replacement under degraded comms is exercised.
- Urban reports explain both movement semantics and communication semantics.
- Urban remains the main applied setting, not a side fixture.

### Automated Tests

Tests that need no refactoring:

- sector ownership prevents duplicate assignment;
- route segment handoff after agent loss works deterministically;
- blocked route policy can trigger replacement mission;
- urban search mission stops or hands off on mocked detection;
- degraded urban mission records coordination delay.

Tests that need light refactoring:

- multi-agent urban fixture builder;
- route-handoff assertion helper;
- Urban replay/event summarizer for degraded operations.

Tests that need heavy refactoring:

- synthetic urban swarm suite over many map fragments;
- long-running degraded urban patrol scenarios;
- route ownership stress under repeated blocked edges and partitions.

---

## M95 - Dual-Autopilot Execution Evidence

### Goal

Move from dual-stack dry-run evidence to dual-stack execution evidence at the
application boundary.

This does not require real hardware, but it does require the project to stop
being “PX4-local with ArduPilot dry-run annotations only”.

### Scope

1. PX4 evidence:
   - keep and strengthen current PX4/SIH execution path;
   - ensure M90-M94 layers reuse it cleanly;
   - refresh current-head evidence for selected mission families.

2. ArduPilot evidence:
   - experimental execute/upload path;
   - command acceptance/result mapping;
   - documented limitations and unsupported areas;
   - local-only runbook, never presented as production proof.

3. Cross-stack comparison discipline:
   - same command IR source where practical;
   - same evidence sections where practical;
   - explicit caveats for non-equivalent behavior;
   - no claim that “MAVLink Common means identical runtime semantics”.

### Non-Goals

- No hardware proof.
- No requirement that both stacks support everything equally.
- No automated external dependency in default tests.

### Done Criteria

- PX4 execution evidence remains healthy on current head.
- ArduPilot is no longer only a dry-run profile.
- Evidence clearly separates “planned”, “uploaded”, “started”, “completed”,
  “aborted”, “unsupported”.
- Reports do not hide stack-specific caveats.

### Automated Tests

Tests that need no refactoring:

- executor lifecycle result mapping for PX4 profile;
- executor lifecycle result mapping for ArduPilot profile;
- report marks stack-specific unsupported areas explicitly;
- current-head validator accepts the new execution evidence schema.

Tests that need light refactoring:

- dual-stack evidence fixture builder;
- lifecycle report assertion helper;
- docs smoke tests for PX4 vs ArduPilot runtime caveats.

Tests that need heavy refactoring:

- local PX4 execution smoke;
- local ArduPilot execution smoke;
- cross-stack comparison harness for selected primitive missions.

---

## M96 - Hardware-Entry Evidence Pack

### Goal

Produce one disciplined machine-checkable artifact that says what is actually
ready before the first controlled hardware experiment.

This should become the bridge between “engineering says it probably works” and
“operator has a concrete documented basis for a first test”.

### Scope

1. Evidence pack fields:
   - source mission/scenario;
   - command IR summary;
   - MAVLink plan summary;
   - selected stack/profile;
   - selected transport;
   - expected ACK/telemetry contract;
   - FC contract and config snapshot;
   - swarm protocol assumptions;
   - topology assumptions;
   - preflight validation result;
   - degraded policy summary;
   - run command;
   - git commit;
   - caveats;
   - limitations.

2. Mission families covered:
   - primitive;
   - Urban single-drone;
   - Urban multi-drone;
   - swarm command-plane mission.

3. Hardware-entry checklist integration:
   - selected autopilot;
   - selected link class;
   - coordinate frame/local origin policy;
   - altitude reference;
   - fence/failsafe assumptions;
   - manual abort/override assumptions;
   - first allowed mission type.

4. Classification:
   - dry-run only;
   - execute path validated locally;
   - degraded behavior partially evidenced;
   - unsupported or unknown;
   - blocked for hardware entry.

### Non-Goals

- No certification.
- No real flight claim.
- No operator training substitute.
- No assumption that SITL or local execute equals safe hardware behavior.

### Done Criteria

- There is one explicit hardware-entry evidence artifact schema.
- Primitive, Urban and swarm families can emit it.
- It can say both “ready enough for first controlled test” and “not ready” with
  explicit reasons.
- It reuses existing validator discipline instead of inventing ad hoc README-only
  evidence.

### Automated Tests

Tests that need no refactoring:

- primitive mission evidence pack validates;
- urban mission evidence pack validates;
- swarm mission evidence pack validates;
- missing preflight section fails validation;
- missing caveat on caveated mission fails validation.

Tests that need light refactoring:

- common evidence-pack fixture builder;
- validator subcommand for evidence packs;
- report summarizer for operator-facing evidence digest.

Tests that need heavy refactoring:

- current-head live-local evidence refresh pipeline;
- schema compatibility tests across mission families;
- replay-integrated evidence trace compression.

## Recommended First Slice

If only one narrow implementation slice should be started next, it should be:

1. `M90` mission upload/start/abort state machine.
2. `M90` fence/param execute-time integration.
3. `M91` minimal swarm protocol with heartbeat + lease + ownership messages.
4. `M92` `in_memory` + `udp_loopback` adapters only.
5. `M93` one partition scenario: GCS lost, drones continue under lease.
6. `M94` one Urban mission: multi-drone perimeter patrol with handoff.

This slice keeps the work tightly aligned to:

- real command path;
- multi-drone execution;
- swarm communication architecture;
- degraded network behavior;
- urban priority setting.

## Final Recommendation

Do not spend the next phase primarily on richer simulation, benchmark refreshes
or abstract topology variants.

Do spend it on:

- real MAVLink-facing execution logic;
- real swarm protocol/state-machine design;
- transport abstraction that preserves protocol logic;
- degraded/partition-aware supervisor decisions;
- urban multi-drone operational missions;
- evidence discipline for first hardware entry.

That is the shortest path from the current codebase to a serious pre-hardware
urban swarm platform.
