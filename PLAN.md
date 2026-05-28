# M38 — Wildfire / Flood v2

## Context

M37 (Realism Scenario Pack) закрыт. Реализованы:
- `RealismProfile` enum (light/medium/heavy) с `RealismParams`;
- Shared `swarm_examples::realism` модуль;
- 4 realism scenario JSON файла (coverage, SAR, inspection, wildfire);
- Battery model metadata в `BenchmarkManifest`;
- Realism поля в `RunMetrics` и `RunConfig`;
- README обновлён.

Следующий шаг по линейному плану DRONE_A.14.linear.md — M38 Wildfire / Flood v2.

## Investigation context

`INVESTIGATION.md` отсутствует. Анализ кода показал:

### 1. Профили существуют только в коде, не в scenario catalog

**Файл:** `crates/swarm-scenarios/src/wildfire.rs:27-31`

Доступно только 2 профиля:
- `SmallStatic` — 2 агента, 2 зоны, 200 тиков, `enable_dynamic_threat: false`
- `MediumDynamic` — 4 агента, 4 зоны, 400 тиков, `enable_dynamic_threat: true`

Нет dedicated scenario JSON файлов с `wildfire_state` (есть только `wildfire.realism.json`, но он generic — без `wildfire_state`).

### 2. Dynamic threat — тривиальная линейная эскалация

**Файл:** `crates/swarm-sim/src/runner.rs:929-951`

```rust
zone.threat_level = (zone.threat_level + 0.1).min(1.0);
zone.priority = (zone.priority + 1).min(10);
```

Проблемы:
- Нет spatial spread (огонь не распространяется между зонами);
- Нет wind influence на fire behavior;
- Нет zone geometry mutation (границы Aabb статичны);
- Нет inter-zone interaction.

### 3. Success semantics — простая mapped-ratio проверка

**Файл:** `crates/swarm-sim/src/runner.rs:279-288`

```rust
let mapped_ratio = mapped / total;
let wildfire_success = mapped_ratio >= wildfire_success_threshold
    && all_expected_failures_detected
    && max_task_unassigned_ticks <= max_unassigned_ticks_config;
```

Проблемы:
- Нет различия между high-priority и low-priority zones;
- Нет time-to-first-high-risk-zone metric;
- Нет unsupported strategy detection для wildfire (в отличие от SAR).

### 4. Replay не обрабатывает wildfire события

**Файл:** `crates/swarm-replay/src/replay.rs`

Event log определяет 3 wildfire события (`AgentObservation`, `HazardMapUpdated`, `TaskPriorityUpdated`), но `replay()` и `summarize()` — no-op для них.

### 5. Metrics экспорт неполный

**Файлы:** `crates/swarm-metrics/src/metrics.rs`, `crates/swarm-sim/src/report_export.rs`

Есть:
- `hazard_zones_mapped`
- `priority_updates`
- `final_avg_threat_level`

Нет:
- `high_priority_zones_mapped` (zones with priority >= threshold);
- `time_to_map_first_high_risk` (tick when first high-threat zone was mapped);
- `threat_level_over_time` (vector of avg threat per tick, like `coverage_over_time`);
- `zone_observations` (count of agent observations).

### 6. Агенты — generic Scout с thermal capability

**Файл:** `crates/swarm-scenarios/src/wildfire.rs:144-168`

- `Role::Scout` вместо wildfire-specific роли;
- Нет fire-specific sensors;
- Нет danger avoidance logic.

## Affected components

| Компонент | Путь | Что меняется |
|---|---|---|
| Scenario profiles | `crates/swarm-scenarios/src/wildfire.rs` | Новые профили: `LargeStatic`, `HighThreatDynamic` |
| Scenario JSONs | `scenarios/` | `wildfire.small-static.json`, `wildfire.medium-dynamic.json` |
| Dynamic behavior | `crates/swarm-sim/src/runner.rs` | Spatial spread, wind influence, zone expansion |
| Metrics | `crates/swarm-metrics/src/metrics.rs` | `high_priority_zones_mapped`, `time_to_map_first_high_risk` |
| Replay | `crates/swarm-replay/src/replay.rs` | Обработка wildfire событий в summarize |
| Report export | `crates/swarm-sim/src/report_export.rs` | Wildfire metrics в JSON/CSV/table |
| Adapter | `crates/swarm-types/src/adapter.rs` | Улучшенный `WildfireAdapter::score` с threat awareness |
| README | `README.md` | Wildfire v2 section, scenario catalog |

## Implementation steps

### 1. Добавить scenario JSON файлы

**Файлы:** `scenarios/wildfire.small-static.json`, `scenarios/wildfire.medium-dynamic.json`

Структура:
```json
{
  "name": "Wildfire Small Static",
  "scenarios": [
    {
      "mission": "wildfire",
      "profile": "small-static",
      "scenario": { ...agents, tasks... },
      "run_config": {
        "max_ticks": 200,
        "wildfire_state": {
          "zones": [
            {"id": "zone-a", "threat_level": 0.7, "priority": 5},
            {"id": "zone-b", "threat_level": 0.3, "priority": 3}
          ],
          "update_interval_ticks": 999,
          "enable_dynamic_threat": false
        }
      }
    }
  ]
}
```

### 2. Расширить профили

**Файл:** `crates/swarm-scenarios/src/wildfire.rs`

Добавить:
- `LargeStatic` — 6 агентов, 6 зон, 300 тиков, mixed threat levels;
- `HighThreatDynamic` — 4 агента, 4 зоны, 500 тиков, `enable_dynamic_threat: true`, быстрая эскалация (update_interval = 25).

### 3. Улучшить dynamic behavior

**Файл:** `crates/swarm-sim/src/runner.rs`

- **Spatial spread**: если зона A граничит с зоной B и threat_level A > 0.8, зона B получает +0.05 к threat_level;
- **Wind influence**: если `run_config.wind` задано, зоны "downwind" от high-threat зон получают ускоренную эскалацию;
- **Zone expansion**: зоны с threat_level > 0.9 увеличивают bounds на 10% (опционально, через `enable_zone_expansion`);
- **Priority-based reallocation**: при significant threat increase (> 0.2 за один update), задача получает `priority = 10` и триггерит `release_task` + `reallocate`.

### 4. Улучшить metrics

**Файлы:** `crates/swarm-metrics/src/metrics.rs`, `crates/swarm-sim/src/runner.rs`

Добавить в `RunMetrics`:
```rust
#[serde(default)]
pub high_priority_zones_mapped: u64,
#[serde(default)]
pub time_to_map_first_high_risk: Option<u64>,
#[serde(default)]
pub threat_level_over_time: Vec<f64>,
#[serde(default)]
pub zone_observations: u64,
```

High-priority threshold: `priority >= 5` или `threat_level >= 0.7` (конфигурируется через `RunConfig`).

### 5. Интегрировать replay

**Файл:** `crates/swarm-replay/src/replay.rs`

- `ReplaySummary`: добавить `zones_mapped: u64`, `hazard_updates: u64`, `observations: u64`;
- `summarize()`: обрабатывать `AgentObservation`, `HazardMapUpdated`;
- ASCII overlay: опционально, добавить hazard indicator в replay snapshot.

### 6. Улучшить WildfireAdapter

**Файл:** `crates/swarm-types/src/adapter.rs`

```rust
fn score(&self, agent: &AllocationAgent, task: &Task) -> f64 {
    let distance = self.route_cost(agent.pose, task);
    let battery_factor = agent.battery / 100.0;
    let threat_urgency = if task.priority >= 8 { 200.0 } else { 0.0 };
    1000.0 - distance + battery_factor * 50.0 + f64::from(task.priority) * 20.0 + threat_urgency
}
```

### 7. Обновить report export

**Файл:** `crates/swarm-sim/src/report_export.rs`

Добавить wildfire metrics в:
- JSON export;
- CSV export;
- Markdown table.

### 8. Обновить README

**Файл:** `README.md`

- Добавить раздел "Wildfire / Flood v2";
- Обновить scenario catalog (новые JSON файлы);
- Обновить Current Status (M38: ✅ Stable);
- Добавить dynamic behavior explanation.

## Testing strategy

### Категория 1 — без рефакторинга

- **scenario load test**: `wildfire.small-static.json` и `wildfire.medium-dynamic.json` загружаются через `load_scenario_suite`;
- **wildfire smoke test**: `strategy_comparison --smoke --mission wildfire` проходит;
- **success/completion consistency**: `support_matrix_wildfire_medium_dynamic_completion_consistency` проверяет success на основе mapped-ratio;
- **replay event roundtrip**: `AgentObservation` и `HazardMapUpdated` сериализуются/десериализуются корректно.

### Категория 2 — лёгкий рефакторинг

- **Hazard fixtures**:
  ```rust
  fn high_threat_zone() -> HazardZone
  fn medium_threat_zone() -> HazardZone
  fn low_threat_zone() -> HazardZone
  ```
- **Benchmark output parsers**: assert на `high_priority_zones_mapped` в JSON/CSV;
- **Mission outcome assertions**: `assert_wildfire_success(scenario, expected_mapped, expected_high_priority_mapped)`.

### Категория 3 — тяжёлый рефакторинг

- **Dynamic hazard property test**: на 10 seeds проверяем, что dynamic threat увеличивает `final_avg_threat_level` относительно static;
- **Multi-seed wildfire benchmark**: `--quick --mission wildfire` с проверкой метрик на стабильность;
- **Spatial spread test**: две смежные зоны, одна high-threat — проверяем, что соседняя получает +0.05;
- **Visualization overlay test**: replay summary содержит корректное количество `zones_mapped` и `hazard_updates`.

## Risks and tradeoffs

| Риск | Вероятность | Влияние | Митигация |
|---|---|---|---|
| Spatial spread ломает existing small-static benchmark | Низкая | Среднее | SmallStatic остаётся `enable_dynamic_threat: false`; spread применяется только при `enable_dynamic_threat: true` |
| Zone expansion изменяет Aabb и ломает pose checks | Средняя | Среднее | Expansion опционально (`enable_zone_expansion`), default = false |
| Новые metrics ломают старые JSON десериализацию | Низкая | Низкое | `#[serde(default)]` на все новые поля |
| Replay summary изменяет формат | Низкая | Низкое | Новые поля — `#[serde(default)]` |

## Open questions

1. **Нужен ли flood как отдельная миссия?**
   - Вариант A: flood = wildfire с другими zone names (рекомендуется);
   - Вариант B: отдельный `TaskKind::FloodZone` и `FloodState` (overkill для текущего scope).

2. **Какой threshold для high-priority?**
   - Вариант A: `priority >= 5` (рекомендуется — совпадает с SmallStatic zone-a);
   - Вариант B: `threat_level >= 0.7`;
   - Вариант C: конфигурируется через `RunConfig`.

3. **Нужна ли zone expansion?**
   - Рекомендуется сделать опциональным и default = false для начала.

4. **Как интегрировать wind influence?**
   - Вариант A: использовать существующее `run_config.wind` (рекомендуется);
   - Вариант B: добавить отдельное `fire_wind_direction` в `WildfireState`.

## Что могло сломаться

- **Поведение**: Dynamic threat теперь включает spatial spread и wind influence. MediumDynamic профиль получает более сложное поведение; baseline может потребовать обновления.
- **API/контракты**: `RunMetrics` получает 4 новых поля. Старые JSON десериализуются (serde default).
- **API/контракты**: `ReplaySummary` получает 3 новых поля. Старые JSON десериализуются.
- **Данные**: Новые scenario JSON файлы добавляются в `scenarios/`. Старые не затронуты.
- **Интеграции**: Regression suites `wildfire_small_static_greedy` и `wildfire_medium_dynamic_greedy` используют `task_completion_rate`. Если dynamic behavior изменится, thresholds могут потребовать recalibration.
- **Производительность**: Spatial spread добавляет O(zones²) проверку на каждом update tick. Для < 10 зон — negligible.

## Критерии готовности

- [ ] `cargo test --workspace` проходит (включая новые wildfire tests).
- [ ] `cargo clippy --all-targets -- -D warnings` проходит.
- [ ] `cargo fmt --all` не меняет код.
- [ ] Созданы `wildfire.small-static.json` и `wildfire.medium-dynamic.json`.
- [ ] Wildfire metrics экспортированы в JSON/CSV/table.
- [ ] Replay summary обрабатывает wildfire события.
- [ ] README обновлён (Wildfire v2 section).
- [ ] Локальный commit сделан.
