# DRONE_B.10 — Итоговый линейный план после DRONE_B.8

Дата фиксации: 2026-05-23

## Источники

Синтез `DRONE_B.9.branches.md`, `DRONE_B.9.linear.md` и `DRONE_A.9.linear.md`.

`DRONE_A.9` точнее диагностировал текущее состояние — обнаружил конкретные баги:

- `scenarios/inspection.*.json` содержат `Infinity` — `serde_json` не принимает,
  `--scenario-suite` падает с `expected value at line 26 column 26`;
- `sitl_agent --mock` отправляет 0 waypoints, потому что coverage/SAR задачи без `pose`;
- Safety Layer не интегрирован в аллокаторы — `filter_safe_tasks` нигде не вызывается.

Итоговый план берёт структуру M18–M23 из `DRONE_A.9.linear` как основу.

## Линейный план

```
M18 Platform Consolidation
M19 Schema / Validation Hardening
M20 Reproducible Benchmark Pack
M21 Benchmark Report / Analysis
M22 Replay / Debuggability
M23 Release Candidate / Golden Path
```

Этот план не требует выбора стратегического направления. Он укрепляет общий
фундамент, полезный для всех трёх веток: research, platform, real-world SITL.

---

## M18 — Platform Consolidation

Цель: убрать текущие шероховатости, при которых часть пользовательских entrypoint-ов
не работает.

Что сделать:

1. Починить `scenarios/inspection.*.json`:
   - заменить `Infinity` на конечный `max_range`;
   - проверить `inspection.linear`, `inspection.perimeter`, `inspection.random`.

2. Добавить тест загрузки всего scenario catalog:
   - пройти по `scenarios/*.json`;
   - каждый файл должен грузиться через `load_scenario_suite`;
   - ошибка указывает конкретный файл.

3. Добавить `scenarios/sitl.waypoints.json`:
   - 1 агент, 2–3 задачи с `pose`;
   - минимальный `RunConfig`;
   - пригоден для `sitl_agent --mock`.

4. Починить `sitl_agent --mock`:
   - реально отправлять waypoints на pose-задачах;
   - если задач с `pose` нет — понятное предупреждение, не молчаливый 0.

5. Интегрировать Safety Layer в аллокаторы:
   - `filter_safe_tasks` вызывается в Greedy, CBBA, Auction, Centralized,
     ConnectivityAware;
   - integration-тест: coverage с no-fly зоной — ни одна задача внутри зоны
     не назначается.

6. Добавить CLI smoke tests:
   - `strategy_comparison --scenario-suite scenarios/coverage.safety.json`;
   - `strategy_comparison --scenario-suite scenarios/sar.uncertain.json`;
   - `strategy_comparison --scenario-suite scenarios/cbba_stress.json`;
   - `strategy_comparison --scenario-suite scenarios/inspection.linear.json`;
   - `sitl_agent --mock --scenario scenarios/sitl.waypoints.json` → ненулевое число waypoints.

Done criteria:

- все `scenarios/*.json` грузятся без ошибок;
- smoke CLI runs проходят;
- `sitl_agent --mock` отправляет waypoints.

---

## M19 — Schema / Validation Hardening

Цель: сделать DSL нормальным пользовательским контрактом с понятными ошибками.

Что сделать:

1. Добавить `"schema_version": "0.1"` в DSL.

2. Добавить validation layer:
   - `validate_scenario_suite` / `validate_entry`;
   - typed validation errors (не panic).

3. Проверять обязательные поля: `mission`, `profile`, `scenario.name`, `agents`,
   `tasks`, `run_config`.

4. Mission-specific constraints:
   - inspection: задачи должны иметь `edge_id`;
   - SAR: нужен `grid_state`;
   - SITL waypoint: должны быть задачи с `pose`;
   - CBBA stress: `enable_cbba` должен быть `true`.

5. Заменить panics в CLI на человекочитаемые ошибки: неверный JSON, неизвестная
   миссия, пустой suite.

6. Документировать DSL формат: `docs/SCENARIO_DSL.md`.

Done criteria:

- invalid scenario tests покрывают основные ошибки;
- CLI возвращает понятные сообщения без backtrace;
- README/docs описывают DSL как стабильный контракт v0.1.

---

## M20 — Reproducible Benchmark Pack

Цель: один CLI запуск создаёт self-contained output directory, привязанный к
git commit и scenario suite.

Что сделать:

1. Разделить режимы запуска: smoke (CI), quick (локальная проверка), full (publishable).

2. Единый output directory:
   - `results.json`;
   - `results.csv`;
   - `manifest.json` (timestamp, git commit, command line, suite name, seed range,
     strategy list, schema version);
   - `scenario_snapshot.json`.

3. Стабильные команды для полного прогона существующих миссий:
   SAR v2, CBBA stress, Infrastructure Inspection, Safety coverage.

Done criteria:

- `strategy_comparison ... --output-dir results/` создаёт ожидаемые файлы;
- output можно привязать к конкретному git commit;
- smoke/quick/full режимы явно различаются.

---

## M21 — Benchmark Report / Analysis

Цель: превратить существующие метрики в технический результат с выводами.

Что прогнать: SAR v2, CBBA stress, Infrastructure Inspection, Safety coverage.

Вопросы, на которые должен ответить отчёт:

- Где CBBA выигрывает, где проигрывает?
- Насколько SAR v2 содержательнее SAR v1?
- Какие стратегии лучше для inspection route coverage?
- Какой overhead даёт distributed consensus?
- Где safety constraints ухудшают allocation?

Что написать:

- `docs/BENCHMARK_RESULTS.md` с таблицами и интерпретацией;
- README summary table;
- команды воспроизведения.

Done criteria:

- `docs/BENCHMARK_RESULTS.md` существует с реальными числами и выводами;
- README не просто перечисляет фичи, а показывает результаты.

---

## M22 — Replay / Debuggability

Цель: сделать странные benchmark outcomes объяснимыми без ручного чтения JSON.

Что сделать:

1. Стабилизировать event log schema (version, event types).

2. CLI утилита `replay`:
   - `cargo run --bin replay -- --log run.jsonl`;
   - summary: число ticks, assignments, conflicts, failures, safety violations,
     SAR detections, inspection edge coverage, CBBA convergence markers;
   - поддержка всех типов миссий.

3. Тесты: replay roundtrip для всех типов событий; summary не паникует на любых
   логах.

Это текстовый инспектор, не UI. Visualization (egui/Bevy) — следующий этап.

Done criteria:

- для любого сценария можно получить replay summary одной командой;
- странный benchmark result можно начать разбирать без debugger-а.

---

## M23 — Release Candidate / Golden Path

Цель: зафиксировать проект как цельный runnable research prototype с чёткой
границей возможностей.

Что сделать:

1. Golden path в README: clone → `cargo test --workspace` → smoke benchmark →
   scenario suite → inspect output → mock SITL → read benchmark results.

2. Docs:
   - `docs/SCENARIO_DSL.md`;
   - `docs/BENCHMARK_RESULTS.md`;
   - `docs/REPLAY.md`;
   - обновить `docs/SITL_SETUP.md`.

3. Честные non-goals:
   - не production flight-control system;
   - не сертифицированный safety layer;
   - PX4 integration — experimental scaffold.

Done criteria:

- новый пользователь проходит golden path без чтения исходников;
- текущий статус честно описан;
- все основные сценарии запускаются через documented commands.

---

## Тестовая стратегия

### Категория 1 — без рефакторинга (реализовать вместе с основными изменениями)

- загрузка всех `scenarios/*.json`;
- CLI smoke для `--scenario-suite`;
- `sitl_agent --mock` на waypoint suite;
- validation errors для типовых плохих входных данных;
- safety integration тест (no-fly зона блокирует аллокацию);
- benchmark pack создаёт ожидаемые файлы;
- replay summary smoke test.

### Категория 2 — лёгкий рефакторинг

- typed validation error tests;
- run manifest deterministic field tests;
- replay summary consistency tests;
- CLI tests с temporary output directories;
- schema version compatibility tests.

### Категория 3 — тяжёлый рефакторинг (backlog, не блокирует M18–M23)

- полноценный replay UI (egui / Bevy);
- real PX4 integration tests;
- property-based validation всего DSL schema;
- long-running full benchmark CI;
- hardware-in-the-loop tests.
