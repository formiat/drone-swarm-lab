# M32 — Reporting & Metrics Hardening

## Context

Работа над M31 завершена. Симуляция поддерживает 3D pose, battery model v2, wind/noise, comms jitter, time-gated no-fly zones и `--realism` preset. Добавлена миссия wildfire/flood mapping (M30). Regression harness запускается (M29).

Однако при запуске `--mission all` benchmark pack выглядит работающим, но каждая строка получает `mission="coverage"` и `scenario="coverage"`, независимо от реальной миссии. Это происходит потому, что `ComparisonReport` хранит mission/scenario как списки верхнего уровня, а exporter берёт `first()` для каждой строки.

Это критично: downstream analysis, baseline/regression, `docs/BENCHMARK_RESULTS.md` нельзя честно обновлять, пока per-row identity неисправна.

## Investigation context

`INVESTIGATION.md` отсутствует. Анализ кода показал:

- `ComparisonReport` (crates/swarm-sim/src/benchmark.rs:11-21) хранит `mission_names: Vec<String>`, `scenario_names: Vec<String>` на уровне отчёта.
- `merge_reports` (crates/swarm-examples/src/bin/strategy_comparison.rs:719-769) создаёт ключи `("strategy", "{mission}/{profile}")` и склеивает `mission_names`/`scenario_names` через `flat_map`. Результат — merged report с N миссий, но `mission_names.first()` всегда "coverage" (потому что coverage запускается первым).
- `export_json`, `export_csv` (crates/swarm-sim/src/report_export.rs:19-20, 124-125) используют `report.mission_names.first()` и `report.scenario_names.first()` для каждой строки.
- `Display for ComparisonReport` (crates/swarm-sim/src/benchmark.rs:24-92) делает то же самое.
- `generate_focused_report` (crates/swarm-sim/src/report_export.rs:298-421) ожидает `Vec<(String, ComparisonReport)>` и строит per-mission таблицы, но это работает только для отдельных отчётов, не для merged.
- Планнер-метрики (`avg_route_length`, `avg_wasted_travel`, `avg_return_reserve`, `avg_infeasible_routes`) и wildfire-метрики (`avg_hazard_zones_mapped`, `avg_priority_updates`, `avg_final_threat_level`) уже собираются в `AggregateMetrics::from_runs`, но не экспортируются в JSON/CSV/table.
- `BenchmarkManifest` не содержит realism metadata.

## Affected components

| Компонент | Путь | Что меняется |
|---|---|---|
| Report data model | `crates/swarm-sim/src/benchmark.rs` | Добавить per-row mission/scenario в `ComparisonReport` |
| JSON/CSV exporter | `crates/swarm-sim/src/report_export.rs` | Использовать per-row mission/scenario; добавить missing metrics |
| Markdown exporter | `crates/swarm-sim/src/report_export.rs` | Использовать per-row mission/scenario |
| Benchmark CLI | `crates/swarm-examples/src/bin/strategy_comparison.rs` | Исправить `merge_reports` |
| AggregateMetrics | `crates/swarm-metrics/src/metrics.rs` | Проверить полноту агрегации |
| BenchmarkManifest | `crates/swarm-sim/src/report_export.rs` | Добавить realism metadata |
| README | `README.md` | Обновить Current Status и Known Limitations |
| Benchmark results | `docs/BENCHMARK_RESULTS.md` | Обновить после фикса |

## Implementation steps

### 1. Per-row mission/scenario в ComparisonReport

Файл: `crates/swarm-sim/src/benchmark.rs`

- Изменить `results: HashMap<(String, String), AggregateMetrics>` на `results: HashMap<ReportKey, ReportRow>`, где:
  ```rust
  #[derive(Clone, Debug, Hash, Eq, PartialEq)]
  pub struct ReportKey {
      pub strategy: String,
      pub profile: String,
  }
  
  pub struct ReportRow {
      pub key: ReportKey,
      pub metrics: AggregateMetrics,
      pub mission: String,
      pub scenario: String,
  }
  ```
- Альтернатива (менее инвазивная): добавить поля `mission: String`, `scenario: String` внутрь `AggregateMetrics` (или в отдельную обёртку).
- Выбранный подход: минимальный — добавить `mission` и `scenario` в `AggregateMetrics` через `#[serde(default)]` для backward compatibility. Это позволяет не менять все call sites сразу.

### 2. Исправить merge_reports

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

- Сохранять исходный `mission` в каждом `AggregateMetrics` при создании отчёта.
- При merge не терять mission: использовать `mission` из метрик, а не `mission_names.first()`.
- Убрать prefix `{mission}/` из profile name (это workaround, который теперь не нужен, потому что mission хранится отдельно).
- Сохранять stable row keys: `(strategy, profile)` без mission prefix.

### 3. Обновить exporters

Файл: `crates/swarm-sim/src/report_export.rs`

**JSON:**
- `ReportRow` получает `mission` и `scenario` из метрик, не из `report.mission_names.first()`.

**CSV:**
- То же самое для каждой записи.

**Markdown (Display for ComparisonReport):**
- Использовать per-row mission/scenario.
- Добавить колонку `Mission` перед `Scenario` (если mixed-mission).

**generate_focused_report:**
- Группировать строки по `mission` из метрик, а не по внешнему вектору.

### 4. Добавить missing metrics в exports

Файл: `crates/swarm-sim/src/report_export.rs`

Добавить в `ReportRow` и в CSV/JSON:
- `avg_route_length`
- `avg_wasted_travel`
- `avg_return_reserve`
- `avg_infeasible_routes`
- `avg_hazard_zones_mapped`
- `avg_priority_updates`
- `avg_final_threat_level`

### 5. Обновить BenchmarkManifest

Файл: `crates/swarm-sim/src/report_export.rs`

Добавить в `BenchmarkManifest`:
- `realism_profile: Option<String>`
- `wind_enabled: bool`
- `pose_noise_m: f64`
- `comms_jitter_ticks: u64`

Заполнять из `RunConfig` при создании manifest.

### 6. Обновить docs

Файлы:
- `README.md` — Current Status: M32 в work; Known Limitations: описать report identity bug до фикса.
- `docs/BENCHMARK_RESULTS.md` — после регенерации с `--mission all`.

### 7. Проверка backward compatibility

- `#[serde(default)]` на новых полях `AggregateMetrics`.
- `schema_version` в manifest оставить "0.1" или bump до "0.2" при breaking change.
- Старые `results.json` без per-row mission должны десериализоваться (mission = default empty string).

## Testing strategy

### Категория 1 — без рефакторинга

- **CLI integration test**: запустить `--smoke --mission all --output-dir tempdir` и проверить, что `results.json` содержит строки с разными `mission` (coverage, sar, inspection, wildfire, emergency-mesh).
- **JSON row assertion**: для SAR rows проверить `mission == "sar"` и `scenario == "standard"` (или соответствующий).
- **CSV row assertion**: для wildfire rows проверить `mission == "wildfire"`.
- **Markdown row assertion**: для mixed-mission таблицы проверить, что строки сгруппированы по миссиям.
- **Unit test merge_reports**: проверить, что merged report содержит 2 миссии с правильными mission/scenario у каждой строки.

### Категория 2 — лёгкий рефакторинг

- Заменить `/tmp/...` в тестах на `tempfile::TempDir`.
- Создать shared report fixture builders (`fn make_single_mission_report(mission: &str) -> ComparisonReport`, `fn make_mixed_mission_report() -> Vec<ComparisonReport>`).
- Создать JSON/CSV parsing helpers для тестов (избежать дублирования `serde_json::from_str` и ручного split по запятым).

### Категория 3 — тяжёлый рефакторинг

- Schema compatibility tests: десериализация старого `results.json` (без per-row mission) → проверка graceful degradation.
- Golden benchmark pack comparison: сравнение output directory с reference snapshot (после фикса).
- Property test: для любого merged report все строки имеют уникальную комбинацию `(mission, scenario, strategy, profile)`.

## Risks and tradeoffs

| Риск | Вероятность | Влияние | Митигация |
|---|---|---|---|
| Breaking change в `AggregateMetrics` serialization | Средняя | Высокое | `#[serde(default)]` на новых полях; bump schema_version |
| merge_reports перестаёт быть backward compatible с внешними скриптами | Низкая | Среднее | Убрать `{mission}/` prefix из profile — внешние парсеры, которые ждут его, сломаются. Документировать в CHANGELOG. |
| Увеличение размера results.json | Низкая | Низкое | Добавляется 2 строки на row (mission, scenario) + 7 чисел (metrics). Незначительно. |
| Tests используют `/tmp` и могут быть flaky | Высокая | Среднее | Категория 2: конвертация в `tempfile::TempDir` |
| Regression baseline станет incompatible | Средняя | Среднее | Baseline сравнивает по `(suite, metric)`, не по report schema. Но если downstream читает `results.json`, может сломаться. |

## Open questions

1. **Нужен ли bump schema_version до 0.2?**
   - Если новые поля только добавляются (no breaking change), можно оставить 0.1.
   - Если убираем `{mission}/` prefix из profile (breaking для парсеров), bump до 0.2.

2. **Как обрабатывать `scenario` при `--mission all`?**
   - Сейчас `scenario` берётся из profile name (e.g., "ideal", "standard"). При mixed-mission это остаётся корректным, потому что scenario — это профиль в рамках миссии.

3. **Нужно ли добавлять `suite_name` в per-row identity?**
   - Нет, `suite_name` уже есть в `BenchmarkManifest`. Per-row identity = `(mission, scenario, strategy, profile)`.

4. **Как поступить с `generate_focused_report`?**
   - Она уже строит per-mission таблицы. Нужно только убедиться, что grouping берёт `mission` из метрик, а не из внешнего списка.

## Что могло сломаться

- **Поведение**: `--mission all` теперь экспортирует правильные mission/scenario. Внешние скрипты, которые парсили `results.json` и ожидали `mission="coverage"` для всех строк, могут сломаться (но это багфикс, не регрессия).
- **API/контракты**: `AggregateMetrics` добавляет поля `mission` и `scenario`. Любой код, который создаёт `AggregateMetrics` через struct literal без `..Default::default()`, не скомпилируется. Нужно проверить все struct literals.
- **Данные**: старые `results.json` без per-row mission десериализуются с `mission=""`, `scenario=""` (благодаря `#[serde(default)]`).
- **Интеграции**: `merge_reports` убирает `{mission}/` prefix из profile name. Внешние инструменты, которые парсили profile name для извлечения mission, сломаются.
- **Производительность**: нет значительных изменений. Добавление 2 строк на row не влияет на benchmark runtime.

## Критерии готовности

- [ ] `cargo test --workspace` проходит (включая новые тесты mixed-mission identity).
- [ ] `cargo clippy --all-targets -- -D warnings` проходит.
- [ ] `cargo fmt --all` не меняет код.
- [ ] `--smoke --mission all` создаёт `results.json` с корректными per-row `mission` и `scenario`.
- [ ] JSON/CSV/table согласованы по row identity.
- [ ] Wildfire metrics (`avg_hazard_zones_mapped`, `avg_priority_updates`, `avg_final_threat_level`) экспортируются.
- [ ] Planner metrics (`avg_route_length`, `avg_wasted_travel`, `avg_return_reserve`, `avg_infeasible_routes`) экспортируются.
- [ ] `BenchmarkManifest` содержит realism metadata при `--realism`.
- [ ] README обновлён (Current Status, Known Limitations).
- [ ] `docs/BENCHMARK_RESULTS.md` обновлён или помечен как stale pending regeneration.
- [ ] Локальный commit сделан.
