# План реализации M81 - MAVLink Common Compiler

## Context

Нужно реализовать M81 из `docs_raw/DRONE_A.25.md`: компиляцию
`MissionCommandPlan` из `swarm-mission-ir` в transport-free typed
`MavlinkCommonPlan` без привязки mission logic к PX4-only или ArduPilot-only
поведению.

Текущая база:

- M80 уже реализован в crate `swarm-mission-ir`: workspace entry есть в
  `Cargo.toml:3`, `Cargo.toml:14`, dependency alias в `Cargo.toml:30`.
- Core IR primitives находятся в
  `crates/swarm-mission-ir/src/command.rs:13`.
- `MissionCommandPlan` находится в
  `crates/swarm-mission-ir/src/plan.rs:35`.
- IR validation находится в
  `crates/swarm-mission-ir/src/validation.rs:18`.
- Публичные exports crate находятся в
  `crates/swarm-mission-ir/src/lib.rs:13`.
- Dry-run artifact уже содержит `command_ir_summary` в
  `crates/swarm-examples/src/sitl_plan.rs:213` и строит его через
  `build_command_ir_summary` в `crates/swarm-examples/src/sitl_plan.rs:806`.
- Существующий MAVLink upload layer уже умеет typed `MissionItem` upload:
  `crates/swarm-comms/src/mavlink/mission_upload.rs:94`,
  `crates/swarm-comms/src/mavlink/mission_upload.rs:108`.
- Existing `MISSION_ITEM_INT` conversion lives in
  `crates/swarm-comms/src/mavlink/mission_items.rs:118`.
- Existing lifecycle command helpers live in
  `crates/swarm-comms/src/mavlink/commands.rs:14` and
  `crates/swarm-comms/src/mavlink/lifecycle.rs:25`.

Важная граница M81: это **не** hardware upload and not serial/UDP/TCP
transport. На этом этапе нужен deterministic typed plan/artifact, который можно
валидировать и использовать как input для будущего execution layer.

## Investigation context

`INVESTIGATION.md` отсутствует. Дополнительных входных расследований нет.

Notion/GitLab протоколы прочитаны. В пользовательском промпте нет Notion task
ID, GitLab MR или review target; `notion_policy=optional`, поэтому Notion/GitLab
чтение не требуется и не выполнялось.

Локально изучены:

- `docs_raw/DRONE_A.25.md`;
- `docs/MISSION_COMMAND_IR.md`;
- `README.md`;
- `Cargo.toml`;
- `crates/swarm-mission-ir/src/{command.rs,plan.rs,validation.rs,lib.rs}`;
- `crates/swarm-comms/src/mavlink/{commands.rs,lifecycle.rs,mission_items.rs,mission_upload.rs}`;
- `crates/swarm-examples/src/{sitl_plan.rs,artifact_validator.rs}`;
- `crates/swarm-examples/src/sitl_agent_runtime/{cli.rs,runtime.rs}`;
- `crates/swarm-examples/tests/{artifact_validator.rs,sitl_agent.rs,sitl_docs.rs}`.

## Affected components

- `crates/swarm-comms`: новый transport-free MAVLink Common plan/compiler layer.
- `crates/swarm-mission-ir`: возможно, только доп. exports/helpers если
  compiler needs shared plan hashing or summary metadata. Основная M80 IR модель
  должна остаться hardware-agnostic.
- `crates/swarm-examples`: dry-run artifact should expose
  `mavlink_common_plan`; `artifact_validator` should validate the new section.
- `docs/`: README/status/SITL/artifact/IR docs must be updated together with code.
- `Cargo.toml` / crate manifests: add `swarm-mission-ir` dependency to
  `swarm-comms` if compiler lives in `swarm-comms`.

## Implementation steps

1. Add `swarm-mission-ir` dependency to `swarm-comms`.

   Files:

   - `crates/swarm-comms/Cargo.toml`
   - `Cargo.toml` already has workspace dependency at `Cargo.toml:30`.

   Result:

   - `swarm-comms` can compile `MissionCommandPlan` into MAVLink plan data
     without introducing a reverse dependency from `swarm-mission-ir`.
   - No circular dependency: `swarm-mission-ir` remains independent.

2. Add transport-free compiler module in `swarm-comms`.

   Files:

   - `crates/swarm-comms/src/lib.rs`
   - new `crates/swarm-comms/src/mavlink_common_plan.rs`

   Materialized result:

   - Public module exports:
     - `MavlinkCommonPlan`;
     - `MavlinkCommonPlanOptions`;
     - `MavlinkCommonCommand`;
     - `MavlinkCommonMissionItem`;
     - `MavlinkPlanPhase`;
     - `MavlinkExpectedAck`;
     - `MavlinkTelemetryMilestone`;
     - `MavlinkUnsupportedFeature`;
     - `MavlinkCommonCompilerError`;
     - `compile_mavlink_common_plan`.

   Planned contract snippet:

   ```rust
   pub fn compile_mavlink_common_plan(
       plan: &MissionCommandPlan,
       options: &MavlinkCommonPlanOptions,
   ) -> Result<MavlinkCommonPlan, MavlinkCommonCompilerError>;

   pub struct MavlinkCommonPlan {
       pub schema_version: String,
       pub source_mission_id: String,
       pub command_ir_hash: String,
       pub backend_profile: String,
       pub command_prelude: Vec<MavlinkCommonCommand>,
       pub geofence_prelude: Option<Vec<MavlinkCommonMissionItem>>,
       pub mission_items: Vec<MavlinkCommonMissionItem>,
       pub mission_start: Option<MavlinkCommonCommand>,
       pub expected_acks: Vec<MavlinkExpectedAck>,
       pub telemetry_milestones: Vec<MavlinkTelemetryMilestone>,
       pub unsupported_features: Vec<MavlinkUnsupportedFeature>,
       pub validation_result: MavlinkPlanValidationResult,
   }
   ```

   Notes:

   - Do not require `mavlink-transport` feature.
   - Use MAVLink Common command names as strongly typed enum variants or stable
     strings, not raw ad-hoc free text.
   - Keep byte/message serialization out of M81.

3. Refactor coordinate conversion helpers to avoid duplication.

   Files:

   - `crates/swarm-comms/src/mavlink/mission_items.rs:14`
   - new shared pure helpers in `crates/swarm-comms/src/mavlink_common_plan.rs`
     or small `crates/swarm-comms/src/mavlink_coords.rs`

   Materialized result:

   - Existing local-to-WGS84 conversion logic from
     `mission_items.rs:44`, `mission_items.rs:59`, `mission_items.rs:83`,
     `mission_items.rs:96` is shared or mirrored through a single helper.
   - Compiler can convert `Position::Local` into MAVLink plan coordinates using
     `home_origin`.
   - `mission_items.rs` should continue to pass existing MAVLink tests.

   Planned helper shape:

   ```rust
   pub struct MavlinkCoordinateOrigin {
       pub lat_deg: f64,
       pub lon_deg: f64,
       pub alt_m: f64,
   }

   pub fn local_to_mavlink_int(
       x_east_m: f64,
       y_north_m: f64,
       z_relative_m: f64,
       origin: MavlinkCoordinateOrigin,
   ) -> Result<MavlinkIntCoordinate, MavlinkCommonCompilerError>;
   ```

4. Implement command-to-MAVLink mapping.

   Files:

   - `crates/swarm-comms/src/mavlink_common_plan.rs`
   - tests inside the same file or
     `crates/swarm-comms/tests/mavlink_common_plan.rs`

   Materialized result:

   - `MissionCommand::Arm` / `Disarm` compile to
     `MAV_CMD_COMPONENT_ARM_DISARM` in `command_prelude`.
   - `MissionCommand::Takeoff` compiles to `MAV_CMD_NAV_TAKEOFF` in
     `command_prelude`.
   - `MissionCommand::Land` compiles to `MAV_CMD_NAV_LAND`.
   - `MissionCommand::ReturnToLaunch` and `Abort` compile to
     `MAV_CMD_NAV_RETURN_TO_LAUNCH` as command/fallback policy entries.
   - `MissionCommand::GoTo` and `FollowRoute` compile to ordered mission items
     with `MAV_CMD_NAV_WAYPOINT`.
   - `MissionCommand::LoiterTime` compiles to `MAV_CMD_NAV_LOITER_TIME` mission
     item.
   - `Pause` / `Resume` produce structured unsupported features in M81 unless a
     conservative Common mapping is explicitly added.

   Planned compiler behavior:

   ```rust
   match &entry.command {
       MissionCommand::Arm => push_command("MAV_CMD_COMPONENT_ARM_DISARM", [1.0, 0.0, ...]),
       MissionCommand::Disarm => push_command("MAV_CMD_COMPONENT_ARM_DISARM", [0.0, 0.0, ...]),
       MissionCommand::Takeoff { altitude_m } => push_command("MAV_CMD_NAV_TAKEOFF", altitude_params(*altitude_m)),
       MissionCommand::GoTo { position } => push_waypoint_item(entry, position),
       MissionCommand::FollowRoute { waypoints, .. } => push_route_waypoint_items(entry, waypoints),
       MissionCommand::LoiterTime { duration_secs } => push_loiter_time_item(entry, *duration_secs),
       MissionCommand::Orbit { .. } => compile_orbit_or_record_unsupported(...),
       MissionCommand::Pause | MissionCommand::Resume => record_unsupported(...),
       MissionCommand::Land => push_command("MAV_CMD_NAV_LAND", land_params()),
       MissionCommand::ReturnToLaunch | MissionCommand::Abort => push_command("MAV_CMD_NAV_RETURN_TO_LAUNCH", rtl_params()),
   }
   ```

5. Add deterministic orbit fallback.

   Files:

   - `crates/swarm-comms/src/mavlink_common_plan.rs`

   Materialized result:

   - `MavlinkCommonPlanOptions` includes orbit handling:
     - `Unsupported`;
     - `WaypointApproximation { segments_per_turn: u16 }`.
   - With fallback enabled, `Orbit` becomes stable ordered
     `MAV_CMD_NAV_WAYPOINT` mission items.
   - With fallback disabled, `Orbit` records/returns structured unsupported
     feature, not silent success.

   Edge cases to handle:

   - `segments_per_turn == 0` fails validation.
   - Clockwise/counter-clockwise order is deterministic.
   - Approximation point count is bounded and checked against `u16::MAX`.

6. Add plan validation, hash and artifact fields.

   Files:

   - `crates/swarm-comms/src/mavlink_common_plan.rs`

   Materialized result:

   - `command_ir_hash` is deterministic from canonical JSON of
     `MissionCommandPlan`.
   - `expected_acks` lists command ACKs and mission upload ACKs in deterministic
     order.
   - `telemetry_milestones` includes at least:
     - heartbeat expected;
     - command ack expected;
     - mission item reached expected for uploaded route items;
     - terminal state expected from IR.
   - `validation_result` is `passed` only when IR validation passes and no
     unsupported required feature remains.

   Avoid adding a new crypto dependency unless needed. A deterministic stable
   non-cryptographic hash can use `std::collections::hash_map::DefaultHasher`
   over canonical JSON for M81. If a cryptographic artifact identity is needed
   later, defer to schema-versioned follow-up.

7. Integrate compiler into SITL dry-run artifacts.

   Files:

   - `crates/swarm-examples/src/sitl_plan.rs:213`
   - `crates/swarm-examples/src/sitl_plan.rs:763`
   - `crates/swarm-examples/src/sitl_plan.rs:806`
   - `crates/swarm-examples/src/sitl_plan.rs:932`

   Materialized result:

   - `SitlDryRunArtifact` gets optional `mavlink_common_plan`.
   - `build_command_ir_summary` should be complemented by a helper that builds
     full `MissionCommandPlan`, not only summary. Suggested split:
     - `build_command_ir_plan(plan: &SitlPlan) -> Option<MissionCommandPlan>`;
     - `MissionCommandSummary::from_plan(&ir_plan)`;
     - `compile_mavlink_common_plan(&ir_plan, &options)`.
   - Dry-run artifact includes:
     - source mission id;
     - command IR hash;
     - MAVLink command list;
     - mission item list;
     - expected ACKs;
     - telemetry milestones;
     - unsupported/degraded features;
     - validation result.

   Do not change real connection behavior in M81. Existing
   `run_connection` should keep using current upload/lifecycle path.

8. Extend `artifact_validator` for M81 dry-run artifacts.

   Files:

   - `crates/swarm-examples/src/artifact_validator.rs:17`
   - `crates/swarm-examples/src/artifact_validator.rs:63`
   - `crates/swarm-examples/src/artifact_validator.rs:116`
   - `crates/swarm-examples/src/artifact_validator.rs:145`
   - `crates/swarm-examples/tests/artifact_validator.rs:26`

   Materialized result:

   - Add dry-run artifact discovery to `ArtifactPackPaths`:
     - `sitl_dry_run_artifact.v1.json`;
     - `dry-run.json` as legacy/test fallback if existing docs use it.
   - In `ArtifactValidationMode::DryRun`, validate dry-run artifact instead of
     requiring supervisor `manifest.json`.
   - New stable rule ids, for example:
     - `artifact.mavlink_plan_missing`;
     - `artifact.mavlink_plan_schema_unsupported`;
     - `artifact.mavlink_plan_command_missing`;
     - `artifact.mavlink_plan_ack_missing`;
     - `artifact.mavlink_plan_unsupported_required`;
     - `artifact.mavlink_plan_ir_hash_missing`.
   - Validator should check:
     - schema version;
     - command IR hash present;
     - supported command names are non-empty;
     - mission item sequences are contiguous from zero;
     - expected ACK list covers commands/items/start where applicable;
     - `validation_result.passed == false` when unsupported required features
       are present.

9. Add or update CLI tests for dry-run artifact emission.

   Files:

   - `crates/swarm-examples/tests/sitl_agent.rs:1`
   - `crates/swarm-examples/tests/sitl_agent/cli_and_connection_tests.rs`
   - `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs`

   Materialized result:

   - Existing `sitl_agent --dry-run --dry-run-artifact` test asserts
     `mavlink_common_plan` exists.
   - Test covers at least `takeoff`, `land`, route waypoint mission items and
     expected ACK section.
   - Test stays portable and uses only repository fixtures/tempdir.

10. Update documentation and docs smoke tests.

    Files:

    - `README.md:597`
    - `README.md:684`
    - `docs/STATUS.md`
    - `docs/MISSION_COMMAND_IR.md:108`
    - new `docs/MAVLINK_COMMON_COMPILER.md`
    - `docs/SITL_SETUP.md`
    - `docs/ARTIFACT_VALIDATION.md`
    - `docs/EXTENSION_GUIDE.md`
    - `docs/HARDWARE_READINESS.md`
    - `crates/swarm-examples/tests/sitl_docs.rs:1`

    Materialized result:

    - README workspace table documents M81 compiler surface.
    - Milestones section marks M81 complete only after code/tests/docs are done.
    - `docs/MISSION_COMMAND_IR.md` next steps updated: M81 no longer future
      once complete.
    - New docs page explains:
      - supported MAVLink Common commands;
      - non-goals;
      - dry-run artifact shape;
      - PX4/ArduPilot neutrality boundary;
      - no hardware upload claim.
    - `docs/ARTIFACT_VALIDATION.md` documents dry-run mode and M81 rule ids.
    - `docs/SITL_SETUP.md` documents how to emit M81 artifact.
    - `sitl_docs` smoke test checks required phrases:
      - `MAVLink Common Compiler`;
      - `MavlinkCommonPlan`;
      - `MAV_CMD_NAV_TAKEOFF`;
      - `MAV_CMD_NAV_WAYPOINT`;
      - `no hardware upload`;
      - `PX4/ArduPilot semantics are not identical`.

11. Run required checks before commit.

    Commands:

    ```bash
    cargo fmt --all
    make clippy
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-mission-ir
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test artifact_validator
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs
    git diff --check
    ```

    If `make clippy` is unavailable, use repo-approved equivalent:

    ```bash
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    ```

    No long benchmark run is required for M81. No PX4/ArduPilot installation is
    required. No SITL/hardware run is required.

## Testing strategy

### 1. Tests that need no refactoring

Planned together with main functional changes:

- `takeoff` compiles to `MAV_CMD_NAV_TAKEOFF`.
- `land` compiles to `MAV_CMD_NAV_LAND`.
- `return_to_launch` / `abort` compile to
  `MAV_CMD_NAV_RETURN_TO_LAUNCH`.
- `arm` and `disarm` compile to `MAV_CMD_COMPONENT_ARM_DISARM` with param1
  `1.0` / `0.0`.
- `go_to` compiles to one `MAV_CMD_NAV_WAYPOINT` mission item.
- `follow_route` compiles to ordered waypoint mission items.
- `loiter_time` compiles to `MAV_CMD_NAV_LOITER_TIME`.
- unsupported `pause` / `resume` returns structured unsupported feature.
- expected ACK list is deterministic.
- command IR hash is stable for identical input.
- orbit fallback produces stable waypoint ordering when enabled.
- orbit fallback disabled returns structured unsupported feature.
- dry-run artifact serializes/deserializes `mavlink_common_plan`.

### 2. Tests that need light refactoring

- Artifact validator dry-run mode validates M81 `mavlink_common_plan` fields.
- Dry-run CLI test asserts emitted artifact includes command list, mission item
  list, expected ACKs and validation result.
- `sitl_docs` smoke tests enforce README/docs/status/M81 wording.
- Preflight-report-to-command-id link can be added if the current safety report
  can attach command provenance without changing its schema too much. If schema
  change is too broad, document as M82/M86 follow-up.
- Golden artifact fixture for `takeoff -> hold -> land` if inline JSON becomes
  too large for unit tests.

### 3. Tests that need heavy refactoring

- Backend-neutral MAVLink message model replacing or unifying
  feature-gated `mavlink` crate types with transport-free plan types.
- Streaming mission upload state machine tests driven from `MavlinkCommonPlan`.
- Versioned golden artifact schema management.
- SITL-backed compiler conformance checks against PX4/ArduPilot.

## Risks and tradeoffs

- **Where compiler lives.** Putting M81 in `swarm-comms` keeps backend planning
  near MAVLink code and avoids polluting pure IR with MAVLink concepts. The cost
  is a new `swarm-comms -> swarm-mission-ir` dependency.
- **No `mavlink` crate in transport-free plan.** Using stable typed command names
  rather than `mavlink::dialects::common::MavCmd` keeps dry-run available without
  `mavlink-transport`. The cost is mapping duplication risk; tests and docs must
  pin command names.
- **Land as command vs mission item.** M81 should choose one deterministic
  representation. Recommended initial rule: direct `Land` command compiles to
  `MAV_CMD_NAV_LAND` command entry, while route/goto/loiter become mission
  upload items. If later SITL execution needs land as final mission item, add a
  profile/compiler option in M82/M83.
- **Orbit fallback realism.** Waypoint approximation is deterministic but not
  equivalent to autopilot-native orbit. Artifact must record fallback mode and
  approximation count.
- **Artifact schema churn.** Adding `mavlink_common_plan` changes dry-run artifact
  shape. Keep field optional to preserve old artifacts and make validator
  behavior mode-aware.
- **Scope creep into PX4/ArduPilot profiles.** M81 should not implement M82.
  Use `backend_profile: "mavlink_common_generic"` and keep PX4/ArduPilot caveats
  as docs, not behavior.

## Что могло сломаться

- **Поведение dry-run artifacts:** consumers of `sitl_dry_run_artifact.v1.json`
  may see a new optional field. Проверка: `sitl_agent --dry-run
  --dry-run-artifact` test and JSON roundtrip.
- **Existing SITL dry-run output:** refactoring `build_command_ir_summary` into a
  full IR builder may alter `command_ir_summary`. Проверка:
  `cargo test -p swarm-examples --test sitl_agent`.
- **MAVLink coordinate conversion:** shared helper changes could regress
  existing `MISSION_ITEM_INT` conversion. Проверка:
  `cargo test -p swarm-comms`.
- **Artifact validator modes:** dry-run validation changes could accidentally
  require supervisor files. Проверка:
  `cargo test -p swarm-examples --test artifact_validator`.
- **Docs smoke tests:** new required phrases may make docs tests brittle.
  Проверка: `cargo test -p swarm-examples --test sitl_docs`.
- **Feature flags:** adding `swarm-mission-ir` dependency or moving helpers must
  not require `mavlink-transport` for normal builds. Проверка:
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` and, if
  useful, `cargo check --workspace`.
- **API/contract exposure:** public plan structs become extension surface.
  Проверка: docs and schema version explicitly state M81 is pre-hardware,
  not semver-stable public SDK.

## Open questions

- Should M81 use direct `MAV_CMD_NAV_LAND` command entry for `Land`, or append
  `MAV_CMD_NAV_LAND` as a mission item when route mission items exist? Plan
  recommendation: direct command entry for M81, profile option later if needed.
- Should `Pause` / `Resume` be unsupported in M81 or mapped to a conservative
  Common command? Plan recommendation: unsupported until M82 profiles define
  stack-specific behavior.
- Should `command_ir_hash` use `DefaultHasher` over canonical JSON or introduce
  a cryptographic hash dependency? Plan recommendation: `DefaultHasher` for M81,
  no new dependency unless reviewers require stronger artifact identity.
- Should validator accept a direct dry-run artifact path, or only an output dir?
  Plan recommendation: keep current `--output-dir` contract and discover
  `sitl_dry_run_artifact.v1.json` / `dry-run.json`; add direct artifact path only
  if implementation finds current runbooks need it.
