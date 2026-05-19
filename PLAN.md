# PLAN: Mission DSL

## Context

Текущие 3 миссии (Coverage, EmergencyMesh, SAR) описаны как Rust-функции-билдеры в `swarm-scenarios`. Добавление новой миссии или профиля требует изменения кода и перекомпиляции. Это ограничивает воспроизводимость и CI-автоматизацию.

**Mission DSL** делает сценарии декларативными артефактами (JSON/YAML), загружаемыми без перекомпиляции.

**Источники контекста:** `docs/DRONE_A.7.md`, `docs/DRONE_B.7.md`. INVESTIGATION.md отсутствует.

**Текущее состояние (M11 hardening complete):**
- `Agent`, `Task`, `Pose`, `Role`, `SearchGrid`, `SensorModel` — все имеют `Serialize`/`Deserialize`
- `RunConfig` — все поля имеют `#[serde(default)]` или explicit
- `Scenario` — struct с `agents: Vec<Agent>`, `tasks: Vec<Task>`, `ground_nodes`, `base_station`
- `SarScenarioConfig`, `EmergencyMeshConfig`, `CoverageConfig` — все serde-совместимы
- `SarProfile`, `EmergencyMeshProfile` — enum с `from_str` парсингом
- `strategy_comparison` — загружает стратегии из кода, сценарии из Rust-билдеров

**Критерий готовности:**
1. `Scenario` сериализуется в JSON и десериализуется обратно без потери данных.
2. `ScenarioSuite` — файл, содержащий массив `Scenario` с metadata (name, description, profiles).
3. `strategy_comparison` поддерживает `--scenario-suite <path>` — загрузка сценариев из JSON.
4. Все 3 миссии имеют примеры JSON-файлов в `scenarios/` директории.
5. `ScenarioRunner` может запустить `Scenario` из JSON без Rust-билдера.
6. README документирует DSL формат и примеры.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.7.md` и `docs/DRONE_B.7.md`:
- Оба документа: Mission DSL как инфраструктура для воспроизводимых сценариев без перекомпиляции.
- "Текущие три миссии уже показали минимальный нужный набор полей — риск зацементировать плохую модель минимален."

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-types/src/lib.rs` | Re-export новых DSL типов |
| `crates/swarm-sim/src/scenario.rs` | Добавить `ScenarioMetadata`; `Scenario` → `impl Serialize/Deserialize` если отсутствует |
| `crates/swarm-sim/src/dsl.rs` | **NEW** — `ScenarioSuite`, `load_scenario_suite(path)`, `export_scenario_to_json(scenario)` |
| `crates/swarm-sim/src/lib.rs` | Export `dsl` модуля |
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | `--scenario-suite <path>` флаг для загрузки из JSON |
| `scenarios/` | **NEW** директория в workspace root — примеры JSON/YAML сценариев |
| `README.md` | Документировать DSL формат, примеры |
| `crates/swarm-scenarios/src/*.rs` | Опционально: `export_*_scenario_to_json()` функции для генерации примеров |

---

## Implementation Steps

### Шаг 1 — Сериализация Scenario и RunConfig

Файлы: `crates/swarm-sim/src/scenario.rs`, `crates/swarm-types/src/*.rs`

Проверить, что `Scenario` имеет `Serialize`/`Deserialize`. Если нет — добавить. То же для `RunConfig`.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub seed: u64,
    pub agents: Vec<Agent>,
    pub tasks: Vec<Task>,
    pub ground_nodes: Vec<GroundNode>,
    pub base_station: Option<Pose>,
}
```

Все поля `Agent`, `Task`, `Pose`, `GroundNode` уже serde-совместимы. `#[serde(default)]` где нужно для backward compat.

**Тесты (категория 1):**
- `scenario_json_roundtrip` — сериализация → десериализация без потерь
- `scenario_json_contains_agents_and_tasks` — JSON содержит ключевые поля
- `run_config_json_roundtrip` — RunConfig roundtrip

### Шаг 2 — ScenarioSuite DSL формат

Файл: `crates/swarm-sim/src/dsl.rs` (новый)

```rust
/// A suite of scenarios for batch benchmarking.
#[derive(Serialize, Deserialize)]
pub struct ScenarioSuite {
    pub name: String,
    pub description: String,
    pub scenarios: Vec<ScenarioSuiteEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct ScenarioSuiteEntry {
    pub mission: String,
    pub profile: String,
    pub scenario: Scenario,
    pub run_config: RunConfig,
}

pub fn load_scenario_suite(path: &str) -> Result<ScenarioSuite, Box<dyn Error>> {
    let json = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&json)?)
}

pub fn export_scenario_to_json(scenario: &Scenario, run_config: &RunConfig) -> String {
    serde_json::to_string_pretty(&ScenarioSuiteEntry { ... }).unwrap()
}
```

**Пример JSON файла** (`scenarios/coverage.ideal.json`):
```json
{
  "name": "Coverage Benchmark Suite",
  "description": "Coverage scenario with standard profiles",
  "scenarios": [
    {
      "mission": "coverage",
      "profile": "ideal",
      "scenario": {
        "name": "coverage_ideal_0",
        "seed": 0,
        "agents": [
          {"id": "agent-0", "role": "scout", "health": "alive", "pose": {"x": 0.0, "y": 0.0}, "capabilities": [], "current_task": null, "battery": 100.0, "comms_range": null, "generation": 1, "speed": 0.0, "max_range": 0.0, "battery_drain_rate": 0.0}
        ],
        "tasks": [
          {"id": "task-0", "status": "unassigned", "assigned_to": null, "priority": 1, "required_capabilities": [], "required_role": null, "preferred_role": null, "expires_at": null, "pose": null, "grid_cell": null}
        ],
        "ground_nodes": [],
        "base_station": null
      },
      "run_config": {
        "max_ticks": 50,
        "timeout_ticks": 3,
        "max_unassigned_ticks": 10,
        "packet_loss_rate": 0.0,
        "latency_ticks": 0,
        "latency_per_hop": 0,
        "failures": [],
        "dynamic_tasks": [],
        "partition_events": [],
        "gossip_interval_ticks": 999,
        "base_id": null,
        "enable_movement": false,
        "tick_duration_ms": 100,
        "grid_state": null,
        "enable_cbba": false
      }
    }
  ]
}
```

### Шаг 3 — --scenario-suite флаг в strategy_comparison

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

```rust
"--scenario-suite" => {
    i += 1;
    if i < args.len() {
        cli.scenario_suite_path = Some(args[i].clone());
    }
}
```

В `main()`, если `cli.scenario_suite_path` указан:
1. Загрузить `ScenarioSuite` из файла
2. Для каждого `ScenarioSuiteEntry`: запустить BenchmarkHarness с `entry.scenario`, `entry.run_config`, `entry.mission`, `entry.profile`
3. Вывести ComparisonReport как обычно

### Шаг 4 — Генерация примеров из существующих билдеров

Файлы: `crates/swarm-scenarios/src/coverage.rs`, `emergency_mesh.rs`, `sar_scenario.rs`

Добавить `#[cfg(test)]` тесты (или отдельные `[[bin]]`), которые вызывают существующие билдеры и экспортируют результаты в `scenarios/` директорию:

```rust
#[test]
fn export_coverage_ideal_to_json() {
    let config = CoverageConfig::from_profile(0, "ideal-no-failures");
    let (scenario, run_config) = build_coverage_scenario(&config);
    let json = export_scenario_to_json(&scenario, &run_config);
    std::fs::write("scenarios/coverage.ideal-no-failures.json", json).unwrap();
}
```

### Шаг 5 — Обновить README

Документировать DSL:
- Пример JSON файла
- Команда `--scenario-suite <path>`
- Как создавать новые сценарии (редактировать JSON или использовать Rust builder)

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run -p swarm-examples --bin strategy_comparison -- --scenario-suite scenarios/coverage.ideal.json
```

---

## Testing Strategy

### Категория 1 — unit тесты

- `scenario_json_roundtrip` — Scenario сериализуется и десериализуется
- `run_config_json_roundtrip` — RunConfig сериализуется и десериализуется
- `scenario_suite_load_from_file` — загрузка Suite из JSON файла
- `scenario_suite_entry_contains_mission_and_profile` — поля mission/profile в entry
- `serialized_json_contains_expected_keys` — ключевые поля в JSON

### Категория 2 — integration

- `strategy_comparison_accepts_scenario_suite` — `--scenario-suite` запускается
- `coverage_from_json_matches_rust_builder` — JSON-сценарий даёт тот же результат что и builder
- `sar_from_json_matches_rust_builder` — SAR сценарий roundtrip

### Категория 3 — manual

- Создание полного scenario suite для всех 3 миссий (run through `strategy_comparison --mission all`)
- YAML/RON поддержка (серде опционально, не для v0.12)

---

## Risks and Tradeoffs

**1. JSON verbosity**

Agent с 15 полями × 10 agents = 150 строк JSON. Для ручного редактирования — неудобно. Митигация: Rust builder для создания JSON, JSON для хранения и CI. YAML как более читаемый формат позже.

**2. RunConfig гиперпараметры**

`RunConfig` содержит множество полей (25+). Некоторые из них machine-specific (seed_range). Митигация: `#[serde(default)]` для всех опциональных полей.

**3. ScenarioSuite vs единичные .json файлы**

`ScenarioSuite` — один большой файл с массивом сценариев. Удобно для CI. Единичные `.json` файлы — удобно для разработки. Поддерживаем оба.

**4. Breaking changes при изменении структуры Agent/Task**

Новые поля в `Agent`/`Task` добавляются с `#[serde(default)]` — старые JSON остаются валидными.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| `Scenario` не может быть десериализован из-за отсутствующего `Deserialize` | Компиляция падает | `cargo check` |
| Старые JSON несовместимы с новыми полями | `#[serde(default)]` на всех новых полях | roundtrip тест |
| `comms_range: null` в JSON | Уже обрабатывается через `default_comms_range` | существующий тест `agent_comms_range_serde_default_infinity` |
| SAR поля в RunConfig не сериализуются | `grid_state` — сложный тип, не serde | unit тест run_config_json_roundtrip |

---

## Open Questions

1. **JSON или YAML?** — JSON для v0.12 (уже используется в replay/export). YAML через `serde_yaml` позже.
2. **Где хранить .json сценарии?** — `scenarios/` в workspace root, с поддиректориями `coverage/`, `emergency-mesh/`, `sar/`.
3. **Нужна ли валидация сценария при загрузке?** — JSON schema для валидации структуры. Для v0.12: базовая проверка через deserialization errors.
4. **Должен ли ScenarioSuite поддерживать seed ranges?** — Да, можно добавить `seed_start`/`seed_end` на suite level, чтобы генерировать N вариантов одного сценария с разными seed.
