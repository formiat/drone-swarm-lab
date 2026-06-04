# Context

Пользователь просит запланировать M83 по цепочке из
`docs_raw/DRONE_A.25.md`: **Primitive Real Mission Pack**. Это workflow
`plan`, поэтому в этом раунде нужен не код, а конкретный план реализации с
актуализацией README и сопутствующих документов.

Цель M83: сделать небольшой набор реальных command missions, которые без
подключённого дрона компилируются в `MissionCommandPlan` и затем в
`MavlinkCommonPlan` с M82 profile classification. Это не simulation movie и не
hardware claim; ценность этапа в дисциплине command lifecycle:

- `arm -> takeoff(3m) -> hold(10s) -> land`;
- `arm -> takeoff(3m) -> orbit(center=current, radius=1m, turns=3) -> land`;
- `arm -> takeoff(3m) -> follow_route(square) -> land`.

Стартовое состояние по коду:

- M80-M82 уже есть и описаны как готовые в `README.md:684`,
  `README.md:685`, `README.md:686` и `docs/STATUS.md:58`,
  `docs/STATUS.md:59`, `docs/STATUS.md:60`.
- Primitive DSL уже частично существует:
  `PrimitiveMission::{Hover, Orbit, TakeoffLand}` в
  `crates/swarm-sim/src/runner/types.rs:248`.
- Есть три fixture-сценария:
  `scenarios/primitive.hover.json`,
  `scenarios/primitive.orbit.json`,
  `scenarios/primitive.takeoff-land.json`.
- Есть primitive dry-run path в
  `crates/swarm-examples/src/sitl_plan.rs:538`,
  `crates/swarm-examples/src/sitl_plan.rs:853`,
  `crates/swarm-examples/src/sitl_plan.rs:957`.
- Есть M81/M82 dry-run artifact emission в
  `crates/swarm-examples/src/sitl_plan.rs:787` и CLI writer в
  `crates/swarm-examples/src/sitl_agent_runtime/runtime.rs:96`.
- Есть `mavlink-transport` live primitive converter в
  `crates/swarm-examples/src/sitl_agent_runtime/connection.rs:526`, но M83 не
  должен запускать hardware.

Что не совпадает с M83 scope прямо сейчас:

- текущий `primitive.hover.json` семантически близок к takeoff-hold-land, но
  называется `hover`, а не canonical `takeoff-hold-land`;
- текущий `primitive.orbit.json` использует `radius_m = 2.0`, а M83 требует
  `radius_m = 1.0`;
- square route primitive отсутствует;
- `MissionCommandSummary` сейчас не отражает `timeout_policy`,
  `expected_terminal_state` и `completion_tolerance`
  (`crates/swarm-mission-ir/src/summary.rs:11`), поэтому artifact не даёт
  явного машинного доказательства timeout/abort policy;
- artifact validator уже проверяет expected ACKs в M81 plan
  (`crates/swarm-examples/src/artifact_validator.rs:537`), но не проверяет
  наличие `telemetry_milestones` и policy summary для primitive artifacts.

# Investigation context

`INVESTIGATION.md` в repo root отсутствует. Дополнительных расследований с
root-cause выводами нет.

Notion/GitLab remote context не читался: в inbox `notion_policy=optional`, а
пользовательский prompt не содержит Notion task id, GitLab MR или review target.
Локальные протоколы Notion/GitLab прочитаны как обязательные инструкции.

# Affected components

- `crates/swarm-sim/src/runner/types.rs:248`:
  `PrimitiveMission`, `PrimitiveMission::describe_items`,
  `PrimitiveMissionItemDesc`.
- `crates/swarm-sim/src/dsl/validate.rs:127`:
  early `is_primitive` gate that exempts primitive scenarios from non-empty
  `scenario.tasks` validation.
- `crates/swarm-sim/src/dsl/validate.rs:253`:
  primitive-specific validation branch for `run_config.primitive_mission` and
  empty task lists.
- `crates/swarm-sim/src/dsl/tests.rs:745`:
  primitive DSL validation/unit tests.
- `crates/swarm-examples/src/sitl_plan.rs:538`:
  `build_primitive_sitl_plan`.
- `crates/swarm-examples/src/sitl_plan.rs:425`:
  runtime dispatch from `ScenarioSuiteEntry.mission` into
  `build_primitive_sitl_plan`.
- `crates/swarm-examples/src/sitl_plan.rs:853`:
  `build_command_ir_plan`.
- `crates/swarm-examples/src/sitl_plan.rs:957`:
  `primitive_mission_ir_commands`.
- `crates/swarm-examples/src/sitl_plan.rs:787`:
  `dry_run_artifact_with_mavlink_profile`.
- `crates/swarm-examples/src/sitl_plan.rs:685`:
  `check_preflight_or_err` and `SafetyValidationReport` propagation before
  dry-run artifact creation.
- `crates/swarm-examples/src/sitl_plan.rs:246`:
  `SitlDryRunArtifact.safety_report`.
- `crates/swarm-examples/src/sitl_safety.rs`:
  connection/supervisor safety config checks that must stay consistent with
  primitive dry-run preflight messaging.
- `crates/swarm-examples/src/sitl_agent_runtime/connection.rs:526`:
  `primitive_mission_to_items` under `mavlink-transport`.
- `crates/swarm-comms/src/mavlink_common_plan.rs:390`:
  M81 compiler command mapping and orbit fallback behavior.
- `crates/swarm-examples/src/artifact_validator.rs:348`:
  dry-run artifact validator for M81/M82 plan sections.
- `crates/swarm-examples/tests/artifact_validator.rs`,
  `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs`,
  `crates/swarm-examples/tests/sitl_docs.rs`:
  integration/docs validator coverage, including existing preflight exit-code
  and `safety_report` assertions.
- Scenario fixtures:
  `scenarios/primitive.hover.json`,
  `scenarios/primitive.orbit.json`,
  `scenarios/primitive.takeoff-land.json`,
  new canonical primitive fixture(s).
- Documentation:
  `README.md`,
  `docs/STATUS.md`,
  `docs/SITL_SETUP.md`,
  `docs/MISSION_COMMAND_IR.md`,
  `docs/MAVLINK_COMMON_COMPILER.md`,
  `docs/MAVLINK_CAPABILITY_PROFILES.md`,
  `docs/PREFLIGHT_SAFETY.md`,
  `docs/ARTIFACT_VALIDATION.md`,
  `docs/EXTENSION_GUIDE.md`,
  `docs/HARDWARE_READINESS.md`,
  `docs/OPERATIONAL_RUNBOOKS.md`,
  and `docs_raw/DRONE_A.25.md` only as context, not as implementation target.

# Implementation steps

1. Нормализовать primitive mission DSL под M83 без ломания старых fixtures.

   Файлы:

   - `crates/swarm-sim/src/runner/types.rs:248`;
   - `crates/swarm-sim/src/dsl/validate.rs:127`;
   - `crates/swarm-sim/src/dsl/validate.rs:253`;
   - `crates/swarm-sim/src/dsl/tests.rs:745`.

   Материализуемый результат:

   - добавить canonical variant для square route, например:

     ```rust
     #[serde(rename_all = "snake_case", tag = "kind")]
     pub enum PrimitiveMission {
         Hover { altitude_m: f64, hold_seconds: f32 },
         Orbit { altitude_m: f64, turns: f32, radius_m: f32 },
         TakeoffLand { altitude_m: f64 },
         WaypointSquare { altitude_m: f64, side_m: f64 },
     }
     ```

   - оставить `Hover` как backward-compatible representation для
     `takeoff-hold-land`, но в docs/scenario names назвать canonical mission
     именно `takeoff-hold-land`;
   - вынести primitive mission names в единый локальный helper/const, чтобы
     ранний `is_primitive` gate и primitive-specific branch не расходились:

     ```rust
     fn is_primitive_mission_name(mission: &str) -> bool {
         matches!(
             mission,
             "hover" | "orbit" | "takeoff-land" | "takeoff-hold-land" | "waypoint-square"
         )
     }
     ```

   - использовать этот helper в `dsl/validate.rs:127`, чтобы
     `takeoff-hold-land` и `waypoint-square` были exempt from
     `"Scenario must contain at least one task"` так же, как старые primitive
     names;
   - добавить validation для `takeoff-hold-land` и `waypoint-square` mission
     strings рядом с `"hover" | "orbit" | "takeoff-land"` в
     `dsl/validate.rs:253`, лучше через тот же helper, но без ослабления
     unknown mission behavior;
   - добавить проверку положительных и finite параметров primitive missions:
     `altitude_m > 0`, `hold_seconds > 0`, `turns > 0`, `radius_m > 0`,
     `side_m > 0`;
   - добавить unit tests: valid canonical `takeoff-hold-land`, valid
     `waypoint-square`, оба проходят empty-task exemption; invalid non-positive
     params fail; primitive mission с non-empty `tasks` всё ещё rejected;
     unknown mission с empty `tasks` не получает primitive exemption и всё ещё
     fails with `"Scenario must contain at least one task"`.

2. Обновить primitive scenario fixtures.

   Файлы:

   - `scenarios/primitive.hover.json`;
   - `scenarios/primitive.orbit.json`;
   - `scenarios/primitive.takeoff-land.json`;
   - new `scenarios/primitive.takeoff-hold-land.json`;
   - new `scenarios/primitive.square.json`.

   Материализуемый результат:

   - добавить canonical `primitive.takeoff-hold-land.json`:
     `mission = "takeoff-hold-land"`, `altitude_m = 3.0`,
     `hold_seconds = 10.0`;
   - обновить или добавить canonical orbit fixture:
     `mission = "orbit"`, `altitude_m = 3.0`, `turns = 3.0`,
     `radius_m = 1.0`;
   - добавить `primitive.square.json`:
     `mission = "waypoint-square"`, `altitude_m = 3.0`,
     `side_m = 1.0`;
   - сохранить старые `primitive.hover.json` и
     `primitive.takeoff-land.json` как compatibility fixtures, если их
     удаление ломает существующие тесты/доки.

3. Синхронизировать dry-run display items и IR generation.

   Файлы:

   - `crates/swarm-sim/src/runner/types.rs:261`;
   - `crates/swarm-examples/src/sitl_plan.rs:425`;
   - `crates/swarm-examples/src/sitl_plan.rs:538`;
   - `crates/swarm-examples/src/sitl_plan.rs:853`;
   - `crates/swarm-examples/src/sitl_plan.rs:957`.

   Материализуемый результат:

   - `PrimitiveMission::describe_items` для square должен возвращать понятные
     dry-run display items с последовательными `seq`/labels и `z = altitude_m`;
   - `primitive_mission_ir_commands` должен строить body commands:

     ```rust
     PrimitiveMission::Hover { hold_seconds, .. } =>
         vec![MissionCommand::Hold { duration_secs: f64::from(*hold_seconds) }]

     PrimitiveMission::Orbit { altitude_m, radius_m, turns } =>
         vec![MissionCommand::Orbit {
             center: Position::Local(LocalPosition { x_m: 0.0, y_m: 0.0, z_m: *altitude_m }),
             radius_m: f64::from(*radius_m),
             turns: f64::from(*turns),
             direction: OrbitDirection::CounterClockwise,
         }]

     PrimitiveMission::WaypointSquare { altitude_m, side_m } =>
         vec![MissionCommand::FollowRoute {
             route_id: RouteId::from("primitive-square".to_owned()),
             waypoints: square_waypoints(*side_m, *altitude_m),
         }]
     ```

   - `build_command_ir_plan` должен сохранять общий lifecycle:
     `Arm -> Takeoff -> body -> Land`;
   - `build_sitl_plan_internal` должен dispatch all primitive mission names в
     `build_primitive_sitl_plan`, а не только legacy names:

     ```rust
     if matches!(
         entry.mission.as_str(),
         "hover" | "orbit" | "takeoff-land" | "takeoff-hold-land" | "waypoint-square"
     ) {
         return build_primitive_sitl_plan(...);
     }
     ```

     Предпочтительно вынести shared helper рядом с `build_sitl_plan_internal`,
     чтобы future primitive aliases не расходились между dispatch и tests.
   - для всех трёх canonical missions `MissionCommandPlan` должен иметь:
     `timeout_policy { command_timeout_secs: 5.0, completion_timeout_secs: 120.0,
     on_timeout: Abort }`, `expected_terminal_state = Landed`,
     `completion_tolerance { position_m: 1.0, altitude_m: 0.5 }`.

4. Сделать policy fields видимыми в dry-run artifact.

   Файлы:

   - `crates/swarm-mission-ir/src/summary.rs:11`;
   - `crates/swarm-examples/src/sitl_plan.rs:225`;
   - `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs`.

   Материализуемый результат:

   - расширить `MissionCommandSummary` полями:

     ```rust
     pub timeout_policy: TimeoutPolicy,
     pub expected_terminal_state: String,
     pub completion_tolerance: CompletionTolerance,
     ```

   - либо, если изменение summary contract слишком широкое, добавить в
     `SitlDryRunArtifact` отдельное поле `command_ir_policy_summary`;
   - предпочитаемый вариант: расширить `MissionCommandSummary`, потому что
     policy уже является частью `MissionCommandPlan` и summary предназначен
     именно для artifact-level доказательств;
   - обновить summary unit tests, чтобы проверять `on_timeout = "abort"` и
     `expected_terminal_state = "landed"`.

5. Проверить M81/M82 compilation для всех canonical primitive missions.

   Файлы:

   - `crates/swarm-comms/src/mavlink_common_plan.rs:390`;
   - `crates/swarm-examples/src/sitl_plan.rs:787`;
   - `crates/swarm-examples/src/sitl_plan.rs:1384`;
   - `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs:210`.

   Материализуемый результат:

   - добавить tests в `sitl_plan.rs`:
     `primitive_takeoff_hold_land_compiles_to_takeoff_loiter_land`,
     `primitive_orbit_compiles_with_waypoint_approximation`,
     `primitive_square_compiles_to_ordered_waypoint_route`;
   - для orbit проверить, что generic/PX4/ArduPilot compatibility report
     явно показывает fallback/caveat или unknown там, где поведение не
     переносимо;
   - для square проверить ordered route:
     `NavTakeoff` в prelude, 4 или 5 `NavWaypoint` items в mission upload
     (решение: замкнутый square должен явно вернуться в стартовую точку),
     `MissionStart`, `NavLand` в postlude;
   - добавить CLI dry-run artifact tests, которые запускают `sitl_agent
     --dry-run --dry-run-artifact ... --mavlink-profile px4|ardupilot` для
     каждого canonical scenario и проверяют наличие:
     `command_ir_summary`, `mavlink_common_plan`, `expected_acks`,
     `telemetry_milestones`, `compatibility.command_results`.

6. Синхронизировать live primitive converter без запуска hardware.

   Файлы:

   - `crates/swarm-examples/src/sitl_agent_runtime/connection.rs:526`;
   - `crates/swarm-comms/src/mavlink/mission_items.rs:61`;
   - `crates/swarm-examples/src/sitl_agent_runtime/tests.rs` или новый
     feature-gated test module.

   Материализуемый результат:

   - добавить `PrimitiveMission::WaypointSquare` в
     `primitive_mission_to_items`;
   - проверить, что hover/orbit/square live converter и dry-run IR path не
     расходятся по mission intent;
   - не открывать MAVLink transport и не запускать PX4/ArduPilot;
   - если прямой live converter test требует `mavlink-transport`, оформить его
     как `#[cfg(feature = "mavlink-transport")]` unit test.

7. Усилить artifact validator для M83-specific expectations.

   Файлы:

   - `crates/swarm-examples/src/artifact_validator.rs:320`;
   - `crates/swarm-examples/tests/artifact_validator.rs`.

   Материализуемый результат:

   - оставить текущие M81/M82 checks, но добавить для current dry-run artifacts
     проверку непустых `telemetry_milestones` для primitive missions, если
     `mavlink_common_plan.mission_items` не пуст;
   - добавить rule id, например
     `artifact.mavlink_plan_telemetry_missing`, если такого rule id ещё нет;
   - добавить проверку policy summary только если выбран вариант расширения
     `command_ir_summary`;
   - добавить negative tests: удалить `telemetry_milestones` из primitive
     artifact; удалить/испортить `command_ir_summary.timeout_policy`.

8. Зафиксировать safety/preflight контракт M83.

   Файлы:

   - `crates/swarm-examples/src/sitl_plan.rs:407`;
   - `crates/swarm-examples/src/sitl_plan.rs:685`;
   - `crates/swarm-examples/src/sitl_plan.rs:840`;
   - `crates/swarm-examples/src/sitl_safety.rs`;
   - `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs:75`;
   - `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs:223`;
   - `crates/swarm-examples/tests/artifact_validator.rs:625`;
   - `docs/PREFLIGHT_SAFETY.md`;
   - `docs/HARDWARE_READINESS.md`;
   - `docs/ARTIFACT_VALIDATION.md`.

   Материализуемый результат:

   - canonical primitive dry-run path должен по-прежнему проходить через
     `check_preflight_or_err(entry)?` до записи `SitlDryRunArtifact`;
   - для `takeoff-hold-land`, `orbit` и `waypoint-square` dry-run artifacts
     должны содержать:

     ```json
     {
       "safety_report": {
         "passed": true
       }
     }
     ```

     и не должны содержать error-severity violations;
   - если primitive route после M83 материализуется в waypoints до preflight,
     добавить primitive-aware static check для altitude/geofence/no-fly: плохая
     высота, waypoint вне geofence или waypoint внутри no-fly зоны должны
     завершать `sitl_agent --dry-run` с exit code `2` и rule ids в stderr/отчёте;
   - `--safety-config`/`sitl_safety.rs` checks не должны расходиться с M83
     wording: если unsafe safety config уже применим к primitive dry-run или
     connection/supervisor path, он должен давать validation/preflight-class
     failure с exit code `2`; если dry-run primitive пока не принимает такой
     config для generated waypoints, это нужно явно отметить в docs как границу
     текущего static preflight gate;
   - если конкретная unsafe primitive input ещё не может быть выражена через
     текущий route representation без расширения модели, отрицательный тест
     должен покрыть уже поддерживаемую validation/preflight ветку: invalid
     primitive params (`altitude_m <= 0`, `radius_m <= 0`, `side_m <= 0`) дают
     validation/preflight-class failure с exit code `2`, а geofence/no-fly
     остаётся отдельным light-refactoring пунктом до появления materialized
     primitive waypoints в `SafetyValidationReport`;
   - `artifact_validator --mode dry-run --strict` должен проверять presence и
     `passed=true` для embedded `safety_report` в current primitive artifacts или
     явно возвращать стабильный rule id, например
     `artifact.dry_run_safety_report_failed`;
   - `docs/PREFLIGHT_SAFETY.md`, `docs/HARDWARE_READINESS.md` и
     `docs/ARTIFACT_VALIDATION.md` должны прямо говорить, что M83 проверяет
     static preflight gate для command missions, но не является certified flight
     safety, не заменяет PX4/ArduPilot failsafe и не доказывает hardware flight
     readiness.

9. Обновить README и сопутствующие документы.

   Файлы:

   - `README.md:684`;
   - `docs/STATUS.md:58`;
   - `docs/SITL_SETUP.md:176`;
   - `docs/MISSION_COMMAND_IR.md`;
   - `docs/MAVLINK_COMMON_COMPILER.md`;
   - `docs/MAVLINK_CAPABILITY_PROFILES.md`;
   - `docs/PREFLIGHT_SAFETY.md`;
   - `docs/ARTIFACT_VALIDATION.md`;
   - `docs/EXTENSION_GUIDE.md`;
   - `docs/HARDWARE_READINESS.md`;
   - `docs/OPERATIONAL_RUNBOOKS.md`;
   - `crates/swarm-examples/tests/sitl_docs.rs:812`.

   Материализуемый результат:

   - добавить M83 row в README/STATUS;
   - описать три primitive missions и exact command sequences;
   - явно написать, что M83 validates dry-run/MAVLink plan artifacts only:
     no real flight, no PX4/ArduPilot equivalence claim, no connected vehicle;
   - описать orbit portability:
     native orbit may be profile-specific, M83 may use waypoint approximation
     or mark unsupported/unknown depending on profile;
   - обновить artifact validator docs с новыми rule ids;
   - добавить раздел/абзац про M83 safety boundary: `safety_report.passed=true`
     означает только успешную static preflight validation текущих inputs;
   - добавить docs smoke tests, требующие фразы:
     `M83`, `Primitive Real Mission Pack`, `takeoff-hold-land`,
     `waypoint-square`, `no real flight`, `orbit portability`,
     `static preflight`, `not certified flight safety`.

10. Добавить automated end-to-end dry-run artifact roundtrip без hardware.

   Файлы:

   - `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs`;
   - `crates/swarm-examples/tests/artifact_validator.rs`;
   - сценарии из шага 2.

   Материализуемый результат:

   - тест для каждого canonical scenario:
     1. запустить `sitl_agent --dry-run --scenario <scenario> --agent-id agent-0
        --dry-run-artifact <tempdir>/artifact.json --mavlink-profile <profile>`;
     2. прочитать JSON;
     3. проверить command order/profile/ACK/telemetry/policy fields и
        `safety_report.passed == true`;
     4. прогнать `validate_artifact_pack(..., mode=DryRun, strict=true)`;
   - параметризовать profiles хотя бы для `px4` и `ardupilot`; generic path
     покрыт default tests.

11. Запустить проверки после реализации.

    Команды должны иметь hard timeout 300s. Для `cargo test` обязательно
    отключить proptest persistence и использовать абсолютный `runlim`.

    Минимальный набор:

    ```bash
    timeout 300s cargo fmt --all
    timeout 300s cargo fmt --all --check
    timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-mission-ir
    timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms mavlink_common_plan
    timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms mavlink_capability_profile
    timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim primitive
    timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent primitive
    timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test artifact_validator dry_run_artifact
    timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs
    timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings
    timeout 300s git diff --check
    find . -name '*.proptest-regressions' -print
    ```

    Не запускать:

    - PX4/SIH;
    - ArduPilot SITL;
    - hardware upload;
    - 500/1000-seed benchmark;
    - любые long runs сверх 5 минут.

# Testing strategy

## 1. Tests that need no refactoring

Эти тесты нужно реализовать вместе с основными изменениями M83:

- `crates/swarm-sim/src/dsl/tests.rs`:
  - valid canonical `takeoff-hold-land` suite validates;
  - valid canonical `orbit` suite validates with `radius_m = 1.0`;
  - valid `waypoint-square` suite validates;
  - `takeoff-hold-land` and `waypoint-square` are covered by the early
    empty-task primitive exemption;
  - unknown mission names with empty `scenario.tasks` still fail and do not
    bypass non-primitive task validation;
  - primitive suite with non-empty `tasks` fails;
  - primitive suite with zero/negative/non-finite params fails.
- `crates/swarm-sim/src/runner/types.rs` tests:
  - `PrimitiveMission::describe_items` for square returns deterministic labels
    and geometry;
  - old `Hover`/`TakeoffLand` fixtures remain compatible.
- `crates/swarm-examples/src/sitl_plan.rs` tests:
  - `build_sitl_plan_internal` dispatches `takeoff-hold-land` and
    `waypoint-square` to `build_primitive_sitl_plan`;
  - takeoff-hold-land command order:
    `arm`, `takeoff`, `hold`, `land`;
  - orbit command order:
    `arm`, `takeoff`, `orbit`, `land`;
  - square command order:
    `arm`, `takeoff`, `follow_route`, `land`;
  - timeout/abort policy exists for every canonical primitive mission;
  - Mavlink plan has deterministic ACKs and telemetry milestones;
  - PX4/ArduPilot profile classification exists for each mission.
- `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs`:
  - dry-run artifact roundtrip for each canonical primitive scenario;
  - explicit runtime-dispatch integration cases:
    `sitl_agent --dry-run --scenario scenarios/primitive.takeoff-hold-land.json
    --agent-id agent-0 --dry-run-artifact <tempdir>/artifact.json` succeeds and
    writes non-null `command_ir_summary` and `mavlink_common_plan`;
  - same runtime-dispatch integration case for `scenarios/primitive.square.json`;
  - each canonical primitive dry-run artifact has
    `safety_report.passed == true` and no error-severity safety violations;
  - invalid primitive params fail before artifact success with exit code `2`;
  - unsafe `--safety-config` keeps the existing exit-code-`2` contract where it
    is already supported by the dry-run/connection/supervisor path;
  - profile selection works for `px4` and `ardupilot`;
  - no hardware connection is opened in dry-run path.
- `crates/swarm-examples/tests/artifact_validator.rs`:
  - strict dry-run validation passes for the three canonical artifacts;
  - missing telemetry/policy fields fail when new M83 validator rules are added;
  - missing or failed embedded `safety_report` in primitive dry-run artifact
    fails in strict mode with a stable rule id.
- `crates/swarm-examples/tests/sitl_docs.rs`:
  - README/STATUS/SITL docs mention M83 scope and non-goals;
  - `docs/PREFLIGHT_SAFETY.md`, `docs/HARDWARE_READINESS.md` and
    `docs/ARTIFACT_VALIDATION.md` mention M83 static preflight boundary and do
    not claim certified flight safety or hardware readiness.

## 2. Tests that need light refactoring

- Add shared fixture helpers for primitive dry-run artifact generation to avoid
  duplicating `tempdir + sitl_agent + JSON assertions` across
  `sitl_agent/report_and_boundary_tests.rs` and `artifact_validator.rs`.
- Add a primitive preflight fixture helper if negative geofence/no-fly tests need
  generated primitive waypoints rather than task-based fixtures. Keep the helper
  portable: inline JSON/tempdir only, no PX4/SITL, no `$HOME`, no absolute paths.
- Add a small helper for asserting command order in `MavlinkCommonPlan`, e.g.
  `assert_mavlink_sequence(plan, expected_prelude, expected_items,
  expected_postlude)`.
- Add a common docs assertion helper in `sitl_docs.rs` if M83 adds many
  required phrases across README/STATUS/SITL/MAVLink docs.
- Add fixture-backed dry-run artifacts for all primitive missions only if
  inline tempdir generation becomes too slow or too verbose. Prefer inline
  generated artifacts first.

## 3. Tests that need heavy refactoring

- Simulated ACK/telemetry state machine that consumes `MavlinkCommonPlan` and
  emits fake execution lifecycle events. This is useful later, but not required
  for M83.
- Command lifecycle replay events for dry-run primitive missions. Current
  `sitl_agent --dry-run` rejects `--replay-log`, and existing replay summaries
  are built from SITL/mock/supervisor event logs, not from pure
  `MissionCommandPlan` compilation. Adding replay lifecycle events for command
  compilation would require a new event schema/emitter rather than a small
  assertion over current artifacts.
- Backend executor integration tests that run upload/execute state machines
  without a real vehicle. This belongs after M83 if command lifecycle needs
  deeper evidence.
- SITL execution harness for primitive missions against PX4/ArduPilot. This is
  out of scope for M83 because M83 explicitly has no connected vehicle and no
  real flight.
- Versioned full `MissionCommandPlan` artifact embedding. For M83, summary
  extension is enough unless implementation reveals that policy evidence needs
  complete IR preservation.

# Risks and tradeoffs

- Backward compatibility: existing `hover`, `orbit`, and `takeoff-land`
  scenarios may be referenced by tests/docs. Prefer adding canonical M83
  scenarios and keeping old fixtures unless deletion is proven safe.
- Naming: `Hover` already means `takeoff -> hold -> land`. Renaming the enum
  variant would churn JSON compatibility. Prefer canonical mission/profile
  names in scenarios/docs while preserving the serde variant.
- Orbit semantics: M81 currently supports orbit via configurable waypoint
  approximation, while live `mavlink-transport` has `LoiterTurns`. M83 must
  document this mismatch and tests must assert artifact caveats instead of
  claiming PX4/ArduPilot equality.
- Square route closure: a square can be represented with four corners or four
  corners plus return-to-origin. Choose one explicit contract before coding.
  Recommended: include return-to-origin as the final waypoint, because the
  mission text says "square route" and a closed loop is less ambiguous.
- Artifact schema drift: extending `MissionCommandSummary` changes JSON shape.
  It is additive and should be safe, but docs/tests must treat older artifacts
  as historical if they lack policy fields.
- Validator strictness: new M83 validator rules can make stale dry-run artifacts
  fail. This is desired for current artifacts, but historical mode behavior
  should remain conservative.
- Performance/resources: all M83 tests should remain tiny dry-run/unit tests.
  No benchmark or SITL run is needed.
- Safety boundary: M83 can prove that primitive command inputs pass the current
  static preflight gate and that artifacts preserve `SafetyValidationReport`.
  It must not imply certified flight safety, dynamic obstacle avoidance,
  autopilot failsafe validation, or hardware readiness.
- Replay lifecycle: the prompt mentions replay summary command lifecycle events
  as light refactoring, but current architecture has no dry-run replay log path
  for command compilation; `--replay-log` is intentionally rejected for dry-run.
  M83 acceptance should use `command_ir_summary`,
  `MavlinkCommonPlan.expected_acks`, `telemetry_milestones` and validator checks
  as command lifecycle evidence. A real replay lifecycle stream should be a
  later executor/simulator milestone.

# Open questions

- Should the canonical square route include 4 waypoints or 5 waypoints
  including explicit return to start? Recommended answer for implementation:
  5 waypoints, closed loop.
- Should `primitive.hover.json` be renamed, kept as alias, or left as legacy
  compatibility fixture? Recommended answer: add
  `primitive.takeoff-hold-land.json` and keep `primitive.hover.json`.
- Should M83 embed full `MissionCommandPlan` in dry-run artifacts or only extend
  `MissionCommandSummary` with policy fields? Recommended answer: extend the
  summary first; full IR embedding is heavier and can wait until a later schema
  milestone.
- Should live `mavlink-transport` primitive converter be made identical to M81
  compiler output now? Recommended answer: only add square support and tests for
  intent consistency; full unification of live upload with `MavlinkCommonPlan`
  is a larger follow-up.
- Should M83 add command lifecycle events to replay summaries? Recommended
  answer: no for M83. Keep replay lifecycle work out of acceptance until there is
  a command-execution simulator or backend executor event source. Dry-run
  artifacts should remain the evidence surface for M83.
