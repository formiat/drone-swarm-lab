# Context

Три ревью зафиксировали оставшийся архитектурный долг после предыдущего
рефакторинга. Цель этого плана — устранить его систематически, не меняя
поведение алгоритмов, CLI, форматы артефактов и benchmark semantics.

Ревью единогласно: главный приоритет — завершить разбивку runner. Остальные
шаги идут по убыванию критичности.

# Investigation context

INVESTIGATION.md отсутствует. Контекст — три ревью из inbox и прямая
инспекция кода.

Ключевые факты:

- `crates/swarm-examples/src/sitl_agent_runtime/connection.rs` содержит
  build-break: строки 390, 411, 474 используют `super::reports::SitlRunFinalStatus`,
  тогда как в `reports.rs` этот тип только **приватно** импортирован из
  `crate::sitl_report`. Enum публичен в `sitl_report.rs:17`.
  Воспроизводится при сборке с `--all-features`.

- `crates/swarm-sim/src/runner/scenario_runner_internal.rs` — 602 строки.
  Sub-step 1.2 (final_metrics) выполнен, 1.1 и 1.3 — нет.
  `run_internal` содержит initialization block (~90 строк), tick loop (~340
  строк), post-loop (~58 строк) и передаёт всё в `assemble_final_metrics`.

- `crates/swarm-sim/src/runner/scenario_runner_urban.rs` — 667 строк.
  Два больших метода: `run_urban_patrol` (строки 10–299) и
  `run_urban_search` (строки 300–667). Разные миссии, разные метрики, разная
  логика обнаружения.

- `crates/swarm-sim/src/runner/urban_helpers.rs` — 585 строк.
  Вспомогательные функции для обоих urban runner-ов.

- `crates/swarm-sim/src/benchmark.rs` — 907 строк.
  `ComparisonReport` содержит `impl Display` с огромной Markdown-таблицей
  (строки 23–109). `BenchmarkHarness` содержит и запуск, и агрегацию.

- `crates/swarm-sim/src/report_export/export_formats.rs` — 574 строки.
  `export_json`, `export_csv`, `export_markdown`, `generate_focused_report`
  в одном файле.

- `crates/swarm-sim/src/dsl/types.rs` — 528 строк.
  Типы, загрузка и валидация в одном файле; `dsl/mod.rs` только делает
  `mod types; pub use types::*`.

- `crates/swarm-examples/src/strategy_comparison_runtime/runs.rs` — 587 строк.
  Mission selection через большой `match mission { ... }` (строки 101–202).
  Добавление новой миссии требует редактировать этот match.

# Affected components

- `crates/swarm-examples/src/sitl_agent_runtime/connection.rs`
- `crates/swarm-sim/src/runner/scenario_runner_internal.rs`
- `crates/swarm-sim/src/runner/internal/` (mod.rs, новые файлы)
- `crates/swarm-sim/src/runner/scenario_runner_urban.rs` → новые файлы
- `crates/swarm-sim/src/runner/urban_helpers.rs` → перераспределить
- `crates/swarm-sim/src/benchmark.rs` → `crates/swarm-sim/src/benchmark/`
- `crates/swarm-sim/src/lib.rs` (re-exports после split)
- `crates/swarm-sim/src/report_export/export_formats.rs`
- `crates/swarm-sim/src/report_export/mod.rs`
- `crates/swarm-sim/src/dsl/types.rs` → разделить
- `crates/swarm-sim/src/dsl/mod.rs`
- `crates/swarm-examples/src/strategy_comparison_runtime/runs.rs`
- `crates/swarm-examples/src/strategy_comparison_runtime/mod.rs`
- `crates/swarm-examples/src/sitl_safety.rs` (низкий приоритет)
- `crates/swarm-examples/src/sitl_plan.rs` (низкий приоритет)

# Implementation steps

---

## Шаг 1 — Исправить build-break в `connection.rs`

**Приоритет: немедленно, блокирует `--all-features`.**

**Проблема:**
`connection.rs` строки 390, 411, 474 используют `super::reports::SitlRunFinalStatus`,
но в `reports.rs` этот тип только приватно импортирован:
```rust
use crate::sitl_report::{..., SitlRunFinalStatus, ...};
```
Он не переэкспортируется из `reports.rs` публично.

**Исправление:**
В `crates/swarm-examples/src/sitl_agent_runtime/connection.rs` заменить все
обращения `super::reports::SitlRunFinalStatus` на
`crate::sitl_report::SitlRunFinalStatus`.

Также удалить неиспользуемый `use std::path::Path` (предупреждение clippy).

**Файлы:**
- `crates/swarm-examples/src/sitl_agent_runtime/connection.rs` — три замены

**Верификация:**
```bash
cargo build --all-features -p swarm-examples
cargo clippy --all-targets -- -D warnings
```

---

## Шаг 2 — Завершить runner: TickLoopState + tick loop extraction

**Цель:** `run_internal` становится тонким coordinatorm (~30 строк).
`scenario_runner_internal.rs` исчезает или содержит < 50 строк.

### 2.1 Создать `runner/internal/loop_state.rs` — `TickLoopState<A>`

Struct без lifetime аннотаций (все поля owned):

```rust
pub(in crate::runner) struct TickLoopState<A: Allocator> {
    pub nodes: Vec<(AgentNode<InMemAgentTransport>, AgentId)>,
    pub bus: Rc<RefCell<InMemNetwork>>,
    pub allocator: SafetyAllocator<A>,
    pub clock: Clock,
    pub log_builder: Option<swarm_replay::EventLogBuilder>,
    pub failure_ticks: HashMap<AgentId, u64>,
    pub crashed_agents: HashSet<AgentId>,
    pub detected_agents: HashSet<AgentId>,
    pub unassigned_durations: HashMap<TaskId, u64>,
    pub max_task_unassigned_ticks: u64,
    pub detection_time_ticks: Option<u64>,
    pub detection_tick: Option<u64>,
    pub reallocation_time_ticks: Option<u64>,
    pub total_ticks: u64,
    pub tasks_injected: u64,
    pub tasks_expired: u64,
    pub conflicting_assignments: u64,
    pub stale_messages_discarded: u64,
    pub partition_events: u64,
    pub partitions_active: bool,
    pub convergence_ticks: Option<u64>,
    pub heal_tick: Option<u64>,
    pub max_view_divergence: u64,
    pub revisit_count: u64,
    pub total_distance_travelled: f64,
    pub time_to_first_exhaustion: Option<u64>,
    pub safety_violations: u64,
    pub cbba_convergence_tick: Option<u64>,
    pub adapter_registry: AdapterRegistry,
    pub wildfire_state: Option<WildfireState>,
    pub priority_updates: u64,
    pub high_priority_zones_mapped: u64,
    pub time_to_map_first_high_risk: Option<u64>,
    pub threat_level_over_time: Vec<f64>,
    pub zone_observations: u64,
    pub coverage_over_time: Vec<f64>,
    pub grid_state: Option<GridState>,
    pub inspection_state: Option<InspectionState>,
    pub availability_per_tick: Vec<f64>,
    pub disconnected_agents_max: u64,
    pub relay_reallocation_ticks: Option<u64>,
    pub relay_detection_tick: Option<u64>,
    pub total_hop_count_sum: f64,
    pub total_hop_count_ticks: u64,
    pub base_id: AgentId,
    pub base_pose: Pose,
}
```

Добавить `TickLoopState::new(config: &RunConfig, scenario: &Scenario, allocator: A) -> Self`
для инициализации всех полей из текущего initialization block `run_internal`
(строки 43–168).

Зарегистрировать в `runner/internal/mod.rs`:
```rust
mod loop_state;
pub(in crate::runner) use loop_state::*;
```

### 2.2 Перенести tick loop в `runner/internal/tick_loop.rs`

`tick_loop.rs` уже существует (55 строк вспомогательных функций).
Добавить:

```rust
pub(in crate::runner) fn run_tick_loop<A: Allocator>(
    state: &mut TickLoopState<A>,
    scenario: &Scenario,
    config: &RunConfig,
) {
    for _ in 0..config.max_ticks {
        // ... тело tick loop из run_internal строки 170–503 ...
    }
}
```

Тело функции использует `state.nodes`, `state.bus`, `state.clock` и т.д.
вместо локальных переменных.

### 2.3 Обновить `scenario_runner_internal.rs`

После 2.1–2.2 `run_internal` сводится к:
```rust
pub(super) fn run_internal<A: Allocator>(
    scenario: &Scenario,
    config: RunConfig,
    allocator: A,
    log_builder: Option<swarm_replay::EventLogBuilder>,
) -> (RunMetrics, Option<swarm_replay::EventLog>) {
    if config.urban_search_state.is_some() { ... }
    if config.urban_state.is_some() { ... }

    let (urban_route_planned, urban_route_length_m,
         urban_route_risk_score, urban_violation_count) =
        compute_urban_foundation_metrics(&config.urban_state.clone());
    let urban_route_completed = false;

    let mut state = TickLoopState::new(&config, scenario, allocator);
    state.log_builder = log_builder;

    run_tick_loop(&mut state, scenario, &config);

    let msgs_attempted = state.bus.borrow().messages_attempted();
    let msgs_dropped   = state.bus.borrow().messages_dropped();
    let bytes_sent     = state.bus.borrow().bytes_sent();
    drop(state.bus);

    // Recompute final conditions (строки 505–546)
    let all_expected_failures_detected = ...;
    let all_tasks_assigned = ...;
    let (success, unsupported_reason) = compute_mission_success(...);

    assemble_final_metrics(MetricsInput {
        nodes: state.nodes,
        crashed_agents: state.crashed_agents,
        // ...etc...
    })
}
```

Если файл сокращается до < 50 строк — инлайнить в
`scenario_runner_public.rs` и удалить.

**Верификация:**
```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim runner
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim
```

---

## Шаг 3 — Разбить `scenario_runner_urban.rs` на доменные модули

Целевая раскладка в `crates/swarm-sim/src/runner/`:

```
urban_patrol.rs     — pub(super) fn run_urban_patrol(...)
urban_search.rs     — pub(super) fn run_urban_search(...)
urban_events.rs     — функции эмиссии urban replay events
urban_metrics.rs    — финальная сборка urban RunMetrics
```

`urban_helpers.rs` (585 строк) перераспределяется:
- функции, используемые только patrol → `urban_patrol.rs` или `urban_events.rs`
- функции, используемые только search → `urban_search.rs`
- общие вспомогательные → остаются в `urban_helpers.rs` (который сокращается)

Шаги:
1. Создать `urban_events.rs` — перенести все функции эмиссии событий
   (`emit_*`, `push_urban_*`) из urban_helpers.rs.
2. Создать `urban_metrics.rs` — перенести финальную сборку urban RunMetrics
   (аналог MetricsInput для urban).
3. Разделить `scenario_runner_urban.rs` на `urban_patrol.rs` и `urban_search.rs`.
4. Обновить `runner/mod.rs`.

**Верификация:**
```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim
```

---

## Шаг 4 — Разбить `benchmark.rs` на директорию

Цель: `benchmark.rs` (907 строк) → директория `benchmark/`.

Целевая раскладка в `crates/swarm-sim/src/benchmark/`:

```
mod.rs          — pub use report::*; pub use harness::*; pub use aggregation::*;
report.rs       — ComparisonReport, BenchmarkResult, BenchmarkManifest и их impl;
                  убрать огромный Display (перенести в render.rs)
harness.rs      — BenchmarkHarness, BenchmarkOptions, run_with_strategy,
                  generate_benchmark_run_id
render.rs       — impl Display for ComparisonReport (Markdown-таблица),
                  generate_focused_report (перенести из export_formats.rs)
aggregation.rs  — merged_benchmark_run_id, утилиты агрегации
```

Шаги:
1. Создать директорию `crates/swarm-sim/src/benchmark/`.
2. Перенести содержимое по модулям.
3. В `crates/swarm-sim/src/lib.rs` заменить `mod benchmark; pub use benchmark::*;`
   — Rust автоматически подберёт `benchmark/mod.rs`.

**Верификация:**
```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --all-targets
```

---

## Шаг 5 — Разбить `report_export/export_formats.rs`

Целевая раскладка в `crates/swarm-sim/src/report_export/`:

```
json.rs      — export_json()
csv.rs       — export_csv()
markdown.rs  — export_markdown() + generate_focused_report()
              (generate_focused_report уже упомянут в шаге 4 — согласовать)
```

Шаги:
1. Создать `json.rs`, `csv.rs`, `markdown.rs`.
2. Удалить `export_formats.rs`.
3. Обновить `report_export/mod.rs`: убрать `mod export_formats`,
   добавить `mod json; mod csv; mod markdown;` и нужные `pub use`.

**Верификация:**
```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim report_export
```

---

## Шаг 6 — Разбить `dsl/types.rs`

Целевая раскладка в `crates/swarm-sim/src/dsl/`:

```
types.rs        — ScenarioSuite, ScenarioSuiteEntry, ValidationError и
                  их базовые impl (без IO, без валидации)
load.rs         — load_scenario_suite(), export_entry(), export_suite()
validate.rs     — validate_scenario_suite(), validate_entry(),
                  validate_mission_specific(), push_urban_state_error(),
                  validate_urban_start_pose(), mission_allows_task_kind()
```

Шаги:
1. Создать `load.rs` и `validate.rs`.
2. Оставить `types.rs` с чистыми типами.
3. Обновить `dsl/mod.rs`:
   ```rust
   mod types;
   pub use types::*;
   mod load;
   pub use load::*;
   mod validate;
   pub use validate::*;
   ```

**Верификация:**
```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim dsl
```

---

## Шаг 7 — Mission registry в `strategy_comparison_runtime`

**Цель:** добавление новой миссии больше не требует редактирования
большого `match mission` в `runs.rs`.

Ввести `MissionDescriptor`:
```rust
pub(super) struct MissionDescriptor {
    pub name: &'static str,
    pub profile_names: fn() -> Vec<String>,
    pub builder: fn() -> ScenarioBuilder,
}
```

Создать `crates/swarm-examples/src/strategy_comparison_runtime/missions.rs`:
```rust
pub(super) fn all_missions() -> &'static [MissionDescriptor] {
    &[
        MissionDescriptor { name: "coverage", ... },
        MissionDescriptor { name: "sar", ... },
        // ...
    ]
}
```

В `runs.rs` main loop заменить `match mission { ... }` на вызов
`missions::all_missions().find(|m| m.name == mission_name(mission))`.

**Верификация:**
```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --all-targets
```

---

## Шаг 8 — Низкий приоритет: крупные плоские файлы swarm-examples

Выполнять **только попутно** при касании файлов в ходе M70+.

| Файл | Целевое разбиение |
|------|-------------------|
| `sitl_plan.rs` (788 строк) | `sitl_plan/types.rs` + `sitl_plan/load.rs` + `sitl_plan/build.rs` |
| `sitl_safety.rs` (800 строк) | `sitl_safety/types.rs` + `sitl_safety/validate.rs` + `sitl_safety/load.rs` |
| `sitl_multi_agent.rs` (584 строк) | `sitl_multi_agent/manifest.rs` + `sitl_multi_agent/config.rs` + `sitl_multi_agent/ownership.rs` |

---

## Шаг 9 — Финальная верификация

После всех шагов:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-sim
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-replay
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test -p swarm-examples --all-targets
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
  /home/formi/.local/bin/runlim cargo test --workspace
```

Убедиться:
- `scenario_runner_internal.rs` удалён или < 50 строк.
- Нет `_and_` файлов в production-коде.
- Нет `#[path]`, нет `#![allow(unused_imports)]`.
- `--all-features` компилируется без ошибок и предупреждений.

# Testing strategy

## 1. Тесты без рефакторинга (запустить вместе с каждым шагом)

- После шага 1: `cargo build --all-features -p swarm-examples`.
- После шага 2:
  `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test -p swarm-sim runner`
  + все тесты swarm-sim.
- После шагов 3–7: соответствующий crate-level test scope.
- После каждого шага: `cargo clippy --all-targets -- -D warnings`.

## 2. Тесты с лёгким рефакторингом

- После шага 2 (TickLoopState): unit-тест `TickLoopState::new(...)` с
  minimal RunConfig → проверить корректную инициализацию полей.
- После шага 3 (urban split): добавить unit-тесты для `urban_metrics.rs`
  если они дают additional coverage по urban RunMetrics.
- После шага 4 (benchmark split): import smoke-тесты для
  `swarm_sim::benchmark::*` публичных путей.
- После шага 6 (dsl split): тесты validate.rs с известными ошибочными
  сценариями (missing mission, wrong task kind).
- После шага 7 (mission registry): тест `all_missions()` возвращает ожидаемый
  набор дескрипторов; все binaries smoke-test с `--help` эквивалентом.

## 3. Тесты с тяжёлым рефакторингом

- Если добавлять `swarm-sitl` crate — полные integration тесты через
  новую crate boundary.
- Property tests над generated TickLoopState если понадобится.

Gap: CLI-тесты sitl_supervisor без PX4 невозможны автоматически; шаг 1
покрывается только компиляцией с `--all-features`.

# Risks and tradeoffs

- **TickLoopState<A>** (шаг 2): generic struct с `Rc<RefCell<InMemNetwork>>`.
  Если Rust выведет lifetime-проблемы из cross-borrow — возможно потребуется
  `Rc` → `Arc` или реструктуризация. Тест после каждого подшага.

- **Tick loop в `tick_loop.rs`**: функция `run_tick_loop` принимает
  `&mut TickLoopState<A>` и `&Scenario` / `&RunConfig`. Это все owned refs —
  lifetime-проблем быть не должно.

- **Benchmark split**: `ComparisonReport` используется в `swarm-examples`.
  После split нужно убедиться, что все pub use в `benchmark/mod.rs`
  сохраняют обратную совместимость старых путей вида
  `swarm_sim::ComparisonReport`.

- **export_formats.rs**: `generate_focused_report` может дублироваться
  с `benchmark/render.rs` (шаг 4). Согласовать при выполнении обоих шагов.

- **Объём diff**: шаги 2–4 затрагивают много файлов. Каждый шаг — отдельный
  коммит с полным тест-прогоном.

# Open questions

- **TickLoopState<A: Allocator>**: все поля owned (нет lifetimes), `Rc` не
  требует `Send`. Если `run_tick_loop` реализован как standalone fn
  (не метод) — generics чистые. Вопрос открыт: делать `new()` конструктор
  или просто `Default` + заполнение в `run_internal`?

- **Судьба swarm-examples**: ревью 2 и 3 согласны — это де-факто
  production/integration crate. Когда переименовывать в `swarm-sitl` или
  `swarm-cli` — зависит от появления железа (M73+). Пока не срочно.

- **generate_focused_report**: сейчас в `report_export/export_formats.rs`.
  Логически относится к benchmark rendering. При выполнении шагов 4 и 5
  нужно решить: оставить в `report_export/markdown.rs` или перенести в
  `benchmark/render.rs`.

- **urban_helpers.rs после шага 3**: может стать маленьким (~50 строк) и
  тогда его содержимое логично инлайнить прямо в `urban_patrol.rs` /
  `urban_search.rs`. Или оставить как shared helpers — решить при реализации.

- **Порядок шагов 4 и 5**: шаг 5 зависит от решения по `generate_focused_report`
  из шага 4. Рекомендуется делать шаг 4 раньше.
