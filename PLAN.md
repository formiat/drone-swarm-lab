# PLAN: M29 — Stress & Regression Harness

## Контекст

M28 завершён: введён `RoutePlanner` trait с 2-opt и battery-aware feasibility, новые метрики
в `RunMetrics`. Сейчас benchmark-платформа (`strategy_comparison`, `BenchmarkHarness`)
умеет запускать сравнения стратегий, но не умеет:

1. **Фиксировать baseline** — нет референсных чисел, с которыми сравнивать последующие прогоны.
2. **Проверять thresholds** — нет автоматической проверки "успешность > X%".
3. **Запускать regression suite** — нет единой команды, которая прогоняет все критичные
   сценарии и проверяет их здоровье.
4. **Собирать stress-профили** — нет параметрических прогонов (packet loss sweep,
   grid size sweep).

M29 превращает benchmark из аналитического инструмента в инженерный контроль качества.

## Investigation Context

`INVESTIGATION.md` отсутствует. Ниже — ключевые наблюдения из инспекции кода.

**Текущий benchmark harness** (`swarm-sim/src/benchmark.rs`):

```rust
pub struct BenchmarkHarness;
impl BenchmarkHarness {
    pub fn run_smoke(...)
    pub fn run_quick(...)
    pub fn run_full(...)
}
```

Уже поддерживает `smoke` (1 seed), `quick` (10 seeds), `full` (1000 seeds).
Возвращает `ComparisonReport` → `AggregateMetrics`.

**Текущие метрики** (`swarm-metrics/src/metrics.rs`):

- `success_rate`, `avg_task_completion_rate`
- `avg_edge_coverage_rate`, `avg_missed_edges`
- `avg_probability_of_detection`, `avg_belief_entropy_final`
- `convergence_ticks_p50`, `convergence_ticks_p95`
- `avg_safety_violations`
- `avg_route_length`, `avg_bundle_travel_distance`

Все поля доступны для threshold checking.

**Текущий CLI** (`swarm-examples/src/bin/strategy_comparison.rs`):

- `--smoke`, `--quick`, `--full`
- `--mission {coverage|sar|inspection|emergency-mesh|all}`
- `--planner {nn|two-opt|battery-aware}` (M28)
- `--output-dir`, `--report`, `--json`, `--csv`

**Baseline хранение**: нет. Нужно добавить:
- `results/baseline.json` — committed artifact с референсными числами.

## Affected Components

| Компонент | Файл | Тип изменения |
|---|---|---|
| `swarm-sim` | `src/regression.rs` (новый) | `RegressionSuite`, `ThresholdChecker`, `Baseline` |
| `swarm-sim` | `src/lib.rs` | re-export regression модулей |
| `swarm-examples` | `src/bin/strategy_comparison.rs` | флаг `--regression`, `--compare-baseline` |
| `swarm-examples` | `src/bin/regression_runner.rs` (новый) | standalone regression binary |
| `results/` | `baseline.json` | checked-in baseline artifact |
| `README.md` | — | документация по regression и baseline |

## Implementation Steps

### Шаг 1: `RegressionSuite` и `Threshold`

**Файл:** `crates/swarm-sim/src/regression.rs`

```rust
use serde::{Deserialize, Serialize};
use swarm_metrics::AggregateMetrics;

/// A single threshold check against an aggregated metric.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Threshold {
    pub metric: String,       // e.g. "success_rate"
    pub min: Option<f64>,     // e.g. Some(0.7)
    pub max: Option<f64>,     // e.g. Some(0.5) for entropy
}

/// One suite = one mission + one profile + one strategy + thresholds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegressionSuite {
    pub name: String,
    pub mission: String,
    pub profile: String,
    pub strategy: String,
    pub thresholds: Vec<Threshold>,
    pub mode: SuiteMode, // Smoke or Quick
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SuiteMode {
    Smoke, // 1 seed, < 5s
    Quick, // 10 seeds, < 30s
}

/// Result of running one suite.
#[derive(Clone, Debug)]
pub struct SuiteResult {
    pub suite: RegressionSuite,
    pub metrics: AggregateMetrics,
    pub violations: Vec<ThresholdViolation>,
}

#[derive(Clone, Debug)]
pub struct ThresholdViolation {
    pub threshold: Threshold,
    pub actual: f64,
}
```

### Шаг 2: `ThresholdChecker`

```rust
pub struct ThresholdChecker;

impl ThresholdChecker {
    pub fn check(metrics: &AggregateMetrics, thresholds: &[Threshold]) -> Vec<ThresholdViolation> {
        let mut violations = Vec::new();
        for t in thresholds {
            let actual = extract_metric(metrics, &t.metric);
            if let Some(min) = t.min {
                if actual < min {
                    violations.push(ThresholdViolation { threshold: t.clone(), actual });
                }
            }
            if let Some(max) = t.max {
                if actual > max {
                    violations.push(ThresholdViolation { threshold: t.clone(), actual });
                }
            }
        }
        violations
    }
}
```

`extract_metric` — match по строке `metric` на поля `AggregateMetrics`:
- `"success_rate"` → `metrics.success_rate`
- `"edge_coverage_rate"` → `metrics.avg_edge_coverage_rate`
- `"belief_entropy_final"` → `metrics.avg_belief_entropy_final`
- и т.д.

### Шаг 3: Regression suites (hardcoded config)

**Файл:** `crates/swarm-sim/src/regression.rs` (в модуле `suites`)

```rust
pub fn default_suites() -> Vec<RegressionSuite> {
    vec![
        // SAR
        RegressionSuite {
            name: "sar_ideal_greedy".to_owned(),
            mission: "sar".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![
                Threshold { metric: "success_rate".to_owned(), min: Some(0.7), max: None },
                Threshold { metric: "probability_of_detection".to_owned(), min: Some(0.5), max: None },
                Threshold { metric: "belief_entropy_final".to_owned(), min: None, max: Some(0.5) },
            ],
            mode: SuiteMode::Smoke,
        },
        RegressionSuite {
            name: "sar_standard_greedy".to_owned(),
            mission: "sar".to_owned(),
            profile: "standard".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![
                Threshold { metric: "success_rate".to_owned(), min: Some(0.6), max: None },
            ],
            mode: SuiteMode::Smoke,
        },
        // Inspection
        RegressionSuite {
            name: "inspection_linear_all".to_owned(),
            mission: "inspection".to_owned(),
            profile: "linear".to_owned(),
            strategy: "all".to_owned(), // check every strategy
            thresholds: vec![
                Threshold { metric: "edge_coverage_rate".to_owned(), min: Some(0.95), max: None },
                Threshold { metric: "success_rate".to_owned(), min: Some(0.9), max: None },
            ],
            mode: SuiteMode::Smoke,
        },
        RegressionSuite {
            name: "inspection_perimeter_all".to_owned(),
            mission: "inspection".to_owned(),
            profile: "perimeter".to_owned(),
            strategy: "all".to_owned(),
            thresholds: vec![
                Threshold { metric: "edge_coverage_rate".to_owned(), min: Some(0.7), max: None },
            ],
            mode: SuiteMode::Smoke,
        },
        // CBBA stress
        RegressionSuite {
            name: "cbba_stress_pl_0_0".to_owned(),
            mission: "coverage".to_owned(),
            profile: "ideal-no-failures".to_owned(),
            strategy: "cbba".to_owned(),
            thresholds: vec![
                Threshold { metric: "success_rate".to_owned(), min: Some(0.9), max: None },
                Threshold { metric: "convergence_ticks_p95".to_owned(), min: None, max: Some(15.0) },
            ],
            mode: SuiteMode::Quick,
        },
        RegressionSuite {
            name: "cbba_stress_pl_0_2".to_owned(),
            mission: "coverage".to_owned(),
            profile: "ideal-no-failures".to_owned(),
            strategy: "cbba".to_owned(),
            thresholds: vec![
                Threshold { metric: "success_rate".to_owned(), min: Some(0.8), max: None },
                Threshold { metric: "convergence_ticks_p95".to_owned(), min: None, max: Some(20.0) },
            ],
            mode: SuiteMode::Quick,
        },
        // Safety
        RegressionSuite {
            name: "safety_coverage".to_owned(),
            mission: "coverage".to_owned(),
            profile: "ideal-no-failures".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![
                Threshold { metric: "safety_violations".to_owned(), min: None, max: Some(0.0) },
            ],
            mode: SuiteMode::Smoke,
        },
        // Emergency mesh
        RegressionSuite {
            name: "emergency_mesh_ideal".to_owned(),
            mission: "emergency-mesh".to_owned(),
            profile: "ideal".to_owned(),
            strategy: "greedy".to_owned(),
            thresholds: vec![
                Threshold { metric: "success_rate".to_owned(), min: Some(0.8), max: None },
            ],
            mode: SuiteMode::Smoke,
        },
    ]
}
```

### Шаг 4: `Baseline` artifact

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Baseline {
    pub version: String,
    pub created_at: String, // ISO 8601
    pub commit: String,
    pub results: HashMap<String, AggregateMetrics>,
}

impl Baseline {
    pub fn from_suites(results: &[(RegressionSuite, AggregateMetrics)]) -> Self { ... }
    pub fn load(path: &str) -> Result<Self, std::io::Error> { ... }
    pub fn save(&self, path: &str) -> Result<(), std::io::Error> { ... }
    pub fn compare(&self, current: &AggregateMetrics, suite_name: &str) -> BaselineDelta { ... }
}

#[derive(Clone, Debug)]
pub struct BaselineDelta {
    pub suite_name: String,
    pub metric: String,
    pub baseline_value: f64,
    pub current_value: f64,
    pub change_pct: f64,
    pub status: DeltaStatus, // Improved, Degraded, Stable
}
```

### Шаг 5: `RegressionRunner`

**Файл:** `crates/swarm-sim/src/regression.rs` (impl block)

```rust
pub struct RegressionRunner;

impl RegressionRunner {
    pub fn run(
        suites: &[RegressionSuite],
        baseline: Option<&Baseline>,
    ) -> RegressionReport {
        // For each suite:
        // 1. Build scenario via existing scenario builders
        // 2. Run BenchmarkHarness::run_smoke or run_quick
        // 3. Extract AggregateMetrics for the specified strategy
        // 4. Check thresholds
        // 5. If baseline present, compute deltas
    }
}

#[derive(Clone, Debug)]
pub struct RegressionReport {
    pub suite_results: Vec<SuiteResult>,
    pub deltas: Vec<BaselineDelta>,
    pub overall_pass: bool,
}
```

### Шаг 6: CLI `--regression` и `--compare-baseline`

**Файл:** `crates/swarm-examples/src/bin/strategy_comparison.rs`

Добавить флаги:
- `--regression` — запускает `default_suites()`, проверяет thresholds, exit code = 0 если все pass, 1 если есть violations.
- `--compare-baseline results/baseline.json` — загружает baseline, вычисляет deltas.
- `--update-baseline path` — записывает текущий прогон как новый baseline.

Пример:
```bash
# CI regression check (< 2 minutes)
cargo run -p swarm-examples --bin strategy_comparison -- --regression

# Compare against baseline
cargo run -p swarm-examples --bin strategy_comparison -- --regression --compare-baseline results/baseline.json

# Update baseline after intentional improvement
cargo run -p swarm-examples --bin strategy_comparison -- --regression --update-baseline results/baseline.json
```

### Шаг 7: Baseline artifact

**Файл:** `results/baseline.json`

Создать через `--update-baseline`, закоммитить в репозиторий.

Формат:
```json
{
  "version": "1.0",
  "created_at": "2025-05-26T12:00:00Z",
  "commit": "76b39db",
  "results": {
    "sar_ideal_greedy": { "success_rate": 0.85, ... },
    "inspection_linear_all": { "edge_coverage_rate": 0.98, ... }
  }
}
```

### Шаг 8: Stress profiles

**Файл:** `crates/swarm-sim/src/stress.rs` (новый)

```rust
/// Parametric sweep over one variable.
pub struct StressSweep {
    pub name: String,
    pub variable: String, // "packet_loss", "agent_count", "grid_size", "false_positive_rate"
    pub values: Vec<f64>,
    pub suite: RegressionSuite,
}

pub fn packet_loss_sweep() -> StressSweep {
    StressSweep {
        name: "packet_loss".to_owned(),
        variable: "packet_loss".to_owned(),
        values: vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5],
        suite: RegressionSuite { ... },
    }
}
```

Результат: `StressReport` с графиком metric vs variable value.

CLI:
```bash
cargo run -p swarm-examples --bin strategy_comparison -- --stress packet-loss
```

### Шаг 9: Актуализация README

Добавить разделы:
- "Regression Testing"
- "Baseline Management"
- "Stress Testing"

## Testing Strategy

### Категория 1 — Без рефакторинга (unit + integration)

**Unit: ThresholdChecker**
- Файл: `crates/swarm-sim/src/regression.rs` (в `#[cfg(test)]`)
- `check` с `min` → violation если actual < min.
- `check` с `max` → violation если actual > max.
- `check` с обоими → оба проверяются.
- `extract_metric` — все поддерживаемые metric name возвращают корректное поле.

**Unit: Baseline serialize/deserialize**
- `Baseline::save` + `Baseline::load` roundtrip.
- `Baseline::compare` — delta вычисляется корректно (+10% = Improved, −10% = Degraded).

**Integration: regression runner smoke**
- Файл: `crates/swarm-examples/tests/regression.rs`
- Запуск `--regression` с одним suite (SAR ideal).
- Assert: exit code 0, `overall_pass = true`.

**Integration: regression runner with forced failure**
- Suite с `min_success_rate = 1.0` (нереалистично).
- Assert: exit code 1, `overall_pass = false`, violation содержит `success_rate`.

### Категория 2 — Лёгкий рефакторинг (integration)

**Integration: baseline comparison**
- Создать baseline с `success_rate = 0.8`.
- Запустить текущий прогон с `--compare-baseline`.
- Assert: delta.status корректен (Stable/Improved/Degraded).

**Integration: stress sweep**
- Файл: `crates/swarm-examples/tests/stress.rs`
- `packet_loss_sweep` с 3 значениями.
- Assert: `convergence_ticks_p95` монотонно растёт (или хотя бы не уменьшается).

### Категория 3 — Тяжёлый рефакторинг

Не требуется. M29 добавляет новые модули, не меняет существующие структуры данных.

## Risks and Tradeoffs

1. **Baseline drift** — baseline устаревает при intentional изменениях алгоритмов.
   Mitigation: `--update-baseline` flag + документация в README как обновлять.

2. **Flaky thresholds** — smoke run (1 seed) имеет высокую дисперсию.
   Mitigation: regression suites используют `Quick` (10 seeds) для критичных метрик.

3. **CI время** — полный `--regression` может занять > 2 минут.
   Mitigation: `--smoke` подмножество (< 30s), `--regression` — nightly.

4. **Threshold tuning** — начальные thresholds — guesswork.
   Mitigation: запустить 3 прогона, взять min − 10% margin.

## Open Questions

1. Нужен ли web dashboard для regression history?
   Нет — M29 ограничен CLI и JSON artifacts. Dashboard — M30+.

2. Как часто обновлять baseline?
   Рекомендация: после каждого значимого milestone (M30, M31) или при
   intentional algorithm improvement.

3. Нужен ли distributed stress testing (много машин)?
   Нет — текущий harness с rayon достаточен для 1000 seeds. Distributed — M35+.
