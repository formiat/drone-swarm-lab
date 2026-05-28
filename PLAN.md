# M39a — Regression Repair

## Context

M38 (Wildfire / Flood v2) закрыт. Реализованы:
- 4 wildfire профиля (SmallStatic, MediumDynamic, LargeStatic, HighThreatDynamic);
- Dynamic threat с spatial spread и wind influence;
- Новые wildfire metrics;
- Scenario JSON файлы;
- Replay integration.

Однако `cargo test --workspace` сейчас падает. Главная проблема — divergence между двумя regression entrypoints:
- `regression_runner` — проходит;
- `strategy_comparison --regression` — падает.

Анализ показал:
1. `strategy_comparison --regression` не поддерживает wildfire (падает в fallback empty scenario);
2. `strategy_comparison --regression` не применяет realism preset (realism suite выполняется без realism);
3. Логика построения сценариев дублируется между двумя бинарниками;
4. `build_coverage_profile` дублируется полностью.

## Investigation context

`INVESTIGATION.md` отсутствует. Анализ кода показал:

### 1. `strategy_comparison --regression` не поддерживает wildfire

**Файл:** `crates/swarm-examples/src/bin/strategy_comparison.rs:997-1011`

```rust
_ => Box::new(|_seed: u64, _profile: &str| {
    let scenario = Scenario {
        name: "empty".to_owned(),
        seed: 0,
        agents: vec![],
        tasks: vec![],
        ground_nodes: vec![],
        base_station: None,
    };
    let run_config = RunConfig {
        max_ticks: 10,
        ..Default::default()
    };
    (scenario, run_config)
}),
```

В `run_regression` match по mission содержит coverage, emergency-mesh, sar, inspection, но **нет wildfire**. Два default suite (`wildfire_small_static_greedy`, `wildfire_medium_dynamic_greedy`) создают пустой сценарий, что даёт `task_completion_rate = 0.000`.

### 2. `strategy_comparison --regression` не применяет realism

**Файл:** `crates/swarm-examples/src/bin/strategy_comparison.rs:967-1083`

В `run_regression` нет проверки `suite.realism`. Набор `realism_coverage_smoke` (где `realism: true`) выполняется как обычный coverage без pose noise, wind, battery model v2.

### 3. Дублирование `build_coverage_profile`

**Файлы:**
- `crates/swarm-examples/src/bin/regression_runner.rs:72-168`
- `crates/swarm-examples/src/bin/strategy_comparison.rs:871-965`

Обе функции идентичны (парсинг составных имён профилей, создание failure/partition events).

### 4. Дублирование mission scenario builder логики

**Файлы:**
- `crates/swarm-examples/src/bin/regression_runner.rs:211-248`
- `crates/swarm-examples/src/bin/strategy_comparison.rs:978-1012`

Оба файла содержат одинаковый match по mission с вызовами `build_*_scenario`.

### 5. Flakiness при параллельном запуске

Наблюдались failures `sar_standard_greedy` и `inspection_perimeter_experimental` при `--jobs 4`, которые не воспроизводятся при `--jobs 1`. Это сигнал о возможной nondeterminism в агрегации или HashMap iteration order.

## Affected components

| Компонент | Путь | Что меняется |
|---|---|---|
| Regression shared logic | `crates/swarm-examples/src/regression_lib.rs` (new) | Общий helper: build scenario, apply realism, run suite, collect metrics |
| Regression runner | `crates/swarm-examples/src/bin/regression_runner.rs` | Использовать shared helper |
| Strategy comparison | `crates/swarm-examples/src/bin/strategy_comparison.rs` | Использовать shared helper; добавить wildfire; добавить realism |
| Default suites | `crates/swarm-sim/src/regression.rs` | Проверить consistency |
| Tests | `crates/swarm-examples/tests/regression.rs` | Починить `strategy_comparison_regression_flag` |
| README | `README.md` | Обновить regression commands |

## Implementation steps

### 1. Создать shared regression helper module

**Файл:** `crates/swarm-examples/src/regression_lib.rs`

```rust
pub type ScenarioBuilder = Box<dyn Fn(u64, &str) -> (Scenario, RunConfig) + Send + Sync>;
pub type StrategyFactory = Box<dyn Fn(&Scenario, &RunConfig) -> Box<dyn Strategy> + Send + Sync>;

/// Build a scenario builder for a given mission name.
pub fn build_mission_scenario_builder(mission: &str) -> Option<ScenarioBuilder> {
    match mission {
        "coverage" => Some(Box::new(|seed, profile| {
            let config = build_coverage_profile(profile, seed);
            build_coverage_scenario(&config)
        })),
        "emergency-mesh" => Some(Box::new(|seed, profile| {
            let profile = EmergencyMeshProfile::from_str(profile)
                .unwrap_or(EmergencyMeshProfile::Ideal);
            build_emergency_mesh_scenario(&profile.config(seed))
        })),
        "sar" => Some(Box::new(|seed, profile| {
            let profile = SarProfile::from_str(profile).unwrap_or(SarProfile::Ideal);
            build_sar_scenario(&profile.config(seed))
        })),
        "inspection" => Some(Box::new(|seed, profile| {
            let profile = InspectionProfile::from_str(profile)
                .unwrap_or(InspectionProfile::Linear);
            build_inspection_scenario(&profile.config(seed))
        })),
        "wildfire" => Some(Box::new(|seed, profile| {
            let profile = WildfireProfile::from_str(profile)
                .unwrap_or(WildfireProfile::SmallStatic);
            build_wildfire_scenario(&profile.config(seed))
        })),
        _ => None,
    }
}

/// Apply realism preset if suite requests it.
pub fn with_realism_if_needed(
    builder: ScenarioBuilder,
    suite: &RegressionSuite,
) -> ScenarioBuilder {
    if suite.realism {
        let profile = RealismProfile::Medium;
        Box::new(move |seed, profile_name| {
            let (scenario, run_config) = builder(seed, profile_name);
            apply_realism_preset(scenario, run_config, profile.clone())
        })
    } else {
        builder
    }
}

/// Run a single regression suite and return metrics map.
pub fn run_regression_suite(
    suite: &RegressionSuite,
    factories: &[StrategyFactory],
    jobs: usize,
) -> Result<HashMap<String, AggregateMetrics>, Box<dyn Error>> {
    let builder = build_mission_scenario_builder(&suite.mission)
        .ok_or_else(|| format!("Unknown mission: {}", suite.mission))?;
    let builder = with_realism_if_needed(builder, suite);
    
    let result = match suite.mode {
        SuiteMode::Smoke => BenchmarkHarness::run_smoke_with_options(...),
        SuiteMode::Quick => BenchmarkHarness::run_quick_with_options(...),
    };
    
    // Extract metrics for suite.profile
    let mut metrics_map = HashMap::new();
    for (strategy_name, _profile_name) in result.report.results.keys() {
        let key = (strategy_name.clone(), suite.profile.clone());
        if let Some(metrics) = result.report.results.get(&key) {
            metrics_map.insert(strategy_name.clone(), metrics.clone());
        }
    }
    Ok(metrics_map)
}
```

### 2. Обновить `regression_runner.rs`

- Удалить дублирующий `build_coverage_profile`;
- Удалить дублирующий mission match;
- Использовать `regression_lib::build_mission_scenario_builder` и `run_regression_suite`.

### 3. Обновить `strategy_comparison.rs`

- Удалить дублирующий `build_coverage_profile`;
- Удалить дублирующий mission match в `run_regression`;
- Добавить wildfire в `run_regression` (через shared helper);
- Добавить realism application в `run_regression` (через shared helper);
- Использовать `regression_lib::run_regression_suite`.

### 4. Проверить determinism / flakiness

Запустить по 3 раза:
```bash
cargo run -p swarm-examples --bin regression_runner -- --jobs 1
cargo run -p swarm-examples --bin regression_runner -- --jobs 4
cargo run -p swarm-examples --bin strategy_comparison -- --regression --jobs 1
cargo run -p swarm-examples --bin strategy_comparison -- --regression --jobs 4
```

Сравнить:
- Suite names;
- Pass/fail status;
- Actual metric values.

Если есть расхождение jobs=1 vs jobs=4 — исследовать HashMap iteration order в агрегации.

### 5. Починить тест `strategy_comparison_regression_flag`

**Файл:** `crates/swarm-examples/tests/regression.rs`

Тест падает из-за file lock contention при вложенном `cargo run`. Добавить retry или убрать вложенный cargo run в пользу library-level вызова.

### 6. Обновить README

- Убедиться, что documented regression commands действительно проходят;
- Уточнить, что оба entrypoint (regression_runner и strategy_comparison --regression) поддерживают одинаковый набор suites;
- Обновить Current Status для M39a.

## Testing strategy

### Категория 1 — без рефакторинга

- **Integration test**: `strategy_comparison --regression --jobs 1` проходит;
- **Integration test**: `strategy_comparison --regression --jobs 4` проходит;
- **Integration test**: wildfire suites не возвращают empty-scenario zero completion;
- **Integration test**: regression output содержит `wildfire_small_static_greedy` и passes;
- **Integration test**: `realism_coverage_smoke` path присутствует и применяет realism preset.

### Категория 2 — лёгкий рефакторинг

- **Shared test helper** для запуска CLI binaries;
- **Shared parsing helper** для regression report output;
- **Parity test**: сравнение `regression_runner` и `strategy_comparison --regression` на одних и тех же suites;
- **Repeated-run smoke test**: gated as `#[ignore]` или slow — проверяет стабильность на 5 прогонах.

### Категория 3 — тяжёлый рефакторинг

- **Library-level regression runner tests**: без вложенного `cargo run`;
- **Deterministic replay/metrics snapshot** для каждого default suite;
- **Property tests** для consistency suite builder.

## Risks and tradeoffs

| Риск | Вероятность | Влияние | Митигация |
|---|---|---|---|
| Shared module сломает существующий `regression_runner` | Низкая | Высокое | Сохранить сигнатуру `RegressionRunner::run`, изменить только внутреннюю реализацию |
| Flakiness не устраняется рефакторингом | Средняя | Среднее | Если flakiness в агрегации — зафиксировать; если в алгоритме — ослабить threshold или перевести в quick |
| `strategy_comparison --regression` меняет behavior из-за realism | Низкая | Среднее | Realism suite уже был designed для realism; применение realism — correction, не regression |
| Test `strategy_comparison_regression_flag` продолжает флейкать | Средняя | Низкое | Заменить вложенный cargo run на library call или retry |

## Open questions

1. **Нужно ли выносить `build_coverage_profile` в shared module?**
   - Да, это самое большое дублирование (~100 строк).

2. **Нужно ли унифицировать CBBA planner selection?**
   - `regression_runner` использует `NearestNeighbourPlanner`;
   - `strategy_comparison` позволяет `--planner`;
   - Рекомендуется: `regression_runner` тоже поддерживает `--planner` для consistency.

3. **Как бороться с flakiness `sar_standard_greedy`?**
   - Вариант A: перевести в `quick` mode (10 seeds вместо 1);
   - Вариант B: ослабить threshold после измерения variance;
   - Вариант C: пометить как `experimental` (не gating).

4. **Нужен ли `regression_lib.rs` как pub mod в lib.rs?**
   - Да, для library-level тестов без cargo run.

## Что могло сломаться

- **Поведение**: `strategy_comparison --regression` теперь поддерживает wildfire и realism. Раньше wildfire suites падали (actual=0.000), теперь будут проходить или фейлиться по реальным threshold. Это correction, не regression.
- **API/контракты**: Новый модуль `regression_lib` добавляет публичные функции. Существующие API не меняются.
- **Интеграции**: Оба CLI entrypoint теперь используют shared logic. Выходные данные (suite names, pass/fail, metrics) должны совпадать.
- **Производительность**: Нет изменений в производительности.
- **Тесты**: `strategy_comparison_regression_flag` может продолжать флейкать из-за cargo file lock. Если не удаётся починить за reasonable time — пометить как `#[ignore]` с комментарием.

## Критерии готовности

- [ ] `cargo test --workspace` проходит (включая regression integration tests).
- [ ] `cargo clippy --all-targets -- -D warnings` проходит.
- [ ] `cargo fmt --all` не меняет код.
- [ ] `cargo run -p swarm-examples --bin regression_runner -- --jobs 4` проходит.
- [ ] `cargo run -p swarm-examples --bin strategy_comparison -- --regression --jobs 4` проходит.
- [ ] Оба CLI возвращают одинаковые suite names и pass/fail state.
- [ ] Wildfire regression suites используют реальные wildfire scenarios.
- [ ] Realism regression suite применяет realism preset.
- [ ] README обновлён.
- [ ] Локальный commit сделан.
