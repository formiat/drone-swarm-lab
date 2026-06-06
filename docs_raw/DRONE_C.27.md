# DRONE_C.27 - итоговый план: real MAVLink edge, autonomous agents, degraded urban swarm

Дата: 2026-06-06

Источник: сравнение `docs_raw/DRONE_A.26.md`, `docs_raw/DRONE_B.26.md`,
`docs_raw/DRONE_C.26.md`, текущее состояние `HEAD` после `M80-M89`,
`README.md`, `docs/STATUS.md`, `docs/HARDWARE_READINESS.md`,
`docs/SWARM_COMMAND_PLANE.md`, `docs/SWARM_TOPOLOGIES.md`,
`docs/FC_CONTRACT.md`, `docs/ARDUPILOT_SITL.md`.

Приоритеты, которые этот план принимает как обязательные:

- реальные дроновые команды и реальный дроновый код;
- несколько одновременно работающих дронов;
- рои дронов и coordination semantics;
- protocol/architecture общения без жёсткой привязки к одному carrier;
- приоритетный сеттинг `urban`;
- движение к pre-hardware / pre-production stack без ложных claim'ов.

## Executive Summary

После `M80-M89` проект уже хорошо закрывает planning/evidence слой:

```text
mission intent
  -> command IR
    -> MAVLink Common plan
      -> capability profiles
        -> FC contract
          -> command-plane / topology contracts
            -> dry-run evidence
```

Это сильный фундамент, но он ещё не превращает репозиторий в настоящий
multi-drone control stack. Основной разрыв сейчас в четырёх местах:

1. Есть хороший compile/contract layer, но всё ещё недостаточно real
   execute-time MAVLink behavior.
2. Есть command-plane и topology semantics, но ещё нет полноценного typed
   swarm protocol + autonomous agent runtime.
3. Есть urban mission semantics, но они ещё не стали главным operational
   полигоном для degraded multi-drone behavior.
4. Есть dry-run discipline, но ещё нет следующего уровня evidence для live-local
   execution и hardware entry.

Поэтому следующий этап должен идти не в сторону "ещё одной симуляции", а в
сторону настоящего pre-hardware control stack:

```text
M90 Live MAVLink Operations Boundary
  -> M91 Swarm Protocol Core
    -> M92 Autonomous Agent Runtime
      -> M93 Transport Abstraction + Multi-Process Swarm
        -> M94 Partition / Degraded Connectivity Autonomy
          -> M95 Urban Multi-Drone Operational Missions
            -> M96 Dual-Stack Live SITL Evidence
              -> M97 Hardware-Entry Evidence Pack
```

Это линейный план. Он сохраняет архитектурную логику `A.26`, использует
agent/FSM concrete detail из `B.26` и удерживает честные hardware-entry
границы из `C.26`.

## What Is Best In A/B/C.26

### Что лучше в `DRONE_A.26`

- Лучший linear roadmap.
- Хорошо отделяет "что уже есть" от "где настоящий разрыв".
- Правильно ставит live MAVLink edge раньше, чем расширение missions.

### Что лучше в `DRONE_B.26`

- Лучшая детализация agent-to-agent architecture.
- Самая полезная конкретика по `DroneMessage`, `AgentMissionState`,
  multi-process runtime и network deconfliction.
- Правильно указывает, что shared-memory coordination нельзя считать
  настоящим swarm runtime.

### Что лучше в `DRONE_C.26`

- Лучшее продолжение текущего `M80-M89` без архитектурного скачка.
- Лучше всего формулирует transport abstraction и degraded/partition behavior.
- Лучше всего держит evidence/hardware-entry discipline.

## Design Boundary

### Autopilot owns

PX4, ArduPilot or another FC owns:

- stabilization;
- attitude/rate control;
- motor output;
- onboard waypoint following;
- EKF/local state estimation;
- airframe-specific modes and onboard failsafes;
- low-level tuning and actuator safety.

### This project should own

This project should own:

- mission intent and mission sequencing;
- MAVLink mission/command/fence/param orchestration at application layer;
- agent runtime state machine;
- swarm communication protocol;
- ownership, lease and reassignment semantics;
- degraded/partition-aware swarm policy;
- urban mission semantics;
- replay, evidence packs, validators and hardware-entry discipline.

### Carrier rule

Проект не должен сейчас выбирать "истинный" carrier.

Нужно стабилизировать:

- protocol;
- state machines;
- authority model;
- failure model;
- transport boundary.

А уже под ней потом могут жить:

- localhost UDP;
- internet-like transport;
- serial relay;
- mesh-like transport;
- LTE/5G modem path;
- future hardware-specific links.

## Non-Goals

Не делать primary workstream из:

- flight controller firmware;
- direct motor/offboard control loops;
- real RF mesh implementation;
- onboard CV/lidar/SLAM;
- hardware-specific serial integration без конкретного hardware target;
- richer physics simulation ради самой simulation;
- pretending SITL equals real airframe behavior;
- pretending dry-run dual-stack artifacts equal live PX4/ArduPilot equivalence.

## Milestone Chain

```text
M89 Dual-Stack Dry-Run Evidence (completed)
  -> M90 Live MAVLink Operations Boundary
    -> M91 Swarm Protocol Core
      -> M92 Autonomous Agent Runtime
        -> M93 Transport Abstraction + Multi-Process Swarm
          -> M94 Partition / Degraded Connectivity Autonomy
            -> M95 Urban Multi-Drone Operational Missions
              -> M96 Dual-Stack Live SITL Evidence
                -> M97 Hardware-Entry Evidence Pack
```

---

## M90 - Live MAVLink Operations Boundary

### Goal

Move from transport-free planning to real MAVLink-facing application logic for:

- mission upload;
- mission start;
- mission abort / replacement;
- geofence upload;
- parameter read / write / verify;
- explicit execute-time failure reporting.

### Why it matters

Current `M80-M89` give the project a strong planning/validation surface, but
the main FC-facing interaction is still too shallow. Before building richer swarm
runtime, the repository needs a real autopilot-facing execution boundary.

Otherwise later milestones would coordinate only plans and reports, not real
operations.

### Scope

1. Mission upload/execute state machine:
   - mission count / item handshake;
   - start command path;
   - mission replacement path;
   - bounded retry policy;
   - timeout policy;
   - ACK correlation;
   - typed execution failures;
   - explicit abort / cancel / rollback outcomes where possible.

2. Live FC config operations:
   - geofence upload path from existing fence plan intent;
   - parameter read path;
   - parameter write path;
   - live snapshot validation against existing FC contract requirements;
   - explicit "blocked by FC contract" outcome.

3. Stack-aware execution behavior:
   - PX4 first-class path;
   - ArduPilot path at the same API boundary;
   - profile-specific differences expressed in executor policy, not hidden in
     mission logic.

4. Runtime evidence integration:
   - upload lifecycle events;
   - ACK mismatch reporting;
   - retry counters;
   - timeout categories;
   - fence/param apply summary;
   - execute-time failure taxonomy.

### Done Criteria

- There is a real execute-time mission upload/start/abort state machine.
- Fence and param operations use real transport-facing code paths.
- FC contract can block mission start on live mismatch.
- PX4 remains the first verified local baseline.
- ArduPilot uses the same abstraction boundary even if the local evidence is
  initially shallower.

### Automated Tests

Tests that need no refactoring:

- mission upload happy path;
- mission upload timeout failure;
- mission start ACK mismatch failure;
- mission replacement completion path;
- parameter snapshot mismatch blocks execution;
- fence upload action ordering;
- explicit abort path after partial failure.

Tests that need light refactoring:

- shared mock MAVLink connection fixture;
- executor event trace assertion helper;
- artifact validator extensions for execute-time sections.

Tests that need heavy refactoring:

- PX4 local SITL execute/config smoke path;
- experimental ArduPilot local SITL execute/config path;
- synthetic packet-loss retry/recovery matrix.

---

## M91 - Swarm Protocol Core

### Goal

Define a typed swarm protocol that covers:

- drone-to-drone communication;
- GCS-to-drone control;
- leader/mothership coordination;
- state reconciliation after reconnect;
- degraded network signaling.

### Why it matters

`M87-M88` already define command-plane and topology semantics, but they are
still stronger as policy and artifact layers than as a real wire protocol.

The next stable contract should be the protocol itself, not one specific
transport such as UDP, mesh or LTE.

### Scope

1. Protocol entities:
   - GCS / mission control;
   - leader / coordinator;
   - ordinary drone agent;
   - reserve / recovery drone;
   - mothership / carrier node.

2. Core message families:
   - heartbeat;
   - presence / capability advertisement;
   - mission assign / accept / reject;
   - ownership claim / grant / deny / release;
   - lease renew / lease expiry;
   - mission replacement;
   - mission abort;
   - degraded / unreachable / coordinator lost;
   - state request / state response;
   - operator-visible status report.

3. Envelope semantics:
   - protocol version;
   - sender / receiver ids;
   - correlation id;
   - causal ordering / tick / generation info;
   - idempotency / duplicate suppression keys;
   - replay/audit metadata.

4. Failure model:
   - delayed delivery;
   - duplicate delivery;
   - dropped messages;
   - stale coordinator messages;
   - reconnect reconciliation.

### Done Criteria

- A versioned `DroneMessage` family exists.
- Protocol semantics are documented in code and docs.
- Ownership / lease / replacement semantics are encoded in protocol terms.
- The protocol does not assume one carrier.

### Automated Tests

Tests that need no refactoring:

- serialization round-trips for all message families;
- duplicate suppression on repeated envelope ids;
- stale generation rejection;
- lease renewal and lease expiry semantics;
- mission replacement correlation behavior.

Tests that need light refactoring:

- protocol fixture builders;
- property-like tests for envelope/idempotency invariants;
- replay/audit trace helpers.

Tests that need heavy refactoring:

- long reconnect/reconciliation scenarios across multiple peers;
- fuzz-like delayed/duplicate/drop delivery matrix.

---

## M92 - Autonomous Agent Runtime

### Goal

Turn each drone into an autonomous runtime unit with an explicit FSM, local
failsafe policy and mission lifecycle logic.

### Why it matters

Without an explicit agent runtime, the project still behaves like centralized
planning with distributed execution hints. Real swarm behavior requires agents
that know how to progress, wait, degrade, abort and reconcile locally.

### Scope

1. Agent mission state machine:
   - `Idle`;
   - `WaitingForMission`;
   - `PreparingExecution`;
   - `UploadingMission`;
   - `Executing`;
   - `WaitingForLease`;
   - `Holding`;
   - `Replanning`;
   - `Degraded`;
   - `Aborting`;
   - `Completed`;
   - `Failed`.

2. Transition rules:
   - mission accepted / rejected;
   - lease granted / denied / expired;
   - FC execution failure;
   - coordinator lost;
   - peer conflict;
   - reconnect reconciliation;
   - operator abort.

3. Local safety policy:
   - when to hold;
   - when to continue autonomously;
   - when to stop accepting new work;
   - when to mark mission failed;
   - when to yield ownership.

4. Runtime integration:
   - local mission queue;
   - mission replacement handling;
   - status emission into protocol layer;
   - replay/event trace output.

### Done Criteria

- There is a typed agent FSM in code.
- Runtime transitions are deterministic and testable.
- Mission replacement and degraded behavior are agent-local behaviors, not only
  supervisor-side intentions.
- Local runtime state is exportable for replay/evidence.

### Automated Tests

Tests that need no refactoring:

- state transition happy paths;
- invalid transition rejection;
- lease-expiry transition to degraded/hold;
- operator abort transition;
- mission replacement while executing;
- reconcile-after-reconnect transition.

Tests that need light refactoring:

- transition table fixtures;
- agent runtime harness for event-driven tests;
- reusable failure injection helpers.

Tests that need heavy refactoring:

- multi-agent concurrent transition races;
- long-running mission lifecycle with repeated disconnect/reconnect phases.

---

## M93 - Transport Abstraction + Multi-Process Swarm

### Goal

Run the same agent/protocol layer over interchangeable transports and support
multi-process swarm execution on one machine.

### Why it matters

Protocol and FSM should be real before transport is specialized. Once they are
stable, the next correct step is to prove they survive outside shared memory and
single-process test loops.

### Scope

1. Transport abstraction:
   - `in_memory`;
   - `udp_loopback`;
   - `internet_like_mock`;
   - `serial_placeholder`.

2. Multi-process runtime:
   - separate agent processes;
   - supervisor / coordinator process;
   - GCS control process or shim;
   - localhost execution topology.

3. Runtime concerns:
   - connection bootstrap;
   - addressing;
   - delivery timeout policy;
   - duplicate suppression;
   - process restart compatibility;
   - traceable logs/artifacts.

4. Shared-memory replacement:
   - move urban coordination semantics from local lock assumptions toward
     message-driven ownership/reservation.

### Done Criteria

- The same swarm protocol runs over at least `in_memory` and `udp_loopback`.
- Multi-process swarm scenarios work on one machine.
- Agent runtime does not rely on shared-memory-only coordination for core swarm
  correctness.
- Transport remains a replaceable detail under stable protocol/runtime layers.

### Automated Tests

Tests that need no refactoring:

- in-memory transport protocol happy path;
- UDP loopback smoke path;
- duplicate message suppression across transport adapters;
- process restart preserves protocol-level idempotency assumptions.

Tests that need light refactoring:

- transport-conformance shared test suite;
- spawn/run/collect helper for local multi-process tests;
- structured log capture for process-scoped assertions.

Tests that need heavy refactoring:

- multi-process stress scenarios with intentional restart/loss patterns;
- internet-like latency/jitter/drop matrix across several agents.

---

## M94 - Partition / Degraded Connectivity Autonomy

### Goal

Define and implement explicit behavior for partial connectivity loss, coordinator
loss and split-brain risk.

### Why it matters

This is the milestone that turns the system from "multi-agent when the network
behaves" into something closer to a serious swarm supervisor.

### Scope

1. Failure cases:
   - GCS lost;
   - mothership/coordinator lost;
   - single peer lost;
   - partial partition;
   - stale ownership;
   - reconnect after partition.

2. Policy rules:
   - when agents may continue locally;
   - when they must hold;
   - when they must abort or RTL;
   - when a lease can be reclaimed;
   - who may reassign work;
   - how conflicting coordinators are rejected.

3. Recovery semantics:
   - state reconciliation after reconnect;
   - ownership cleanup;
   - rejoin without duplicate execution;
   - event trace continuity.

4. Evidence/reporting:
   - degraded-policy engagement report;
   - partition timelines;
   - ownership loss / reclaim history;
   - operator-facing failure summaries.

### Done Criteria

- Degraded policy exists as code, not only as documentation.
- Partition behavior is explicit and testable.
- Ownership and lease semantics survive disconnect/reconnect cycles.
- Replay/evidence can explain why the swarm continued, held or aborted.

### Automated Tests

Tests that need no refactoring:

- GCS loss engages expected degraded policy;
- coordinator loss transitions to safe local behavior;
- lease expiry enables or blocks reclaim as specified;
- stale coordinator messages are rejected after generation change.

Tests that need light refactoring:

- partition scenario harness;
- reconnect/reconciliation assertion helpers;
- policy matrix fixtures for different mission classes.

Tests that need heavy refactoring:

- split-brain race scenarios;
- long degraded mission execution with intermittent reconnection;
- multi-partition merge/rejoin matrices.

---

## M95 - Urban Multi-Drone Operational Missions

### Goal

Make `urban` the main proving ground for the new runtime by implementing
operational multi-drone mission families over the real protocol/runtime layers.

### Why it matters

`urban` is currently the most useful setting for future real-world mission logic.
It naturally exercises:

- route/segment ownership;
- replacement and reassignment;
- deconfliction;
- degraded communication;
- operator-visible mission progress.

### Scope

1. Urban mission families:
   - perimeter patrol;
   - corridor / route inspection;
   - sector search;
   - blocked-route response;
   - mission handoff between agents;
   - reserve drone takeover.

2. Runtime semantics:
   - segment reserve / grant / deny / release over network;
   - mission replacement while peers are active;
   - route ownership reconciliation after reconnect;
   - explicit urban-specific degraded behaviors.

3. Operator/evidence surfaces:
   - who owns what segment;
   - why an agent is blocked;
   - when a handoff happened;
   - when reserve agent was activated;
   - which degraded policy path was used.

### Done Criteria

- Urban scenarios use protocol/runtime semantics, not only centralized local
  orchestration.
- Segment ownership is network-visible and replayable.
- Replacement/handoff paths work under degraded conditions.
- Urban becomes the primary operational benchmark family for swarm behavior.

### Automated Tests

Tests that need no refactoring:

- segment reserve/grant/release happy path;
- blocked-route handoff behavior;
- reserve-drone takeover path;
- urban replacement mission completion.

Tests that need light refactoring:

- urban protocol-driven fixture builder;
- agent-role scenario helpers;
- segment ownership timeline assertion helpers.

Tests that need heavy refactoring:

- multi-agent urban long-run scenarios with repeated partitions;
- large-route mission pack stress cases.

---

## M96 - Dual-Stack Live SITL Evidence

### Goal

Move from dry-run dual-stack evidence to stronger live-local execution evidence
for both PX4 and ArduPilot paths.

### Why it matters

After M90-M95, the repository should prove not only that it can plan and
coordinate consistently, but that the same upper-layer architecture survives
real local SITL execution across both autopilot families.

### Scope

1. PX4 live-local evidence:
   - mission upload/start;
   - fence/param path where supported;
   - execute-time event capture;
   - failure classification.

2. ArduPilot experimental live-local evidence:
   - same upper-layer path;
   - explicit divergence documentation where behavior differs;
   - same artifact schema as far as possible.

3. Evidence discipline:
   - reproducible runbook;
   - validator extensions for live-local evidence packs;
   - stack-specific caveats kept explicit.

### Done Criteria

- PX4 path has stronger live-local evidence than dry-run only.
- ArduPilot path has a real local execution/evidence path at the same boundary.
- Evidence format makes clear what is proven and what is still experimental.

### Automated Tests

Tests that need no refactoring:

- artifact schema validation for live-local evidence;
- stack-specific caveat presence checks;
- execute-time evidence consistency checks.

Tests that need light refactoring:

- validator support for dual live-local stacks;
- reusable evidence fixture pack generators.

Tests that need heavy refactoring:

- locally orchestrated dual-SITL integration smoke paths;
- repeated evidence collection under induced failures.

---

## M97 - Hardware-Entry Evidence Pack

### Goal

Produce one disciplined, machine-checkable package that says whether a mission
stack is ready for the first tightly controlled hardware trial.

### Why it matters

This is the milestone that keeps the project honest before real hardware.
It should prevent the common mistake of treating SITL success as hardware
readiness without an explicit boundary.

### Scope

1. Evidence pack contents:
   - mission IR;
   - compiled MAVLink plan;
   - selected stack profile;
   - selected transport profile;
   - FC contract assumptions;
   - fence/param intent;
   - degraded-policy assumptions;
   - swarm protocol assumptions;
   - operator checklist snapshot;
   - known unsupported paths;
   - hazard/caveat section.

2. Validation:
   - machine-checkable schema;
   - strict top-level artifact validation;
   - required field presence by mission class;
   - explicit unsupported/untested declarations.

3. Mission classes covered:
   - primitive missions;
   - urban missions;
   - swarm missions;
   - degraded swarm missions.

### Done Criteria

- There is one versioned evidence-pack schema for hardware entry.
- The validator can reject incomplete or misleading packs.
- The pack states not only what works, but what is not proven.
- Hardware-entry readiness is expressed as a disciplined engineering artifact,
  not only as prose documentation.

### Automated Tests

Tests that need no refactoring:

- schema validation for complete hardware-entry pack;
- rejection of missing required readiness sections;
- rejection of contradictory supported/unsupported declarations;
- mission-class-specific required field validation.

Tests that need light refactoring:

- reusable pack fixture builders across mission classes;
- validator helper for cross-section consistency checks.

Tests that need heavy refactoring:

- end-to-end generation of pack from execute-time + swarm-runtime evidence;
- large matrix of mission classes and stack/transport combinations.

## Final Recommendation

If the project wants to move toward a real urban swarm software stack without
hardware yet, this is the right sequence:

1. make the MAVLink-facing edge real;
2. stabilize the swarm protocol;
3. make each drone a real autonomous runtime unit;
4. prove the same runtime survives transport swaps and multi-process execution;
5. implement explicit degraded autonomy;
6. make urban the main operational proving ground;
7. only then strengthen live-local dual-stack evidence and hardware-entry
   discipline.

That order minimizes wasted work and keeps every next layer grounded in code
that is likely to survive contact with real hardware.
