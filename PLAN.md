# PLAN — M18: Integration & Scenario Catalog Hardening

## Context

M1–M17 реализованы. В кодовой базе 10 scenarios JSON, 5 mission entrypoints, DSL loader, SITL scaffold, safety layer, benchmark export. Однако часть сценариев не загружается (Infinity в inspection), нет теста на весь catalog, smoke-запуски не проверяются автоматически, safety integration tests отсутствуют, sitl waypoints сценарий не создан.

**Цель M18:** сделать существующие сценарии и пользовательские entrypoint-ы реально запускаемыми и проверяемыми.

**Источники контекста:** `docs/DRONE_A.10.linear.md` — финальный линейный roadmap после сравнения веток. M18 — первый шаг интеграционной линейки.

**Текущее состояние:**
- `scenarios/` содержит 10 JSON файлов (coverage ×2, emergency-mesh ×1, SAR ×3, cbba ×1, inspection ×3)
- `scenarios/inspection.linear.json` — 3 поля `max_range: Infinity` (не сериализуется через serde_json)
- `scenarios/inspection.random.json` — 5 полей `max_range: Infinity`
- Тест загрузки catalog отсутствует
- Smoke-run тесты для ключевых suite отсутствуют
- Safety integration tests отсутствуют
- `sitl.waypoints.json` не существует
- 241 тест, clippy чист

**Критерий готовности:**
1. Все `scenarios/*.json` файлы валидны для `load_scenario_suite`
2. Ключевые suite запускаются smoke-командами
3. Safety behavior покрыт integration tests
4. `sitl.waypoints.json` загружается и содержит pose-задачи

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.10.linear.md`:
- `scenarios/inspection.*.json` содержат `Infinity` в `max_range` — serde_json не может сериализовать/десериализовать f64::INFINITY (Product → null → ошибка при десериализации в f64).
- Требуется замена на конечное значение (напр. `1000.0`).
- Safety behavior не покрыт интеграционными тестами — только unit-тесты `check_agent`.
- SITL scaffold (`sitl_agent`) нуждается в waypoint-сценарии для тестирования `--mock` режима.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `scenarios/inspection.linear.json` | Fix: заменить `Infinity` → конечный `max_range` |
| `scenarios/inspection.random.json` | Fix: заменить `Infinity` → конечный `max_range` |
| `scenarios/sitl.waypoints.json` | **NEW** — 1 агент, 3 задачи с pose для `sitl_agent --mock` |
| `crates/swarm-sim/tests/scenario_catalog.rs` | **NEW** — тест загрузки всех `scenarios/*.json` |
| `crates/swarm-sim/tests/smoke_suites.rs` | **NEW** — smoke-run ключевых suite через runner |
| `crates/swarm-sim/tests/safety_integration.rs` | **NEW** — safety integration tests |
| `crates/swarm-sim/src/runner.rs` | Расширение safety violation logging (если нужно для тестов) |
| `README.md` | Обновление раздела M18, scenario catalog |

---

## Implementation Steps

### Шаг 1 — Fix inspection JSONs (Infinity → finite max_range)

Файлы: `scenarios/inspection.linear.json`, `scenarios/inspection.random.json`

Заменить все `"max_range": Infinity` на `"max_range": 1000.0` (конечное значение, достаточное для любого inspection сценария). 
Также заменить `"battery_drain_rate": Infinity` на конечные значения в `inspection.random.json`.

Проверка: `load_scenario_suite("scenarios/inspection.linear.json").is_ok()`

### Шаг 2 — Scenario catalog load test

Файл: `crates/swarm-sim/tests/scenario_catalog.rs` (новый)

```rust
#[test]
fn all_scenario_files_load() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios");
    for entry in std::fs::read_dir(dir).expect("scenarios dir exists") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "json") {
            let result = swarm_sim::load_scenario_suite(path.to_str().unwrap());
            assert!(result.is_ok(), "Failed to load: {}", path.display());
        }
    }
}
```

### Шаг 3 — Smoke-run tests for key suites

Файл: `crates/swarm-sim/tests/smoke_suites.rs` (новый)

Запустить через `ScenarioRunner` следующие suite с минимальными параметрами (1 seed, quick):

- `coverage.safety.json` — проверка safety_violations == 0
- `sar.uncertain.json` — проверка успешного завершения
- `sar.noisy.json` — проверка false_positive_rate > 0
- `cbba_stress.json` — проверка CBBA convergence
- `inspection.linear.json` — проверка edge_coverage_rate > 0.8

```rust
fn smoke_run(path: &str) -> RunMetrics {
    let suite = load_scenario_suite(path).unwrap();
    let entry = &suite.scenarios[0];
    use swarm_alloc::GreedyAllocator;
    ScenarioRunner::run_with(&entry.scenario, entry.run_config.clone(), GreedyAllocator)
}
```

### Шаг 4 — Safety integration tests

Файл: `crates/swarm-sim/tests/safety_integration.rs` (новый)

4.1. **Задачи в no-fly zone не назначаются:**
- Создать сценарий с no-fly зоной и задачей внутри неё
- Запустить через ScenarioRunner с SafetyAllocator wrapper
- Проверить, что задача в no-fly не назначена ни одному агенту

4.2. **safety_violations считаются:**
- Создать сценарий с geofence и агентом вне geofence
- Проверить `metrics.safety_violations > 0`

4.3. **JSON/CSV export содержит safety_violations:**
- Запустить `strategy_comparison --scenario-suite scenarios/coverage.safety.json --json /tmp/safety.json`
- Проверить наличие `safety_violations` в JSON

4.4. **Separation не паникует:**
- Создать сценарий с separation constraint, агентами на расстоянии < min_distance
- Запустить — не паникует
- Проверить `safety_violations > 0` (separation breach)

### Шаг 5 — sitl.waypoints.json

Файл: `scenarios/sitl.waypoints.json` (новый)

```json
{
  "name": "SITL Waypoints",
  "description": "Minimal scenario for sitl_agent --mock with 3 waypoint tasks",
  "scenarios": [
    {
      "mission": "sitl",
      "profile": "waypoints",
      "scenario": {
        "name": "sitl_waypoints_0",
        "seed": 0,
        "agents": [
          {"id":"agent-0","role":"scout","health":"alive","pose":{"x":0.0,"y":0.0},"capabilities":[],"current_task":null,"battery":100.0,"comms_range":1000.0,"generation":1,"speed":0.0,"max_range":1000.0,"battery_drain_rate":0.0}
        ],
        "tasks": [
          {"id":"wp-0","status":"unassigned","assigned_to":null,"priority":1,"required_capabilities":[],"required_role":null,"preferred_role":null,"expires_at":null,"pose":{"x":10.0,"y":20.0},"grid_cell":null},
          {"id":"wp-1","status":"unassigned","assigned_to":null,"priority":1,"required_capabilities":[],"required_role":null,"preferred_role":null,"expires_at":null,"pose":{"x":50.0,"y":30.0},"grid_cell":null},
          {"id":"wp-2","status":"unassigned","assigned_to":null,"priority":1,"required_capabilities":[],"required_role":null,"preferred_role":null,"expires_at":null,"pose":{"x":100.0,"y":0.0},"grid_cell":null}
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

Тест: `load_scenario_suite("scenarios/sitl.waypoints.json").is_ok()` + проверка, что все задачи имеют `pose.is_some()`.

### Шаг 6 — README

Добавить раздел **M18 — Integration & Scenario Catalog Hardening**:
- Список исправленных сценариев
- Команда запуска catalog теста
- Smoke-run примеры
- Safety integration test описание
- `sitl.waypoints.json` пример использования

---

## Testing Strategy

### Категория 1 — unit тесты (no refactor)

- `scenario_catalog_load_all` — все `scenarios/*.json` грузятся без ошибок
- `sitl_waypoints_has_poses` — все задачи в `sitl.waypoints.json` имеют `pose`
- `inspection_linear_loads` — `inspection.linear.json` загружается
- `inspection_random_loads` — `inspection.random.json` загружается

### Категория 2 — integration (runner)

- `smoke_coverage_safety` — `coverage.safety.json` запускается, safety_violations == 0
- `smoke_sar_uncertain` — `sar.uncertain.json` запускается, success == true
- `smoke_cbba_stress` — `cbba_stress.json` запускается, CBBA converges
- `smoke_inspection_linear` — `inspection.linear.json` запускается, coverage > 0.8
- `safety_nofly_tasks_not_assigned` — задачи в no-fly не назначаются
- `safety_violations_counted` — violations увеличиваются при geofence нарушении
- `safety_separation_no_panic` — separation constraint не вызывает паники
- `safety_json_export_contains_column` — CSV/JSON export содержит `safety_violations`

### Категория 3 — manual

- SITL waypoints E2E: `sitl_agent --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0` — проверяется в CI через smoke

---

## Risks and Tradeoffs

**1. max_range = 1000.0**
Достаточно ли 1000.0 для всех inspection сценариев? Для grid_perimeter(10,10,10) — total_length = 400 м, 1000.0 на агента достаточно. Для linear_route(10,10) — 100 м, достаточно. Для random_graph(15) — < 500 м, достаточно.

**2. Smoke-run время**
5 ключевых suite × 1 seed каждый ≈ 1-2s суммарно. В пределах 5-минутного лимита.

**3. Safety integration tests изоляция**
Каждый safety тест создаёт изолированный сценарий — не зависит от других тестов.

**4. Scenario catalog порядок**
Тест загрузки catalog выполняется первым и быстро (0.01s на файл).

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| Замена Infinity на 1000.0 в inspection JSON | Агенты исчерпывают батарею при battery_constraint > 0 | `smoke_inspection_linear` проверяет coverage > 0.8 |
| Старые JSON не грузятся из-за новых тестов | `scenario_catalog_load_all` падает | Тест показывает конкретный файл |
| Safety тесты ломают существующие runner тесты | Race condition в runner | Каждый тест изолирован |
| sitl.waypoints.json не соответствует ожидаемой структуре | `sitl_agent --mock` падает | Тест `sitl_waypoints_has_poses` |

---

## Open Questions

1. **Нужен ли `--scenario-suite` флаг в catalog smoke тесте?** Нет — `load_scenario_suite` + `ScenarioRunner::run_with` достаточно.
2. **Должен ли `scenario_catalog_load_all` быть #[ignore]?** Нет — он быстрый (~0.1s), должен выполняться всегда.
3. **Какой max_range для inspection.random?** 1000.0 — достаточно для случайного графа с 15 узлами в квадрате 100×100.

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test --workspace
cargo test -p swarm-sim --test scenario_catalog
cargo run --bin sitl_agent -- --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0
```
