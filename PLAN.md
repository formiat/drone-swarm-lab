# M37 — Realism Scenario Pack

## Context

M36 закрыт. Regression Harness v2 реализован:
- Откалиброваны пороги для всех suites;
- Добавлены wildfire, realism, inspection_perimeter_experimental suites;
- Baseline обновлён;
- `/tmp` заменён на tempfile.

Следующий шаг по линейному плану DRONE_A.14.linear.md — M37 Realism Scenario Pack.

## Investigation context

`INVESTIGATION.md` отсутствует. Анализ кода показал:

### 1. Realism preset — единый "default", нет профилей

**Файлы:** `crates/swarm-examples/src/bin/strategy_comparison.rs:312-330`, `crates/swarm-examples/src/bin/regression_runner.rs:21-39`

```rust
fn apply_realism_preset(scenario: Scenario, run_config: RunConfig) -> (Scenario, RunConfig) {
    run_config.pose_noise_m = 0.5;
    run_config.wind = Some((0.1, 0.1, 0.0));
    run_config.comms_jitter_ticks = 1;
    let battery = BatteryModel {
        hover_drain_per_tick: 0.01,
        climb_drain_per_meter: 0.05,
        cruise_drain_per_meter: 0.02,
        reserve_fraction: 0.1,
    };
    // ... apply to agents
}
```

Проблемы:
- Функция дублируется в двух бинарниках;
- Только один "default" профиль — нет light/medium/heavy;
- Параметры захардкожены, нет конфигурации.

### 2. Нет dedicated scenario JSON файлов для realism

**Файл:** `scenarios/`

Существующие сценарии:
- `coverage.ideal.json`, `coverage.safety.json`
- `sar.ideal.json`, `sar.noisy.json`, `sar.uncertain.json`
- `inspection.linear.json`, `inspection.perimeter.json`, `inspection.random.json`
- `cbba_stress.json`, `emergency-mesh.ideal.json`, `sitl.waypoints.json`

Нет сценариев с предустановленными realism-параметрами (wind, pose_noise, battery_model).

### 3. Manifest не содержит battery model metadata

**Файл:** `crates/swarm-sim/src/report_export.rs:280-300`

`BenchmarkManifest` имеет:
- `realism_profile: Option<String>`
- `wind_enabled: bool`
- `pose_noise_m: f64`
- `comms_jitter_ticks: u64`

Нет:
- `battery_model: Option<BatteryModel>`
- `hover_drain_per_tick`
- `climb_drain_per_meter`
- `cruise_drain_per_meter`
- `reserve_fraction`

### 4. Runner применяет wind и pose noise, но не логирует параметры

**Файл:** `crates/swarm-sim/src/runner.rs:651-670`

Wind drift и pose noise применяются через `apply_environment_effects`, но параметры не сохраняются в `RunMetrics` — только в `RunConfig`.

### 5. Раздел Realism в README отсутствует

В README нет детального описания:
- что моделируется реализмом;
- какие параметры используются;
- как запускать realism-сценарии;
- как интерпретировать результаты.

## Affected components

| Компонент | Путь | Что меняется |
|---|---|---|
| Realism profiles | `crates/swarm-examples/src/bin/strategy_comparison.rs` | `RealismProfile` enum (light/medium/heavy) |
| Realism preset | `crates/swarm-examples/src/bin/regression_runner.rs` | Использовать `RealismProfile` вместо хардкода |
| Scenario JSONs | `scenarios/` | Новые файлы `*.realism.json` |
| Manifest | `crates/swarm-sim/src/report_export.rs` | Battery model metadata |
| Runner metrics | `crates/swarm-sim/src/runner.rs` | Log realism params в `RunMetrics` |
| Scenario loader | `crates/swarm-sim/src/dsl.rs` | Поддержка `realism_profile` в JSON |
| README | `README.md` | Realism section, scenario catalog |

## Implementation steps

### 1. Создать `RealismProfile` enum и конфигурацию

**Файлы:** `crates/swarm-examples/src/bin/strategy_comparison.rs`, `crates/swarm-examples/src/bin/regression_runner.rs`

**Текущий код:** дублирующаяся `apply_realism_preset` с захардкоженными значениями.

**Исправление:**
```rust
#[derive(Clone, Debug, PartialEq)]
pub enum RealismProfile {
    Light,
    Medium,
    Heavy,
}

impl RealismProfile {
    pub fn params(&self) -> RealismParams {
        match self {
            Self::Light => RealismParams {
                pose_noise_m: 0.2,
                wind: Some((0.05, 0.05, 0.0)),
                comms_jitter_ticks: 1,
                battery: BatteryModel {
                    hover_drain_per_tick: 0.005,
                    climb_drain_per_meter: 0.03,
                    cruise_drain_per_meter: 0.01,
                    reserve_fraction: 0.1,
                },
            },
            Self::Medium => RealismParams {
                pose_noise_m: 0.5,
                wind: Some((0.1, 0.1, 0.0)),
                comms_jitter_ticks: 1,
                battery: BatteryModel {
                    hover_drain_per_tick: 0.01,
                    climb_drain_per_meter: 0.05,
                    cruise_drain_per_meter: 0.02,
                    reserve_fraction: 0.15,
                },
            },
            Self::Heavy => RealismParams {
                pose_noise_m: 1.0,
                wind: Some((0.2, 0.2, 0.0)),
                comms_jitter_ticks: 2,
                battery: BatteryModel {
                    hover_drain_per_tick: 0.02,
                    climb_drain_per_meter: 0.08,
                    cruise_drain_per_meter: 0.03,
                    reserve_fraction: 0.2,
                },
            },
        }
    }
}

pub struct RealismParams {
    pub pose_noise_m: f64,
    pub wind: Option<(f64, f64, f64)>,
    pub comms_jitter_ticks: u64,
    pub battery: BatteryModel,
}
```

- Убрать дублирование `apply_realism_preset` между `strategy_comparison.rs` и `regression_runner.rs`;
- Создать shared module `crates/swarm-examples/src/realism.rs`.

### 2. Добавить scenario JSON файлы с realism

**Файлы:** `scenarios/coverage.realism.json`, `scenarios/sar.realism.json`, `scenarios/inspection.realism.json`, `scenarios/wildfire.realism.json`

**Структура JSON:**
```json
{
  "name": "coverage_realism",
  "schema_version": "0.1",
  "realism_profile": "medium",
  "wind": [0.1, 0.1, 0.0],
  "pose_noise_m": 0.5,
  "comms_jitter_ticks": 1,
  "battery_model": {
    "hover_drain_per_tick": 0.01,
    "climb_drain_per_meter": 0.05,
    "cruise_drain_per_meter": 0.02,
    "reserve_fraction": 0.15
  },
  "agents": [...],
  "tasks": [...]
}
```

### 3. Добавить battery model metadata в manifest

**Файл:** `crates/swarm-sim/src/report_export.rs`

Добавить в `BenchmarkManifest`:
```rust
#[serde(default)]
pub battery_model: Option<BatteryModel>,
```

Обновить `write_benchmark_pack`:
```rust
if let Some(ref battery) = battery_model {
    manifest.battery_model = Some(battery.clone());
}
```

### 4. Добавить realism params в `RunMetrics`

**Файл:** `crates/swarm-sim/src/runner.rs`

Добавить поля в `RunConfig` (уже есть) и `RunMetrics`:
```rust
#[serde(default)]
pub realism_profile: Option<String>,
#[serde(default)]
pub wind: Option<(f64, f64, f64)>,
```

### 5. Обновить DSL loader для realism scenario JSON

**Файл:** `crates/swarm-sim/src/dsl.rs`

Добавить парсинг `realism_profile` из scenario JSON:
```rust
if let Some(profile) = scenario_json.get("realism_profile") {
    run_config.pose_noise_m = ...; // по профилю
}
```

### 6. Сравнение baseline vs realism

**Файл:** `crates/swarm-examples/src/bin/strategy_comparison.rs`

Добавить `--compare-realism` флаг:
```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --smoke --mission coverage --compare-realism --output-dir results/coverage_realism/
```

### 7. Обновить README

**Файл:** `README.md`

Добавить:
- Раздел "Realism Profiles" (light/medium/heavy params);
- Раздел "Realism Scenarios" (какие JSON файлы доступны);
- Команда запуска: `--realism` и `--realism-profile medium`;
- Объяснение expected impact на метрики.

## Testing strategy

### Категория 1 — без рефакторинга

- **scenario load test**: `scenarios/coverage.realism.json` загружается через `load_scenario_suite`;
- **battery model test**: `Agent` с `battery_model` deserializes корректно;
- **sensor altitude test**: `scan_cell` с `pose.z > 0` корректно применяет altitude penalty;
- **no-fly time window test**: `SafetyConfig` с time-gated zones работает корректно.

### Категория 2 — лёгкий рефакторинг

- **Realism fixture builders**:
  ```rust
  fn realism_light_fixture() -> RealismParams
  fn realism_medium_fixture() -> RealismParams
  fn realism_heavy_fixture() -> RealismParams
  ```
- **Realism manifest assertions**:
  ```rust
  assert!(manifest.realism_profile.is_some());
  assert!(manifest.battery_model.is_some());
  ```
- **Scenario builder helpers**:
  ```rust
  fn build_realism_scenario(profile: RealismProfile) -> (Scenario, RunConfig)
  ```

### Категория 3 — тяжёлый рефакторинг

- **Stochastic realism regression**: property test на 10 seeds, проверяющий что realism ухудшает метрики относительно baseline (success_rate, coverage, battery margin);
- **Full old vs realism comparison**: скрипт, который запускает `--smoke --mission coverage` с и без `--realism` и сравнивает метрики;
- **SITL-aligned trajectory tests**: проверка, что wind drift + pose noise создаёт траекторию, совместимую с SITL mock agent.

## Risks and tradeoffs

| Риск | Вероятность | Влияние | Митигация |
|---|---|---|---|
| Realism params слишком агрессивны/слишком мягкие | Средняя | Среднее | Начать с conservative values; calibrate через regression |
| JSON schema change ломает старые сценарии | Низкая | Среднее | Новые поля — `#[serde(default)]`; старые JSON загружаются |
| Battery model metadata увеличивает manifest size | Низкая | Низкое | BatteryModel — маленькая структура (~4 floats) |
| Дублирование `apply_realism_preset` не устранено полностью | Средняя | Низкое | Shared module `swarm-examples/src/realism.rs` |

## Open questions

1. **Какие значения для light/medium/heavy?**
   - Вариант A: эвристические (как в плане);
   - Вариант B: основанные на реальных flight data;
   - Рекомендуется A для начала, B для будущих milestones.

2. **Нужен ли `RealismProfile` в `RegressionSuite`?**
   - Уже есть поле `realism: bool`;
   - Можно заменить на `realism_profile: Option<RealismProfile>`;
   - Рекомендуется добавить `realism_profile: Option<String>` для гибкости.

3. **Как сравнивать baseline vs realism?**
   - Вариант A: один `--compare-realism` флаг;
   - Вариант B: два отдельных прогона + post-hoc diff script;
   - Рекомендуется B для простоты.

4. **Нужны ли realism-сценарии для emergency-mesh и cbba_stress?**
   - Рекомендуется покрыть coverage, SAR, inspection, wildfire;
   - Emergency-mesh и CBBA stress — optional.

## Что могло сломаться

- **Поведение**: Realism preset теперь выбирается из профиля (light/medium/heavy). `--realism` без указания профиля должен использовать Medium (backward compat).
- **API/контракты**: `BenchmarkManifest` получает `battery_model`. Старые JSON десериализуются (serde default).
- **Данные**: Новые scenario JSON файлы добавляются в `scenarios/`. Старые файлы не затронуты.
- **Интеграции**: Regression suite `realism_coverage_smoke` использует `realism: true`. Если профили изменятся, baseline потребует обновления.
- **Производительность**: Дополнительные поля в manifest/metrics — negligible overhead.

## Критерии готовности

- [ ] `cargo test --workspace` проходит (включая новые realism tests).
- [ ] `cargo clippy --all-targets -- -D warnings` проходит.
- [ ] `cargo fmt --all` не меняет код.
- [ ] Созданы 4+ realism scenario JSON файлы.
- [ ] `BenchmarkManifest` содержит battery model metadata.
- [ ] README обновлён (Realism section).
- [ ] Локальный commit сделан.
