# M61 - Platform / API Stabilization

## Context

Нужно запланировать M61 из `docs_raw/DRONE_A.19.md`: после закрытия M57-M60 зафиксировать понятные extension points проекта, не превращая это в обещание стабильного публичного API, semver или публикацию crates.

Текущий локальный контекст:

- `docs_raw/DRONE_A.19.md` ставит M61 после supervisor/SIH hardening и явно ограничивает scope: документация extension path, crate boundaries, schema version policy и один минимальный test-only extension path.
- В `README.md` и `docs/STATUS.md` уже описаны M57-M60, но M61 еще отсутствует.
- `swarm-types` уже содержит `TaskKind`, `MissionAdapter`, `RunState` и `AdapterRegistry`.
- `swarm-alloc` уже содержит `Allocator`, `Strategy` и `StrategyRegistry`.
- `swarm-metrics` уже содержит `RunMetrics` и `AggregateMetrics`.
- `swarm-sim` уже содержит DSL validation, `ScenarioRunner::run_with_log`, report export и schema-version проверки.
- `swarm-replay` уже содержит replay schema `0.2`; SITL event log/report schemas живут в `swarm-examples`.

M61 должен оформить существующую архитектуру как практический guide для будущих изменений, но не должен создавать новый production contract. После реализации формулировки в публичных документах должны оставаться осторожными: "stable-ish extension points", "internal APIs may change", "not semver-stable public API".

## Investigation context

`INVESTIGATION.md` в корне репозитория отсутствует, поэтому отдельных investigation findings нет.

Что проверено для планирования:

- прочитан `docs_raw/DRONE_A.19.md`, включая секцию M61;
- проверено, что `PLAN.md` до начала работы отсутствовал;
- просмотрены текущие документы `README.md`, `docs/STATUS.md`, `docs/SCENARIO_DSL.md`, `docs/REPLAY.md`, `docs/SITL_SETUP.md`;
- просмотрены релевантные Rust-модули:
  - `crates/swarm-types/src/mission.rs`;
  - `crates/swarm-types/src/adapter.rs`;
  - `crates/swarm-alloc/src/allocator.rs`;
  - `crates/swarm-alloc/src/strategy.rs`;
  - `crates/swarm-metrics/src/metrics.rs`;
  - `crates/swarm-sim/src/runner.rs`;
  - `crates/swarm-examples/tests/sitl_docs.rs`.

Notion/GitLab не читались: в prompt нет Notion task id, GitLab MR id или требования читать внешние задачи/MR; `notion_policy=optional`.

## Affected components

- `docs/EXTENSION_GUIDE.md` - новый основной документ M61.
- `README.md` - добавить M61 в milestone/status секции, список docs и краткое описание extension boundaries.
- `docs/STATUS.md` - добавить статус M61 после реализации и синхронизировать recommended next steps.
- `docs/SCENARIO_DSL.md` - добавить ссылку на extension guide и явную политику изменения scenario schema.
- `docs/REPLAY.md` - добавить ссылку на extension guide и политику для новых replay/SITL events.
- `docs/SITL_SETUP.md` - точечно добавить cross-link, если extension guide будет ссылаться на SITL report/event schemas.
- `crates/swarm-examples/tests/sitl_docs.rs` - расширить docs smoke на `docs/EXTENSION_GUIDE.md`, README/status links и обязательные M61 формулировки.
- `crates/swarm-types/src/adapter.rs` или `crates/swarm-types/src/mission.rs` - добавить test-only minimal mission adapter/fixture, если удобнее держать это рядом с `MissionAdapter`.
- `crates/swarm-alloc/src/strategy.rs` - добавить test-only custom strategy registry smoke, если он нужен для проверки strategy extension path без CLI subprocess.
- `crates/swarm-sim/src/runner.rs` - добавить runner/replay smoke для минимального fixture, если это можно сделать без нового публичного API и без хрупких таймингов.
- `crates/swarm-sim/src/report_export.rs` или существующие tests рядом с `benchmark.rs` - добавить/расширить метрики/export smoke только если текущие тесты не покрывают описанный metric extension path.

## Implementation steps

1. Создать `docs/EXTENSION_GUIDE.md`.
   Документ должен быть практическим checklist, а не абстрактным design doc. Обязательные разделы:
   - scope and non-goals;
   - crate boundaries;
   - how to add a mission;
   - how to add a strategy;
   - how to add a metric;
   - schema version policy;
   - regression/replay/report checklist;
   - support matrix / unsupported behavior checklist;
   - test checklist.

2. В `docs/EXTENSION_GUIDE.md` описать добавление новой миссии.
   Минимальный путь:
   - выбрать или добавить `TaskKind` в `crates/swarm-types/src/task.rs`;
   - реализовать `MissionAdapter` из `crates/swarm-types/src/mission.rs`;
   - добавить adapter wiring в `AdapterRegistry` в `crates/swarm-types/src/adapter.rs`;
   - добавить scenario builder в `crates/swarm-scenarios/src/...`;
   - добавить scenario JSON/DSL в `scenarios/...`;
   - определить completion semantics через `RunState` и/или mission-specific runtime state;
   - добавить mission-specific metrics в `RunMetrics`/`AggregateMetrics`, если нужны;
   - добавить replay events только если existing generic events недостаточны;
   - добавить regression smoke или явно отметить mission/strategy combination как unsupported.

3. В `docs/EXTENSION_GUIDE.md` описать добавление новой стратегии.
   Минимальный путь:
   - реализовать `Allocator` из `crates/swarm-alloc/src/allocator.rs`;
   - при необходимости реализовать `allocate_with_adapter`, `allocate_with_registry`, `allocate_with_connectivity`, `allocation_metrics`, `is_distributed`;
   - реализовать `Strategy` и зарегистрировать в `StrategyRegistry`;
   - подключить CLI/benchmark matrix там, где strategy должна быть видна пользователю;
   - обновить support matrix и regression thresholds;
   - добавить benchmark/regression coverage.

4. В `docs/EXTENSION_GUIDE.md` описать добавление новой метрики.
   Минимальный путь:
   - добавить поле в `RunMetrics`;
   - добавить default/backward-compatible serde behavior;
   - агрегировать в `AggregateMetrics`, если метрика участвует в report/thresholds;
   - протащить значение в runner/benchmark;
   - обновить JSON/CSV/Markdown exports в `crates/swarm-sim/src/report_export.rs` и/или `benchmark.rs`;
   - добавить docs/tests, включая CSV header/table assertions.

5. В `docs/EXTENSION_GUIDE.md` явно разделить crate boundaries.
   Предлагаемая классификация:
   - stable-ish extension points: `swarm-types` traits/types, `swarm-alloc::Allocator`/`Strategy`, scenario DSL surface, metrics structs, report/replay schemas;
   - semi-internal: `swarm-sim` runner/config/report helpers, которые можно использовать внутри workspace, но не обещать как published API;
   - internal/experimental: `swarm-examples` SITL binaries, supervisor controller internals, MAVLink transport details, manual PX4/SIH workflows;
   - not for external use: test-only helpers, concrete supervisor state machine internals, undocumented report fields.

6. В `docs/EXTENSION_GUIDE.md` описать schema version policy.
   Обязательные схемы:
   - scenario DSL: `docs/SCENARIO_DSL.md`, текущая версия `0.1`, required `schema_version`;
   - simulation replay: `docs/REPLAY.md`, текущая версия `0.2`, backward-compatible default for legacy logs;
   - SITL event log: `sitl_event_log.v1`;
   - SITL reports: `sitl_run_report.v1`, `sitl_multi_agent_run_report.v1`;
   - multi-agent SITL config/manifest: `multi_sitl.v1`, `multi_sitl_manifest.v1`;
   - benchmark/report exports: documented fields must stay additive or require explicit compatibility note.

7. Обновить `README.md`.
   Нужно добавить:
   - M61 в current status/milestones overview;
   - ссылку на `docs/EXTENSION_GUIDE.md` в docs table;
   - короткое предупреждение, что extension guide не означает semver-stable public API;
   - краткую подсказку, где искать mission/strategy/metrics extension points.

8. Обновить `docs/STATUS.md`.
   Нужно добавить M61 как planned/complete после реализации:
   - "Complete" только если guide, README/docs sync и тесты реально добавлены;
   - явно указать, что M61 не публикует crates и не закрывает hardware/production readiness;
   - recommended next steps должны указывать на следующий milestone, а не на уже закрытый M61.

9. Обновить сопутствующие docs.
   Минимум:
   - `docs/SCENARIO_DSL.md`: добавить link на extension guide и правила, когда менять `schema_version`;
   - `docs/REPLAY.md`: добавить link на extension guide и правила добавления новых event types/schema fields;
   - `docs/SITL_SETUP.md`: добавить link только в разделе SITL schemas/reporting, чтобы не раздувать setup doc.

10. Добавить docs smoke в `crates/swarm-examples/tests/sitl_docs.rs`.
    Тест должен подключить `docs/EXTENSION_GUIDE.md` через `include_str!` и проверять обязательные строки:
    - `TaskKind`;
    - `MissionAdapter`;
    - `StrategyRegistry`;
    - `RunMetrics`;
    - `AggregateMetrics`;
    - `schema_version`;
    - `sitl_event_log.v1`;
    - `sitl_run_report.v1`;
    - `not semver-stable` или эквивалентную осторожную формулировку;
    - ссылки из README/status docs на `docs/EXTENSION_GUIDE.md`.

11. Добавить test-only minimal mission adapter path.
    Предпочтительный вариант без refactoring:
    - в `crates/swarm-types/src/adapter.rs` добавить test-only adapter, который реализует `MissionAdapter` поверх существующего `TaskKind::Waypoint` или `TaskKind::CoverageCell`;
    - проверить `task_kind`, `route_cost`, `is_completed` через `RunState`, `score`;
    - не добавлять новый real `TaskKind` и не менять production registry, если это не нужно.

12. Добавить runner/replay smoke where practical.
    Предпочтительный вариант:
    - в `crates/swarm-sim/src/runner.rs` добавить tiny in-memory scenario с `TaskKind::Waypoint` или `TaskKind::CoverageCell`;
    - запустить `ScenarioRunner::run_with_log` с existing allocator;
    - assert: metrics не пустые/успешные по ожидаемой semantics, event log создан, содержит assign/start/complete path where deterministic;
    - если runner timing делает test хрупким, оставить только adapter test как required M61 test и явно записать runner/replay gap в `docs/EXTENSION_GUIDE.md` и `docs/STATUS.md`.

13. Добавить strategy extension smoke, если это можно сделать без CLI subprocess.
    В `crates/swarm-alloc/src/strategy.rs` добавить test-only fake strategy:
    - реализует `Allocator` и `Strategy`;
    - регистрируется в `StrategyRegistry::new()`;
    - возвращает stable `name()`/`description()`;
    - подтверждает, что registry принимает внешнюю boxed strategy без изменения built-in default registry.

14. Проверить metric extension coverage.
    Если текущие тесты report/export уже проверяют JSON/CSV/Markdown headers и aggregate fields, не добавлять хрупкий новый test. Если gap есть, добавить маленький unit test в `crates/swarm-sim/src/report_export.rs` или рядом с существующими benchmark export tests:
    - минимальный `AggregateMetrics`;
    - экспорт в Markdown/CSV;
    - assert, что новая/описанная метрика явно появляется в output.
    M61 не должен добавлять новую реальную метрику ради самого milestone.

15. Синхронизировать все docs формулировки.
    После кода и тестов проверить, что:
    - README, `docs/STATUS.md`, `docs/SCENARIO_DSL.md`, `docs/REPLAY.md`, `docs/SITL_SETUP.md` не противоречат друг другу;
    - M61 не обещает real hardware readiness;
    - M61 не обещает external semver API;
    - docs называют test-only fixture именно test-only, а не новой поддерживаемой миссией.

16. Запустить verification commands и убрать побочные артефакты.
    Для реализации M61 ожидаемый набор:
    - `cargo fmt --all`;
    - `cargo clippy --workspace --all-targets --all-features -- -D warnings` или repo-approved equivalent, если полный clippy реалистично проходит в текущем workspace;
    - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`;
    - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-types adapter`;
    - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-alloc strategy`;
    - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim runner`;
    - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim dsl`;
    - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay event_log`;
    - `git diff --check`;
    - `find . -name '*.proptest-regressions' -print` и удалить/не коммитить такие файлы, если появились.

## Testing strategy

### 1. Tests that need no refactoring - планируются вместе с основной реализацией

- Docs smoke в `crates/swarm-examples/tests/sitl_docs.rs`:
  - happy path: `docs/EXTENSION_GUIDE.md` существует и содержит mission/strategy/metrics/schema/crate-boundary sections;
  - negative path: отсутствие обязательной строки ломает тест;
  - edge case: README/status должны ссылаться на guide и не обещать semver-stable API.
- Minimal adapter unit test в `crates/swarm-types/src/adapter.rs` или `mission.rs`:
  - happy path: test-only adapter correctly reports `TaskKind`, completion, route cost, score;
  - negative path: `RunState` без completed task не завершает task;
  - edge case: task без optional mission-specific fields не panic'ует.
- Strategy registry smoke в `crates/swarm-alloc/src/strategy.rs`:
  - happy path: custom boxed strategy can be registered and iterated;
  - negative path: empty registry remains valid and does not imply default strategies;
  - edge case: custom strategy with no assignments compiles through `Allocator`/`Strategy` contract.
- Existing schema version tests:
  - `crates/swarm-sim/src/dsl.rs` for scenario schema `0.1`;
  - `crates/swarm-replay/src/event_log.rs` for replay schema `0.2`;
  - `crates/swarm-examples/src/sitl_observability.rs` and `sitl_report.rs` for SITL schemas.
  Если они уже достаточно покрывают schema presence, M61 только добавляет docs assertions, а не дублирует identical tests.

### 2. Tests that need light refactoring

- Runner/replay extension fixture in `crates/swarm-sim/src/runner.rs`:
  - tiny in-memory scenario through `ScenarioRunner::run_with_log`;
  - assert event log and metrics path;
  - может потребовать небольшой reusable test helper для scenario/agent/task construction.
- Metric export smoke in `crates/swarm-sim/src/report_export.rs` or benchmark tests:
  - minimal aggregate/report fixture;
  - JSON/CSV/Markdown output assertions;
  - может потребовать helper, чтобы не собирать большой benchmark result вручную.
- Schema version validation helper:
  - общий docs/test helper для проверки, что guide mentions the same schema strings as constants/docs;
  - стоит делать только если это уменьшит дублирование, а не добавит абстракцию ради одного теста.

### 3. Tests that need heavy refactoring

- External strategy harness:
  - настоящий out-of-crate plugin-like example сейчас не нужен, потому что проект не обещает published API;
  - потребует отдельного crate/example и четкой политики dependencies.
- Cross-version schema compatibility tests:
  - полноценная матрица старых/new fixtures для scenario/replay/report schemas;
  - полезно позже, когда появится стабильная versioning policy и реальные schema migrations.
- End-to-end "new mission" harness:
  - новая mission от `TaskKind` до CLI benchmark matrix и regression baseline;
  - это уже scope отдельного milestone, не M61, потому что M61 не должен добавлять новую real mission.

Manual checks в M61 допустимы только как дополнение:

- визуально проверить читаемость `docs/EXTENSION_GUIDE.md`;
- проверить, что links в README/docs корректны;
- не запускать live PX4/SIH и не делать benchmark sweeps, потому что M61 про extension docs/tests, а не runtime evidence.

## Risks and tradeoffs

- Документ может переобещать API stability. Нужно использовать осторожные формулировки и явно отделить stable-ish extension points от internal crates.
- Test-only adapter может выглядеть как новая mission support. Нужно назвать его fixture/test-only и не регистрировать как production mission.
- Docs smoke на строки может стать слишком хрупким. Проверять только ключевые contract terms, не точные абзацы.
- Runner/replay smoke может стать nondeterministic, если завязан на movement/ticks. Держать fixture минимальным, in-memory и без внешних файлов; при хрупкости не форсировать runner test в M61.
- Metric extension docs могут устареть быстрее, чем код. Нужен docs test на ключевые paths и README/status links.
- Full clippy по workspace может выявить старый unrelated debt. Если такое случится, зафиксировать failure отдельно и дополнительно прогнать targeted tests для M61 changes.
- Нельзя менять `.agent-io/*`, target artifacts или proptest persistence files в коммите.

## Open questions

- Нужно ли в M61 добавлять отдельный `docs/API_BOUNDARIES.md`, или достаточно раздела в `docs/EXTENSION_GUIDE.md`? Предпочтение: один guide, чтобы не плодить документы.
- Достаточно ли test-only adapter + docs smoke для done criteria, если runner/replay fixture окажется хрупким? Предпочтение: да, но gap должен быть явно записан в status/docs.
- Должен ли M61 добавить crate-level Rust docs для `MissionAdapter`/`Strategy`, или пока достаточно markdown guide и tests? Предпочтение: markdown guide сейчас, Rust docs только точечно если обнаружится явный пробел.
- Нужно ли включать `docs/EXTENSION_GUIDE.md` в какие-то generated docs или site pipeline? В текущем репозитории такого pipeline не видно, поэтому достаточно README/docs links.
