# PLAN: M31 — Simulation Realism Foundation

## Контекст

M30 завершён: введена миссия Wildfire / Flood Mapping с `TaskKind::MappingZone`,
`WildfireState`, hazard zones, dynamic threat levels и `AgentObservation` replay events.

Цель M31 — добавить mission-level realism без ухода в полный физический движок.
Реализм касается 3D позиционирования, улучшенной модели батареи, продвинутого
сенсора, окружающего шума и временных no-fly зон.

## Investigation Context

`INVESTIGATION.md` отсутствует. Ниже — ключевые наблюдения из инспекции кода.

**Текущая позиция** (`swarm-types/src/pose.rs`):

- `Pose { x: f64, y: f64 }` — чисто 2D.
- `Aabb` добавлен в M30, также 2D.
- Расстояние `distance_to` — евклидово в плоскости XY.

**Текущая модель батареи** (`swarm-runtime/src/membership.rs::apply_movement`):

```rust
let drain = distance_moved * entry.battery_drain_rate;
entry.battery = (entry.battery - drain).max(0.0);
```

- Единственный фактор расхода — горизонтальное перемещение.
- Нет hover drain, нет climb drain, нет reserve fraction.
- `Agent` хранит `battery_drain_rate: f64` как legacy поле.

**Текущий сенсор** (`swarm-types/src/grid.rs::SensorModel`):

- `scout_pod`, `thermal_pod`, `relay_pod` — base probability of detection по роли.
- `detection_probability`, `false_positive_rate` — v2 Bayesian параметры.
- Нет `detection_range_m`, `field_of_view_deg`, `altitude_factor`.
- BeliefMap обновляется через `scan_cell` в `swarm-runtime/src/grid_state.rs`.

**Текущая безопасность** (`swarm-safety/src/lib.rs`):

- `NoFlyZone { bounds: Aabb }` — статичная зона, активна всегда.
- `SafetyConfig { geofence, no_fly_zones, separation }`.
- `check_agent` проверяет попадание в зону, но не учитывает tick.

**Текущее движение** (`swarm-runtime/src/membership.rs::apply_movement`):

- Агент движется к задаче по прямой в XY.
- Скорость ограничена `entry.speed * dt`.
- Нет wind drift, нет pose noise.

**Текущая коммуникация** (`swarm-comms/src/in_mem.rs`):

- `latency_ticks` фиксирована, `packet_loss_rate` фиксирован.
- Нет jitter или вариации задержки.

**Текущий DSL** (`swarm-sim/src/dsl.rs`):

- `schema_version: "0.1"`.
- `ScenarioSuiteEntry` содержит `scenario: Scenario` и `run_config: RunConfig`.
- Все новые поля должны иметь `#[serde(default)]`.

## Affected Components

| Компонент | Файл | Тип изменения |
|---|---|---|
| `swarm-types` | `src/pose.rs` | добавить `z: f64` в `Pose` с `#[serde(default)]` |
| `swarm-types` | `src/agent.rs` | добавить `battery_model: Option<BatteryModel>` |
| `swarm-types` | `src/grid.rs` | расширить `SensorModel` v3 полями |
| `swarm-safety` | `src/lib.rs` | расширить `NoFlyZone` временными полями |
| `swarm-runtime` | `src/membership.rs` | обновить `apply_movement` для battery v2 и 3D pose |
| `swarm-runtime` | `src/grid_state.rs` | обновить `scan_cell` для sensor v3 (altitude_factor) |
| `swarm-comms` | `src/in_mem.rs` | добавить `comms_jitter_ticks` в `NetworkConfig` |
| `swarm-sim` | `src/runner.rs` | передать wind/pose_noise в runner; учесть active no-fly periods |
| `swarm-sim` | `src/dsl.rs` | `validate_scenario_suite` — проверить новые поля |
| `swarm-examples` | `src/bin/strategy_comparison.rs` | добавить `--realism` preset flag |
| `docs/` | `SCENARIO_DSL.md` | migration guide для новых полей |
| `README.md` | — | актуализация статуса M31 |

## Implementation Steps

### Шаг 1: 3D Pose

**Файл:** `crates/swarm-types/src/pose.rs`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub z: f64,
}

impl Pose {
    pub fn distance_to(&self, other: &Pose) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Horizontal (XY) distance — used for legacy 2D calculations.
    pub fn distance_to_2d(&self, other: &Pose) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}
```

**Backward compat:** старый JSON без `z` десериализуется с `z = 0.0`.

### Шаг 2: Battery Model v2

**Файл:** `crates/swarm-types/src/agent.rs`

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BatteryModel {
    #[serde(default)]
    pub hover_drain_per_tick: f64,
    #[serde(default)]
    pub climb_drain_per_meter: f64,
    #[serde(default)]
    pub cruise_drain_per_meter: f64,
    #[serde(default)]
    pub reserve_fraction: f64,
}

// In Agent:
pub battery_model: Option<BatteryModel>,
```

**Файл:** `crates/swarm-runtime/src/membership.rs`

В `apply_movement` заменить единый drain на:

```rust
if let Some(ref bm) = entry.battery_model {
    let horizontal_dist = distance_moved;
    let vertical_dist = (target_pose.z - entry.pose.z).abs();
    let hover_ticks = 1.0; // simplistic: 1 tick per call
    let drain = horizontal_dist * bm.cruise_drain_per_meter
              + vertical_dist * bm.climb_drain_per_meter
              + hover_ticks * bm.hover_drain_per_tick;
    entry.battery = (entry.battery - drain).max(0.0);
} else {
    // Legacy v1 path
    let drain = distance_moved * entry.battery_drain_rate;
    entry.battery = (entry.battery - drain).max(0.0);
}
```

**Важно:** обновить `entry.pose.z` при движении:
```rust
entry.pose.z += (target_pose.z - entry.pose.z) * ratio;
```

### Шаг 3: Sensor Model v3

**Файл:** `crates/swarm-types/src/grid.rs`

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SensorModel {
    pub scout_pod: f64,
    pub thermal_pod: f64,
    pub relay_pod: f64,
    #[serde(default = "default_detection_probability")]
    pub detection_probability: f64,
    #[serde(default = "default_false_positive_rate")]
    pub false_positive_rate: f64,
    // v0.31 Sensor v3
    #[serde(default)]
    pub detection_range_m: f64,
    #[serde(default)]
    pub field_of_view_deg: f64,
    #[serde(default)]
    pub altitude_factor: f64,
}
```

**Файл:** `crates/swarm-runtime/src/grid_state.rs`

В `scan_cell` (или где вычисляется `detected`):

```rust
let altitude_penalty = if sensor.altitude_factor > 0.0 && agent_pose.z > 0.0 {
    (1.0 - sensor.altitude_factor * agent_pose.z).max(0.0)
} else {
    1.0
};
let effective_pod = base_pod * altitude_penalty;
let detected = rng.gen::<f64>() < effective_pod;
```

### Шаг 4: Environment Noise

**Файл:** `crates/swarm-sim/src/runner.rs`

Добавить в `RunConfig`:

```rust
#[serde(default)]
pub wind: Option<(f64, f64, f64)>, // (vx, vy, vz) drift per tick
#[serde(default)]
pub pose_noise_m: f64,
```

В `apply_movement` (или в runner цикл после `apply_movement`):

```rust
// Wind drift
if let Some((wx, wy, wz)) = config.wind {
    entry.pose.x += wx * dt;
    entry.pose.y += wy * dt;
    entry.pose.z += wz * dt;
}

// Pose noise
if config.pose_noise_m > 0.0 {
    let noise_x = rng.gen::<f64>() * config.pose_noise_m - config.pose_noise_m / 2.0;
    let noise_y = rng.gen::<f64>() * config.pose_noise_m - config.pose_noise_m / 2.0;
    let noise_z = rng.gen::<f64>() * config.pose_noise_m - config.pose_noise_m / 2.0;
    entry.pose.x += noise_x;
    entry.pose.y += noise_y;
    entry.pose.z += noise_z;
}
```

**Файл:** `crates/swarm-comms/src/in_mem.rs`

Добавить в `NetworkConfig`:
```rust
#[serde(default)]
pub comms_jitter_ticks: u64,
```

В логике доставки сообщений вместо фиксированной `latency_ticks`:
```rust
let jitter = if config.comms_jitter_ticks > 0 {
    rng.gen::<u64>() % (config.comms_jitter_ticks * 2 + 1)
} else {
    0
};
let effective_latency = config.latency_ticks + jitter as i64; // clamp to >= 0
```

### Шаг 5: Time-varying No-Fly Zones

**Файл:** `crates/swarm-safety/src/lib.rs`

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NoFlyZone {
    pub bounds: Aabb,
    #[serde(default)]
    pub active_from_tick: Option<u64>,
    #[serde(default)]
    pub active_until_tick: Option<u64>,
}

impl NoFlyZone {
    pub fn is_active_at(&self, tick: u64) -> bool {
        let after_start = self.active_from_tick.map_or(true, |t| tick >= t);
        let before_end = self.active_until_tick.map_or(true, |t| tick <= t);
        after_start && before_end
    }
}
```

**Файл:** `crates/swarm-safety/src/lib.rs` (check_agent)

```rust
for nofly in &config.no_fly_zones {
    if nofly.is_active_at(current_tick) && nofly.bounds.contains(&agent.pose) {
        violations.push(SafetyViolation { ... });
    }
}
```

**Важно:** `current_tick` должен передаваться в `check_agent`. Это breaking change
для сигнатуры функции. Альтернатива: добавить `check_agent_at_tick`.

### Шаг 6: Backward Compat & Validation

**Файл:** `crates/swarm-sim/src/dsl.rs`

В `validate_scenario_suite` добавить проверки:
- Если `pose.z` отсутствует — warning (не error, т.к. `serde(default)`).
- Если `battery_model` задан — проверить, что `hover_drain_per_tick >= 0`, etc.
- Если `sensor.detection_range_m < 0` — error.
- Если `no_fly_zone.active_from_tick > active_until_tick` — error.

**Файл:** `docs/SCENARIO_DSL.md`

Добавить раздел "M31 Migration Guide":
- Добавление `z` к `pose` — опционально, default 0.0.
- Добавление `battery_model` — опционально, legacy `battery_drain_rate` работает.
- Добавление полей к `SensorModel` — опционально, defaults 0.0.
- Добавление `active_from_tick` / `active_until_tick` к `NoFlyZone` — опционально.

### Шаг 7: CLI `--realism` preset

**Файл:** `crates/swarm-examples/src/bin/strategy_comparison.rs`

Добавить флаг `--realism`:
```bash
cargo run -p swarm-examples --bin strategy_comparison -- --smoke --mission coverage --realism
```

При `--realism` preset активирует:
- `pose_noise_m = 0.5`
- `wind = Some((0.1, 0.1, 0.0))`
- `comms_jitter_ticks = 1`
- `BatteryModel` с hover_drain = 0.01, climb_drain = 0.05, cruise_drain = 0.02

### Шаг 8: Актуализация README

- Добавить M31 в Current Status.
- Добавить M31 в Milestones Overview.
- Упомянуть `--realism` preset в Quick Start.

## Testing Strategy

### Категория 1 — Без рефакторинга (unit + integration)

**Unit: Backward compat JSON loading**
- Файл: `crates/swarm-types/src/pose.rs`
- Десериализация старого JSON без `z` → `z == 0.0`.
- Десериализация нового JSON с `z` → корректное значение.

**Unit: Battery v2 hover drain**
- Файл: `crates/swarm-runtime/src/membership.rs`
- `apply_movement` с `BatteryModel { hover_drain_per_tick: 1.0, ... }`:
  агент без задач теряет батарею каждый tick.
- Legacy path без `battery_model` — поведение идентично v1.

**Unit: Battery v2 climb drain**
- Агент движется от `z=0` к `z=10` — drain включает `climb_drain_per_meter * 10`.

**Unit: Sensor v3 altitude penalty**
- Файл: `crates/swarm-runtime/src/grid_state.rs`
- `scan_cell` с `altitude_factor = 0.1`, `z = 5.0`:
  `effective_pod = base_pod * 0.5` (50% penalty).
- `altitude_factor = 0.0` — нет penalty.

**Unit: Time-varying no-fly zone**
- Файл: `crates/swarm-safety/src/lib.rs`
- `NoFlyZone { active_from_tick: Some(10), active_until_tick: Some(20) }`:
  - tick 5: `is_active_at` → false, нет violation.
  - tick 15: `is_active_at` → true, violation.
  - tick 25: `is_active_at` → false, нет violation.

**Integration: All old scenarios load without errors**
- Файл: `crates/swarm-sim/tests/scenario_catalog.rs`
- Загрузить все `scenarios/*.json` после изменений — assert no parse errors.

### Категория 2 — Лёгкий рефакторинг (integration)

**Integration: Wind drift**
- Файл: `crates/swarm-sim/src/runner.rs`
- Запуск с `wind = (0.5, 0.0, 0.0)` — финальная позиция агента смещена по X.

**Integration: Pose noise**
- Запуск с `pose_noise_m = 1.0` — позиция агента отличается от идеальной.

**Integration: Comms jitter**
- Файл: `crates/swarm-comms/tests/` (или integration через runner)
- `comms_jitter_ticks = 2` — latency сообщений варьируется.

**Integration: `--realism` preset**
- Запуск `strategy_comparison --smoke --mission coverage --realism`:
  - success_rate < 1.0 (realism делает миссию сложнее).
  - avg_battery_margin_min < 100.0.

### Категория 3 — Тяжёлый рефакторинг

**Proptest: Battery drain invariant**
- Файл: `crates/swarm-runtime/tests/battery_proptest.rs`
- Для случайных `hover_drain`, `climb_drain`, `cruise_drain`, `speed`, `distance`:
  - `drain >= 0`.
  - `drain <= 100` (battery не уходит ниже 0 за 1 шаг).
  - После N шагов: `battery == 100 - sum(drain_i)`.

**Proptest: Sensor v3 altitude factor**
- Для случайных `altitude_factor ∈ [0, 1]`, `z ∈ [0, 100]`:
  - `effective_pod <= base_pod`.
  - `effective_pod >= 0`.

## Risks and Tradeoffs

1. **Pose z = 0.0 default может сломать distance_to** — старые сценарии без `z`
   получат `z = 0.0`, а новые сценарии с `z > 0` будут иметь 3D distance.
   Mitigation: `distance_to_2d` для legacy horizontal-only вычислений;
   `distance_to` становится полноценным 3D евклидовым.

2. **BatteryModel заменяет battery_drain_rate** — если оба заданы, нужна чёткая
   приоритизация (v2 имеет приоритет). Mitigation: `if battery_model.is_some() { v2 } else { v1 }`.

3. **SafetyConfig signature change** — добавление `current_tick` в `check_agent`
   ломает все callers. Mitigation: добавить `check_agent_at_tick` с новой сигнатурой,
   оставить `check_agent` как deprecated wrapper (всегда active).

4. **Wind + pose noise делают тесты недетерминированными** — если `pose_noise` включён
   в default сценарии, regression baseline drift гарантирован.
   Mitigation: `--realism` preset опционален; default сценарии без noise.

5. **Comms jitter влияет на CBBA convergence** — variable latency может сломать
   deterministic convergence tests.
   Mitigation: jitter default = 0; включается только через `--realism`.

## Open Questions

1. **Нужен ли полноценный 3D Aabb (Aabb3) для NoFlyZone?**
   Нет — M31 ограничен `z` в Pose, но зоны остаются 2D (XY) с неограниченной высотой.
   3D зоны — M33+.

2. **Как моделировать wind gusts (порывы)?**
   M31 использует постоянный wind vector. Гауссовские порывы — M32+.

3. **Нужен ли thermal column model (восходящие потоки)?**
   Нет — это domain-specific для sailplane/drone energy harvesting. M35+.

4. **Как часто обновлять baseline после M31?**
   `--realism` preset меняет метрики существенно. Baseline для realism — отдельный
   артефакт (`results/baseline_realism.json`). Standard baseline не трогаем.

5. **Поддержка GPS-denied navigation?**
   `pose_noise_m` моделирует GPS ошибку. Полный SLAM / visual odyssey — M34+.
