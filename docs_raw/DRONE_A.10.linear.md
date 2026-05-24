# DRONE_A.10 — Итоговый линейный план после сравнения A.9 / B.9

Дата фиксации: 2026-05-23

## Короткий вывод

После сравнения `DRONE_A.9.branches.md`, `DRONE_A.9.linear.md`, `DRONE_B.9.branches.md` и `DRONE_B.9.linear.md` лучший следующий roadmap — линейный, без выбора стратегической ветки.

Причина:

> сейчас в проекте уже много реализованных вертикалей, но главная ценность появится только после интеграции, валидации, воспроизводимого benchmark workflow и понятного golden path.

Итоговый маршрут:

```text
M18 Integration & Scenario Catalog
-> M19 DSL Schema / Validation
-> M20 SITL Path Consolidation
-> M21 Reproducible Benchmark Pack
-> M22 Benchmark Report / Analysis
-> M23 Replay / Debuggability
-> M24 Release Candidate / Golden Path
```

Этот план не требует выбирать между:

- research / publishable benchmark;
- platform / productization;
- real-world / SITL bridge.

Он укрепляет общий фундамент для всех трёх направлений.

## Что B.9 добавляет к A.9

`DRONE_B.9` правильно поднимает приоритет интеграции.

Сейчас многие части уже реализованы:

- Safety Layer;
- SAR v2;
- CBBA robustness;
- Infrastructure Inspection;
- Mission DSL;
- MAVLink / SITL scaffold;
- benchmark export;
- replay infrastructure.

Но ценность этих частей появляется только когда они связаны end-to-end.

`DRONE_B.9` также правильно выделяет Visualization / Replay UI как полезный следующий слой после стабилизации: сейчас поведение агентов видно в основном через CSV/JSON/Markdown, а не глазами.

## Что A.9 уточняет относительно B.9

`DRONE_A.9` точнее фиксирует конкретные текущие gaps:

- `scenarios/inspection.*.json` уже есть, но часть файлов не грузится из-за `Infinity`;
- `sitl_agent --connection` парсится, но реальный `MavlinkTransport` фактически не используется;
- нужен тест на весь `scenarios/*.json`;
- нужен валидный waypoint-сценарий для mock SITL;
- нужен schema/validation layer до больших benchmark/report работ.

Также B.9 не совсем точен про Safety Layer:

- `filter_safe_tasks` не вызывается внутри каждого allocator-а;
- но в `ScenarioRunner` уже есть `SafetyAllocator` wrapper;
- значит задача не "с нуля интегрировать safety в аллокаторы", а достроить и проверить safety semantics на runner-level:
  - no-fly filtering;
  - geofence/separation checks;
  - `safety_violations`;
  - export;
  - тесты.

## M18 — Integration & Scenario Catalog Hardening

Цель:

> сделать существующие сценарии и пользовательские entrypoint-ы реально запускаемыми и проверяемыми.

Что сделать:

1. Починить `scenarios/inspection.*.json`:
   - убрать `Infinity`;
   - заменить на конечный `max_range`;
   - проверить `inspection.linear`;
   - проверить `inspection.perimeter`;
   - проверить `inspection.random`.

2. Добавить тест загрузки всего scenario catalog:
   - пройти по `scenarios/*.json`;
   - каждый файл должен грузиться через `load_scenario_suite`;
   - ошибка должна указывать конкретный файл.

3. Добавить smoke-run для ключевых suite:
   - `scenarios/coverage.safety.json`;
   - `scenarios/sar.uncertain.json`;
   - `scenarios/sar.noisy.json`;
   - `scenarios/cbba_stress.json`;
   - `scenarios/inspection.linear.json`.

4. Достроить safety integration tests через runner:
   - задачи в no-fly zone не назначаются;
   - `safety_violations` считаются;
   - JSON/CSV export содержит safety metric;
   - geofence/separation не паникуют и дают ожидаемые violations.

5. Добавить `scenarios/sitl.waypoints.json`:
   - 1 агент;
   - 2-3 задачи с `pose`;
   - минимальный `RunConfig`;
   - сценарий должен быть пригоден для `sitl_agent --mock`.

Done criteria:

- все `scenarios/*.json` валидны для `load_scenario_suite`;
- ключевые suite запускаются smoke-командами;
- safety behavior покрыт integration tests;
- `sitl.waypoints.json` загружается и содержит pose-задачи.

## M19 — DSL Schema / Validation

Цель:

> сделать DSL не просто serde-форматом, а нормальным пользовательским контрактом.

Что сделать:

1. Добавить `schema_version`:
   - например `"schema_version": "0.1"`;
   - определить поведение для legacy-файлов без версии.

2. Ввести validation API:
   - `validate_scenario_suite`;
   - `validate_entry`;
   - typed validation errors.

3. Проверять общие обязательные поля:
   - `mission`;
   - `profile`;
   - `scenario.name`;
   - agents;
   - tasks;
   - run_config.

4. Проверять mission-specific constraints:
   - SAR требует `grid_state`;
   - inspection требует `edge_id` у inspection tasks;
   - SITL waypoint scenario требует задачи с `pose`;
   - safety scenario требует валидный `safety_config`;
   - CBBA stress требует expected CBBA/network parameters.

5. Заменить panics в CLI на понятные ошибки:
   - неверный JSON;
   - неизвестная mission;
   - пустой suite;
   - suite без runnable entries;
   - невалидные mission-specific поля.

6. Документировать DSL:
   - структура suite;
   - минимальный пример;
   - mission-specific требования;
   - частые ошибки;
   - schema versioning policy.

Done criteria:

- invalid scenario tests покрывают основные ошибки;
- CLI возвращает человекочитаемые сообщения;
- README/docs описывают DSL как стабильный контракт v0.1.

## M20 — SITL Path Consolidation

Цель:

> честно довести SITL path до состояния "mock works, real PX4 path wired but experimental".

Что сделать:

1. `sitl_agent --mock`:
   - должен отправлять waypoints из `scenarios/sitl.waypoints.json`;
   - должен явно предупреждать, если найдено 0 pose-задач;
   - должен иметь тест на waypoint extraction.

2. `sitl_agent --connection`:
   - должен реально использовать `MavlinkTransport` при feature `mavlink-transport`;
   - без feature должен выдавать понятную ошибку;
   - не должен молча падать обратно в mock mode.

3. Разделить paths:
   - mock path;
   - real MAVLink path;
   - unsupported build without feature.

4. Обновить `docs/SITL_SETUP.md`:
   - mock mode;
   - real PX4 mode;
   - prerequisites;
   - known limitations;
   - troubleshooting.

5. Real PX4 прогон:
   - оставить manual / optional done criterion;
   - не блокировать весь roadmap на внешнем окружении.

Done criteria:

- `sitl_agent --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0` отправляет ненулевое число waypoints;
- `--connection` не игнорируется;
- feature-gated real path компилируется;
- документация честно отделяет mock от experimental PX4.

## M21 — Reproducible Benchmark Pack

Цель:

> сделать benchmark output воспроизводимым пакетом артефактов, а не набором разрозненных файлов.

Что сделать:

1. Разделить режимы:
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

5. Сделать стабильные команды для текущих миссий:
   - SAR v2;
   - CBBA stress;
   - Infrastructure Inspection;
   - Safety coverage.

6. Не добавлять новые миссии:
   - этот milestone стабилизирует текущие возможности.

Done criteria:

- один CLI запуск создаёт self-contained output directory;
- output можно привязать к git commit и scenario suite;
- smoke/quick/full режимы явно различаются.

## M22 — Benchmark Report / Analysis

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
- ссылка на generated artifacts или описание, где они лежат;
- краткие выводы по стратегиям.

Вопросы, на которые должен ответить отчёт:

- Где CBBA выигрывает?
- Где CBBA проигрывает?
- Насколько SAR v2 отличается от SAR v1 по содержательности метрик?
- Какие стратегии лучше для inspection route coverage?
- Какой overhead даёт distributed consensus?
- Где safety constraints ломают или ухудшают allocation?

Done criteria:

- есть воспроизводимый benchmark pack;
- есть документ с интерпретацией;
- README показывает результаты, а не только список фич.

## M23 — Replay / Debuggability

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

3. Добавить replay summary CLI:
   - ticks;
   - assignments;
   - conflicts;
   - failures;
   - safety violations;
   - SAR detections;
   - inspection edge coverage;
   - CBBA convergence markers.

4. Добавить минимальную ASCII/grid replay-визуализацию:
   - `--tick N`;
   - `--follow`;
   - agents;
   - task states;
   - basic SAR grid view;
   - basic inspection edge state if practical.

5. Полноценный egui/Bevy UI:
   - не делать в этом milestone;
   - оставить как отдельный будущий этап.

Done criteria:

- для любого нового сценария можно получить replay summary;
- странный benchmark result можно начать разбирать без debugger-а;
- есть минимальная текстовая визуализация.

## M24 — Release Candidate / Golden Path

Цель:

> зафиксировать проект как цельный runnable research prototype с понятной границей возможностей.

Что сделать:

1. README cleanup:
   - убрать устаревшие фрагменты;
   - обновить список milestones;
   - отделить simulation-only от experimental SITL;
   - описать current known limitations.

2. Документировать golden path:
   - clone;
   - `cargo test --workspace`;
   - run smoke benchmark;
   - run scenario suite;
   - create benchmark pack;
   - inspect benchmark output;
   - run replay summary;
   - run mock SITL.

3. Добавить / обновить docs:
   - `docs/SCENARIO_DSL.md`;
   - `docs/BENCHMARK_RESULTS.md`;
   - `docs/REPLAY.md`;
   - `docs/SITL_SETUP.md`.

4. Зафиксировать статус feature areas:
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

Done criteria:

- новый пользователь может пройти golden path без чтения исходников;
- текущий статус проекта честно описан;
- все основные сценарии запускаются documented commands.

## Тестовая стратегия

### Категория 1 — без рефакторинга

Запланировать вместе с основными изменениями:

- загрузка всех `scenarios/*.json`;
- CLI smoke для `--scenario-suite`;
- safety integration tests через runner;
- CSV/JSON schema checks;
- `sitl_agent --mock` на waypoint suite;
- validation errors для типовых плохих сценариев;
- benchmark pack creates expected files;
- replay summary smoke test.

### Категория 2 — лёгкий рефакторинг

Добавить по мере стабилизации API:

- typed validation error tests;
- run manifest deterministic field tests;
- CLI tests с temporary output directories;
- schema version compatibility tests;
- replay summary consistency tests;
- feature-gated SITL path tests.

### Категория 3 — тяжёлый рефакторинг

Не блокировать M18-M24, но держать в backlog:

- полноценный replay UI;
- real PX4 integration tests;
- property-based validation всего DSL schema;
- long-running full benchmark CI;
- hardware-in-the-loop tests.

## Итог

Этот линейный план закрывает главный риск текущего состояния:

> фич много, но нужен цельный, проверяемый, воспроизводимый workflow.

После M24 выбор стратегического направления станет дешевле:

- research path получит воспроизводимые benchmark artifacts и report;
- platform path получит schema, validation, docs и replay tooling;
- SITL path получит честный mock/real split и понятный experimental PX4 contour.
