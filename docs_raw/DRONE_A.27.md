# DRONE_A.27 - итоговый линейный план: live MAVLink, swarm protocol, urban operations

Дата: 2026-06-06

Источник: сравнение `docs_raw/DRONE_A.26.md`, `docs_raw/DRONE_B.26.md`,
`docs_raw/DRONE_C.26.md`, текущее состояние `HEAD` после `M80-M89`,
`README.md`, `docs/STATUS.md`, `docs/HARDWARE_READINESS.md`,
`docs/SWARM_COMMAND_PLANE.md`, `docs/SWARM_TOPOLOGIES.md`,
`docs/FC_CONTRACT.md` и обсуждение следующих приоритетов:

- реальные дроновые команды и реальный дроновый код;
- несколько одновременно работающих дронов;
- рои дронов;
- протокол/архитектура общения без привязки к одному carrier;
- приоритетный сеттинг `urban`;
- движение к реальному pre-hardware stack без преждевременного ухода в
  физическое железо.

## Краткий вывод

Лучший итоговый план получается не из одной версии `A/B/C.26`, а из их
комбинации:

- `DRONE_C.26` даёт лучший архитектурный backbone:
  `live MAVLink edge -> protocol -> transport -> degraded runtime -> urban`.
- `DRONE_B.26` даёт полезную operational конкретику:
  `DroneMessage`, `AgentMissionState`, autonomous FSM, localhost UDP,
  уход от shared-memory-only coordination.
- `DRONE_A.26` правильно удерживает приоритеты:
  real MAVLink-facing behavior first, urban-first missions, честная
  pre-hardware boundary, без ложных hardware claims.

Итоговая линейная цепочка:

```text
M90 Live MAVLink Config + Execution Layer
  -> M91 Swarm Protocol + Agent FSM
    -> M92 Transport Boundary + Local Multi-Process Runtime
      -> M93 Partition / Degraded Swarm Runtime
        -> M94 Urban Multi-Drone Operational Missions
          -> M95 Dual-Stack Live SITL Evidence
            -> M96 Hardware-Entry Preparation Pack
```

Это не ветвление и не набор альтернатив. Это один последовательный ствол.

## Почему именно такой порядок

### Что уже сделано

После `M80-M89` проект уже умеет:

- выражать mission intent как `MissionCommandPlan`;
- компилировать его в transport-free `MavlinkCommonPlan`;
- явно фиксировать `PX4` / `ArduPilot` capability profiles;
- описывать FC safety / fence / param contract;
- строить swarm command-plane artifacts;
- моделировать logical topologies;
- выпускать honest dry-run and dual-stack evidence.

### Где сейчас главный разрыв

Главный разрыв до "серьёзного боевого software stack" сейчас такой:

1. Есть сильный planning/evidence layer, но live FC-facing execution всё ещё
   недостаточно глубок.
2. Есть сильный command-plane/topology слой, но нет полного transport-agnostic
   swarm protocol runtime.
3. Есть urban semantics, но urban ещё не стал главным полигоном для
   communication-aware multi-drone operations.

### Почему не надо начинать с другого

Сейчас невыгодно делать главным workstream:

- новую simulation physics;
- RF mesh implementation;
- onboard CV/lidar/SLAM;
- hardware-specific board code;
- красивую multi-agent demo без protocol/runtime hardening.

Сначала надо усилить то, что переживёт контакт с реальным железом:

- MAVLink execution/config state machines;
- swarm protocol;
- per-agent autonomy and authority model;
- degraded/partition behavior;
- local multi-process swarm runtime;
- Urban operational workflow as applied proving ground.

## Архитектурная граница

### Autopilot owns

PX4, ArduPilot or another FC owns:

- stabilization;
- attitude and rate control;
- motor output;
- low-level waypoint following;
- EKF / vehicle-local state estimate;
- onboard failsafes;
- airframe-specific mode semantics;
- airframe-specific tuning.

### This project should own

This project should own:

- mission intent and mission sequencing;
- MAVLink command/mission/geofence/param orchestration;
- live FC-facing upload/start/abort/config behavior;
- swarm communication protocol;
- agent autonomy policy and mission FSM;
- task, segment and route ownership;
- reassignment and reconciliation;
- degraded and partition-aware policy;
- Urban mission semantics;
- replay, metrics, validator rules and evidence packs.

### Transport rule

The project should not choose one network carrier now.

It should define:

- protocol schema;
- authority and lease semantics;
- failure and reconnect behavior;
- transport boundary.

Then `in_memory`, `udp_loopback`, `mesh`, `LTE modem`, `internet uplink`,
`serial relay` or future carriers can fit under that boundary.

## Non-Goals

Do not make these the main workstream of `M90-M96`:

- writing FC firmware;
- low-level motor or offboard control loops;
- production RF mesh stack;
- onboard perception stack;
- certified obstacle avoidance;
- claiming dry-run equals hardware;
- claiming SITL equals hardware;
- claiming PX4/ArduPilot equivalence because of shared IR;
- adding many new mission families before execution/protocol/runtime layers are
  hardened.

## Target State After M96

After `M90-M96` the repository should be able to:

- take real mission intent and push it through a real MAVLink-facing
  application layer;
- validate and apply FC execution prerequisites before mission start;
- run a transport-agnostic swarm protocol with explicit authority semantics;
- execute several agents as a distributed runtime, not only as shared-memory
  scenario actors;
- survive GCS/coordinator/neighbor connectivity loss through explicit degraded
  policy;
- use Urban as the main operational proving ground for multi-drone logic;
- produce evidence that is meaningful for future hardware-entry planning.

This still is not product readiness.

It is the point where the repository becomes a serious pre-hardware
mission/supervisor/protocol stack.

---

## M90 - Live MAVLink Config + Execution Layer

### Goal

Move from transport-free mission/config planning to real MAVLink-facing
application logic that can upload, configure, start, observe and abort
missions against a live local SITL endpoint.

### Why it matters

Current milestones already give:

- `M81` transport-free mission compilation;
- `M82` compatibility profiles;
- `M86` FC contract modeling;
- `M89` dry-run dual-stack evidence.

But much of that still ends at "plan and validate". `M90` turns it into real
autopilot-facing code.

### Scope

1. Mission upload/execute state machine:
   - mission count / item upload handshake;
   - expected ACK sequence handling;
   - mission start path;
   - execute-time timeout policy;
   - bounded retry policy;
   - explicit abort/RTL/land fallback path;
   - typed execution failures.

2. FC config execution path:
   - geofence upload from `MavlinkFencePlan`;
   - parameter read path;
   - parameter write path;
   - validation of live `FcParamSnapshot` before start;
   - explicit "blocked by FC contract" outcome.

3. Stack-aware behavior:
   - PX4 path first-class and verified first;
   - ArduPilot path implemented at the same API boundary;
   - stack differences remain explicit in profiles and executor policy.

4. Runtime evidence:
   - upload/start/abort lifecycle events;
   - execute-time ACK mismatch reporting;
   - fence/param apply summaries;
   - timing and retry counters;
   - stable failure reason categories.

### Non-Goals

- No real hardware run.
- No MAVLink stack rewrite.
- No FC-internal failsafe implementation.
- No guarantee that PX4 and ArduPilot semantics are identical.

### Done Criteria

- There is real execute-time mission upload/start/abort logic.
- Fence and param interactions use live transport-facing code.
- Execution can be blocked by live FC contract mismatch.
- Failure outcomes are typed and observable.
- PX4 local SITL path remains the first verified baseline.

### Automated Tests

Tests that need no refactoring:

- mission upload happy path;
- mission upload timeout returns typed failure;
- mission start ACK mismatch returns typed failure;
- abort path is selected after bounded retry exhaustion;
- fence upload sequence ordering is stable;
- parameter snapshot mismatch blocks mission start.

Tests that need light refactoring:

- shared mock MAVLink connection fixture;
- reusable upload/start/abort assertion helper;
- artifact validator support for execute-time fence/param sections.

Tests that need heavy refactoring:

- local PX4/SIH execute-and-config smoke path;
- experimental local ArduPilot SITL execute path;
- synthetic packet-loss and retry stress matrix.

---

## M91 - Swarm Protocol + Agent FSM

### Goal

Define the transport-agnostic swarm protocol and the per-agent mission/autonomy
FSM that together describe how drones, a coordinator, a mothership and a GCS
communicate and react to failures.

### Why it matters

This is the key architectural bridge between "single runtime with swarm
artifacts" and "real distributed multi-agent behavior".

Without it:

- communication remains ad hoc or transport-bound;
- agents remain passive execution targets;
- degraded behavior remains under-specified.

### Scope

1. Typed protocol schema:
   - `DroneMessage`;
   - `DroneMessageEnvelope`;
   - schema version;
   - message id / correlation id;
   - sender/receiver identity;
   - TTL / expiry metadata.

2. Core message families:
   - `heartbeat`;
   - `presence` / `capability_advertisement`;
   - `status_report`;
   - `mission_offer` / `mission_assign`;
   - `mission_accept` / `mission_reject` / `mission_ack`;
   - `ownership_claim`;
   - `ownership_release`;
   - `lease_renew`;
   - `segment_reserve` / `segment_grant` / `segment_deny` / `segment_release`;
   - `replacement_offer`;
   - `abort_notice`;
   - `degraded_notice`;
   - `state_request` / `state_response`;
   - `mission_result`.

3. Agent mission/autonomy state:
   - `idle`;
   - `waiting_for_mission`;
   - `executing_segment`;
   - `waiting_for_segment`;
   - `replanning`;
   - `gcs_lost`;
   - `aborting`;
   - `completed`;
   - `failed`.

4. Failsafe policies:
   - `GcsLostPolicy`;
   - `MothershipLostPolicy`;
   - `NeighborLostPolicy`;
   - policy-specific thresholds and timeout fields.

5. Protocol semantics:
   - lease-based ownership;
   - duplicate suppression;
   - stale message rejection;
   - explicit degraded state transitions;
   - deterministic handling of delayed results after reassignment.

6. Observability:
   - replay protocol message events;
   - replay autonomy/failsafe events;
   - `StateReconcileReport`;
   - protocol and policy summaries in artifacts.

### Non-Goals

- No carrier-specific transport yet.
- No full consensus protocol.
- No security/PKI stack.
- No production distributed autonomy research system.

### Done Criteria

- There is a typed swarm protocol schema with stable serde shape.
- Agent mission state and autonomy policies are explicit and serializable.
- Ownership semantics are lease-based and visible.
- Replay can explain protocol and failsafe decisions.
- Existing scenarios still load with defaulted autonomy fields.

### Automated Tests

Tests that need no refactoring:

- `DroneMessage` serde roundtrip for all variants;
- `AgentMissionState` serde roundtrip for all variants;
- duplicate message is ignored idempotently;
- stale lease removes ownership;
- delayed result after reassignment is downgraded or rejected deterministically;
- GCS loss engages configured policy after threshold;
- reconnect emits `StateReconcileReport`;
- neighbor loss policy updates mission state correctly.

Tests that need light refactoring:

- message fixture builder;
- lease clock helper;
- replay assertions for protocol events;
- partition/reconnect test helper for autonomy policies.

Tests that need heavy refactoring:

- multi-node deterministic protocol simulation harness;
- property-style fuzzing for retries, duplicates and stale messages;
- stress suite with repeated GCS/coordinator loss and reconnect cycles.

---

## M92 - Transport Boundary + Local Multi-Process Runtime

### Goal

Run the same swarm protocol and agent FSM over multiple transports and support
one-agent-per-process local swarm execution on localhost.

### Why it matters

This is where the project stops being only a shared-memory swarm runtime.

After this milestone:

- one process can represent one drone;
- `udp_loopback` can stand in for future real network carriers;
- protocol/runtime logic becomes transport-independent in practice, not only
  in principle.

### Scope

1. Transport interface:
   - send one typed envelope;
   - receive one typed envelope;
   - local node identity;
   - transport error classification;
   - optional delivery metadata;
   - explicit connection class.

2. First transport adapters:
   - `in_memory`;
   - `udp_loopback`;
   - `internet_like_mock`;
   - `serial_placeholder`.

3. Local multi-process workflow:
   - one agent = one process;
   - localhost addressing and launch config;
   - explicit transport selection in CLI/runtime;
   - common artifact/report recording selected transport.

4. Runtime integration:
   - protocol and mission logic do not import transport-specific behavior;
   - dry-run remains transport-free;
   - in-memory and UDP use the same protocol semantics.

### Non-Goals

- No real RF mesh.
- No hardware serial deployment.
- No public stable networking SDK promise.
- No multi-host ops story beyond local/controlled mock execution.

### Done Criteria

- Same protocol logic runs over at least `in_memory` and `udp_loopback`.
- Local multi-process swarm workflow exists and is reproducible.
- Transport selection is explicit and artifact-visible.
- Mission/protocol/runtime layers remain carrier-agnostic.

### Automated Tests

Tests that need no refactoring:

- in-memory send/receive roundtrip;
- udp loopback roundtrip;
- transport selection persists into artifact/report;
- dry-run works without transport;
- same protocol message sequence is accepted on both in-memory and UDP paths.

Tests that need light refactoring:

- common transport conformance helper;
- localhost multi-process fixture launcher;
- CLI parser tests for transport selection.

Tests that need heavy refactoring:

- multi-process UDP swarm harness;
- fault-injection wrapper for delay/drop/duplication;
- transport soak tests across latency and partition distributions.

---

## M93 - Partition / Degraded Swarm Runtime

### Goal

Upgrade the swarm runtime from simple failure handling to real
partition-aware, degraded-network coordination with deterministic authority and
reconciliation semantics.

### Why it matters

This is the point where the system starts to look like a field-oriented swarm
stack rather than a supervisor that assumes perfect communication.

### Scope

1. Degraded conditions:
   - GCS lost, peers still reachable;
   - coordinator lost, peers still active;
   - mothership lost, children still active;
   - one drone isolated from all others;
   - split partition with stale mission knowledge;
   - reconnect after prolonged partition.

2. Policy decisions:
   - continue locally under valid lease;
   - hold and wait for renew;
   - release ownership after timeout;
   - forbid conflicting reassignment until lease expiry;
   - return/abort when authority becomes unsafe;
   - optional fallback coordinator election only when configured.

3. Runtime invariants:
   - no duplicate active ownership after lease accounting;
   - no silent task disappearance;
   - every degraded decision has a reason code;
   - restored connectivity triggers reconciliation, not blind overwrite.

4. Evidence/report outputs:
   - degraded summary;
   - lease expiry decisions;
   - reconciliation timeline;
   - stale-authority suppression events;
   - topology + protocol cause chain.

### Non-Goals

- No consensus algorithm guarantee.
- No cryptographic trust model.
- No radio-level validation.
- No swarm AI beyond explicit operational policy.

### Done Criteria

- Runtime distinguishes node death from link loss.
- Ownership and reassignment remain deterministic under partition.
- Reconnect reconciliation is explicit and reviewable.
- Unsafe stale-authority actions are blocked.

### Automated Tests

Tests that need no refactoring:

- GCS loss produces configured degraded transition;
- isolated agent continues only while lease remains valid;
- stale partitioned owner loses authority after lease expiry;
- reconnect rejects stale ownership and preserves current valid owner;
- restored link emits reconciliation events.

Tests that need light refactoring:

- partition scenario fixture builder;
- lease/reconnect time helper;
- artifact validator support for degraded/reconciliation sections.

Tests that need heavy refactoring:

- large split-brain runtime harness;
- repeated partition/heal reconciliation stress suite;
- protocol/runtime fuzz tests for delayed duplicates after reconnect.

---

## M94 - Urban Multi-Drone Operational Missions

### Goal

Turn `urban` into the main applied proving ground for protocol-aware,
communication-aware multi-drone mission execution.

### Why it matters

Urban is the best match for the stated priorities:

- route-based real mission semantics;
- several simultaneous drones;
- explicit sector/corridor ownership;
- natural degraded-connectivity cases;
- realistic supervisor and recovery decisions without needing physical
  obstacle avoidance yet.

### Scope

1. Mission families:
   - urban perimeter patrol by sectors;
   - urban corridor inspection by route splits;
   - urban block-loop patrol;
   - urban search-until-detection;
   - urban unfinished-segment recovery after failure or partition.

2. Multi-drone coordination:
   - area and segment ownership;
   - patrol sector split;
   - inspection corridor split;
   - handoff of unfinished route segments;
   - reserve drone activation;
   - explicit no-safe-route outcome.

3. Communication-aware behavior:
   - continue locally under valid lease;
   - wait at checkpoint if authority is ambiguous;
   - replacement mission after route release;
   - degraded mission outcome if no safe continuation policy exists.

4. Urban evidence:
   - sector and route ownership timelines;
   - handoff report;
   - segment conflict report;
   - blocked/degraded mission outcomes;
   - coordination/network-induced delay metrics.

### Non-Goals

- No real perception.
- No physical collision avoidance.
- No lidar/raycast/SLAM.
- No claim that Urban simulation equals real field behavior.

### Done Criteria

- At least two meaningful Urban multi-drone mission families run end-to-end
  through command plans, protocol and runtime policy.
- Urban reassignment and handoff are deterministic and observable.
- Degraded mission outcomes are explicit, not silent.
- Urban becomes the main operational setting for swarm behavior validation.

### Automated Tests

Tests that need no refactoring:

- sector ownership split remains disjoint;
- unfinished segment is released after owner failure;
- reserve drone deterministically takes over unfinished sector;
- no-safe-route case produces explicit degraded outcome;
- ambiguous authority causes checkpoint wait;
- degraded Urban mission records coordination delay.

Tests that need light refactoring:

- Urban multi-agent fixture builder with protocol/runtime events;
- route-handoff assertion helper;
- replay summary checks for Urban coordination categories.

Tests that need heavy refactoring:

- generated Urban connected-mission suite;
- wider sector/corridor split combinatorics;
- multi-agent Urban soak with repeated blocked-route and handoff cycles.

---

## M95 - Dual-Stack Live SITL Evidence

### Goal

Upgrade dual-stack support from dry-run evidence to real local SITL execution
evidence on the same runtime and protocol boundary.

### Why it matters

`M89` already proves that the same mission IR can be compiled for PX4 and
ArduPilot profiles, but it is still dry-run evidence only.

`M95` should prove that:

- the same runtime layer can drive both stacks locally;
- stack-specific differences remain explicit;
- dual-stack support is not only compile-time annotation.

### Scope

1. PX4 path:
   - preserve and align current PX4 live SITL baseline with `M90-M94`
     runtime/evidence structure.

2. ArduPilot path:
   - local SITL execute path at the same API boundary;
   - capability-driven caveats;
   - explicit degraded/unsupported outcomes where needed.

3. Comparative evidence:
   - same mission intent;
   - same command IR hash;
   - same protocol/runtime architecture;
   - stack-specific differences documented explicitly.

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
- No broad cross-version compatibility guarantee.

### Done Criteria

- There is local live SITL evidence for both stacks at the same boundary.
- Profile differences remain explicit and reviewable.
- Unsupported or degraded behavior is visible, not silent.
- Dual-stack claims move beyond dry-run only.

### Automated Tests

Tests that need no refactoring:

- dual-stack evidence pack schema validation;
- same command IR hash appears in both stack artifacts;
- stack caveat summary is preserved;
- unsupported stack behavior is explicit.

Tests that need light refactoring:

- shared dual-stack evidence assertion helper;
- common execute-summary comparator;
- artifact validator support for live dual-stack packs.

Tests that need heavy refactoring:

- local ArduPilot SITL execute smoke harness;
- paired PX4/ArduPilot scenario runner;
- repeated dual-stack comparison sweeps on canonical primitive missions.

---

## M96 - Hardware-Entry Preparation Pack

### Goal

Prepare one coherent pre-hardware entry pack for future real-drone experiments
without claiming hardware readiness.

### Why it matters

If `M90-M95` succeed, the natural next question will be whether the repository
is ready for the first hardware candidate experiment.

Before that happens, the project needs one explicit discipline layer that says:

- what evidence is required;
- what checklist must pass;
- what still remains unverified.

### Scope

1. Pre-hardware evidence contract:
   - required dry-run artifact;
   - required FC contract result;
   - required protocol/runtime report;
   - required degraded-policy evidence;
   - required stack-profile caveat review.

2. Operational packaging:
   - canonical runbooks for single-drone and future multi-drone experiments;
   - explicit no-go gates;
   - required output artifact bundle;
   - required manual checklist.

3. Boundary honesty:
   - no hidden hardware-ready language;
   - no SITL-equals-hardware wording;
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

- Future hardware-candidate work has one documented preparation pack.
- Required evidence is explicit and machine-checkable where practical.
- Docs remain conservative and honest.
- The repository can state exactly what is still missing before hardware.

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
- future validation split for single-drone vs multi-drone entry paths.

## Итог

Итоговый план после `M80-M89` должен быть таким:

1. сначала довести live MAVLink execution/config boundary;
2. затем формализовать swarm protocol и agent autonomy/FSM;
3. затем сделать transport boundary и local multi-process runtime;
4. затем harden degraded and partition-aware behavior;
5. затем использовать `urban` как главный applied proving ground;
6. затем перевести dual-stack story из dry-run в live SITL evidence;
7. и только потом собирать coherent hardware-entry preparation pack.

Это лучший следующий линейный ствол, если приоритеты действительно такие:

- real drone code over pure simulation;
- several simultaneous drones;
- swarms and communication architecture;
- no carrier lock-in;
- urban-first mission setting;
- движение к реальному software/hardware stack без преждевременного ухода в
  физическое железо.
