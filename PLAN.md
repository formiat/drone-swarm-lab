# PLAN — M19: DSL Schema / Validation

## Context

M18 завершён — все 11 `scenarios/*.json` валидны и проходят catalog load test. Однако DSL сейчас — это просто serde-десериализация: неверный JSON → panic/непонятная ошибка, нет семантической валидации mission-specific полей, нет schema_version для forward compatibility, CLI использует `unwrap()` и `panic!`.

**Цель M19:** превратить DSL из serde-формата в стабильный пользовательский контракт с валидацией, понятными ошибками и документацией.

**Источники контекста:** `docs/DRONE_A.10.linear.md` — M19 в линейном roadmap.

**Текущее состояние:**
- `ScenarioSuite`, `ScenarioSuiteEntry`, `RunConfig`, `Scenario` — все serde-совместимы
- `load_scenario_suite()` возвращает `Result<_, Box<dyn Error>>` с serde_json ошибкой
- `scenarios/` — 11 файлов, все валидны после M18
- `strategy_comparison` CLI использует `unwrap()` / `panic!` при ошибках загрузки
- `sitl_agent` CLI использует `unwrap()` / `eprintln! + exit(1)`
- Валидации mission-specific полей нет (SAR без grid_state — ошибка на позднем этапе)
- `schema_version` отсутствует во всех JSON-файлах

**Критерий готовности:**
1. `ScenarioSuite` и `ScenarioSuiteEntry` поддерживают `schema_version`
2. `validate_scenario_suite()` и `validate_entry()` реализованы с typed errors
3. Проверяются обязательные поля и mission-specific constraints
4. CLI заменяет panics на человекочитаемые ошибки
5. Invalid scenario tests покрывают основные классы ошибок
6. README/docs описывают DSL как стабильный контракт v0.1

---

## Investigation context

INVESTIGATION.md отсутствует. DRONE_A.10.linear.md определяет M19 как «DSL Schema / Validation» — второй шаг после M18 в линейном roadmap.

Ключевое наблюдение: `load_scenario_suite` уже использует `serde_json::from_str` — при невалидном JSON ошибка содержит `line:column`, но без контекста «какой файл, какая mission, какое поле». При mission-specific ошибках (SAR без grid_state) ошибка возникает глубоко в runner, а не при загрузке.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-sim/src/dsl.rs` | `schema_version`, `ValidationError`, `validate_scenario_suite`, `validate_entry`, обновить `ScenarioSuite`/`ScenarioSuiteEntry` |
| `scenarios/*.json` (11 файлов) | Добавить `"schema_version": "0.1"` |
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | Заменить panics/unwrap на человекочитаемые ошибки |
| `crates/swarm-examples/src/bin/sitl_agent.rs` | Заменить panics/unwrap на человекочитаемые ошибки |
| `crates/swarm-sim/tests/dsl_validation.rs` | **NEW** — invalid scenario tests |
| `README.md` | Документировать DSL контракт v0.1 |

---

## Implementation Steps

### Шаг 1 — schema_version в ScenarioSuite

Файл: `crates/swarm-sim/src/dsl.rs`

Добавить поле:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioSuite {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    pub name: String,
    pub description: String,
    pub scenarios: Vec<ScenarioSuiteEntry>,
}

fn default_schema_version() -> String {
    "0.1".to_owned()
}
```

Обновить все 11 JSON-файлов — добавить `"schema_version": "0.1"`.

### Шаг 2 — Validation API

Файл: `crates/swarm-sim/src/dsl.rs`

```rust
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

pub fn validate_scenario_suite(suite: &ScenarioSuite) -> Vec<ValidationError>;
pub fn validate_entry(entry: &ScenarioSuiteEntry) -> Vec<ValidationError>;
```

Проверки `validate_entry`:
- `mission` не пустая
- `profile` не пустая
- `scenario.name` не пустой
- `scenario.agents` не пуст
- `scenario.tasks` не пуст (или хотя бы один агент с задачами)
- `run_config.max_ticks > 0`

Проверки `validate_scenario_suite`:
- `name` не пустой
- `scenarios` не пуст
- `schema_version` — поддерживаемая версия (0.1)

### Шаг 3 — Mission-specific constraints

Файл: `crates/swarm-sim/src/dsl.rs`

```rust
pub fn validate_mission_specific(entry: &ScenarioSuiteEntry) -> Vec<ValidationError> {
    match entry.mission.as_str() {
        "sar" => validate_sar(entry),
        "inspection" => validate_inspection(entry),
        "sitl" => validate_sitl(entry),
        "cbba-stress" => validate_cbba_stress(entry),
        _ => vec![], // coverage, emergency-mesh — no specific constraints
    }
}
```

- **SAR:** требует `run_config.grid_state` (или хотя бы один агент для SAR scan), задачи с `grid_cell`
- **Inspection:** задачи должны иметь `edge_id` (хотя бы одна), `run_config.enable_movement = true`
- **SITL:** хотя бы одна задача с `pose`
- **Safety scenarios:** `run_config.safety_config.is_some()`
- **CBBA stress:** `run_config.enable_cbba = true`, `gossip_interval_ticks` мал (≤5)

### Шаг 4 — CLI error messages

Файлы: `strategy_comparison.rs`, `sitl_agent.rs`

Заменить:
- `load_scenario_suite(path).unwrap()` → `match { Ok(s) => s, Err(e) => eprintln!("...") + exit(1) }`
- Паники при пустом suite → eprintln + exit(1)
- Добавить вывод validation errors перед запуском

```rust
let suite = load_scenario_suite(path).unwrap_or_else(|e| {
    eprintln!("Error loading {}: {}", path, e);
    std::process::exit(1);
});
let errors = validate_scenario_suite(&suite);
if !errors.is_empty() {
    for err in &errors {
        eprintln!("Validation error: {} — {}", err.field, err.message);
    }
    std::process::exit(1);
}
```

### Шаг 5 — Invalid scenario tests

Файл: `crates/swarm-sim/tests/dsl_validation.rs` (новый)

```rust
#[test]
fn validate_rejects_empty_mission() { ... }
#[test]
fn validate_rejects_empty_profile() { ... }
#[test]
fn validate_rejects_no_agents() { ... }
#[test]
fn validate_rejects_zero_max_ticks() { ... }
#[test]
fn validate_sar_rejects_no_grid_state() { ... }
#[test]
fn validate_inspection_rejects_no_edge_id() { ... }
#[test]
fn validate_sitl_rejects_no_pose_tasks() { ... }
#[test]
fn validate_cbba_stress_rejects_no_enable_cbba() { ... }
#[test]
fn validate_accepts_valid_coverage_scenario() { ... }
#[test]
fn schema_version_defaults_to_0_1() { ... }
```

### Шаг 6 — README / DSL документация

Файл: `README.md`

Добавить раздел **M19 — DSL Schema / Validation**:
- Описание schema_version и validation API
- Минимальный пример валидного JSON
- Таблица mission-specific требований
- Примеры ошибок и их исправлений

---

## Testing Strategy

### Категория 1 — unit тесты (dsl_validation)

- `validate_rejects_empty_mission` — пустая mission → error
- `validate_rejects_empty_profile` — пустой profile → error
- `validate_rejects_no_agents` — agents пуст → error
- `validate_rejects_zero_max_ticks` — max_ticks=0 → error
- `schema_version_defaults_to_0_1` — legacy JSON без version → defaults to 0.1
- `validate_accepts_valid_entry` — валидный entry → ok

### Категория 2 — integration (dsl_validation)

- `validate_sar_rejects_no_grid_state` — SAR с grid_state=None → error
- `validate_inspection_rejects_no_edge_id` — inspection без edge_id → warn/error
- `validate_sitl_rejects_no_pose_tasks` — sitl без pose-задач → error
- `validate_cbba_rejects_no_enable_cbba` → error

### Категория 3 — e2e

- `cli_reports_validation_errors` — невалидный JSON → exit(1) с понятным сообщением
- `catalog_remains_valid` — все 11 scenarios/*.json валидны после добавления schema_version

---

## Risks and Tradeoffs

**1. schema_version и forward compatibility**
Legacy JSON без `schema_version` получает `"0.1"` по умолчанию. При повышении версии (0.2) — валидатор должен отклонять старые файлы или мигрировать.

**2. Mission-specific валидация — coupling**
Валидация mission-specific полей в `dsl.rs` создаёт coupling между swarm-sim и swarm-scenarios. Альтернатива — вынести в `swarm-scenarios`, но тогда валидация требует импорта scenario кода. Решение: оставить в `dsl.rs` — проверки только на уровне структуры данных.

**3. Breaking change в JSON**
Добавление `schema_version` ломает serde-десериализацию только если файл имеет `deny_unknown_fields`. Все наши структуры имеют `#[serde(deny_unknown_fields = false)]` по умолчанию — лишние поля игнорируются, недостающие — `#[serde(default)]`.

**4. Validation overhead**
Валидация вызывается один раз при загрузке suite. Для 11 файлов — < 1ms.

---

## Что могло сломаться

| Риск | Проверка |
|---|---|
| Добавление `schema_version` ломает десериализацию старых JSON | catalog test + dsl tests |
| Удаление PANIC из CLI ломает E2E flow | запуск `strategy_comparison --scenario-suite` с невалидным JSON |
| Mission-specific валидация даёт ложные срабатывания | `validate_accepts_valid_entry` тест |
| SAR валидация требует grid_state которого нет в некоторых SAR сценариях | проверить все SAR JSON на наличие grid_state |

---

## Open Questions

1. **Версионирование: semantic (0.1.0) или simple (0.1)?** — simple "0.1" для v0.1, "0.2" для v0.2.
2. **Mission-specific валидация: strict или warn?** — strict для явно сломанных сценариев (SAR без grid_state = error), warn для stylistic (inspection без edge_id у всех задач = warn).
3. **Должна ли валидация проверять числовые диапазоны?** — не в v0.1 (max_ticks=1e6 не ошибка, просто неэффективно).

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test --workspace
cargo test -p swarm-sim --test dsl_validation
cargo run -p swarm-examples --bin strategy_comparison -- --scenario-suite scenarios/coverage.ideal.json --json /tmp/test.json
```
