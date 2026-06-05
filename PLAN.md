# Plan: M87 - Swarm Command Plane

## Context

M87 должен поднять проект с уровня "один агент получает один MAVLink/IR command plan" до уровня "supervisor строит согласованный multi-agent command plane": один swarm mission plan, N per-agent command plans, явные роли, ownership, state transitions, synchronized GCS operations и replay/artifact evidence.

Ключевое ограничение: это mission-level coordination. M87 не должен превращаться в RF mesh, distributed consensus, collision avoidance, hardware simultaneous takeoff guarantee или FC-specific execution semantics. Low-level полёт и реакция flight controller остаются вне M87.

Исследованный текущий контекст:

- `docs_raw/DRONE_A.25.md:732` описывает M87 scope/done criteria/test buckets.
- `crates/swarm-types/src/agent.rs:37` уже содержит `Role::{Scout, Relay, Mapper, Inspector, Carrier, Thermal}`. Для M87 нужно расширить роли или добавить command-plane role mapping без ломки существующих сценариев.
- `crates/swarm-mission-ir/src/plan.rs:10` содержит `MissionCommandEntry`, а `crates/swarm-mission-ir/src/plan.rs:35` содержит `MissionCommandPlan` для single-agent command sequence.
- `crates/swarm-comms/src/mavlink_common_plan.rs:29` уже компилирует `MissionCommandPlan` в `MavlinkCommonPlan`; `crates/swarm-comms/src/mavlink_common_plan.rs:109` хранит mission items, ACK expectations, telemetry milestones и FC contract output.
- `crates/swarm-examples/src/sitl_multi_agent.rs:48` содержит `MultiAgentSitlManifest`, `crates/swarm-examples/src/sitl_multi_agent.rs:87` содержит `TaskOwnershipSummary`, а validation уже ловит duplicate agent/task ownership.
- `crates/swarm-examples/src/sitl_supervisor/ports.rs:33` содержит `LiveAgentController`, а `crates/swarm-examples/src/sitl_supervisor/ports.rs:56` содержит `AgentController` с `upload/start/poll/abort`.
- `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:535` содержит fake/controller supervisor loop, который уже можно использовать как тестовую интеграционную точку.
- `crates/swarm-examples/src/sitl_supervisor/config.rs:46` содержит `SupervisorMetrics`; там уже есть reallocation/failure/degraded counters, но нет command fanout/sync-command counters.
- `crates/swarm-replay/src/event_log.rs:28` содержит replay event schema; `crates/swarm-replay/src/event_log.rs:269` уже имеет Urban ownership-like events, но нет generic swarm command-plane ownership/state/sync events.
- `crates/swarm-replay/src/replay/summary.rs:5` содержит `ReplaySummary`; M87 counters отсутствуют.
- `crates/swarm-examples/src/artifact_validator.rs:21` содержит artifact validation rule ids; M87-specific rules отсутствуют.
- `README.md:685` и `docs/STATUS.md:58` отражают M80-M86, но не M87.

Планируемое решение: добавить транспорт-независимый crate `swarm-command-plane`, который принимает уже существующие assignment/manifest inputs, строит per-agent `MissionCommandPlan`, компилирует их в `MavlinkCommonPlan`, валидирует ownership/fanout policies и отдаёт структуры для supervisor/replay/artifacts. SITL supervisor должен стать consumer этого слоя, а не местом, где живёт вся M87 логика.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует, поэтому отдельного investigation artifact нет.

Notion/GitLab протоколы прочитаны:

- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`

В prompt нет Notion task id, GitLab MR или remote target, поэтому Notion/GitLab CLI не запускались. Remote SSH/HTTP не использовались.

## Affected components

1. Workspace/crate wiring:
   - `Cargo.toml`
   - new `crates/swarm-command-plane/Cargo.toml`
   - new `crates/swarm-command-plane/src/lib.rs`

2. Command-plane core:
   - new `crates/swarm-command-plane/src/types.rs`
   - new `crates/swarm-command-plane/src/fanout.rs`
   - new `crates/swarm-command-plane/src/validation.rs`
   - new `crates/swarm-command-plane/src/policy.rs`
   - new `crates/swarm-command-plane/src/sync.rs`
   - new `crates/swarm-command-plane/src/summary.rs`

3. Existing mission/transport integration:
   - `crates/swarm-types/src/agent.rs:37`
   - `crates/swarm-mission-ir/src/plan.rs:10`
   - `crates/swarm-mission-ir/src/plan.rs:35`
   - `crates/swarm-comms/src/mavlink_common_plan.rs:29`
   - `crates/swarm-comms/src/mavlink_common_plan.rs:109`

4. SITL/multi-agent artifacts:
   - `crates/swarm-examples/src/sitl_multi_agent.rs:15`
   - `crates/swarm-examples/src/sitl_multi_agent.rs:48`
   - `crates/swarm-examples/src/sitl_multi_agent.rs:87`
   - `crates/swarm-examples/src/sitl_report.rs:45`
   - `crates/swarm-examples/src/sitl_supervisor/config.rs:46`
   - `crates/swarm-examples/src/sitl_supervisor/ports.rs:33`
   - `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:535`
   - `crates/swarm-examples/src/artifact_validator.rs:21`

5. Replay:
   - `crates/swarm-replay/src/event_log.rs:28`
   - `crates/swarm-replay/src/replay/summary.rs:5`
   - `docs/REPLAY.md`

6. Docs/status:
   - `README.md`
   - `docs/STATUS.md`
   - `docs/HARDWARE_READINESS.md`
   - `docs/OPERATIONAL_RUNBOOKS.md`
   - `docs/ARTIFACT_VALIDATION.md`
   - new `docs/SWARM_COMMAND_PLANE.md`

## Implementation steps

1. Add the `swarm-command-plane` crate and workspace dependency.

   Files:
   - `Cargo.toml`
   - `crates/swarm-command-plane/Cargo.toml`
   - `crates/swarm-command-plane/src/lib.rs`

   Result:
   - Workspace member exists.
   - Workspace dependency `swarm-command-plane = { path = "crates/swarm-command-plane" }` exists.
   - New crate depends on `swarm-types`, `swarm-mission-ir`, `swarm-comms`, `swarm-replay`, `serde`, `thiserror`.
   - Public modules are intentionally small: `types`, `fanout`, `validation`, `policy`, `sync`, `summary`.

   Sketch:

   ```rust
   pub mod fanout;
   pub mod policy;
   pub mod summary;
   pub mod sync;
   pub mod types;
   pub mod validation;

   pub use fanout::{build_swarm_command_plan, SwarmCommandFanoutInput};
   pub use types::{SwarmCommandPlan, SwarmAgentCommandPlan, SwarmSupervisorState};
   pub use validation::{validate_swarm_command_plan, SwarmCommandPlaneError};
   ```

2. Define M87 command-plane data contracts.

   Files:
   - `crates/swarm-command-plane/src/types.rs`
   - `crates/swarm-types/src/agent.rs:37`

   Result:
   - Either extend `swarm_types::Role` with `Observer`, `Leader`, `Coordinator`, `Mothership`, `Reserve`, `Recovery`, or add `SwarmCommandRole` in the new crate with `From<Role>` mapping for existing roles.
   - Preferred approach: add `SwarmCommandRole` in `swarm-command-plane`, keep `swarm-types::Role` stable for existing scenario semantics, and map existing `Role::Scout/Relay/Carrier` into command-plane roles where possible.
   - Add schema-stable structs/enums:
     - `SwarmCommandPlan { schema, plan_id, supervisor_state, agents, ownership, global_abort_policy, sync_operations, summary }`
     - `SwarmAgentCommandPlan { agent_id, role, command_plan, mavlink_plan, expected_acks, telemetry_milestones, abort_policy, ownership_refs }`
     - `SwarmOwnershipKind::{Task, RouteSegment, Target, ReplacementMission}`
     - `SwarmOwnershipRecord`
     - `SwarmSupervisorState::{Planned, Dispatched, Active, Degraded, Replacing, Aborting, Completed, Failed}`
     - `SwarmAbortPolicy::{AbortAgentOnly, AbortMission, ContinueDegraded, ReplaceFromReserve}`
     - `SynchronizedCommandKind::{ArmAll, TakeoffAll, StartAll, AbortAll}`

   Sketch:

   ```rust
   #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
   #[serde(rename_all = "snake_case")]
   pub enum SwarmCommandRole {
       Scout,
       Observer,
       Relay,
       Leader,
       Coordinator,
       Mothership,
       Carrier,
       Reserve,
       Recovery,
   }

   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct SwarmAgentCommandPlan {
       pub agent_id: AgentId,
       pub role: SwarmCommandRole,
       pub command_plan: MissionCommandPlan,
       pub mavlink_plan: MavlinkCommonPlan,
       pub expected_acks: Vec<String>,
       pub telemetry_milestones: Vec<String>,
       pub abort_policy: SwarmAbortPolicy,
       pub ownership_refs: Vec<SwarmOwnershipRef>,
   }
   ```

3. Implement command fanout from existing assignments/manifests into per-agent command plans.

   Files:
   - `crates/swarm-command-plane/src/fanout.rs`
   - `crates/swarm-mission-ir/src/plan.rs:10`
   - `crates/swarm-mission-ir/src/plan.rs:35`
   - `crates/swarm-comms/src/mavlink_common_plan.rs:29`

   Result:
   - `build_swarm_command_plan(input, options)` creates exactly one `SwarmAgentCommandPlan` per assigned agent.
   - Each per-agent `MissionCommandEntry` preserves `source_agent_id`, `source_task_id`, and route/target metadata where available.
   - Each per-agent `MissionCommandPlan` is compiled through existing `compile_mavlink_common_plan` so M81-M86 behavior remains reused instead of reimplemented.
   - Unassigned reserve/recovery agents may be represented as `Reserve` plans with no mission items but valid abort/replacement policy.

   Sketch:

   ```rust
   pub fn build_swarm_command_plan(
       input: SwarmCommandFanoutInput,
       options: SwarmCommandFanoutOptions,
   ) -> Result<SwarmCommandPlan, SwarmCommandPlaneError> {
       let mut agents = Vec::new();
       for assignment in input.assignments {
           let command_plan = build_agent_mission_command_plan(&assignment, &options)?;
           let mavlink_plan = compile_mavlink_common_plan(&command_plan, options.mavlink.clone())?;
           agents.push(SwarmAgentCommandPlan::from_parts(assignment, command_plan, mavlink_plan)?);
       }
       let plan = SwarmCommandPlan::new(input.plan_id, agents, options.global_abort_policy);
       validate_swarm_command_plan(&plan)?;
       Ok(plan)
   }
   ```

4. Add command-plane validation with explicit failure codes.

   Files:
   - `crates/swarm-command-plane/src/validation.rs`
   - `crates/swarm-command-plane/src/types.rs`

   Result:
   - Validation rejects duplicate ownership of the same `(kind, resource_id)` unless it is represented as a handoff with a release/acquire transition.
   - Validation rejects duplicate agent command plans.
   - Validation rejects command entries whose `source_agent_id` contradicts the owning agent.
   - Validation rejects replacement policy that references no reserve/recovery agent when policy is `ReplaceFromReserve`.
   - Validation rejects global abort policy that cannot produce per-agent abort commands.
   - Errors are structured and stable enough for artifact validator and tests.

   Sketch:

   ```rust
   #[derive(Debug, thiserror::Error, PartialEq, Eq)]
   pub enum SwarmCommandPlaneError {
       #[error("duplicate command plan for agent {agent_id}")]
       DuplicateAgentPlan { agent_id: AgentId },
       #[error("duplicate ownership for {kind:?}:{resource_id}")]
       DuplicateOwnership { kind: SwarmOwnershipKind, resource_id: String },
       #[error("replacement policy requires reserve or recovery agent")]
       MissingReplacementAgent,
   }
   ```

5. Implement per-agent failure policy and replacement/abort decisions.

   Files:
   - `crates/swarm-command-plane/src/policy.rs`
   - `crates/swarm-examples/src/sitl_supervisor/config.rs:132`
   - `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:535`

   Result:
   - `apply_agent_failure(plan, failed_agent_id)` returns a deterministic `SwarmFailureDecision`.
   - Supported decisions:
     - `AbortAgentOnly`
     - `AbortMission`
     - `ContinueDegraded`
     - `ReplaceFromReserve { survivor_agent_id, recovered_ownership }`
   - Existing `MissionReplacementPlan` remains usable, but M87 should wrap/derive it from command-plane ownership instead of duplicating policy logic in supervisor flows.
   - Fake supervisor tests can trigger a failed agent and assert replacement or global abort by policy.

   Sketch:

   ```rust
   pub enum SwarmFailureDecision {
       AbortAgentOnly { agent_id: AgentId },
       AbortMission { failed_agent_id: AgentId, abort_agent_ids: Vec<AgentId> },
       ContinueDegraded { failed_agent_id: AgentId, released: Vec<SwarmOwnershipRecord> },
       ReplaceFromReserve {
           failed_agent_id: AgentId,
           replacement_agent_id: AgentId,
           handoffs: Vec<SwarmOwnershipHandoff>,
       },
   }
   ```

6. Represent synchronized GCS operations and fake-test partial success.

   Files:
   - `crates/swarm-command-plane/src/sync.rs`
   - `crates/swarm-examples/src/sitl_supervisor/ports.rs:33`
   - `crates/swarm-examples/src/sitl_supervisor/tests_support.rs`

   Result:
   - Add `SynchronizedCommandWindow` and `SynchronizedCommandResult`.
   - Add a small fake sync executor or fake controller extension that can deterministically return:
     - all success;
     - one failed agent;
     - one timed-out agent;
     - partial success below/above threshold.
   - Do not require real MAVLink/PX4 execution for M87.
   - Keep real `LiveAgentController` behavior unchanged unless a later M89 SITL integration needs it.

   Sketch:

   ```rust
   pub struct SynchronizedCommandWindow {
       pub kind: SynchronizedCommandKind,
       pub agent_ids: Vec<AgentId>,
       pub timeout_ms: u64,
       pub partial_success_policy: PartialSuccessPolicy,
   }

   pub struct SynchronizedCommandResult {
       pub succeeded: Vec<AgentId>,
       pub failed: Vec<AgentId>,
       pub timed_out: Vec<AgentId>,
       pub accepted: bool,
   }
   ```

7. Add generic M87 replay events and summary counters.

   Files:
   - `crates/swarm-replay/src/event_log.rs:28`
   - `crates/swarm-replay/src/replay/summary.rs:5`
   - `docs/REPLAY.md`

   Result:
   - Replay can explain command fanout, ownership transitions, supervisor state changes, and synchronized command outcomes.
   - Add event variants:
     - `SwarmCommandPlanDispatched`
     - `SwarmAgentCommandDispatched`
     - `SwarmOwnershipAcquired`
     - `SwarmOwnershipReleased`
     - `SwarmOwnershipHandoff`
     - `SwarmSupervisorStateChanged`
     - `SwarmSyncCommandIssued`
     - `SwarmSyncCommandResult`
   - Update `ReplaySummary` with counters:
     - `swarm_command_plan_dispatched_count`
     - `swarm_agent_command_dispatched_count`
     - `swarm_ownership_handoff_count`
     - `swarm_sync_partial_failure_count`
     - `swarm_supervisor_state_change_count`
   - Keep existing Urban events as domain-specific events; optionally bridge them into generic command-plane ownership only in report generation, not by deleting M85 event types.

8. Add command-plane sections to SITL manifests/reports without breaking old artifacts.

   Files:
   - `crates/swarm-examples/src/sitl_multi_agent.rs:15`
   - `crates/swarm-examples/src/sitl_multi_agent.rs:48`
   - `crates/swarm-examples/src/sitl_report.rs:45`
   - `crates/swarm-examples/src/sitl_supervisor/config.rs:46`

   Result:
   - `MultiAgentSitlAgentConfig` gets optional/default command-plane role/policy fields.
   - `MultiAgentSitlManifest` gets optional `command_plane` section with `#[serde(default, skip_serializing_if = "Option::is_none")]` for backward compatibility.
   - `SitlMultiAgentRunReport` gets optional command-plane summary fields.
   - Existing `multi_sitl.v1` artifacts remain parseable.
   - M87 artifacts can include `swarm_command_plane.v1` without pretending that old M55/M58/M59 artifacts had this evidence.

   Suggested compatibility rule:

   ```rust
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub command_plane: Option<SwarmCommandArtifactSummary>,
   ```

9. Integrate command-plane output into supervisor fake/controller flow.

   Files:
   - `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:535`
   - `crates/swarm-examples/src/sitl_supervisor/ports.rs:56`
   - `crates/swarm-examples/src/sitl_supervisor/tests_support.rs`

   Result:
   - Fake/controller supervisor can build a command-plane plan before upload/start.
   - Supervisor records dispatch/state/ownership events into event log.
   - Failure path uses `apply_agent_failure` to decide replacement/abort.
   - Global abort emits one per-agent abort command result in fake tests.
   - This step must not require parallel real PX4 execution; it exercises the M87 semantics over existing fake controllers.

10. Extend artifact validation for M87.

   Files:
   - `crates/swarm-examples/src/artifact_validator.rs:21`
   - `docs/ARTIFACT_VALIDATION.md`

   Result:
   - Add rule ids:
     - `artifact.swarm_command_plane_missing`
     - `artifact.swarm_agent_plan_missing`
     - `artifact.swarm_duplicate_ownership`
     - `artifact.swarm_ack_mismatch`
     - `artifact.swarm_handoff_missing`
     - `artifact.swarm_sync_partial_unreported`
   - Existing historical artifacts should not fail M87 validation unless the user explicitly asks for M87 strict mode.
   - M87 result packs should fail validation if command-plane sections or replay categories are missing.

11. Add docs/status updates as part of the implementation commit.

   Files:
   - `README.md`
   - `docs/STATUS.md`
   - new `docs/SWARM_COMMAND_PLANE.md`
   - `docs/REPLAY.md`
   - `docs/ARTIFACT_VALIDATION.md`
   - `docs/HARDWARE_READINESS.md`
   - `docs/OPERATIONAL_RUNBOOKS.md`

   Result:
   - README gains an M87 status row and concise user-facing description.
   - `docs/STATUS.md` marks M87 as fake-tested mission-level command-plane foundation, not real RF mesh or hardware guarantee.
   - `docs/SWARM_COMMAND_PLANE.md` documents roles, policies, states, ownership kinds, synchronized GCS operations, artifact schema, replay semantics, and non-goals.
   - `docs/REPLAY.md` documents new M87 events and summary counters.
   - `docs/ARTIFACT_VALIDATION.md` documents M87 validation rules and historical-artifact behavior.
   - `docs/HARDWARE_READINESS.md` states that M87 does not prove simultaneous hardware takeoff or FC equivalence.
   - `docs/OPERATIONAL_RUNBOOKS.md` gets a fake/local M87 workflow and explicitly says no long benchmark/SITL/HIL/hardware run is required for M87.

12. Add/adjust automated tests and run checks.

   Files:
   - new `crates/swarm-command-plane/src/*` unit tests
   - `crates/swarm-replay/src/event_log.rs`
   - `crates/swarm-replay/src/replay/summary.rs`
   - `crates/swarm-examples/src/sitl_multi_agent.rs`
   - `crates/swarm-examples/src/artifact_validator.rs`
   - `crates/swarm-examples/tests/sitl_docs.rs`

   Result:
   - Happy path, negative path, and edge cases are covered by automated tests.
   - No manual PX4/SITL/HIL/hardware run is required for M87.
   - No benchmark run is required for M87.

   Required implementation checks:

   ```bash
   cargo fmt --all
   /home/formi/.local/bin/runlim cargo clippy --workspace --all-targets -- -D warnings
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-command-plane -- --nocapture
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay -- --nocapture
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs -- --nocapture
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples artifact_validator -- --nocapture
   git diff --check
   rg --files -g '*.proptest-regressions'
   ```

   If `make clippy` is the repo-approved wrapper at implementation time, use it instead of direct `cargo clippy`; otherwise direct workspace clippy is acceptable.

## Testing strategy

### 1. Tests that need no refactoring

These should be implemented together with the main M87 changes.

- `swarm-command-plane` unit test: fanout creates exactly one `SwarmAgentCommandPlan` per assigned active agent.
- `swarm-command-plane` unit test: unassigned reserve/recovery agent can exist as a reserve plan without mission items.
- `swarm-command-plane` unit test: duplicate task ownership fails validation.
- `swarm-command-plane` unit test: duplicate route/segment ownership fails validation.
- `swarm-command-plane` unit test: handoff releases old ownership and acquires new ownership without duplicate-ownership error.
- `swarm-command-plane` unit test: failed agent with `ReplaceFromReserve` selects deterministic reserve/recovery agent.
- `swarm-command-plane` unit test: failed agent with `AbortMission` emits per-agent abort targets.
- `swarm-command-plane` unit test: global abort emits per-agent abort commands for all active/dispatched agents.
- `swarm-command-plane` serde test: roles, states, policies, ownership kinds serialize in `snake_case`.
- `swarm-command-plane` unit test: per-agent ACK expectations and telemetry milestones match the compiled `MavlinkCommonPlan`.
- `swarm-command-plane` sync test: `ArmAll` all-success is accepted.
- `swarm-command-plane` sync test: `TakeoffAll` one failed agent produces deterministic partial failure.
- `swarm-command-plane` sync test: command timeout is represented separately from command failure.
- `swarm-replay` event serde test: new M87 events roundtrip JSON.
- `swarm-replay` summary test: ownership handoff and sync partial failure increment M87 counters.
- `swarm-examples` compatibility test: existing `MultiAgentSitlManifest` JSON without command-plane section still deserializes.
- `swarm-examples` docs smoke test: README/STATUS mention M87 limitations and do not claim RF mesh/hardware guarantees.

### 2. Tests that need light refactoring

These are still expected in the M87 implementation, but require small helper extraction or fixture cleanup.

- Add a shared scenario fixture for scout/reserve replacement in `crates/swarm-examples/src/sitl_supervisor/tests_support.rs`.
- Add artifact validator fixture helper for M87 result packs, then test missing command-plane section and duplicate ownership rule ids.
- Add metrics/report helper to assert command success/failure per agent without duplicating JSON traversal in tests.
- Add fake synchronized command controller/window helper for `arm_all`, `takeoff_all`, `start_all`, `abort_all`.
- Add a replay assertion helper for ordered ownership transition events.

### 3. Tests that need heavy refactoring

These should be documented as future work unless implementation chooses the larger refactor explicitly.

- Transport-independent swarm executor that can run command windows over MAVLink, mock, and future transports through the same trait.
- Temporal route/segment reservation across agents, including overlap in time and not only duplicate static ownership.
- CBBA/gossip integration through command-plane events instead of centralized assignment input only.
- Concurrent agent command runner that starts/polls multiple agents in parallel and handles races between completion, failure, abort, and replacement.
- Real multi-agent PX4/SITL synchronized command execution. This belongs closer to M89 and must not block M87 fake-tested foundation.

## Risks and tradeoffs

- New crate vs extending `swarm-examples`: a new crate is slightly more wiring, but avoids trapping reusable command-plane semantics inside SITL binaries. This is the preferred tradeoff.
- Role duplication risk: `swarm-types::Role` already has scenario roles. Adding `SwarmCommandRole` avoids breaking existing semantics, but creates mapping code. This is acceptable because M87 roles are command-plane roles, not necessarily physical capability roles.
- Schema compatibility risk: adding required fields to `MultiAgentSitlManifest` would break historical artifacts. Use optional/default command-plane sections and strict validation only for M87 artifacts.
- Ownership overlap with Urban M85: Urban segment lock events should remain domain-specific. M87 should add generic ownership events and optionally bridge Urban segment ownership into summaries, not delete or rewrite M85 semantics.
- Fake-test limitation: synchronized GCS operation tests can prove command-plane policy handling, not real flight-controller behavior. Docs must say this clearly.
- Scope creep into M88/M89: topology routing and real dual-stack SITL are later milestones. M87 should define command-plane contracts that they can consume, not implement those milestones early.
- Artifact size: per-agent command plans plus MAVLink plans may enlarge reports. Summaries should be compact, and full plans should be optional or stored once per artifact.
- Performance: fanout is small compared with benchmark runs, but validation should use maps/sets for ownership checks to avoid quadratic behavior on larger mission packs.

## Open questions

1. Should `SwarmCommandRole` stay separate from `swarm_types::Role`, or should `Role` be expanded? Recommended answer for M87: keep `SwarmCommandRole` separate and add explicit mapping from existing roles.

2. Should `MultiAgentSitlManifest` schema be bumped from `multi_sitl.v1` to `multi_sitl.v2`? Recommended answer for M87: keep old schema parse-compatible, add optional command-plane section, and use `swarm_command_plane.v1` as the new embedded schema marker.

3. What is the default partial-success policy for synchronized commands? Recommended M87 default: `ArmAll` and `AbortAll` require all target agents; `TakeoffAll` and `StartAll` default to all target agents for safety, with configurable fake-test-only threshold support.

4. Should real MAVLink synchronized GCS commands be wired in M87? Recommended answer: no. M87 should represent and fake-test synchronized commands. Real PX4/ArduPilot execution should be handled in M89 or a dedicated later milestone.

5. Should command-plane validation become mandatory for all existing multi-agent artifacts? Recommended answer: no. Historical artifacts should remain readable; M87 strict validation should apply only when a command-plane section is present or when a new explicit validation mode is selected.
