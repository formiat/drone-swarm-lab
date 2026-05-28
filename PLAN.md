# Plan: M36 — Regression Harness v2

## Context

M36 следует за M32–M35, которые исправили идентичность строк report, интеграцию mission semantics,
planner correctness и dynamic mission semantics. Теперь regression harness работает и проходит,
но не защищает будущую работу должным образом: пороги слишком слабые, baseline устарел, наборы
тестов не покрывают wildfire и realism, а тесты непортируемы из-за путей `/tmp`.

## Investigation Context

Нет `INVESTIGATION.md`. Анализ выполнен по исходному коду:

- `crates/swarm-sim/src/regression.rs` — ядро harness: `Threshold`, `RegressionSuite`,
  `ThresholdChecker`, `Baseline`, `RegressionRunner`, `default_suites()`.
- `crates/swarm-examples/src/bin/regression_runner.rs` — CLI-бинарник.
- `crates/swarm-examples/tests/regression.rs` — интеграционные тесты.
- `results/baseline.json` — текущий baseline (commit `c785014`, до M32–M35).

### Выявленные проблемы

**Слабые пороги:**
- `sar_standard_greedy`: только `success_rate >= 0.0` — passes at 0% success.
- `emergency_mesh_ideal`: только `success_rate >= 0.0` — аналогично.
- `inspection_perimeter_all`: `edge_coverage_rate >= 0.3` — слишком низкий порог.
- Smoke и Quick режимы используют одни и те же метрики без разделения.
- Нет mission-specific порогов (нет `probability_of_detection`, `targets_found`, `time_to_find`
  для SAR; нет wildfire-метрик).

**Устаревший baseline:**
- Commit `c785014` — до M32–M35; comparison с текущим кодом некорректен.
- Процесс обновления baseline нигде не задокументирован.

**Непокрытые сценарии:**
- Wildfire есть в `profiles.rs` (SmallStatic, MediumDynamic), но нет regression suite.
- Realism (`--realism`) не представлен ни в одной suite.
- SAR suites используют greedy; supported strategies (CBBA/centralized) не тестируются отдельно.
- `inspection_perimeter_all` — режим "all" (все стратегии), но экспериментальные пороги
  не отделены.

**Непортируемые тесты — использование `/tmp`:**
- `crates/swarm-sim/src/regression.rs` — `/tmp/test_baseline.json`
- `crates/swarm-examples/tests/regression.rs` — `/tmp/test_forced_fail_baseline.json`
- `crates/swarm-examples/tests/wildfire.rs` — `/tmp/test_wildfire_baseline.json`
- `crates/swarm-sim/src/dsl.rs` — `/tmp/test_scenario_suite.json`
- `crates/swarm-examples/tests/replay_cli.rs` — `/tmp/replay_test_dir/...`
- `crates/swarm-examples/tests/benchmark_pack.rs` — `/tmp/bench_*_test_dir/`

**Слабый failure output:**
- `ThresholdViolation` содержит только `threshold` и `actual`; нет delta, нет mode/seed range.
- Сообщение об ошибке не позволяет сразу понять степень нарушения.

**Именование stress profiles:**
- `cbba_stress_pl_0_0`, `cbba_stress_pl_0_2` — непрозрачные суффиксы, не отражают реальные
  условия профиля.

## Affected Components

| Компонент | Файл | Изменение |
|---|---|---|
| Regression core | `crates/swarm-sim/src/regression.rs` | пороги, новые suites, failure output |
| Regression runner | `crates/swarm-examples/src/bin/regression_runner.rs` | README update process, именование suites |
| Integration tests | `crates/swarm-examples/tests/regression.rs` | tempdir, новые тесты |
| Wildfire tests | `crates/swarm-examples/tests/wildfire.rs` | tempdir |
| DSL tests | `crates/swarm-sim/src/dsl.rs` | tempdir |
| Replay tests | `crates/swarm-examples/tests/replay_cli.rs` | tempdir |
| Benchmark tests | `crates/swarm-examples/tests/benchmark_pack.rs` | tempdir |
| Baseline file | `results/baseline.json` | обновить после M32–M35 |
| README | `README.md` | актуализировать Current Status, Known Limitations, regression section |

## Implementation Steps

### Шаг 1. Улучшить `ThresholdViolation` и failure output

**Файл:** `crates/swarm-sim/src/regression.rs`

1.1. Добавить поле `delta: f64` в `ThresholdViolation` — разница actual vs threshold
     (`actual - min` или `max - actual`).

1.2. Добавить `Display` для `ThresholdViolation`:
```
[suite_name] metric=success_rate actual=0.42 threshold=min:0.70 delta=-0.28
```

1.3. Добавить в `SuiteResult` поле `mode: SuiteMode` и `seed_range: (u64, u64)` —
     для включения в failure output.

1.4. Обновить печать в `RegressionRunner::run()` — выводить mode и seed range при провале.

### Шаг 2. Переименовать stress profiles

**Файл:** `crates/swarm-sim/src/regression.rs` (функция `default_suites`)

2.1. Переименовать `cbba_stress_pl_0_0` → `cbba_coverage_ideal_no_failures`
     (profile `ideal-no-failures`, 0 packet loss, 0 failures).

2.2. Переименовать `cbba_stress_pl_0_2` → `cbba_coverage_light_loss_no_failures`
     (profile `light-loss-no-failures`, packet_loss=0.05).

2.3. Убедиться, что именование suite отражает mission + profile + strategy.

### Шаг 3. Откалибровать существующие пороги

**Файл:** `crates/swarm-sim/src/regression.rs` (функция `default_suites`)

3.1. `sar_ideal_greedy` (Smoke):
- `success_rate >= 0.70` (было: `>= 0.0`);
- `belief_entropy_final <= 0.5` (оставить);
- добавить `probability_of_detection >= 0.60`.

3.2. `sar_standard_greedy` (Smoke):
- `success_rate >= 0.50` (было: `>= 0.0`);
- добавить `belief_entropy_final <= 0.6`.

3.3. `inspection_linear_all` (Smoke):
- `edge_coverage_rate >= 0.85` (оставить);
- `success_rate >= 0.90` (оставить).

3.4. `inspection_perimeter_all` (Smoke):
- `edge_coverage_rate >= 0.50` (было: `>= 0.3`, повысить);
- добавить отдельный experimental suite с более мягким порогом `>= 0.30`.

3.5. `cbba_coverage_ideal_no_failures` (Quick):
- `success_rate >= 0.90` (оставить);
- `convergence_ticks_p95 <= 15.0` (оставить);
- добавить `task_completion_rate >= 0.95`.

3.6. `cbba_coverage_light_loss_no_failures` (Quick):
- `success_rate >= 0.80` (оставить);
- `convergence_ticks_p95 <= 20.0` (оставить).

3.7. `safety_coverage` (Smoke):
- `safety_violations <= 0.0` (оставить — правильно).

3.8. `emergency_mesh_ideal` (Smoke):
- `success_rate >= 0.80` (было: `>= 0.0`);
- добавить `network_availability >= 0.90`.

### Шаг 4. Добавить новые regression suites

**Файл:** `crates/swarm-sim/src/regression.rs` (функция `default_suites`)

4.1. **wildfire_small_static_greedy** (Smoke):
- mission: wildfire, profile: small-static, strategy: greedy;
- thresholds: `success_rate >= 0.70`, `task_completion_rate >= 0.80`.

4.2. **wildfire_medium_dynamic_greedy** (Smoke):
- mission: wildfire, profile: medium-dynamic, strategy: greedy;
- thresholds: `success_rate >= 0.50`, `task_completion_rate >= 0.60`
  (экспериментальный — dynamic semantics непредсказуемы).

4.3. **realism_coverage_smoke** (Smoke):
- mission: coverage, profile: ideal-no-failures + realism preset, strategy: greedy;
- thresholds: `success_rate >= 0.75` (мягче идеального из-за realism overhead);
- Для включения: в `regression.rs` добавить поддержку realism flag в `RegressionSuite`
  или создать отдельный profile `ideal-no-failures-realism`.

4.4. **sar_cbba_supported** (Smoke):
- mission: sar, profile: ideal, strategy: cbba;
- thresholds: `success_rate >= 0.60`, `belief_entropy_final <= 0.5`;
- Нужно проверить, что CBBA для SAR поддерживается после M35.

4.5. **inspection_perimeter_experimental** (Smoke):
- mission: inspection, profile: perimeter, strategy: greedy;
- thresholds: `edge_coverage_rate >= 0.30` — мягкий experimental порог;
- отдельно от `inspection_perimeter_all` с более строгим `>= 0.50`.

### Шаг 5. Добавить поддержку realism в `RegressionSuite`

**Файл:** `crates/swarm-sim/src/regression.rs`

5.1. Добавить в `RegressionSuite` опциональное поле `realism: bool` (по умолчанию `false`).

5.2. В `RegressionRunner` — при `realism: true` применять realism preset к сценарию
     (аналогично флагу `--realism` в `strategy_comparison`).

5.3. В baseline key включать `_realism` суффикс для realism suites.

### Шаг 6. Зафиксировать актуальный baseline

**Файл:** `results/baseline.json`

6.1. Запустить `regression_runner` после M32–M35 (текущего состояния кода):
```bash
cargo run -p swarm-examples --bin regression_runner -- \
  --update-baseline results/baseline.json --jobs 4
```

6.2. Проверить, что baseline содержит записи для всех новых suites из Шагов 3–4.

6.3. Закоммитить `results/baseline.json` с сообщением, указывающим commit code state.

6.4. Добавить секцию в README (`docs/BENCHMARK_RESULTS.md` или раздел в `README.md`)
     с описанием процесса обновления baseline:
     - когда обновлять: после каждого milestone;
     - команда обновления;
     - требование: commit hash должен совпадать с состоянием кода.

### Шаг 7. Устранить использование `/tmp` — перейти на tempdir

**Файлы:** все перечисленные в разделе "Investigation Context"

7.1. `crates/swarm-sim/src/regression.rs` (unit test `test_baseline_roundtrip`):
- Заменить `/tmp/test_baseline.json` на `tempfile::TempDir`.

7.2. `crates/swarm-examples/tests/regression.rs`:
- `regression_runner_with_forced_failure`: заменить `/tmp/test_forced_fail_baseline.json`
  на `tempfile::NamedTempFile`.

7.3. `crates/swarm-examples/tests/wildfire.rs`:
- Заменить `/tmp/test_wildfire_baseline.json` на `tempfile::NamedTempFile`.

7.4. `crates/swarm-sim/src/dsl.rs`:
- Заменить `/tmp/test_scenario_suite.json` на `tempfile::NamedTempFile`.

7.5. `crates/swarm-examples/tests/replay_cli.rs`:
- Заменить `/tmp/replay_test_dir/...` на `tempfile::TempDir`.

7.6. `crates/swarm-examples/tests/benchmark_pack.rs`:
- Заменить `/tmp/bench_*_test_dir/` на `tempfile::TempDir`.

7.7. Убедиться, что `tempfile` уже есть в `[dev-dependencies]` соответствующих crates
     (если нет — добавить).

### Шаг 8. Добавить forced-failure тест для новых порогов

**Файл:** `crates/swarm-examples/tests/regression.rs`

8.1. Добавить тест `regression_new_thresholds_fail_on_bad_data`:
- Создать baseline с завышенными метриками;
- Запустить harness с намеренно плохой конфигурацией;
- Убедиться, что `overall_pass == false` и violations содержат нужные метрики.

8.2. Добавить тест `threshold_delta_in_violation`:
- Проверить, что `ThresholdViolation.delta` корректно вычисляется.

### Шаг 9. Актуализировать README

**Файл:** `README.md`

9.1. Раздел "Current Status":
- Обновить список milestone: M32–M36 выполнены.
- Убрать устаревшие Known Limitations, закрытые M32–M35.

9.2. Раздел Regression / Testing:
- Описать команду запуска regression;
- Описать процесс обновления baseline;
- Перечислить текущие suites и их назначение.

9.3. Добавить раздел "Regression Thresholds Policy":
- Правило: no `>= 0.0` thresholds;
- Разграничение smoke vs quick;
- Как добавлять новый suite.

## Testing Strategy

### Категория 1: Тесты без рефакторинга (реализуются вместе с основными изменениями)

- **`test_threshold_violation_delta`** — unit test: `ThresholdViolation.delta` верно считается
  для min и max bounds.
- **`test_threshold_checker_zero_min`** — unit test: suite с `success_rate >= 0.0` проходит
  при любом значении; проверяем, что новые meaningful пороги не допускают этого.
- **`test_regression_forced_failure_new_thresholds`** — интеграционный: принудительно плохие
  метрики → `overall_pass == false`, violations содержат корректные delta.
- **`test_baseline_compare_stability`** — unit test: baseline roundtrip сохраняет все поля,
  compare возвращает Stable при идентичных данных.
- **`test_failure_output_includes_mode_and_seeds`** — unit test: `SuiteResult` при провале
  содержит mode и seed range в сообщении.

### Категория 2: Тесты с лёгким рефакторингом

- **`regression_runner_smoke_passes`** — уже существует; нужно обновить ожидаемые суиты
  после переименования и добавления новых.
- **`regression_runner_with_forced_failure`** — уже существует; нужно перейти на tempdir.
- **`test_baseline_roundtrip`** — уже существует в `regression.rs`; нужно перейти на tempdir.
- **Все тесты с `/tmp`** в `wildfire.rs`, `dsl.rs`, `replay_cli.rs`, `benchmark_pack.rs` —
  механическая замена на tempdir.
- **`test_wildfire_suites_pass`** — новый integration test для wildfire_small_static и
  wildfire_medium_dynamic suites; использует shared wildfire fixture builder.
- **`test_realism_suite_smoke`** — новый smoke test для realism suite; проверяет, что
  `success_rate >= 0.75` выполняется при включённом realism preset.

### Категория 3: Тесты с тяжёлым рефакторингом

- **`test_statistical_threshold_stability`** — property test: на N runs (proptest) пороги
  не должны флакать на stable scenarios; требует отдельной test harness для proptest.
- **`test_1000_seed_regression`** — полный режим 1000 seeds; нужен отдельный test binary
  или feature flag (сейчас Quick = 10 seeds; 1000 seeds — отдельный mode `Full`).
- **`test_baseline_migration`** — тест совместимости формата baseline v1.0 с будущими
  версиями; требует versioning схемы и миграционного кода.

### Gap-анализ

- **Realism smoke suite** зависит от корректной интеграции realism preset в `RegressionSuite`
  (Шаг 5); без этого — тест покрыть нельзя.
- **SAR CBBA supported** зависит от результатов M35; если CBBA SAR остался unsupported —
  suite создаётся как skipped с явным документированием причины.
- **1000-seed mode** требует решения о добавлении `SuiteMode::Full` — вынесено в
  открытые вопросы.

## Что Могло Сломаться

### Поведение
- Повышение порогов может сломать существующие CI-прогоны, если метрики после M32–M35
  реально деградировали. Нужно сначала запустить и проверить, прежде чем поднимать пороги.
- Переименование suites изменит ключи в `baseline.json`; старый baseline станет несовместимым.
  Baseline нужно обновить в том же PR/commit.

### API/Контракты
- `ThresholdViolation` получает новое поле `delta` — это breaking change для кода,
  который деструктурирует структуру. Найти все call sites в тестах и обновить.
- `SuiteResult` получает `mode` и `seed_range` — аналогично, проверить все usages.
- `RegressionSuite` получает поле `realism` — убедиться, что `default_suites()` корректно
  инициализирует старые suites с `realism: false`.

### Данные/Файлы
- `results/baseline.json` будет перезаписан; старая версия потеряна, если нет git.
  Решение: обновление baseline — всегда через git commit.

### Интеграции
- `strategy_comparison_regression_flag` тест использует regression через CLI;
  при переименовании suites output может измениться, тест нужно обновить.

### Производительность/Ресурсы
- Добавление новых suites (wildfire, realism) увеличит время regression run.
  Wildfire и realism — Smoke (1 seed), влияние минимально.

### Как проверить
```bash
# Убедиться, что всё проходит после изменений:
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim regression
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-examples regression
# Запустить полный regression runner:
/home/formi/.local/bin/runlim cargo run -p swarm-examples --bin regression_runner -- --jobs 4
```

## Risks and Tradeoffs

| Риск | Вероятность | Митигация |
|---|---|---|
| После M35 реальные метрики SAR CBBA ниже новых порогов | Средняя | Сначала запустить smoke, откалибровать пороги по факту |
| Wildfire medium-dynamic нестабилен (success/completion mismatch из M35) | Высокая | Использовать мягкий порог `>= 0.50`, пометить как experimental |
| Realism suite требует существенного рефакторинга `RegressionSuite` | Средняя | Реализовать как отдельный profile вместо нового поля |
| `tempfile` crate отсутствует в dev-dependencies | Низкая | Проверить Cargo.toml перед началом; добавить при необходимости |
| Baseline outdated сразу после обновления (если код дорабатывается) | Средняя | Обновлять baseline только когда код стабилен; фиксировать commit hash |

## Open Questions

1. **SuiteMode::Full (1000 seeds):** Нужен ли отдельный режим `Full` для 1000-seed regression,
   или достаточно Quick (10 seeds)? Требует решения перед реализацией Категории 3 тестов.

2. **SAR CBBA status после M35:** Если CBBA для SAR по-прежнему unsupported — suite
   `sar_cbba_supported` нужно пропустить или превратить в documented-unsupported тест.

3. **Realism как поле vs profile:** Лучше добавить `realism: bool` в `RegressionSuite`
   или создать отдельный "realism" network profile? Profile-подход проще, но менее явный.

4. **Baseline versioning:** Нужна ли схема версионирования baseline JSON (v1.0 → v2.0)
   при добавлении новых полей, или достаточно commit hash?

5. **CI integration:** Запускается ли regression_runner в CI? Если да — нужно убедиться,
   что новые suites не сломают CI time budget.
