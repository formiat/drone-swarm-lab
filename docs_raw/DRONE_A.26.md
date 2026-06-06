# DRONE_A.26 - следующий линейный план: real MAVLink edge, swarm protocol, urban-first operations

Дата: 2026-06-06

Источник: текущее состояние `HEAD` после `M80-M89`, `README.md`,
`docs/STATUS.md`, `docs/HARDWARE_READINESS.md`,
`docs/SWARM_COMMAND_PLANE.md`, `docs/SWARM_TOPOLOGIES.md`,
`docs/FC_CONTRACT.md`, `docs/ARDUPILOT_SITL.md`, история последних коммитов и
обсуждение приоритетов:

- реальные дроновые команды и реальный дроновый код;
- несколько одновременно работающих дронов;
- рои дронов;
- протокол/архитектура общения без привязки к одному каналу связи;
- приоритетный сеттинг `urban`;
- без реального железа на текущем этапе, но с движением в сторону
  pre-production stack.

## Краткий вывод

После `M80-M89` проект уже хорошо закрывает слой:

```text
mission intent
  -> command IR
    -> MAVLink Common plan
      -> capability profiles
        -> dry-run evidence / validator
          -> command-plane / topology contracts
```

Это уже не просто симулятор, но ещё не настоящий swarm runtime для реальных
автопилотов.

Главный разрыв до "боевого" состояния сейчас не в physics, UI или новых
mission demos. Главный разрыв в трёх местах:

1. `M86` пока в основном transport-free contract, а не live FC operations.
2. `M87-M88` пока сильнее как artifact/policy layer, чем как исполняемый
   swarm communication runtime.
3. `urban` уже хорош как mission semantics, но ещё не стал главным полигоном
   для degraded multi-drone coordination.

Поэтому следующий этап должен быть таким:

```text
M90 Live MAVLink Operations
  -> M91 Swarm Protocol Core
    -> M92 Executable Swarm Runtime
      -> M93 Urban Connected Missions
        -> M94 Degraded Connectivity Autonomy
          -> M95 Dual-Stack Live SITL Evidence
            -> M96 Hardware-Entry Preparation Pack
```

Это линейный план без ветвления. Он не требует железа, но двигает код в
сторону реального software stack над PX4 / ArduPilot + MAVLink.

## Почему именно этот вектор

### Что уже сделано хорошо

Судя по текущему коду и документации, у проекта уже есть сильный фундамент:

- hardware-agnostic `Mission Command IR`;
- transport-free `MavlinkCommonPlan`;
- явные `PX4` / `ArduPilot` capability profiles;
- primitive and Urban mission packs;
- mission-level multi-agent deconfliction;
- FC safety contract;
- swarm command-plane contracts;
- topology contracts;
- dry-run dual-stack evidence.

### Что ещё не доведено

Но проект всё ещё недостаточно силён там, где начинается настоящая operational
работа:

- live geofence / param / preflight / execute interactions с FC;
- network/protocol semantics между несколькими агентами;
- lease/authority/degraded behavior как runtime, а не только как artifact;
- multi-drone Urban workflow under partitions and coordinator loss;
- один и тот же coordination layer поверх разных транспортов;
- систематический live SITL evidence beyond one local PX4 path.

### Принцип следующей фазы

Не надо сейчас вкладываться главным образом в:

- ещё более красивую simulation physics;
- настоящую RF mesh implementation;
- onboard perception / lidar / SLAM / CV;
- контроллер низкого уровня;
- hardware-specific firmware under unknown board.

Надо вкладываться в то, что потом переживёт контакт с реальным железом:

- MAVLink-facing state machines;
- protocol semantics;
- authority / ownership / lease rules;
- deterministic degraded behavior;
- operational evidence discipline.

## Архитектурная граница

### Autopilot owns

PX4, ArduPilot or another FC owns:

- stabilization;
- attitude/rate control;
- motor output;
- low-level waypoint following;
- EKF and vehicle-local state estimation;
- airframe-specific modes;
- onboard failsafes;
- tuning and actuator-level safety.

### This project should own

This project should own:

- mission intent and mission sequencing;
- mission compilation into MAVLink Common plans;
- FC-facing mission upload / fence / param orchestration;
- supervisor lifecycle;
- swarm communication protocol;
- task ownership, reassignment and recovery;
- degraded / partition-aware policy;
- Urban mission semantics;
- replay, metrics, evidence packs and validator rules.

### Key rule

The project should not choose one radio carrier now.

It should define:

- protocol;
- state machines;
- authority model;
- failure model;
- transport boundary.

`mesh`, `LTE modem`, `internet uplink`, `serial relay` and future carrier
choices should fit under that boundary.

## Non-Goals

Do not make these the primary workstream of the next phase:

- writing FC firmware;
- direct motor or offboard low-level control loops;
- physical obstacle avoidance;
- real RF mesh stack;
- perception stack;
- vendor SDK as the main architecture;
- claiming SITL equals hardware;
- claiming dual-stack dry-run equals PX4/ArduPilot equivalence;
- adding more simulation-only missions without strengthening execution and
  communication layers.

## Target State After M96

After `M90-M96` the project should be able to:

- take real mission intent and push it through a live MAVLink-facing
  application layer;
- verify or reject missions against live FC config expectations;
- coordinate several drones through a transport-agnostic swarm protocol;
- survive coordinator, peer and link loss through explicit degraded policy;
- run Urban-first multi-drone workflows with real ownership and reassignment
  semantics;
- keep one code path where transport is replaceable but protocol is stable;
- produce evidence that is meaningful for future hardware-entry planning.

This still is not production readiness.

It is the point where the repository becomes a serious pre-hardware
mission/supervisor/communication stack.

---

## M90 - Live MAVLink Operations Boundary

### Goal

Move from transport-free planning to real MAVLink-facing application logic for
mission execution and FC configuration.

This is the most important immediate step because it converts existing M80-M89
planning/evidence into actual autopilot-facing code.

### Why it matters

Right now:

- `M81` builds `MavlinkCommonPlan`;
- `M82` classifies stack support;
- `M86` models fence and parameter contract;
- `M89` records dual-stack evidence.

But a major part of this is still "plan and report" rather than "apply and
observe". `M90` closes that gap.

### Scope

1. Mission upload/execute state machine:
   - mission count / item upload handshake;
   - mission start path;
   - execute-time ACK correlation;
   - timeout handling;
   - retry budget;
   - explicit abort path;
   - typed execution failures.

2. Live FC config operations:
   - geofence upload execution path;
   - parameter read path;
   - parameter write path;
   - live snapshot validation against `FcParamRequirement`;
   - explicit "blocked by contract" outcome.

3. Stack-specific execution boundary:
   - first-class PX4 path;
   - real ArduPilot-facing API/state-machine path at the same abstraction;
   - profile differences expressed openly, not hidden in generic mission logic.

4. Runtime evidence:
   - upload phases;
   - command ACK mismatches;
   - fence/param apply summaries;
   - execute-time failure reason categories;
   - timing and retry counters.

### Non-Goals

- No real hardware requirement.
- No MAVLink library rewrite.
- No promise that PX4 and ArduPilot behave identically.
- No FC-internal failsafe implementation in this repository.

### Done Criteria

- There is real code for mission upload/start/abort execution flow.
- Fence and param interactions use live transport-facing code paths.
- FC contract can block execution on live snapshot mismatch.
- Failures are structured and observable in reports/replay/artifacts.
- PX4 local SITL path remains the first verified baseline.

### Automated Tests

Tests that need no refactoring:

- mission upload happy path;
- mission upload timeout failure;
- mission start ACK mismatch failure;
- fence upload action ordering;
- parameter snapshot mismatch blocks start;
- retry budget exhaustion returns typed failure.

Tests that need light refactoring:

- mock MAVLink connection fixture with scripted ACK flow;
- reusable execute-time assertion helper for upload/start/abort phases;
- artifact validator support for execute-time FC sections.

Tests that need heavy refactoring:

- local PX4/SIH execute smoke with live fence/param path;
- experimental local ArduPilot SITL execute smoke;
- synthetic packet-loss/reorder execute-time stress harness.

---

## M91 - Swarm Protocol Core

### Goal

Define the real transport-agnostic protocol by which drones, a mothership and
a GCS coordinate mission work.

This is not about choosing `mesh` vs `LTE`. It is about defining:

- message model;
- ownership model;
- liveness model;
- degraded semantics;
- idempotency and conflict behavior.

### Why it matters

Without this milestone, multi-drone work risks staying supervisor-local and
process-local. With this milestone, swarm behavior gains a stable protocol
surface that can later run over different carriers.

### Scope

1. Roles and identities:
   - `gcs`;
   - `coordinator`;
   - `peer_drone`;
   - `reserve_drone`;
   - `mothership`.

2. Core message families:
   - `heartbeat`;
   - `presence`;
   - `capability_advertisement`;
   - `mission_offer`;
   - `mission_accept` / `mission_reject`;
   - `ownership_claim`;
   - `ownership_release`;
   - `lease_renew`;
   - `progress_update`;
   - `replacement_offer`;
   - `abort_notice`;
   - `degraded_notice`;
   - `topology_update`;
   - `mission_result`.

3. Protocol semantics:
   - correlation ids;
   - message ids;
   - TTL / expiry;
   - ack/nack model;
   - retry hints;
   - duplicate suppression;
   - lease-based authority;
   - partial knowledge tolerance.

4. Failure semantics:
   - GCS unavailable;
   - coordinator unavailable;
   - mothership unavailable;
   - isolated peer;
   - stale lease;
   - conflicting ownership;
   - delayed result after reassignment.

5. Observability:
   - protocol trace events;
   - lease history;
   - ownership decisions;
   - degraded transitions;
   - explicit reason codes.

### Non-Goals

- No radio-specific implementation.
- No consensus protocol.
- No PKI/security stack in this phase.
- No full distributed autonomy research agenda.

### Done Criteria

- There is a typed swarm protocol schema.
- Ownership is lease-based and explicit.
- Duplicate and delayed messages are handled deterministically.
- Replay/artifacts can explain why a drone continued, waited, released or
  rejected work.
- Protocol semantics do not depend on one transport.

### Automated Tests

Tests that need no refactoring:

- heartbeat timeout marks peer unavailable;
- stale lease removes authority;
- duplicate message is ignored idempotently;
- delayed result after reassignment is downgraded or rejected deterministically;
- degraded notice changes node state as expected.

Tests that need light refactoring:

- message fixture builder;
- lease clock helper;
- replay assertions for protocol traces;
- shared message-id/correlation-id test helper.

Tests that need heavy refactoring:

- multi-node deterministic protocol simulation harness;
- reordering/drop/duplication matrix;
- property-style protocol fuzzing on retries and duplicates.

---

## M92 - Executable Swarm Runtime

### Goal

Turn `M87-M88` from a strong command-plane/topology contract into a real
runtime coordination layer that uses the swarm protocol and drives multiple
agents as a system.

### Why it matters

Today the project already knows:

- how command-plane artifacts should look;
- how topology routes should be classified;
- how ownership should be recorded.

The next step is making those same semantics executable and stateful.

### Scope

1. Runtime state model:
   - per-agent lifecycle state;
   - per-agent authority/lease state;
   - per-task ownership state;
   - per-session swarm state;
   - protocol-visible degraded state.

2. Execution behavior:
   - assign work through protocol-facing runtime logic;
   - accept or reject work based on authority and capability;
   - reassign unfinished work after failure or lease expiry;
   - enforce single active ownership;
   - generate shared session log.

3. Supervisor responsibilities:
   - maintain session epoch;
   - reconcile delayed reports;
   - reject stale authority;
   - expose explicit command suppression when authority is ambiguous;
   - separate node loss from link loss.

4. Artifact/report outputs:
   - runtime ownership timeline;
   - accepted/rejected offers;
   - lease expiry actions;
   - reassignment cause chain;
   - per-agent lifecycle summary.

### Non-Goals

- No full distributed consensus.
- No real RF routing.
- No onboard autonomy stack rewrite.
- No hardware claim.

### Done Criteria

- There is a protocol-driven multi-agent runtime, not only a manifest builder.
- Task ownership and reassignment are enforced in runtime state.
- Shared event log explains swarm decisions over time.
- Topology and command-plane assumptions survive contact with runtime events.

### Automated Tests

Tests that need no refactoring:

- one task cannot have two active owners;
- reassignment after lease expiry activates only one replacement owner;
- stale completion report is rejected after reassignment;
- ambiguous authority suppresses command dispatch;
- runtime log includes authority decision chain.

Tests that need light refactoring:

- reusable fake protocol runtime fixture;
- shared agent lifecycle assertion helper;
- artifact validator support for runtime ownership timelines.

Tests that need heavy refactoring:

- many-agent in-memory runtime harness;
- repeated reassignment soak test;
- fault-injection runtime simulation with delayed protocol updates.

---

## M93 - Urban Connected Missions

### Goal

Make `urban` the main operational mission family for multi-drone coordination,
not only a semantic planning testbed.

### Why it matters

Urban is already the best fit for the project’s stated priorities:

- real route-based mission logic;
- coordination between several drones;
- explicit sector/corridor ownership;
- natural degraded-connectivity cases;
- realistic supervisor decisions without needing perception or hardware yet.

### Scope

1. Mission families:
   - perimeter patrol by sectors;
   - corridor inspection by route splits;
   - block loop patrol;
   - search-until-detection with sector handoff;
   - unfinished-route recovery after agent loss.

2. Multi-drone mission behavior:
   - segment ownership;
   - area split;
   - unfinished segment release;
   - replacement mission generation;
   - reserve drone activation;
   - explicit no-safe-route outcome.

3. Protocol/runtime integration:
   - ownership and progress updates over swarm protocol;
   - coordinator-to-peer mission fanout;
   - degraded mission continuation under valid lease;
   - checkpoint wait when authority is uncertain.

4. Urban evidence:
   - sector ownership timelines;
   - route handoff events;
   - blocked/degraded mission outcomes;
   - per-drone progress and abandonment summaries.

### Non-Goals

- No real perception.
- No physical collision avoidance.
- No lidar/raycast/SLAM.
- No claim that Urban simulation equals field deployment.

### Done Criteria

- At least one Urban mission family runs as a real multi-drone protocol-aware
  workflow.
- Urban reassignment and handoff are observable and deterministic.
- Degraded mission outcomes are explicit, not silent.
- Urban becomes the main operational proving ground for swarm logic.

### Automated Tests

Tests that need no refactoring:

- sector ownership split stays disjoint;
- unfinished segment is released after owner failure;
- reserve drone takes over unfinished sector deterministically;
- no-safe-route case produces explicit degraded outcome;
- checkpoint wait occurs on ambiguous authority.

Tests that need light refactoring:

- Urban multi-agent fixture builder with protocol events;
- shared route-handoff assertion helper;
- replay summary checks for Urban coordination categories.

Tests that need heavy refactoring:

- generated Urban connected-mission suite;
- wider sector-split combinatorics;
- multi-agent Urban soak with repeated blocked-route and handoff cycles.

---

## M94 - Degraded Connectivity Autonomy

### Goal

Define and implement the policy layer for what each drone does when GCS,
coordinator, mothership or peer links disappear.

### Why it matters

This is one of the most important "field-like" behaviors you can implement
before hardware. It determines whether the swarm layer is robust or only works
under perfect orchestration.

### Scope

1. Connectivity-loss cases:
   - GCS lost, peers still reachable;
   - coordinator lost, peers still reachable;
   - mothership lost, children still active;
   - isolated drone with no peers;
   - split partition with stale knowledge;
   - reconnect after prolonged partition.

2. Policy decisions:
   - continue locally under lease;
   - pause and wait for renew;
   - release ownership after timeout;
   - return/abort if authority is unsafe;
   - allow reserve activation only under explicit rule;
   - reconcile after reconnect instead of blind overwrite.

3. Conflict handling:
   - duplicate completion after reconnect;
   - conflicting ownership claims;
   - delayed topology knowledge;
   - late task release after reassignment;
   - stale coordinator commands.

4. Evidence:
   - degraded state reason;
   - recovery timeline;
   - reconciliation summary;
   - unsafe-action suppression events.

### Non-Goals

- No distributed consensus guarantee.
- No cryptographic trust layer.
- No RF-level validation.
- No autonomous swarm AI beyond explicit operational policy.

### Done Criteria

- Node loss and link loss are distinguished explicitly.
- Degraded decisions follow a documented policy matrix.
- Reconnect reconciliation is deterministic and observable.
- Unsafe actions are blocked rather than silently accepted.

### Automated Tests

Tests that need no refactoring:

- GCS loss enters the configured degraded mode;
- isolated drone continues only while lease is valid;
- stale coordinator command is rejected;
- reconnect triggers reconciliation instead of overwrite;
- duplicate completion after reconnect is handled deterministically.

Tests that need light refactoring:

- degraded policy matrix fixture helper;
- reconnect timeline helper;
- artifact validator checks for degraded/reconciliation sections.

Tests that need heavy refactoring:

- split-brain simulation harness;
- repeated partition/heal stress suite;
- runtime/protocol fuzzing for delayed reconnect storms.

---

## M95 - Dual-Stack Live SITL Evidence

### Goal

Upgrade dual-stack support from dry-run evidence to real local execution
evidence at the same architectural boundary.

### Why it matters

`M89` already proves that:

- the same command IR can be compiled for PX4 and ArduPilot profiles;
- differences are explicit in artifacts.

But that is still dry-run evidence. `M95` should prove that the same runtime
layer can reach real local SITL execution on both stacks, even if the evidence
scope remains conservative.

### Scope

1. PX4 path:
   - preserve the existing verified PX4 baseline;
   - align it with `M90-M94` runtime/evidence structure.

2. ArduPilot path:
   - local SITL execute path at the same API boundary;
   - capability-driven caveats;
   - explicit unsupported or degraded outcomes where necessary.

3. Comparative evidence:
   - same mission intent;
   - same command IR hash;
   - same protocol/runtime architecture;
   - stack-specific differences documented in evidence.

4. Evidence pack:
   - stack profile;
   - FC contract result;
   - execute-time ACK/telemetry summary;
   - known caveats;
   - comparison summary.

### Non-Goals

- No claim of identical PX4/ArduPilot semantics.
- No hardware proof.
- No certification.
- No broad compatibility guarantee across versions and airframes.

### Done Criteria

- There is local live SITL evidence for both stacks at the same runtime
  boundary.
- Profile differences remain explicit and reviewable.
- Evidence is honest about unsupported/degraded behavior.
- Dual-stack claims move beyond dry-run only.

### Automated Tests

Tests that need no refactoring:

- dual-stack evidence pack schema validation;
- same command IR hash appears in both stack artifacts;
- stack caveat summary is preserved in report;
- unsupported stack behavior is explicit, not silent.

Tests that need light refactoring:

- shared dual-stack evidence assertion helper;
- common execute-summary comparator;
- artifact validator support for live dual-stack pack.

Tests that need heavy refactoring:

- local ArduPilot SITL execute smoke harness;
- paired PX4/ArduPilot scenario runner;
- repeated dual-stack comparison sweeps on canonical primitive missions.

---

## M96 - Hardware-Entry Preparation Pack

### Goal

Prepare the repository for a future first hardware experiment without claiming
hardware readiness.

This milestone is about discipline, not about flying.

### Why it matters

If `M90-M95` succeed, the next natural temptation will be "let's try real
hardware". Before that happens, the repository should have one coherent
hardware-entry preparation layer.

### Scope

1. Pre-hardware evidence contract:
   - required dry-run artifact;
   - required FC contract result;
   - required protocol/runtime report;
   - required degraded-policy matrix;
   - required stack profile caveat review.

2. Operational packaging:
   - canonical runbooks for single-drone and future multi-drone experiments;
   - explicit no-go gates;
   - required output artifact bundle;
   - required manual checklist.

3. Boundary honesty:
   - no hidden hardware-ready language;
   - no SITL equals hardware wording;
   - explicit single-drone-first gate;
   - separate review requirement for multi-drone hardware work.

4. Validator/report integration:
   - hardware-entry checklist artifact;
   - proof that prerequisite evidence exists;
   - explicit blocker status when it does not.

### Non-Goals

- No real hardware run.
- No certification package.
- No regulatory approval.
- No product-readiness claim.

### Done Criteria

- Future hardware-candidate work has a single documented preparation pack.
- Required evidence is explicit and machine-checkable where practical.
- Documentation remains conservative and honest.
- The repository can say exactly what is still missing before real hardware.

### Automated Tests

Tests that need no refactoring:

- required preparation-pack files are enforced;
- missing prerequisite evidence yields explicit validator failure;
- forbidden hardware-ready wording is absent from user-facing docs.

Tests that need light refactoring:

- shared docs/status wording assertion helper;
- preparation-pack manifest fixture;
- validator checks for checklist/report linkage.

Tests that need heavy refactoring:

- structured machine-readable readiness manifest across docs and artifacts;
- future multi-pack validation for single-drone vs multi-drone entry paths.

## Итог

Если приоритеты действительно такие:

- real drone commands and real drone code;
- several simultaneous drones;
- swarm behavior;
- communication architecture not tied to one carrier;
- urban-first use cases;

то лучший следующий план - не новая simulation branch и не hardware-specific
fork.

Лучший следующий план - это:

1. довести live MAVLink execution/config boundary;
2. формализовать swarm protocol;
3. сделать command-plane/topology слой исполняемым runtime;
4. использовать `urban` как главный operational proving ground;
5. формализовать degraded autonomy;
6. только потом усиливать dual-stack live evidence и готовить hardware-entry
   discipline.

Это максимально приближает проект к реальному software/hardware stack без
преждевременного выбора конкретной аппаратуры или сети связи.
