# M71 — Preflight Safety And Invariant Contract

## Контекст

Второй milestone в цепочке `BEFORE_HARDWARE_A.23` (после M70 Urban Route Export + Geo Origin).
Цель — создать строгий preflight-контракт для safety и invariant проверок, который обязаны
пройти все mission inputs до dry-run, SITL upload или hardware bench. M71 важнее добавления
новых mission features: без него проект не имеет явного гейта корректности входных данных.

После M70 проект умеет экспортировать Urban маршруты в waypoint missions, валидировать
DSL сценарии и проверять safety конфиг во время runtime. M71 добавляет:

- Структурированные preflight violations (`SafetyValidationReport`) с `rule_id`, `severity`,
  `affected_id`, `reason`.
- Проверки до execution: geofence, altitude, route length, ownership invariants, urban safety,
  semantics invariants.
- CLI gate: unsafe mission → non-zero exit с перечислением rule_ids нарушений.
- Стабильную exit code конвенцию (2/3/4/5).
- Документацию с полным списком preflight rules и явными ограничениями.

## Контекст исследования

`INVESTIGATION.md` отсутствует. Исследование проведено в рамках этапа планирования.

Ключевые находки по кодовой базе:

- `crates/swarm-safety/src/lib.rs` — `Aabb`, `Geofence`, `NoFlyZone`, `SafetyConfig`,
  `ViolationType`, `SafetyViolation` (runtime), `check_agent_at_tick`. `UrbanEdge.blocked: bool`
  (в `swarm-types`) уже существует.
- Preflight-специфичных типов `SafetyValidationReport` / preflight `SafetyViolation` /
  `ViolationSeverity` нет — нужно добавить.
- DSL validation (`swarm-sim/src/dsl/validate.rs`) уже проверяет сценарии, но возвращает
  `Vec<ValidationError>`, не `SafetyValidationReport`. M71 добавляет отдельный preflight-слой.
- **Naming collision**: `swarm_safety::SafetyViolation` — runtime тип (имеет `agent_id`,
  `violation_type`). Preflight тип с одноимённой struct живёт в sub-module `preflight`.
  Исполнитель обязан использовать qualified paths.
- Текущие exit codes (2/3/20/21/22/23/30/40) не соответствуют новой конвенции M71.
  Нужна миграция с обновлением всех тестов, проверяющих конкретные коды.
- `SitlDryRunArtifact` не содержит safety report — поле добавляется в M71.

## Затронутые компоненты

| Файл | Тип изменения |
|---|---|
| `crates/swarm-safety/src/lib.rs` | Новые optional поля в `SafetyConfig`; `pub mod preflight` |
| `crates/swarm-safety/src/preflight.rs` | **Новый**: типы `SafetyValidationReport`, `SafetyViolation`, `ViolationSeverity` |
| `crates/swarm-sim/src/lib.rs` | Re-export `pub mod preflight` |
| `crates/swarm-sim/src/preflight.rs` | **Новый**: логика preflight validation |
| `crates/swarm-sim/src/dsl/validate.rs` | Интеграция preflight в DSL validation; новая pub функция |
| `crates/swarm-examples/src/sitl_plan.rs` | Новый variant `SitlError::PreflightFailed`; поле в `SitlDryRunArtifact` |
| `crates/swarm-examples/src/sitl_supervisor_cli/exit_codes.rs` | Новая exit code конвенция (2/3/4/5) |
| `crates/swarm-examples/src/bin/sitl_supervisor.rs` | Preflight call перед execution |
| `crates/swarm-examples/src/bin/sitl_agent.rs` | Preflight call перед dry-run |
| `crates/swarm-examples/tests/sitl_agent/supervisor_tests.rs` | Обновление exit code assertions |
| `docs/PREFLIGHT_SAFETY.md` | **Новый**: список правил, ограничения, non-certified disclaimer |
| `docs/STATUS.md` | Строка M71 в milestone table |
| `docs/HARDWARE_READINESS.md` | Ссылка на M71 preflight gate |
| `README.md` | Обновление milestone table |

## Шаги реализации

### Шаг 1 — Preflight типы в `swarm-safety`

**Файл:** `crates/swarm-safety/src/preflight.rs` (новый)

Добавить публичные типы:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationSeverity {
    Error,
    Warning,
}

/// A single preflight rule violation.
/// value: `(rule_id, severity, affected_id, reason)`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SafetyViolation {
    pub rule_id: String,
    pub severity: ViolationSeverity,
    pub affected_id: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SafetyValidationReport {
    pub passed: bool,
    pub violations: Vec<SafetyViolation>,
}

impl SafetyValidationReport {
    pub fn ok() -> Self {
        Self { passed: true, violations: vec![] }
    }

    /// Build report from violations list. `passed` is `true` only
    /// when there are no Error-severity violations.
    pub fn from_violations(violations: Vec<SafetyViolation>) -> Self {
        let passed = violations.iter()
            .all(|v| v.severity != ViolationSeverity::Error);
        Self { passed, violations }
    }
}
```

**Файл:** `crates/swarm-safety/src/lib.rs` — добавить `pub mod preflight;`

Ожидаемый результат: типы компилируются, serde round-trip работает.

---

### Шаг 2 — Расширение `SafetyConfig` preflight-параметрами

**Файл:** `crates/swarm-safety/src/lib.rs`, struct `SafetyConfig` (строка 54)

Добавить поля с `#[serde(default)]`:

```rust
pub struct SafetyConfig {
    // существующие поля без изменений
    pub geofence: Option<Geofence>,
    pub no_fly_zones: Vec<NoFlyZone>,
    pub separation: Option<SeparationConstraint>,
    // M71: preflight parameters
    #[serde(default)]
    pub max_altitude_m: Option<f64>,
    #[serde(default)]
    pub min_altitude_m: Option<f64>,
    #[serde(default)]
    pub max_route_length_m: Option<f64>,
    #[serde(default)]
    pub max_duration_ticks: Option<u64>,
}
```

Ожидаемый результат: все существующие JSON fixtures с `safety_config` десериализуются без
изменений (новые поля по умолчанию `None`).

---

### Шаг 3 — Preflight validation logic в `swarm-sim`

**Файл:** `crates/swarm-sim/src/preflight.rs` (новый)

Публичная входная точка:

```rust
pub fn run_preflight(
    entry: &crate::dsl::ScenarioSuiteEntry,
) -> swarm_safety::preflight::SafetyValidationReport
```

Внутренняя структура — четыре группы проверок, каждая возвращает
`Vec<swarm_safety::preflight::SafetyViolation>`:

**3a. `check_mission_level(entry, config)` — mission-level safety:**

| rule_id | Условие нарушения | Severity |
|---|---|---|
| `geofence.waypoint_outside` | `task.pose` outside `config.geofence.bounds` | Error |
| `nofly.waypoint_inside` | `task.pose` inside `no_fly_zone.bounds` (постоянная зона) | Error |
| `altitude.above_max` | `task.pose.z > config.max_altitude_m` | Error |
| `altitude.below_min` | `task.pose.z < config.min_altitude_m` | Warning |
| `route.length_exceeds_max` | urban: `route_length_m > config.max_route_length_m` | Error |
| `route.duration_exceeds_max` | `max_ticks * tick_duration_ms > max_duration_ticks * 1000` | Warning |
| `pose.invalid_coordinate` | `!x.is_finite() || !y.is_finite() || !z.is_finite()` | Error |
| `id.missing_task_id` | `task.id` is empty string | Error |

Для геофенс/nofly: использовать `swarm_safety::Aabb::contains` из `swarm_safety`.
Для altitude: `Pose.z` — высота в метрах.

**3b. `check_ownership_invariants(entry)` — ownership invariants:**

| rule_id | Условие нарушения | Severity |
|---|---|---|
| `ownership.duplicate_task_id` | два task в `scenario.tasks` имеют одинаковый `task.id` | Error |
| `ownership.task_assigned_and_unassigned` | `assigned_to` != None у task со статусом `Unassigned` | Error |

Полная проверка `released_not_resolved` требует runtime assignment history — недоступна в
static preflight. Gap задокументирован в `docs/PREFLIGHT_SAFETY.md` (раздел
"Что не проверяется").

**3c. `check_urban_safety(entry)` — Urban-specific (только если `urban_state` присутствует):**

| rule_id | Условие нарушения | Severity |
|---|---|---|
| `urban.unknown_edge` | segment `edge_id` not in `urban_state.map.edges` | Error |
| `urban.blocked_edge` | `edge.blocked == true` в маршруте | Error |
| `urban.aabb_intersection` | waypoint pose inside `static_obstacle.bounds` | Error |
| `urban.waypoint_outside_assumptions` | waypoint outside nominal AABB всей карты | Warning |

Для urban checks: получить expanded route через
`swarm_sim::urban::expand_route_loop_with_planner_name()` (уже используется в
`urban_validate.rs:79`). Если route expansion fails — пропустить urban checks (ошибка
зафиксирована в DSL validate).

Поле `urban.aabb_intersection` использует `swarm_safety::Aabb::contains` и
`UrbanStaticObstacle.bounds`.

**3d. `check_mission_semantics(entry)` — mission semantics invariants:**

| rule_id | Условие нарушения | Severity |
|---|---|---|
| `semantics.unsupported_strategy_pair` | `cbba_stress` + mission != "cbba-stress"; или SAR + CBBA | Warning |

Полная матрица поддержки как programmatic data реализуется в M77. В M71 — минимальный
захардкоженный список известных unsupported пар.

`semantics.completion_predicate_missing` откладывается: формальное определение предиката
зависит от MissionAdapter API, которое в M77 может измениться. Зафиксировано в Open questions.

**Re-export из `crates/swarm-sim/src/lib.rs`:**

```rust
pub mod preflight;
```

Ожидаемый результат: `run_preflight(entry)` возвращает `SafetyValidationReport` со всеми
violations для каждого нарушения.

---

### Шаг 4 — Интеграция preflight в DSL validation

**Файл:** `crates/swarm-sim/src/dsl/validate.rs`

В конец `validate_entry()` добавить вызов preflight и конвертацию ошибок:

```rust
let preflight = crate::preflight::run_preflight(entry);
for v in preflight.violations.iter()
    .filter(|v| v.severity == swarm_safety::preflight::ViolationSeverity::Error)
{
    errors.push(ValidationError {
        field: v.rule_id.clone(),
        message: v.reason.clone(),
    });
}
```

Добавить публичную функцию для прямого использования:

```rust
pub fn run_preflight_report(
    entry: &ScenarioSuiteEntry,
) -> swarm_safety::preflight::SafetyValidationReport {
    crate::preflight::run_preflight(entry)
}
```

Ожидаемый результат: невалидные сценарии (geofence, altitude, urban blocked) падают при
suite validation с конкретными rule_ids.

---

### Шаг 5 — `SitlDryRunArtifact` и `SitlError`

**Файл:** `crates/swarm-examples/src/sitl_plan.rs`

**5a.** Добавить поле в `SitlDryRunArtifact` (после строки ~224):
```rust
pub safety_report: Option<swarm_safety::preflight::SafetyValidationReport>,
```

**5b.** Добавить variant в `SitlError` (после строки ~153):
```rust
#[error("preflight validation failed: {rule_ids}")]
PreflightFailed { rule_ids: String },
```

**5c.** В `dry_run_artifact()`: заполнить `safety_report` из результата preflight.

**5d.** В `format_dry_run_plan()`: отобразить safety status (passed/failed + rule_ids).

**5e.** Добавить helper:
```rust
pub fn check_preflight_or_err(
    entry: &swarm_sim::dsl::ScenarioSuiteEntry,
) -> Result<swarm_safety::preflight::SafetyValidationReport, SitlError> {
    let report = swarm_sim::preflight::run_preflight(entry);
    if !report.passed {
        let ids = report.violations.iter()
            .filter(|v| v.severity == swarm_safety::preflight::ViolationSeverity::Error)
            .map(|v| v.rule_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(SitlError::PreflightFailed { rule_ids: ids });
    }
    Ok(report)
}
```

Ожидаемый результат: dry-run artifact содержит `safety_report`; preflight failure возвращает
`SitlError::PreflightFailed` с перечислением rule_ids.

---

### Шаг 6 — Рефакторинг exit codes

**Файл:** `crates/swarm-examples/src/sitl_supervisor_cli/exit_codes.rs`

Новая стабильная конвенция:

| Код | Категория | Примеры `SitlError` variants |
|---|---|---|
| `2` | validation / preflight | `InvalidScenario`, `SafetyValidationFailed`, `PreflightFailed`, `SafetyConfig{Read,Parse,Invalid}`, `UnsupportedCoordinateFrame`, `UrbanRouteExport` |
| `3` | runtime / supervisor | `ConnectionFailed` (lifecycle/heartbeat/abort), lifecycle errors, final status |
| `4` | artifact / report | `RunReportWrite`, `ReplayLogWrite`, `ReplaySummaryWrite`, `MultiAgentManifestWrite`, `DryRunArtifactWrite`, `OutputAlreadyExists` |
| `5` | environment | `FeatureMissing`, `BadConnectionString`, `HardwareCandidateRequiresExplicitAllow` |

Обновить `supervisor_exit_code()` и `classify_connection_failure_exit_code()`:
- `HardwareCandidateRequiresExplicitAllow` переезжает с кода 3 на код 5.
- I/O ошибки (бывший код 40) переезжают на код 4.
- MAVLink lifecycle (бывшие 20/21/22/23) переезжают на 3 или 5 в зависимости от смысла.

**Важно:** обновить все тесты, которые проверяют конкретные exit codes
(`supervisor_tests.rs`, `cli_and_connection_tests.rs`).

Ожидаемый результат: новая конвенция используется; `cargo test -p swarm-examples` проходит.

---

### Шаг 7 — Wire preflight в CLI binaries

**Файл:** `crates/swarm-examples/src/bin/sitl_supervisor.rs`

Перед dry-run/mock execution вызвать `check_preflight_or_err(&entry)?`. При
`--output-dir`: записать `safety_validation_report.v1.json` в output dir.

**Файл:** `crates/swarm-examples/src/bin/sitl_agent.rs`

Аналогично для `--dry-run` mode: вызвать preflight и включить результат в
`SitlDryRunArtifact`.

Ожидаемый результат: unsafe mission → exit code 2, stderr содержит rule_ids нарушений.

---

### Шаг 8 — Тесты категории 1

**Новые тесты в `crates/swarm-safety/src/preflight.rs` (секция `#[cfg(test)]`):**

1. `geofence_violation_fails_preflight` — task.pose outside geofence → violation
   `rule_id = "geofence.waypoint_outside"`, `passed = false`.
2. `nofly_aabb_violation_fails_preflight` — task.pose inside no-fly zone → violation
   `rule_id = "nofly.waypoint_inside"`, `passed = false`.
3. `nonfinite_coordinate_rejected` — task.pose.x = `f64::NAN` → violation
   `rule_id = "pose.invalid_coordinate"`.
4. `report_ok_when_no_violations` — valid poses, no config limits → `passed = true`.
5. `violation_severity_serde_roundtrip` — `ViolationSeverity::Error` сериализуется как
   `"error"`, `Warning` → `"warning"`.

**Новые тесты в `crates/swarm-sim/src/preflight.rs` (секция `#[cfg(test)]`):**

6. `duplicate_task_id_rejected` — два task с одинаковым `id` → `rule_id =
   "ownership.duplicate_task_id"`.
7. `unsupported_strategy_pair_returns_warning` — mission="sar", cbba=true → Warning violation.
8. `urban_blocked_edge_fails_preflight` — edge с `blocked=true` в маршруте → `rule_id =
   "urban.blocked_edge"`.
9. `urban_aabb_intersection_fails_preflight` — waypoint inside static obstacle → `rule_id =
   "urban.aabb_intersection"`.
10. `valid_urban_route_passes_preflight` — корректный urban сценарий → `passed = true`.

**Новые тесты в `crates/swarm-examples/tests/sitl_agent/` (CLI-level):**

11. `preflight_failure_exits_nonzero_with_rule_ids` — невалидный сценарий с
    geofence violation → exit code 2, stderr содержит `"geofence.waypoint_outside"`.
12. `valid_scenario_passes_preflight_and_succeeds` — существующий валидный dry-run fixture
    → exit code 0.
13. `safety_report_written_when_output_dir_requested` — `--output-dir` + preflight pass →
    `safety_validation_report.v1.json` создан.

**Обновить существующие тесты:**

14. Обновить exit code assertions в `supervisor_tests.rs` и `cli_and_connection_tests.rs`
    под новую конвенцию (2/3/4/5). Запустить после шага 6.

---

### Шаг 9 — Документация и README

**Новый файл:** `docs/PREFLIGHT_SAFETY.md`

Структура:
- Раздел "Preflight Rules" — таблица всех rule_ids с описанием и severity.
- Раздел "What Is Not Checked" — список: runtime obstacle avoidance, real sensor data,
  hardware failsafe, released task history (requires runtime), full support matrix,
  regulatory compliance.
- Раздел "Not Certified Flight Safety" — обязательный disclaimer.
- Раздел "Exit Code Convention" — описание 2/3/4/5.
- Раздел "Non-Goals" — цитата из M71 spec.

**Обновить:** `docs/STATUS.md` — добавить строку M71 в milestone table (после M70).

**Обновить:** `docs/HARDWARE_READINESS.md` — добавить ссылку на preflight gate
(no-hardware-candidate-run without passing preflight).

**Обновить:** `README.md` — строка M71 в milestone table.

Ожидаемый результат: docs проходят smoke-test `sitl_docs.rs`, preflight rules перечислены,
disclaimer присутствует.

---

## Стратегия тестирования

### Категория 1: без рефакторинга (реализуется вместе с основными изменениями)

| # | Тест | Файл |
|---|---|---|
| 1 | `geofence_violation_fails_preflight` | `swarm-safety/src/preflight.rs` |
| 2 | `nofly_aabb_violation_fails_preflight` | `swarm-safety/src/preflight.rs` |
| 3 | `nonfinite_coordinate_rejected` | `swarm-safety/src/preflight.rs` |
| 4 | `report_ok_when_no_violations` | `swarm-safety/src/preflight.rs` |
| 5 | `violation_severity_serde_roundtrip` | `swarm-safety/src/preflight.rs` |
| 6 | `duplicate_task_id_rejected` | `swarm-sim/src/preflight.rs` |
| 7 | `unsupported_strategy_pair_returns_warning` | `swarm-sim/src/preflight.rs` |
| 8 | `urban_blocked_edge_fails_preflight` | `swarm-sim/src/preflight.rs` |
| 9 | `urban_aabb_intersection_fails_preflight` | `swarm-sim/src/preflight.rs` |
| 10 | `valid_urban_route_passes_preflight` | `swarm-sim/src/preflight.rs` |
| 11 | `preflight_failure_exits_nonzero_with_rule_ids` | `swarm-examples/tests/sitl_agent/` |
| 12 | `valid_scenario_passes_preflight_and_succeeds` | `swarm-examples/tests/sitl_agent/` |
| 13 | `safety_report_written_when_output_dir_requested` | `swarm-examples/tests/sitl_agent/` |

### Категория 2: лёгкий рефакторинг

- Shared assertion helper `assert_violation(report, rule_id)` в `swarm-safety/src/preflight.rs`.
- Fixture builder `make_entry_with_geofence(pose_outside: Pose) -> ScenarioSuiteEntry` в
  `swarm-sim/src/preflight.rs` tests.
- CLI rule-id assertion helper `assert_rule_id_in_stderr(output, rule_id)`.
- Shared ownership invariant assertion helper.

### Категория 3: тяжёлый рефакторинг

- Property tests (proptest) для сгенерированных waypoints vs geofence/no-fly rules —
  требуют `proptest` в `swarm-safety` + portable генератор.
- Cross-mission preflight compatibility suite (all missions × all profiles) — требует
  рефакторинга fixture generation.
- Battery reserve estimator tests с mission-duration model — требует нового поля
  `estimated_duration_ticks` в RunConfig.
- Versioned safety report compatibility tests (backward compat JSON) — требуют
  test fixture management.

### Покрытие: что не покрывается автотестом

- `ownership.released_not_resolved` — требует runtime assignment history, недоступной
  в static preflight. **Gap зафиксирован** в `docs/PREFLIGHT_SAFETY.md`.
- `urban.planner_metadata_mismatch` — требует реального dry-run export artifact.
  Будет покрыт в M72 artifact validator.
- Hardware-adjacent CLI тесты (реальный PX4) — manual-only по определению.

## Что могло сломаться

| Область | Потенциальная регрессия | Как проверить |
|---|---|---|
| Exit codes | Все тесты, проверяющие конкретные exit codes (20/21/22/30/40), сломаются после шага 6 | `cargo test -p swarm-examples` — проходит только после обновления assertions |
| `SafetyConfig` serde | Старые JSON файлы с `safety_config` без новых полей | Существующие тесты `serde_roundtrip`; запустить `cargo test -p swarm-safety` |
| DSL validation | Новый preflight call в `validate_entry()` может отклонить ранее валидные сценарии | `cargo test -p swarm-sim -- smoke_suites scenario_catalog` |
| Urban route expand | Если route expansion в preflight падает, urban checks пропускаются без ошибки | `urban_blocked_edge_fails_preflight` тест |
| `SitlDryRunArtifact` schema | Новое поле `safety_report` в artifact JSON — читаемо старым кодом? | `cargo test -p swarm-examples -- benchmark_pack` |
| Naming collision | Путаница `swarm_safety::SafetyViolation` (runtime) vs `swarm_safety::preflight::SafetyViolation` (preflight) | clippy + grep на импорты |
| `HardwareCandidateRequiresExplicitAllow` | Переезжает с кода 3 на 5 — сломает тесты если есть | Поиск в тестах grep на exit code 3 + SafetyConfig |

## Открытые вопросы

1. **`completion predicate` invariant** — `semantics.completion_predicate_missing`:
   что конкретно является predicate в текущей архитектуре? `MissionAdapter::is_completed`
   всегда определена для известных mission types. Если речь идёт о документальном
   predicate, это задача M71 docs, не кода. Без уточнения rule реализуется как Warning
   для неизвестных mission types.

2. **Full support matrix as data** — `semantics.unsupported_strategy_pair` реализуется
   в M71 как минимальный hardcoded list. Полная машиночитаемая матрица — M77/M78.
   Нужно ли уже в M71 создавать `SupportMatrix` тип или достаточно `fn is_pair_supported`?

3. **`min_altitude_m` severity** — Warning vs Error? Если drone летит ниже минимальной
   высоты, это может быть нормой для посадки. Предложение: Warning. Подтвердить.

4. **`sitl_agent` exit codes** — нужно ли обновлять `sitl_agent` binary (есть ли там
   отдельные exit codes помимо `sitl_supervisor`)? Если да, `exit_codes.rs` может стать
   shared между обоими бинарниками.

5. **`safety_validation_report.v1.json` format** — нужно ли добавить `schema_version` поле
   в `SafetyValidationReport` для совместимости с M72 artifact validator?
