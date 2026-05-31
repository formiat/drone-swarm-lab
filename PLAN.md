# Context

Планируется M67 - Urban Replay / Analysis + Multi-Agent Prep.

Исходная линия взята из `docs_raw/DRONE_C.21.md`: после M63-M66 проект
движется как mission-level simulation, planning, coordination, replay and
metrics layer. M67 не должен превращаться в визуализатор, физический симулятор,
low-level flight control или полноценную политику avoidance. Его задача -
сделать Urban Patrol/Search runs диагностируемыми из текстовых артефактов и
подготовить минимальную базу для будущих multi-agent Urban сценариев.

Текущее состояние по локальному коду:

- M64 Urban Foundations уже дал `UrbanMap`, road graph, deterministic Dijkstra,
  AABB static obstacle judge, `urban-patrol` DSL и базовые Urban metrics.
- M65 Urban Patrol уже выполняет один ordered road-graph loop, пишет
  `UrbanRoutePlanned`, `UrbanSegmentEntered`, `UrbanSegmentCompleted`,
  `UrbanViolation`, `UrbanPatrolCompleted` и pose updates.
- M66 Urban Search уже добавил `urban-search`, mocked bus detector,
  `BusObserved`, `BusDetected`, `BusFalsePositive`, `UrbanSearchCompleted`,
  focused report metrics и smoke regression coverage.
- В `replay` CLI сейчас есть `--summary`, `--tick`, `--follow` и
  `--sitl-summary`; `--timeline`, `--agent` и `--category urban` пока нет.
- `UrbanViolation` в simulation-level event log сейчас содержит reason, pose,
  segment/edge, но не сохраняет structured `obstacle_id`, хотя
  `swarm_types::UrbanViolation::ObstacleIntersection` этот id уже имеет.
- Runner сейчас выбирает одного alive agent для Urban Patrol/Search; M67 должен
  добавить измерительную multi-agent подготовку, но не обязан менять semantics
  Urban Patrol/Search на полноценное multi-agent execution policy.

Не делать в M67:

- GUI, Bevy, egui, browser viewer.
- Full traffic simulator.
- Complex avoidance/deconfliction policy.
- Реальный lidar/raycast, CV, SLAM, физику или PX4/SITL export.
- 500/1000-seed benchmark refresh и новые PX4/SIH прогоны.

# Investigation context

`INVESTIGATION.md` в workspace отсутствует, поэтому входного расследования для
M67 нет.

Прочитаны обязательные протоколы:

- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`

Notion policy для запуска - `optional`. В пользовательском prompt нет Notion
task id, поэтому Notion CLI не использовался. GitLab/MR в prompt не запрошены,
поэтому `glab` не использовался. Удаленные SSH/HTTP обращения не выполнялись.

Локальный контекст, на котором основан план:

- `docs_raw/DRONE_C.21.md`
- `README.md`
- `docs/STATUS.md`
- `docs/REPLAY.md`
- `docs/SCENARIO_DSL.md`
- `docs/EXTENSION_GUIDE.md`
- `crates/swarm-replay/src/event_log.rs`
- `crates/swarm-replay/src/replay.rs`
- `crates/swarm-examples/src/bin/replay.rs`
- `crates/swarm-examples/tests/replay_cli.rs`
- `crates/swarm-examples/tests/sitl_docs.rs`
- `crates/swarm-sim/src/urban.rs`
- `crates/swarm-sim/src/runner.rs`
- `crates/swarm-sim/src/report_export.rs`
- `crates/swarm-metrics/src/metrics.rs`
- `crates/swarm-scenarios/src/urban.rs`
- `crates/swarm-sim/tests/scenario_catalog.rs`
- `scenarios/urban.patrol.json`
- `scenarios/urban.search.json`

# Affected components

- `crates/swarm-types/src/urban.rs` - при необходимости добавить reusable
  analysis/report structs только если они должны жить в shared type crate. По
  умолчанию M67 лучше держать analysis artifacts вне domain model, чтобы не
  раздувать core Urban map types.
- `crates/swarm-replay/src/event_log.rs` - additive replay field для
  `UrbanViolation.obstacle_id` с serde default, если judge report должен
  восстанавливаться из replay log без доступа к исходному `UrbanMap`.
- `crates/swarm-replay/src/replay.rs` - shared deterministic timeline formatter,
  event categorization, filters by `agent_id` and `category`.
- `crates/swarm-replay/src/lib.rs` - re-export новых timeline API.
- `crates/swarm-examples/src/bin/replay.rs` - CLI flags `--timeline`,
  `--agent <id>`, `--category urban`; корректные conflict checks с
  `--sitl-summary`.
- `crates/swarm-sim/src/urban_analysis.rs` - новый модуль для route trace,
  judge report, Urban event counts, separation metrics и route conflict
  representation. Это лучше, чем добавлять analysis-only код в `urban.rs`.
- `crates/swarm-sim/src/lib.rs` - re-export M67 analysis helpers.
- `crates/swarm-sim/src/runner.rs` - сохранить structured `obstacle_id` в
  `UrbanViolation` event, при необходимости возвращать/передавать route trace
  metadata рядом с `EventLog`.
- `crates/swarm-sim/src/benchmark.rs` - если artifact export будет встроен в
  benchmark pack, передавать replay logs and generated artifact paths без
  nondeterministic ordering.
- `crates/swarm-examples/src/bin/strategy_comparison.rs` - artifact export CLI:
  либо писать Urban trace artifacts рядом с `--output-dir` / `replay_logs`,
  либо добавить явный флаг вроде `--urban-analysis-dir`.
- `crates/swarm-sim/src/report_export.rs` - JSON/CSV/Markdown focused reports:
  route trace path if artifact is written, urban event counts in summary, новые
  separation/conflict aggregate fields.
- `crates/swarm-metrics/src/metrics.rs` - per-run and aggregate metrics:
  minimum separation, separation violation count, route conflict count, optional
  event counters if они становятся metrics-level.
- `crates/swarm-scenarios/src/urban.rs` - two-agent Urban fixture/profile.
- `scenarios/urban.multi-agent.json` - committed portable two-agent fixture for
  analysis/prep.
- `crates/swarm-sim/tests/scenario_catalog.rs` - загрузка и проверка нового
  scenario suite.
- `crates/swarm-examples/tests/replay_cli.rs` - CLI tests for timeline/filtering.
- `crates/swarm-examples/tests/sitl_docs.rs` - docs smoke tests for M67 status,
  replay docs and limitations.
- `README.md` - обновить статус M67, команды, limitations.
- `docs/STATUS.md` - обновить Last audit, milestone status, readiness and known
  limitations.
- `docs/REPLAY.md` - задокументировать timeline CLI, category filter, route
  trace/judge artifacts and schema compatibility.
- `docs/SCENARIO_DSL.md` - задокументировать two-agent Urban fixture and clarify
  that it measures conflicts but does not implement avoidance.
- `docs/EXTENSION_GUIDE.md` - добавить M67 guidance for replay/timeline/analysis
  artifacts and schema-compatible event additions.
- `docs/BENCHMARK_RESULTS.md` - не обновлять benchmark claims, но при наличии
  новых report fields пояснить, что M67 не является benchmark refresh.

# Implementation steps

1. Зафиксировать M67 data contracts.
   - Добавить `crates/swarm-sim/src/urban_analysis.rs`.
   - Описать serializable structs:
     - `UrbanRouteTrace`;
     - `UrbanAgentRouteTrace`;
     - `UrbanTraceSegment`;
     - `UrbanSegmentStatus`;
     - `UrbanPoseTracePoint`;
     - `UrbanJudgeReport`;
     - `UrbanJudgeViolationRecord`;
     - `UrbanRouteConflict`;
     - `UrbanSeparationSummary`;
     - `UrbanEventCounts`.
   - Держать schema additive and text-artifact friendly: stable field names in
     snake_case, deterministic ordering by `(agent_id, tick, segment_index)`.
   - Для CSV сделать narrow table для segments/poses/judge violations, а не
     пытаться сплющить весь nested JSON в одну строку.

2. Добавить route trace builder from replay logs.
   - Реализовать helper в `crates/swarm-sim/src/urban_analysis.rs`, который
     принимает `&swarm_replay::EventLog`.
   - Planned route брать из `UrbanRoutePlanned` and `UrbanSegmentEntered`
     events: edge ids, from/to, segment indexes, route length.
   - Executed route строить из `UrbanSegmentEntered`,
     `UrbanSegmentCompleted`, `PoseUpdated`, `UrbanPatrolCompleted`,
     `UrbanSearchCompleted`.
   - Per-segment status:
     - `planned`;
     - `entered`;
     - `completed`;
     - `violated`;
     - `not_completed`.
   - Pose trace писать per-agent from `PoseUpdated`. Если trace size станет
     проблемой, добавить deterministic cap в artifact writer, но не менять
     event log behavior.
   - Убедиться, что builder корректно обрабатывает Urban Patrol success,
     Urban Search detection, timeout/no-detection and violation paths.

3. Добавить structured judge report.
   - В `crates/swarm-replay/src/event_log.rs` расширить
     `Event::UrbanViolation` полем:
     `obstacle_id: Option<UrbanObstacleId>` с `#[serde(default)]` и
     `skip_serializing_if = "Option::is_none"`.
   - В `crates/swarm-sim/src/runner.rs` в `push_urban_violation_event`
     переносить obstacle id из
     `swarm_types::UrbanViolation::ObstacleIntersection`.
   - В `crates/swarm-sim/src/urban_analysis.rs` строить
     `UrbanJudgeReport` from replay events with:
     violation type/reason, agent id, tick, point/pose, segment index, edge id,
     obstacle id.
   - Backward compatibility: старые logs без `obstacle_id` должны
     десериализоваться, а judge report должен иметь `obstacle_id = null`.

4. Реализовать replay timeline API.
   - В `crates/swarm-replay/src/replay.rs` добавить:
     - `ReplayEventCategory` with at least `Urban` and `Generic`;
     - `ReplayTimelineFilter { agent_id: Option<AgentId>, category: Option<...> }`;
     - `ReplayTimelineItem`;
     - `format_timeline(log, filter) -> String` или iterator + formatter.
   - Timeline должен быть deterministic:
     - сохранять исходный order events внутри одинакового tick;
     - включать tick, agent id where applicable, event name, concise details;
     - не использовать HashMap iteration order в выводе.
   - Category `urban` должна покрывать:
     - `UrbanRoutePlanned`;
     - `UrbanSegmentEntered`;
     - `UrbanSegmentCompleted`;
     - `UrbanViolation`;
     - `UrbanPatrolCompleted`;
     - `BusObserved`;
     - `BusDetected`;
     - `BusFalsePositive`;
     - `UrbanSearchCompleted`;
     - optional `PoseUpdated` только когда фильтр `--category urban` нужен для
       route trace debugging; если это сделает output шумным, оставить
       `PoseUpdated` в generic и явно документировать.

5. Расширить replay CLI.
   - В `crates/swarm-examples/src/bin/replay.rs` добавить parsing:
     - `--timeline`;
     - `--agent <id>`;
     - `--category urban`.
   - `--agent` и `--category` должны работать только с replay log mode, не с
     `--sitl-summary`.
   - Разрешить комбинировать `--timeline --agent agent-0 --category urban`.
   - При отсутствии action обновить usage: `--summary`, `--tick`, `--follow`,
     `--timeline`.
   - Для неизвестной category возвращать non-zero exit с понятной ошибкой.

6. Добавить artifact writers.
   - В `crates/swarm-sim/src/urban_analysis.rs` добавить:
     - `write_urban_route_trace_json`;
     - `write_urban_route_trace_csv`;
     - `write_urban_judge_report_json`;
     - `write_urban_judge_report_csv`.
   - Не писать файлы из core runner автоматически. Runner должен оставаться
     pure simulation path.
   - В `crates/swarm-examples/src/bin/strategy_comparison.rs` добавить
     интеграцию на CLI/output уровне:
     - если есть `--output-dir` and replay logs contain Urban events, писать
       artifacts в `<output-dir>/urban_analysis/`;
     - либо добавить explicit `--urban-analysis-dir <dir>`.
   - Практичный вариант для M67: автоматическая запись в `--output-dir` только
     when replay logs are already enabled or generated for output pack. Если
     `--output-dir` без replay logs, не пересчитывать runs второй раз.

7. Добавить route trace path and event counts в reports.
   - В `crates/swarm-sim/src/report_export.rs` расширить focused report for
     `urban-patrol` and `urban-search`:
     - `UrbanEventCounts`;
     - `RouteTraceArtifacts` / route trace path where available.
   - Если `ComparisonReport` не подходит для per-run artifact paths, не ломать
     его модель: добавить отдельный `urban_analysis_manifest.json` в output
     pack и ссылку на этот manifest в docs/README. В report table оставить
     aggregate event counts and conflict/separation metrics.
   - В `docs/BENCHMARK_RESULTS.md` явно не называть M67 artifacts benchmark
     refresh.

8. Добавить two-agent Urban fixture.
   - В `crates/swarm-scenarios/src/urban.rs` добавить profile
     `MultiAgentSmallBlock` or separate builder
     `build_urban_multi_agent_scenario`.
   - Добавить `scenarios/urban.multi-agent.json`.
   - Fixture должен быть portable and deterministic:
     - два alive scout agents;
     - same road graph or disjoint/offset route loop;
     - controlled start nodes/poses;
     - enough ticks for traces;
     - no avoidance policy claim.
   - В M67 runner semantics можно оставить one-agent Urban Patrol/Search.
     Multi-agent fixture нужен для analysis helpers, separation/conflict
     measurement and future M68+ work. Если меняется runner, scope должен быть
     только "emit/analyze traces for agents", not "solve deconfliction".

9. Добавить separation metrics and route conflict representation.
   - В `crates/swarm-sim/src/urban_analysis.rs` реализовать deterministic
     analyzer over `UrbanRouteTrace`:
     - min distance between agents by tick;
     - count of ticks below threshold;
     - route conflict records with `agent_a`, `agent_b`, `tick`,
       `distance_m`, `edge_id/segment_index` if known.
   - В `crates/swarm-metrics/src/metrics.rs` добавить per-run and aggregate
     fields:
     - `urban_min_agent_separation_m`;
     - `urban_separation_violation_count`;
     - `urban_route_conflict_count`.
   - Если metrics cannot be derived from ordinary one-agent runs, default them
     to `0` / `None` via serde defaults and document that they are meaningful
     only for multi-agent Urban analysis artifacts.

10. Обновить documentation.
    - `README.md`:
      - добавить M67 в current status/milestone table;
      - показать команды для `replay --timeline`;
      - показать где лежат route trace/judge artifacts;
      - сохранить limitation: no GUI, no complex avoidance, no real lidar/CV.
    - `docs/STATUS.md`:
      - Last audit = M67;
      - отметить M67 as debugging/analysis milestone;
      - уточнить, что multi-agent prep измеряет conflicts, но не решает их.
    - `docs/REPLAY.md`:
      - timeline CLI;
      - filters;
      - route trace and judge report artifact schema;
      - backward compatibility for `UrbanViolation.obstacle_id`.
    - `docs/SCENARIO_DSL.md`:
      - `urban.multi-agent.json`;
      - meaning of two-agent fixture;
      - no avoidance semantics yet.
    - `docs/EXTENSION_GUIDE.md`:
      - how to add replay timeline details;
      - how to add analysis artifacts without breaking event schema.
    - `docs/BENCHMARK_RESULTS.md`:
      - no new benchmark evidence;
      - M67 artifacts are diagnostic evidence, not benchmark refresh.

11. Запланированные команды проверки и прогоны.
    - Форматирование:
      - `cargo fmt --all`
    - Компиляция affected crates:
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo check -p swarm-replay -p swarm-sim -p swarm-scenarios -p swarm-examples`
    - Targeted unit/integration tests:
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay timeline`
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay event_log`
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban_analysis`
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim report_export`
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-scenarios urban`
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test replay_cli`
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`
    - Smoke run with artifacts, if implementation adds CLI artifact export:
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo run -p swarm-examples --bin strategy_comparison -- --smoke --mission urban-patrol --output-dir target/m67_urban_patrol_smoke --replay-log target/m67_urban_patrol_replay`
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo run -p swarm-examples --bin replay -- --log target/m67_urban_patrol_replay/<generated>.replay.json --timeline --category urban`
    - Lint:
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo clippy --all-targets -- -D warnings`
    - Не планировать M67 как 500/1000-seed benchmark milestone. Если smoke run
      превышает разумное время или требует generated filename discovery, заменить
      его integration test over in-memory/tempdir fixtures.

# Testing strategy

## 1. Tests that need no refactoring - планировать вместе с основной реализацией

- `crates/swarm-replay/src/replay.rs`:
  - timeline formatter prints deterministic lines for an inline event fixture;
  - timeline preserves event order within identical ticks;
  - `ReplayTimelineFilter` filters by `agent_id`;
  - `ReplayTimelineFilter` filters by `urban` category;
  - unknown/empty category handling is tested at CLI layer.
- `crates/swarm-replay/src/event_log.rs`:
  - old `UrbanViolation` JSON without `obstacle_id` deserializes;
  - new `UrbanViolation` with `obstacle_id` round-trips;
  - schema remains backward-compatible without requiring migration of old logs.
- `crates/swarm-examples/tests/replay_cli.rs`:
  - `replay --log <file> --timeline` succeeds;
  - `replay --log <file> --timeline --agent agent-0` excludes other agents;
  - `replay --log <file> --timeline --category urban` includes Urban events;
  - `replay --sitl-summary <file> --timeline` exits non-zero;
  - unknown category exits non-zero with actionable stderr.
- `crates/swarm-sim/src/urban_analysis.rs`:
  - route trace JSON includes planned route, executed route, per-segment status;
  - route trace CSV header test;
  - pose trace includes deterministic tick/agent/pose rows;
  - judge report JSON serializes violation type, point, segment, obstacle id,
    tick and agent id;
  - judge report handles old logs with missing obstacle id;
  - Urban event counts match a known M65/M66 fixture;
  - two-agent separation measurement fixture computes min separation and
    violation count;
  - route conflict representation is deterministic and sorted.
- `crates/swarm-scenarios/src/urban.rs`:
  - two-agent Urban profile builds a valid scenario and `RunConfig`;
  - profile list includes the new fixture if it is public;
  - generated route loop remains plannable.
- `crates/swarm-sim/tests/scenario_catalog.rs`:
  - `scenarios/urban.multi-agent.json` loads and validates.
- `crates/swarm-metrics/src/metrics.rs`:
  - aggregate metrics include min separation/conflict fields with serde defaults;
  - old metrics JSON without M67 fields deserializes.
- `crates/swarm-sim/src/report_export.rs`:
  - JSON/CSV headers include new aggregate fields if added to metrics;
  - focused Urban report mentions event counts and/or analysis manifest path.
- `crates/swarm-examples/tests/sitl_docs.rs`:
  - README/status/replay/scenario docs mention M67 artifacts and limitations;
  - docs do not claim GUI, lidar, real perception, complex avoidance, PX4/SITL
    export, or hardware readiness for M67.

Coverage expectations:

- Happy path: successful Urban Patrol/Search replay produces timeline and trace.
- Negative path: judge violation produces judge report and failed status.
- Edge cases: old logs without `schema_version` / without `obstacle_id`,
  one-agent logs for separation metrics, empty event log, unknown replay
  category.

## 2. Tests that need light refactoring

- Extract compact Urban replay fixture builder from
  `crates/swarm-examples/tests/replay_cli.rs` or duplicate it as a small shared
  helper in test modules only if cross-crate sharing would add dependency
  friction.
- Extract Urban summary/assertion helper in `crates/swarm-sim` tests to avoid
  brittle string-only checks for event counts.
- Add a shared route trace fixture builder in `crates/swarm-sim/src/urban_analysis.rs`
  tests for Patrol success, Search detected, Search timeout and Judge violation.
- If `strategy_comparison` artifact export is tested, factor file discovery into
  a helper so tests do not depend on machine-specific temp paths or glob order.

## 3. Tests that need heavy refactoring

- Versioned replay schema migration tests covering multiple historical schema
  snapshots. M67 should only add focused backward-compatibility tests for the
  additive `UrbanViolation.obstacle_id` field.
- Large replay performance tests for long trace files. M67 should avoid this
  unless trace size becomes a measured problem.
- Cross-run replay diff tooling. Useful later, but not necessary for M67.
- Multi-agent deconfliction property tests. M67 only measures route/separation
  conflicts; it should not assert an avoidance policy that does not exist yet.
- Full multi-agent Urban runner semantics. If later needed, that should become a
  separate milestone after M67 analysis artifacts are stable.

# Risks and tradeoffs

- Replay schema compatibility: adding `obstacle_id` to `UrbanViolation` is safe
  only if the field has serde default and old logs are covered by tests.
- Artifact ownership: route trace and judge reports are analysis artifacts, not
  core runner state. Keeping file writes in CLI/output layer avoids surprising
  side effects in `ScenarioRunner::run_with_log`.
- Output size: pose trace can grow with ticks and agents. M67 should start with
  small deterministic fixtures and document/cap trace export if needed.
- Timeline stability: tests should assert semantic lines, but avoid making every
  whitespace choice part of an accidental public API.
- Multi-agent expectations: a two-agent fixture and conflict metrics can be
  misunderstood as avoidance/deconfliction. README/status/docs must say this is
  measurement/prep only.
- Metrics defaults: adding fields to `RunMetrics`/`AggregateMetrics` requires
  serde defaults so old JSON result packs continue to parse.
- Report shape: `ComparisonReport` is aggregate-oriented, while route trace
  paths are per-run artifacts. Prefer a separate analysis manifest if embedding
  per-run paths would distort the report model.
- Performance/resource risk: no long benchmark or PX4 run is required for M67.
  All planned checks should be portable and runnable locally; if a smoke command
  gets slow, replace it with tempdir integration tests.

# Open questions

1. Should Urban analysis artifacts be written automatically under
   `--output-dir` when replay logs exist, or should M67 add an explicit
   `--urban-analysis-dir` flag?
2. Should `PoseUpdated` be included in `replay --category urban` timeline by
   default, or only in route trace artifacts to keep timeline readable?
3. Should `obstacle_id` live directly in `Event::UrbanViolation`, or should it
   only be present in `UrbanJudgeReport` generated at runtime before the replay
   log loses typed violation details?
4. Should the two-agent fixture be public in `UrbanStandardProfiles`, or only a
   scenario file and test fixture until M68 decides actual multi-agent Urban
   semantics?
5. Should route conflict metrics use a fixed threshold from scenario config, a
   hardcoded M67 default, or an argument to the analysis helper?
6. Should M67 update `docs/BENCHMARK_RESULTS.md` only with a "no benchmark
   refresh" note, or leave it untouched because no benchmark evidence changes?
