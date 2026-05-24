# DRONE_A.9 — Линейный план без выбора стратегического направления

Дата фиксации: 2026-05-23

## Короткий вывод

Если сейчас не выбирать стратегическую ветку, лучший путь — сделать direction-agnostic линейный план.

Идея:

> не добавлять новые большие фичи, а довести уже сделанную широкую платформу до цельного, воспроизводимого и проверяемого состояния.

Такой план не требует выбирать между:

- research / publishable benchmark;
- platform / productization;
- real-world / SITL bridge.

Он укрепляет общий фундамент, полезный для всех трёх направлений.

## M18 — Platform Consolidation

Цель:

> убрать текущие шероховатости пользовательских entrypoint-ов и сделать существующие сценарии реально запускаемыми через DSL.

Что сделать:

1. Починить `scenarios/inspection.*.json`:
   - убрать `Infinity`;
   - заменить на конечный `max_range`;
   - проверить `inspection.linear`, `inspection.perimeter`, `inspection.random`.

2. Добавить тест загрузки всего scenario catalog:
   - пройти по `scenarios/*.json`;
   - каждый файл должен грузиться через `load_scenario_suite`;
   - ошибка должна указывать конкретный файл.

3. Добавить CLI smoke tests для существующих suite-файлов:
   - `strategy_comparison --scenario-suite scenarios/coverage.safety.json`;
   - `strategy_comparison --scenario-suite scenarios/sar.uncertain.json`;
   - `strategy_comparison --scenario-suite scenarios/cbba_stress.json`;
   - `strategy_comparison --scenario-suite scenarios/inspection.linear.json`.

4. Добавить валидный `scenarios/sitl.waypoints.json`:
   - 1 агент;
   - 2-3 задачи с `pose`;
   - минимальный `RunConfig`;
   - сценарий должен быть пригоден для `sitl_agent --mock`.

5. Починить / уточнить `sitl_agent --mock`:
   - он должен реально отправлять waypoints на pose-задачах;
   - если задач с `pose` нет, выводить понятное сообщение;
   - не считать "0 waypoints" успешной демонстрацией без явного предупреждения.

Критерий готовности:

- все `scenarios/*.json` валидны для `load_scenario_suite`;
- smoke CLI runs проходят;
- `sitl_agent --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0` отправляет ненулевое число waypoints.

## M19 — Schema / Validation Hardening

Цель:

> сделать DSL не просто serde-форматом, а нормальным пользовательским контрактом.

Что сделать:

1. Версионировать DSL schema:
   - добавить `"schema_version": "0.1"`;
   - поддержать отсутствие версии как legacy only, если нужно.

2. Добавить validation layer:
   - `validate_scenario_suite`;
   - `validate_entry`;
   - typed validation errors.

3. Проверять общие обязательные поля:
   - `mission`;
   - `profile`;
   - `scenario.name`;
   - `agents`;
   - `tasks`;
   - `run_config`.

4. Проверять mission-specific constraints:
   - inspection: задачи должны иметь `edge_id`, если scenario заявлен как inspection;
   - SAR: нужен `grid_state`;
   - SITL waypoint scenario: должны быть задачи с `pose`;
   - safety scenario: `safety_config` должен быть валиден;
   - CBBA stress: `enable_cbba` и сетевые параметры должны соответствовать ожиданиям stress-сценария.

5. Заменить panics в CLI на понятные ошибки:
   - неверный JSON;
   - неизвестная mission;
   - пустой suite;
   - suite без runnable entries.

6. Документировать формат scenario suite:
   - структура;
   - минимальный пример;
   - mission-specific требования;
   - частые ошибки.

Критерий готовности:

- invalid scenario tests покрывают основные ошибки;
- CLI возвращает человекочитаемые сообщения;
- README/docs описывают DSL как стабильный контракт v0.1.

## M20 — Reproducible Benchmark Pack

Цель:

> сделать benchmark output воспроизводимым пакетом артефактов, а не набором разрозненных файлов.

Что сделать:

1. Разделить режимы запуска:
   - smoke — минимально быстрый, для CI;
   - quick — локальная проверка;
   - full — publishable / long run.

2. Добавить единый output directory:
   - `results.json`;
   - `results.csv`;
   - `manifest.json`;
   - `scenario_snapshot.json`;
   - optional replay logs;
   - optional markdown table fragment.

3. Добавить run manifest:
   - timestamp;
   - git commit;
   - command line;
   - suite name;
   - schema version;
   - seed range;
   - strategy list;
   - metric schema version.

4. Сохранять config snapshot рядом с результатами:
   - чтобы benchmark можно было повторить без догадок;
   - чтобы README-таблицы можно было связать с конкретным input.

5. Сделать стабильные команды для полного прогона существующих миссий:
   - SAR v2;
   - CBBA stress;
   - Infrastructure Inspection;
   - Safety coverage.

6. Не добавлять новые миссии в этом milestone:
   - только стабилизировать текущие.

Критерий готовности:

- один CLI запуск создаёт self-contained output directory;
- output можно привязать к git commit и scenario suite;
- smoke/quick/full режимы явно различаются.

## M21 — Benchmark Report / Analysis

Цель:

> превратить существующие метрики в понятный технический результат.

Что прогнать:

- SAR v2;
- CBBA stress;
- Infrastructure Inspection;
- Safety coverage.

Какие таблицы собрать:

- success rate;
- task completion rate;
- PoD;
- belief entropy;
- false positive rate;
- confirmation scans;
- convergence p50/p95;
- bundle travel distance;
- edge coverage;
- missed edges;
- route efficiency;
- safety violations;
- communication cost.

Что написать:

- `docs/BENCHMARK_RESULTS.md`;
- README summary table;
- команды воспроизведения;
- ссылка на generated artifacts или описание где они лежат;
- краткие выводы по стратегиям.

Вопросы, на которые должен ответить отчёт:

- Где CBBA выигрывает?
- Где CBBA проигрывает?
- Насколько SAR v2 отличается от SAR v1 по содержательности метрик?
- Какие стратегии лучше для inspection route coverage?
- Какой overhead даёт distributed consensus?
- Где safety constraints ломают или ухудшают allocation?

Критерий готовности:

- есть воспроизводимый benchmark pack;
- есть документ с интерпретацией;
- README не просто перечисляет фичи, а показывает текущие результаты.

## M22 — Replay / Debuggability

Цель:

> сделать странные benchmark outcomes объяснимыми без ручного чтения огромного JSON.

Что сделать:

1. Проверить replay logs на новых сценариях:
   - SAR v2;
   - CBBA stress;
   - inspection;
   - safety.

2. Стабилизировать event log schema:
   - version;
   - event types;
   - backward compatibility note.

3. Добавить replay summary CLI или простой textual inspector:
   - число ticks;
   - assignments;
   - conflicts;
   - failures;
   - safety violations;
   - SAR detections;
   - inspection edge coverage;
   - CBBA convergence markers.

4. Добавить tests:
   - replay roundtrip для новых event types;
   - summary не паникует на logs разных миссий;
   - basic consistency checks.

5. Это ещё не UI:
   - цель milestone — debugging/tooling;
   - visualization можно оставить на следующий этап.

Критерий готовности:

- для любого нового сценария можно получить replay summary;
- странный benchmark result можно начать разбирать без debugger-а.

## M23 — Release Candidate / Golden Path

Цель:

> зафиксировать проект как цельный runnable research prototype с понятной границей возможностей.

Что сделать:

1. Пройтись по README:
   - убрать устаревшие фрагменты;
   - обновить список milestones;
   - отделить simulation-only от experimental SITL;
   - описать текущие known limitations.

2. Документировать golden path:
   - clone;
   - `cargo test --workspace`;
   - run smoke benchmark;
   - run scenario suite;
   - inspect output;
   - run mock SITL;
   - read benchmark results.

3. Добавить docs:
   - `docs/SCENARIO_DSL.md`;
   - `docs/BENCHMARK_RESULTS.md`;
   - `docs/REPLAY.md`;
   - обновить `docs/SITL_SETUP.md`.

4. Проверить статус всех feature areas:
   - M11 benchmark;
   - DSL;
   - safety;
   - SAR v2;
   - CBBA robustness;
   - inspection;
   - mock SITL;
   - real PX4 path experimental.

5. Зафиксировать non-goals:
   - не production flight-control system;
   - не сертифицированный safety layer;
   - не готовая система для реальных роевых полётов;
   - PX4 integration experimental.

Критерий готовности:

- новый пользователь может пройти golden path без чтения исходников;
- текущий статус проекта честно описан;
- все основные сценарии запускаются через documented commands.

## Почему этот план не требует выбора направления

Этот план укрепляет общий фундамент.

Он полезен для всех будущих веток:

- для research direction нужен воспроизводимый benchmark и интерпретация;
- для platform/productization нужна schema validation, docs, golden path;
- для real-world/SITL bridge нужны valid scenarios, safety constraints и честный SITL scaffold;
- для visualization нужны стабильные replay/report schemas.

Поэтому можно спокойно двигаться линейно до M23, не выбирая сейчас между тремя стратегическими направлениями.

## Тестовая стратегия

### Категория 1 — без рефакторинга

Запланировать вместе с основными изменениями:

- загрузка всех `scenarios/*.json`;
- CLI smoke для `--scenario-suite`;
- CSV/JSON schema checks;
- `sitl_agent --mock` на waypoint suite;
- validation errors для типовых плохих сценариев;
- export manifest smoke test;
- benchmark pack creates expected files;
- replay summary smoke test.

### Категория 2 — лёгкий рефакторинг

Добавить по мере стабилизации API:

- typed validation error tests;
- run manifest deterministic field tests;
- replay summary consistency tests;
- CLI tests с temporary output directories;
- schema version compatibility tests.

### Категория 3 — тяжёлый рефакторинг

Не блокировать M18-M23, но держать в backlog:

- полноценный replay UI;
- real PX4 integration tests;
- property-based validation всего DSL schema;
- long-running full benchmark CI;
- hardware-in-the-loop tests.

## Итоговый линейный roadmap

```text
M18 Platform Consolidation
-> M19 Schema / Validation Hardening
-> M20 Reproducible Benchmark Pack
-> M21 Benchmark Report / Analysis
-> M22 Replay / Debuggability
-> M23 Release Candidate / Golden Path
```

После M23 выбор направления будет проще и дешевле, потому что база станет чистой:

- все сценарии грузятся;
- отчёты стабильны;
- benchmark воспроизводим;
- SITL честно обозначен как mock/experimental;
- docs объясняют, что проект уже умеет и чего ещё не умеет.
