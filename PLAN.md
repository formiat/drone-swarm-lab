# Context

Планируем M82 — `PX4 / ArduPilot Capability Profiles` из
`docs_raw/DRONE_A.25.md`.

Текущее состояние:

- M80 уже создал hardware-agnostic `MissionCommandPlan` в
  `crates/swarm-mission-ir`.
- M81 уже компилирует IR в transport-free `MavlinkCommonPlan` через
  `compile_mavlink_common_plan` в
  `crates/swarm-comms/src/mavlink_common_plan.rs:21`.
- `MavlinkCommonPlan` сейчас содержит `backend_profile`, `command_prelude`,
  `mission_items`, `mission_start`, `command_postlude`, `expected_acks`,
  `telemetry_milestones`, `unsupported_features` и `validation_result`
  (`crates/swarm-comms/src/mavlink_common_plan.rs:88`).
- `MavlinkCommonPlanOptions` сейчас имеет строковый `backend_profile`,
  `home_origin`, `default_hold_position` и `orbit_strategy`
  (`crates/swarm-comms/src/mavlink_common_plan.rs:38`).
- Dry-run artifact хранит `mavlink_common_plan: Option<MavlinkCommonPlan>`
  (`crates/swarm-examples/src/sitl_plan.rs:220`) и создаётся в
  `dry_run_artifact` (`crates/swarm-examples/src/sitl_plan.rs:772`).
- CLI `sitl_agent` пока не умеет выбирать MAVLink profile: аргументы парсятся
  в `crates/swarm-examples/src/sitl_agent_runtime/cli.rs:51`, а dry-run пишет
  artifact в `crates/swarm-examples/src/sitl_agent_runtime/runtime.rs:91`.
- `artifact_validator --mode dry-run` уже проверяет M81 section
  (`crates/swarm-examples/src/artifact_validator.rs:299`), но не проверяет
  profile compatibility.

Архитектурное решение для M82: сохранить M81 как generic compiler, а M82
сделать отдельным compatibility/profile pass поверх уже скомпилированного
`MavlinkCommonPlan`. Profile pass может аннотировать или отклонять, но не должен
молча менять mission semantics.

# Investigation context

`INVESTIGATION.md` отсутствует.

Прочитаны локальные входные данные:

- `/home/formi/Documents/RustProjects/drone/.agent-io/inbox.txt`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`;
- `docs_raw/DRONE_A.25.md`, блок M82;
- `crates/swarm-comms/src/mavlink_common_plan.rs`;
- `crates/swarm-examples/src/sitl_plan.rs`;
- `crates/swarm-examples/src/sitl_agent_runtime/cli.rs`;
- `crates/swarm-examples/src/sitl_agent_runtime/runtime.rs`;
- `crates/swarm-examples/src/artifact_validator.rs`;
- `crates/swarm-examples/tests/artifact_validator.rs`;
- `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs`;
- `crates/swarm-examples/tests/sitl_docs.rs`;
- `README.md`, `docs/MAVLINK_COMMON_COMPILER.md`,
  `docs/MISSION_COMMAND_IR.md`, `docs/SITL_SETUP.md`,
  `docs/HARDWARE_READINESS.md`, `docs/STATUS.md`.

Notion/GitLab remote reads не выполнялись: prompt не содержит Notion task id,
GitLab MR или review target; `notion_policy=optional`, а обязательные протоколы
прочитаны как локальные инструкции.

# Affected components

- `crates/swarm-comms/src/mavlink_capability_profile.rs` — новый модуль M82 с
  typed profile model, static profile data, classification logic и tests.
- `crates/swarm-comms/src/mavlink_common_plan.rs:38` — расширение
  `MavlinkCommonPlanOptions` выбранным profile id и добавление compatibility
  report в `MavlinkCommonPlan`.
- `crates/swarm-comms/src/lib.rs:12` — re-export новых M82 типов и функций.
- `crates/swarm-examples/src/sitl_plan.rs:220` и
  `crates/swarm-examples/src/sitl_plan.rs:772` — запись выбранного profile и
  compatibility report в dry-run artifact.
- `crates/swarm-examples/src/sitl_agent_runtime/cli.rs:5` и
  `crates/swarm-examples/src/sitl_agent_runtime/cli.rs:51` — новый CLI flag для
  выбора profile.
- `crates/swarm-examples/src/sitl_agent_runtime/runtime.rs:91` — передача
  profile в dry-run artifact writer.
- `crates/swarm-examples/src/artifact_validator.rs:42` и
  `crates/swarm-examples/src/artifact_validator.rs:332` — новые rule ids и
  checks для compatibility section.
- `crates/swarm-examples/tests/*` и `crates/swarm-comms/src/*` tests —
  автоматическое покрытие profile classification, CLI, artifact, docs.
- User-facing docs: `README.md`, `docs/STATUS.md`,
  `docs/MAVLINK_COMMON_COMPILER.md`, новый
  `docs/MAVLINK_CAPABILITY_PROFILES.md`, `docs/MISSION_COMMAND_IR.md`,
  `docs/SITL_SETUP.md`, `docs/HARDWARE_READINESS.md`,
  `docs/EXTENSION_GUIDE.md`, `docs/ARTIFACT_VALIDATION.md`.

# Implementation steps

1. Добавить typed capability profile model в `swarm-comms`.

   Файлы:

   - создать `crates/swarm-comms/src/mavlink_capability_profile.rs`;
   - обновить `crates/swarm-comms/src/lib.rs:1` и re-exports в
     `crates/swarm-comms/src/lib.rs:12`.

   Материализуемый результат:

   - профильные типы существуют в коде, сериализуются через `serde`, доступны
     публично из `swarm_comms`.
   - есть три профиля: `mavlink_common_generic`, `px4`, `ardupilot`.
   - профили заданы как data/static definitions, не как комментарии в docs.

   Контракт типов:

   ```rust
   pub enum MavlinkCapabilityProfileId {
       MavlinkCommonGeneric,
       Px4,
       ArduPilot,
   }

   pub enum MavlinkCompatibilityClass {
       Supported,
       SupportedWithCaveats,
       RequiresStackSpecificMapping,
       SupportedViaFallback,
       Unsupported,
       UnknownUntilSitlOrHardware,
   }

   pub struct MavlinkCapabilityProfile {
       pub id: MavlinkCapabilityProfileId,
       pub stack_name: &'static str,
       pub supported_frames: &'static [&'static str],
       pub command_rules: &'static [MavlinkCommandCapabilityRule],
       pub mission_start_semantics: &'static str,
       pub takeoff_landing_constraints: &'static [&'static str],
       pub geofence_support: MavlinkCompatibilityClass,
       pub parameter_support: MavlinkCompatibilityClass,
       pub known_caveats: &'static [&'static str],
   }
   ```

   Profile data must stay conservative:

   - `mavlink_common_generic` describes syntax-level Common support only.
   - `px4` may mark current core commands as supported or
     `supported_with_caveats` where the existing local PX4/SIH path has been
     exercised.
   - `ardupilot` should not pretend evidence exists; uncertain mode/start/orbit
     behavior should be `unknown_until_sitl_or_hardware` or
     `requires_stack_specific_mapping`.

2. Добавить compatibility pass поверх `MavlinkCommonPlan`.

   Файлы:

   - `crates/swarm-comms/src/mavlink_capability_profile.rs`;
   - `crates/swarm-comms/src/mavlink_common_plan.rs:88`;
   - `crates/swarm-comms/src/mavlink_common_plan.rs:708`.

   Материализуемый результат:

   - новая функция классифицирует уже скомпилированный generic plan без
     изменения команд, mission item ordering или параметров;
   - output содержит per-command/per-item classification, aggregate result,
     caveats и hardware-facing block flag;
   - `validation_result.passed` должен учитывать required unsupported profile
     failures, но dry-run-only unknown/caveats остаются видимыми как report,
     а не как silent success.

   Псевдокод:

   ```rust
   pub fn classify_mavlink_plan_compatibility(
       plan: &MavlinkCommonPlan,
       profile: &MavlinkCapabilityProfile,
   ) -> MavlinkCompatibilityReport {
       let mut items = Vec::new();
       for command in plan.command_prelude.iter()
           .chain(plan.mission_start.iter())
           .chain(plan.command_postlude.iter()) {
           items.push(classify_command(command.command, command.phase, profile));
       }
       for item in &plan.mission_items {
           items.push(classify_mission_item(item.command, item.frame.as_str(), profile));
       }
       MavlinkCompatibilityReport::from_items(profile.id, items)
   }
   ```

   В `MavlinkCommonPlan` добавить:

   ```rust
   #[serde(default, skip_serializing_if = "Option::is_none")]
   pub compatibility: Option<MavlinkCompatibilityReport>;
   ```

   Новые artifacts должны всегда писать `Some(report)`. `serde(default)` нужен,
   чтобы старые M81 JSON artifacts продолжали читаться validator-ом.

3. Расширить `MavlinkCommonPlanOptions` и compiler wiring.

   Файлы:

   - `crates/swarm-comms/src/mavlink_common_plan.rs:38`;
   - `crates/swarm-comms/src/mavlink_common_plan.rs:51`;
   - `crates/swarm-comms/src/mavlink_common_plan.rs:708`.

   Материализуемый результат:

   - добавить поле `capability_profile: MavlinkCapabilityProfileId` или
     аналогичный typed selector;
   - default должен остаться `mavlink_common_generic`, чтобы existing PX4/SITL
     and dry-run paths remained backward compatible;
   - `backend_profile` в artifact должен соответствовать выбранному profile
     label, но если string поле сохраняется для совместимости, оно не должно
     конфликтовать с typed selector;
   - `compile_mavlink_common_plan` сначала строит generic Common plan, затем
     прикрепляет compatibility report.

   Не делать:

   - не менять `MAVLINK_COMMON_PLAN_SCHEMA_VERSION` только ради optional
     backward-compatible field;
   - не добавлять real MAVLink upload или stack-specific command shims;
   - не менять M81 mapping silently на PX4/ArduPilot-specific behavior.

4. Реализовать conservative profile rules для core primitive missions.

   Файл:

   - `crates/swarm-comms/src/mavlink_capability_profile.rs`.

   Материализуемый результат:

   - `ComponentArmDisarm`, `NavTakeoff`, `NavWaypoint`, `NavLoiterTime`,
     `NavLand`, `NavReturnToLaunch`, `MissionStart` получают явные rules для
     generic, PX4 и ArduPilot;
   - `MAV_FRAME_GLOBAL_RELATIVE_ALT_INT` из текущего compiler output
     (`crates/swarm-comms/src/mavlink_common_plan.rs:19`) явно поддержан или
     классифицирован с caveat per profile;
   - orbit fallback remains `supported_via_fallback` only when current M81
     waypoint approximation emitted waypoints; direct orbit остается
     `requires_stack_specific_mapping` или `unknown_until_sitl_or_hardware`;
   - geofence/parameter support fields существуют, но first implementation can
     mark them as `unknown_until_sitl_or_hardware` / future until M86.

   Минимальные expected classifications:

   - generic: syntax-level supported for emitted Common commands, with caveat
     “generic Common does not prove autopilot acceptance”.
   - PX4: primitive hover/takeoff-land route commands are supported or
     `supported_with_caveats`, with caveat about local SIH-only evidence.
   - ArduPilot: core Common command syntax recognized, but mode/start/acceptance
     semantics requiring evidence should be `unknown_until_sitl_or_hardware`
     rather than `supported`.

5. Добавить CLI selection для dry-run profile.

   Файлы:

   - `crates/swarm-examples/src/sitl_agent_runtime/cli.rs:5`;
   - `crates/swarm-examples/src/sitl_agent_runtime/cli.rs:51`;
   - `crates/swarm-examples/src/sitl_agent_runtime/runtime.rs:91`;
   - `crates/swarm-examples/src/bin/sitl_agent.rs:6`.

   Материализуемый результат:

   - новый flag: `--mavlink-profile mavlink_common_generic|px4|ardupilot`;
   - flag разрешен в dry-run, mock and connection parsing, but only dry-run
     artifact consumes it in M82;
   - invalid profile returns deterministic `SitlError` / unknown argument style
     failure;
   - usage string includes the new flag;
   - default behavior without flag is unchanged and uses generic profile.

   Псевдокод парсинга:

   ```rust
   "--mavlink-profile" => {
       i += 1;
       mavlink_profile = Some(parse_mavlink_profile(args.get(i)?)?);
   }
   ```

6. Передать selected profile в dry-run artifact.

   Файлы:

   - `crates/swarm-examples/src/sitl_plan.rs:772`;
   - `crates/swarm-examples/src/sitl_plan.rs:1010`;
   - `crates/swarm-examples/src/sitl_agent_runtime/runtime.rs:96`.

   Материализуемый результат:

   - сохранить backward-compatible wrappers:
     `dry_run_artifact(...)` and `write_dry_run_artifact(...)` remain available
     and use generic profile;
   - добавить profile-aware variants, например:

     ```rust
     pub fn dry_run_artifact_with_mavlink_profile(
         plan: &SitlPlan,
         command: Vec<String>,
         profile: MavlinkCapabilityProfileId,
     ) -> SitlDryRunArtifact
     ```

   - `sitl_agent --dry-run --dry-run-artifact ... --mavlink-profile px4`
     записывает `mavlink_common_plan.backend_profile == "px4"` и
     `mavlink_common_plan.compatibility.profile == "px4"`;
   - old tests and callers without profile keep generic artifact.

7. Усилить `artifact_validator --mode dry-run` для compatibility section.

   Файл:

   - `crates/swarm-examples/src/artifact_validator.rs:42`;
   - `crates/swarm-examples/src/artifact_validator.rs:332`;
   - `docs/ARTIFACT_VALIDATION.md`.

   Материализуемый результат:

   - новые stable rule ids, например:
     - `artifact.mavlink_profile_missing`;
     - `artifact.mavlink_profile_unknown`;
     - `artifact.mavlink_profile_unsupported`;
     - `artifact.mavlink_profile_hardware_blocking`;
   - validator checks:
     - compatibility section exists in current dry-run artifacts;
     - selected profile is one of known profile ids;
     - every command/mission item has a classification;
     - `unsupported` and `unknown_until_sitl_or_hardware` are visible in report;
     - hardware-facing allowed flag is false when unknown/unsupported remains.

   Не требовать legacy M81 artifacts to pass strict current validation unless
   `--allow-historical` or historical mode is used.

8. Добавить profile compatibility tests в `swarm-comms`.

   Файл:

   - `crates/swarm-comms/src/mavlink_capability_profile.rs`;
   - при необходимости дополнительные tests в
     `crates/swarm-comms/src/mavlink_common_plan.rs:921`.

   Материализуемый результат:

   - `profile_marks_supported_primitive_commands`;
   - `unknown_command_is_not_treated_as_supported`;
   - `unsupported_frame_fails_compatibility_pass`;
   - `caveat_text_appears_for_supported_with_caveats`;
   - `px4_and_ardupilot_profiles_classify_core_primitive_missions`;
   - `profile_pass_does_not_change_mission_items_or_command_order`.

9. Добавить dry-run CLI/artifact tests.

   Файлы:

   - `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs:198`;
   - `crates/swarm-examples/tests/sitl_agent/cli_and_connection_tests.rs:430`;
   - `crates/swarm-examples/src/sitl_plan.rs:1351`.

   Материализуемый результат:

   - existing dry-run artifact test asserts default generic compatibility
     section;
   - new test runs `sitl_agent --dry-run --dry-run-artifact <path>
     --mavlink-profile px4` and asserts selected profile and classification;
   - invalid `--mavlink-profile nope` fails with deterministic error;
   - primitive hover/takeoff-land test validates caveat propagation for PX4 or
     ArduPilot profile;
   - existing dry-run stdout and existing PX4 path tests remain green.

10. Добавить validator tests для compatibility.

    Файл:

    - `crates/swarm-examples/tests/artifact_validator.rs:267`.

    Материализуемый результат:

    - valid dry-run artifact with compatibility passes;
    - artifact without compatibility fails current strict dry-run validation;
    - artifact with unknown/unsupported profile classification triggers expected
      rule id;
    - artifact with caveats remains valid but reports caveats;
    - historical or allow-historical path for pre-M82 M81 artifacts is
      documented/tested if needed.

11. Обновить user-facing docs и docs smoke tests.

    Файлы:

    - создать `docs/MAVLINK_CAPABILITY_PROFILES.md`;
    - обновить `docs/MAVLINK_COMMON_COMPILER.md`;
    - обновить `docs/MISSION_COMMAND_IR.md`;
    - обновить `docs/SITL_SETUP.md`;
    - обновить `docs/HARDWARE_READINESS.md`;
    - обновить `docs/EXTENSION_GUIDE.md`;
    - обновить `docs/ARTIFACT_VALIDATION.md`;
    - обновить `docs/STATUS.md`;
    - обновить `README.md`;
    - обновить `crates/swarm-examples/tests/sitl_docs.rs:839`.

    Материализуемый результат:

    - docs explain difference between Common syntax, PX4 support, and ArduPilot
      support;
    - docs list compatibility classes and exact meaning of
      `unknown_until_sitl_or_hardware`;
    - docs say M82 is not certification, not exhaustive autopilot validation,
      not vendor SDK integration;
    - README milestone table gets M82 row;
    - docs index links new profile doc;
    - docs smoke test asserts key phrases:
      `M82`, `PX4 / ArduPilot Capability Profiles`,
      `supported_with_caveats`, `unknown_until_sitl_or_hardware`,
      `no exhaustive autopilot certification`.

12. Проверки после реализации.

    Выполнить все команды с hard timeout 300s:

    - `timeout 300s cargo fmt --all`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms mavlink_capability_profile`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms mavlink_common_plan`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test artifact_validator`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`;
    - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent`;
    - `timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings`;
    - `git diff --check`.

    Не запускать PX4/SITL live, ArduPilot SITL, benchmark seeds или любые
    установки/прогоны длиннее 5 минут в рамках M82 implementation.

# Testing strategy

## 1. Tests that need no refactoring

- Unit tests in `crates/swarm-comms/src/mavlink_capability_profile.rs`:
  profile marks supported primitive commands correctly.
- Unit tests in `crates/swarm-comms/src/mavlink_capability_profile.rs`:
  unknown command/profile cases are not treated as supported.
- Unit tests in `crates/swarm-comms/src/mavlink_capability_profile.rs`:
  unsupported coordinate frame fails the compatibility pass.
- Unit tests in `crates/swarm-comms/src/mavlink_capability_profile.rs`:
  caveat text appears for `supported_with_caveats`.
- Unit tests in `crates/swarm-comms/src/mavlink_common_plan.rs`: profile pass
  leaves command order, mission item seq and `command_postlude` order unchanged.
- Docs smoke test in `crates/swarm-examples/tests/sitl_docs.rs`: PX4/ArduPilot
  caveats and certification boundary are documented.

## 2. Tests that need light refactoring

- `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs`:
  dry-run artifact fixture becomes profile-aware and asserts profile selection.
- `crates/swarm-examples/tests/sitl_agent/cli_and_connection_tests.rs`:
  CLI parser/runtime test for `--mavlink-profile px4|ardupilot` and invalid
  value.
- `crates/swarm-examples/tests/artifact_validator.rs`: dry-run artifact
  validation adds compatibility section rules.
- `crates/swarm-examples/src/sitl_plan.rs` tests: parameterize the existing
  primitive hover artifact test by profile or add a focused profile variant.

## 3. Tests that need heavy refactoring

- SITL-backed profile conformance checks for actual PX4/ArduPilot acceptance.
  Not in M82 automated scope.
- Version-specific profile registry for concrete PX4/ArduPilot releases.
  Not in first profile implementation.
- Parameter schema validation from real autopilot metadata.
  Requires external metadata/source and should be future M86/M89 work.

Manual checks:

- No mandatory manual check for M82. Live PX4/ArduPilot SITL is explicitly not
  required; if done later, it must be recorded as separate evidence.

# Risks and tradeoffs

- Conservative classifications may mark ArduPilot or PX4 behavior as
  `unknown_until_sitl_or_hardware` even when a human expects support. This is
  preferable to false support claims without evidence.
- Adding `compatibility: Option<_>` to `MavlinkCommonPlan` changes current
  artifact JSON. `serde(default)` and optional field should keep old artifacts
  readable; validator must distinguish current vs historical artifacts.
- Existing `backend_profile: String` is already present. Adding a typed profile
  selector risks duplication. Implementation must keep them synchronized or
  migrate carefully without breaking old consumers.
- Validator strict mode may start rejecting stale M81 artifacts without
  compatibility section. This is acceptable for current artifacts, but docs/tests
  must explain historical mode or `--allow-historical` behavior if needed.
- CLI flag naming can create churn. Use one stable flag (`--mavlink-profile`)
  and the exact profile ids from the plan.

## Что могло сломаться

- API/contract: downstream code deserializing `MavlinkCommonPlan` could miss the
  new compatibility section. Проверить через serde roundtrip tests and artifact
  validator tests.
- Behavior: dry-run default profile could accidentally change from generic to
  PX4. Проверить existing dry-run artifact tests without `--mavlink-profile`.
- Data/artifacts: old `sitl_dry_run_artifact.v1.json` could fail strict current
  validation. Проверить historical/allow-historical path or explicitly document
  that M82 current validation expects compatibility.
- Integration: existing PX4 connection path could accidentally consume profile
  data and reject current local SIH flow. Проверить that M82 profile selection is
  dry-run artifact-only unless a later milestone wires it into hardware-facing
  execution.
- Performance/resources: profile pass should be linear in number of commands and
  mission items. Проверить unit tests and avoid allocations beyond report data.

# Open questions

- Should profile data live only as Rust static data in
  `mavlink_capability_profile.rs`, or should M82 also include external JSON/RON
  profile files? Recommendation for M82: Rust static data only, because the done
  criterion is “data/config, not comments only”, and external config loading
  would add schema/versioning work better suited for a later milestone.
- Should `unknown_until_sitl_or_hardware` make `validation_result.passed` false
  for all profiles, or only set `hardware_facing_allowed = false` while allowing
  dry-run artifacts to validate? Recommendation: dry-run can pass with visible
  unknowns; hardware-facing success must be blocked unless explicitly allowed by
  a future hardware workflow.
- Should ArduPilot profile be intentionally more conservative than PX4 in M82?
  Recommendation: yes. The repo has PX4/SIH evidence history, but no committed
  ArduPilot SITL evidence yet.
