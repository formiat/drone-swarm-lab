# PLAN: Phase 2 — Unified Experiment Runner

## Context

Phase 1 реализовал True Distributed CBBA с message-driven consensus. Теперь есть 5 стратегий на 3 reference missions (Coverage, EmergencyMesh, SAR). Но `strategy_comparison` binary использует только Coverage сценарий. Нет способа запустить бенчмарк на EmergencyMesh или SAR без ручного изменения кода.

**Phase 2** делает `strategy_comparison` unified experiment runner: `--mission` флаг, 3 миссии + `all`, SAR profiles, README с публикуемыми числовыми таблицами.

**Источники контекста:** `docs/DRONE_A.5.md`, `docs/DRONE_B.5.md`. INVESTIGATION.md отсутствует.

**Текущее состояние (v0.10 Phase 1):**
- `strategy_comparison` binary с 5 стратегиями (greedy, auction, connectivity-aware, centralized, cbba)
- `BenchmarkHarness` с `run_quick()` (10 seeds) и `run_full()` (100 seeds, CI)
- `StandardProfiles` для Coverage сценария (ideal, low-loss, medium-loss, high-loss, etc.)
- `ComparisonReport` с markdown таблицей
- JSON/CSV export (`export_json`, `export_csv`)
- Replay logs через `--replay-log <dir>`
- `--full` флаг для 1000 seed benchmark
- CLI: `--json`, `--csv`, `--replay-log`, `--run-id-prefix`

**Критерий готовности:**
1. `--mission coverage|emergency-mesh|sar|all` флаг в `strategy_comparison`.
2. `--mission all` запускает бенчмарк на всех 3 миссиях, каждая со своими profiles.
3. SAR metrics (`time_to_find`, `coverage_over_time`, `probability_of_detection`) включены в JSON/CSV export.
4. `ComparisonReport` содержит столбец `mission` в каждой строке.
5. README содержит таблицу с реальными числами из бенчмарк-прогона (Coverage + EmergencyMesh + SAR, 5 стратегий).

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.5.md` и `docs/DRONE_B.5.md`:
- DRONE_A.5: Phase 2 — unified experiment runner с mission support, единый output, README с таблицами.
- DRONE_B.5: после CBBA и experiment инфраструктуры нужен unified benchmark для получения публикуемых результатов.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | Добавить `--mission` флаг; mission-based builder selection; SAR scenario builder |
| `crates/swarm-sim/src/benchmark.rs` | `ComparisonReport` добавить `mission: String` в key; `ScenarioBuilder` принимает `mission` параметр |
| `crates/swarm-scenarios/src/coverage.rs` | Без изменений (уже есть builder + StandardProfiles) |
| `crates/swarm-scenarios/src/emergency_mesh.rs` | Добавить `StandardProfiles` (как в coverage) |
| `crates/swarm-scenarios/src/sar_scenario.rs` | Добавить `StandardProfiles` и `SarProfile` enum для разных конфигураций SAR |
| `crates/swarm-scenarios/src/lib.rs` | Re-export новых profiles |
| `crates/swarm-metrics/src/metrics.rs` | Без изменений (SAR поля уже есть) |
| `README.md` | Таблица с реальными числами из бенчмарка |

---

## Implementation Steps

### Шаг 1 — SAR StandardProfiles

Файл: `crates/swarm-scenarios/src/sar_scenario.rs`

Добавить `SarProfile` enum с предопределёнными конфигурациями:

```rust
pub enum SarProfile {
    /// Small grid, 2 targets, PoD=1.0 (all targets found)
    Ideal,
    /// Medium grid, 3 targets, PoD=0.6 (probabilistic)
    Standard,
    /// Large grid, 5 targets, 10% packet loss
    Challenging,
    /// Small grid, battery-constrained agents
    BatteryConstrained,
}

pub struct SarProfileParams {
    pub grid_width: u32,
    pub grid_height: u32,
    pub cell_size: f64,
    pub target_count: u32,
    pub scout_count: u32,
    pub thermal_count: u32,
    pub relay_count: u32,
    pub scout_pod: f64,
    pub thermal_pod: f64,
    pub relay_pod: f64,
    pub packet_loss_rate: f64,
    pub enable_movement: bool,
    pub max_ticks: u64,
}

impl SarProfile {
    pub fn params(&self) -> SarProfileParams { ... }
}
```

### Шаг 2 — EmergencyMesh StandardProfiles

Файл: `crates/swarm-scenarios/src/emergency_mesh.rs`

Добавить profiles аналогично coverage (Ideal, LowLoss, MediumLoss, HighLoss, SingleFailure, PacketLoss10, etc.):

```rust
pub enum EmergencyMeshProfile {
    Ideal,
    LowLoss { packet_loss_rate: f64 },
    MediumLoss { packet_loss_rate: f64 },
    SingleFailure { failure_agent: &'static str },
}

impl EmergencyMeshProfile {
    pub fn params(&self) -> EmergencyMeshParams { ... }
}
```

### Шаг 3 — Mission enum и builder selection

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

```rust
#[derive(Clone)]
enum Mission {
    Coverage,
    EmergencyMesh,
    Sar,
}

fn parse_mission(arg: &str) -> Vec<Mission> {
    match arg {
        "coverage" => vec![Mission::Coverage],
        "emergency-mesh" => vec![Mission::EmergencyMesh],
        "sar" => vec![Mission::Sar],
        "all" => vec![Mission::Coverage, Mission::EmergencyMesh, Mission::Sar],
        _ => panic!("unknown mission: {arg}"),
    }
}

fn build_mission_scenario(
    mission: &Mission,
    seed: u64,
    profile: &str,
) -> (Scenario, RunConfig) {
    match mission {
        Mission::Coverage => build_coverage_scenario(&CoverageConfig::from_profile(seed, profile)),
        Mission::EmergencyMesh => build_emergency_mesh_scenario(&EmergencyMeshConfig::from_profile(seed, profile)),
        Mission::Sar => build_sar_scenario(&SarScenarioConfig::from_profile(seed, profile)),
    }
}
```

### Шаг 4 — ComparisonReport с mission столбцом

Файл: `crates/swarm-sim/src/benchmark.rs`

Изменить key с `(strategy, profile)` на `(mission, strategy, profile)`:

```rust
pub struct ComparisonReport {
    pub benchmark_run_id: String,
    pub mission_names: Vec<String>,
    pub strategy_names: Vec<String>,
    pub profile_names: Vec<String>,
    pub results: HashMap<(String, String, String), AggregateMetrics>,
}
```

Обновить `Display` impl: добавить колонку `mission` в таблицу.

### Шаг 5 — CLI --mission флаг

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

Добавить парсинг:
```rust
"--mission" => {
    i += 1;
    cli.missions = parse_mission(&args[i]);
}
```

В `CliArgs`:
```rust
struct CliArgs {
    full_mode: bool,
    missions: Vec<Mission>,
    json_path: Option<String>,
    csv_path: Option<String>,
    replay_log_dir: Option<String>,
    run_id_prefix: Option<String>,
}
```

По умолчанию: `missions = vec![Mission::Coverage]` (backward compat).

### Шаг 6 — SAR metrics в JSON/CSV export

Файл: `crates/swarm-sim/src/runner.rs` (или `benchmark.rs`)

`export_json` и `export_csv` уже сериализуют все поля `RunMetrics`. SAR поля (`time_to_find`, `coverage_over_time`, `probability_of_detection`) уже есть в `RunMetrics` через `#[serde(default)]`. Проверить, что они корректно экспортируются и добавить их в CSV columns если отсутствуют.

### Шаг 7 — Обновить README

Добавить таблицу с результатами бенчмарка:
```
| Mission | Strategy | Profile | Success | Detection | Coverage | ... |
|---------|----------|---------|---------|-----------|----------|-----|
| coverage | greedy | ideal | 1.000 | 1.77 | 1.000 | ... |
| sar | cbba | standard | 0.85 | - | 0.72 | ... |
```

Реальные числа из прогона `cargo run -p swarm-examples --bin strategy_comparison --mission all --json /tmp/results.json`.

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run -p swarm-examples --bin strategy_comparison --mission all --json /tmp/results.json
cargo run -p swarm-examples --bin strategy_comparison --mission sar --csv /tmp/sar.csv
```

---

## Testing Strategy

### Категория 1 — unit тесты

- `sar_profile_params_small_grid` — Ideal profile даёт маленький grid
- `sar_profile_params_battery_constrained` — BatteryConstrained profile
- `emergency_mesh_profile_params` — profiles coverage
- `parse_mission_all_returns_three_missions`
- `comparison_report_includes_mission_column`

### Категория 2 — integration

- `strategy_comparison_coverage_default` — `--mission coverage` (backward compat) работает
- `strategy_comparison_sar_runs` — `--mission sar` запускается без panic

### Категория 3 — тяжёлый (run manually)

- `cargo run --bin strategy_comparison --mission all --json results.json` — полный benchmark

---

## Risks and Tradeoffs

**1. SAR StandardProfiles не покрывают все комбинации**

SAR имеет много измерений (grid, targets, roles, PoD, battery). 4 профиля покрывают базовые сценарии. Расширение до полной матрицы — manual benchmark.

**2. ComparisonReport переделан на 3-part key**

Код `ComparisonReport::Display` и `export_json`/`export_csv` должны быть обновлены. Возможно breaking change для существующих скриптов, разбирающих JSON.

**3. `--mission` default = coverage**

Backward compat: существующие команды без `--mission` продолжают работать как раньше (coverage сценарий).

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| `ComparisonReport` новый key формат | JSON/CSV парсеры, ожидающие старый формат | `cargo test -p swarm-sim` |
| `SarProfile` builder вызывает movement | SAR без движения (enable_movement=false) → coverage не растёт | unit тест профилей |
| `--mission all` занимает много времени | CI timeout | `--full` флаг для 1000 seeds, `--mission all` для малых прогонов |
