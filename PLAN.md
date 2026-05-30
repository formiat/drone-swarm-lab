# PLAN.md - M50 Mock Regression & Docs Hardening

## Context

M50 идет по дорожной карте `docs_raw/DRONE_A.17.md` в направлении "Ветка 6: Real SITL/PX4 integration". Предыдущие этапы уже сформировали базу: dry-run/mock режимы, MAVLink upload path, safety validation, lifecycle/execute, telemetry mapping, run-report и replay/observability.

Задача M50 не в том, чтобы расширять реальный PX4 workflow. Ее цель - закрепить portable SITL path как regression-safe и честно описанный: проверяемый без внешнего PX4, без аппаратуры, без локально установленного симулятора. Реальный PX4/SITL остается experimental/manual boundary, а mock/dry-run становятся дефолтным способом проверки reviewers/CI.

README тоже должен быть обновлен: сейчас в нем уже есть dry-run/mock/PX4 команды, но M50 должен явно сказать, какие SITL проверки автоматические и portable, а какие остаются ручными.

## Investigation Context

`INVESTIGATION.md` отсутствует.

Перед планированием были просмотрены:

- `.agent-io/inbox.txt`;
- `docs_raw/DRONE_A.17.md`;
- `README.md`;
- `docs/SITL_SETUP.md`;
- `crates/swarm-examples/tests/sitl_agent.rs`;
- `crates/swarm-examples/src/sitl_plan.rs`;
- `crates/swarm-examples/src/sitl_safety.rs`;
- `crates/swarm-examples/src/bin/regression_runner.rs`;
- `crates/swarm-examples/tests/regression.rs`;
- protocol docs for Notion/GitLab access.

Текущая кодовая база уже содержит полезное покрытие, но оно разрознено:

- `dry_run_outputs_mission_upload_plan` проверяет dry-run вывод;
- mock path умеет писать replay log и уже частично покрыт;
- safety fail path проверяется через unsafe scenario/config;
- valid safety config проверяется до feature gate в no-feature build;
- `sitl_plan.rs` уже имеет явные `load_sitl_suite`, `build_sitl_plan`, `format_dry_run_plan`;
- `sitl_safety.rs` уже имеет `validate_pre_upload_safety`;
- `regression_runner` сейчас относится к simulation/benchmark suite, а не к SITL CLI smoke.

Пробел M50: нет одного явного portable SITL regression smoke, который бы демонстрировал весь минимальный путь без PX4: scenario load -> waypoint extraction -> safety pass -> expected mission item count -> mock/dry-run success. Документация тоже должна жестче отделять automated/mock/dry-run от manual/PX4/hardware.

## Affected Components

- `crates/swarm-examples/tests/sitl_agent.rs` - основной кандидат для portable SITL smoke test, потому что там уже есть temp scenario fixtures и CLI runner helpers.
- `crates/swarm-examples/src/sitl_plan.rs` - менять только если потребуется небольшой тестовый helper или unit-level test для plan extraction.
- `crates/swarm-examples/src/sitl_safety.rs` - менять только если потребуется точечный unit/integration test для valid safety pass.
- `crates/swarm-examples/tests/sitl_docs.rs` - новый небольшой docs sanity test, если команда решит закрепить обязательные anchors документации автоматически.
- `docs/SITL_SETUP.md` - главный документ для mock/dry-run/PX4 boundary, warning и troubleshooting.
- `README.md` - короткое публичное описание portable SITL verification и границ PX4/manual режима.
- `crates/swarm-examples/src/bin/regression_runner.rs` - не менять по умолчанию; добавлять SITL suite туда только если получится без смешивания benchmark regression и CLI smoke.

## Implementation Steps

1. Добавить consolidated portable SITL smoke.

   В `crates/swarm-examples/tests/sitl_agent.rs` добавить тест уровня `portable_sitl_regression_smoke` или близкое имя. Он должен использовать temp scenario fixture и проверять весь минимальный путь без внешнего PX4:

   - scenario file загружается CLI или library path;
   - `--dry-run` завершается успешно;
   - stdout содержит suite/scenario identity, coordinate frame, altitude source, ожидаемые waypoint/task identifiers;
   - mission plan содержит ожидаемое число mission items/waypoints для fixture scenario;
   - `--mock` завершается успешно;
   - mock stderr/stdout сообщает, что отправлены ожидаемые waypoints;
   - `--mock --replay-log <tempfile>` пишет replay log/summary с ожидаемыми счетчиками.

2. Закрепить safety validation pass без PX4.

   Предпочтительно добавить прямую проверку через `load_sitl_suite`/`build_sitl_plan`/`validate_pre_upload_safety`, чтобы тест не зависел от cfg вокруг `mavlink-transport`.

   Проверить:

   - valid safety config passes;
   - planned waypoints count equals fixture expectation;
   - waypoint ids/task ids/altitudes соответствуют scenario fixture;
   - тест не открывает сеть, не запускает PX4 и не требует локальных файлов вне tempdir.

3. Не дублировать уже существующие негативные тесты без необходимости.

   Перед добавлением negative coverage перечитать существующие тесты в `sitl_agent.rs`. Если уже есть проверки bad safety config, missing scenario, unsupported connection, no-feature connection gate, их нужно оставить как есть и не размазывать M50. Если отсутствует компактный regression case для "scenario loads but has no pose tasks", можно добавить его как отдельный edge test.

4. Добавить docs sanity test, если он не станет слишком хрупким.

   Создать `crates/swarm-examples/tests/sitl_docs.rs` с `include_str!` на repo-local docs:

   - `README.md`;
   - `docs/SITL_SETUP.md`.

   Проверять стабильные anchors, а не длинные фразы:

   - `--dry-run`;
   - `--mock`;
   - `--connection`;
   - `mavlink-transport`;
   - `PX4 SITL`;
   - `Real Hardware Warning`;
   - `CI / Manual Boundary` или аналогичный заголовок;
   - troubleshooting section.

   Такой тест должен быть self-contained и portable: никаких абсолютных путей, `$HOME`, локального PX4 или discovery по файловой системе.

5. Обновить `docs/SITL_SETUP.md`.

   Документ должен явно разделять режимы:

   - dry-run: automated/default, no transport, no external simulator;
   - mock: automated/default, replay/report/log friendly, no external simulator;
   - PX4 SITL: manual/local, requires `mavlink-transport` feature and external simulator setup;
   - real hardware: не поддерживается как production workflow, только explicit warning.

   Добавить секцию `CI / Manual Boundary` или эквивалентную:

   - automated/CI: dry-run CLI, mock CLI, waypoint extraction, safety validation, replay summary smoke;
   - manual/local PX4: upload-only, execute lifecycle, telemetry observation, timeout tuning;
   - out of scope: HIL, real aircraft safety guarantees, production autopilot certification, real PX4 CI.

   Troubleshooting должен отдельно покрывать:

   - command works in mock but fails with PX4;
   - missing `mavlink-transport`;
   - connection string issues;
   - safety validation failures before upload;
   - coordinate frame/altitude assumptions.

6. Обновить `README.md`.

   В README нужно добавить короткий portable verification path:

   - команда для SITL smoke test без PX4;
   - команда для docs sanity test, если он добавлен;
   - явное указание, что PX4/SITL и hardware не являются CI default.

   В status/quick-start sections стоит отразить M50:

   - mock/dry-run now regression-safe baseline;
   - real PX4 remains experimental/manual;
   - reviewers can validate SITL foundation without external simulator.

7. Не добавлять SITL в `regression_runner` по умолчанию.

   Текущий `regression_runner` - это benchmark/simulation metrics harness. M50 про CLI/SITL smoke. Смешивать эти области стоит только если появится чистый portable suite group без PX4 и без long-run expectations. Базовая рекомендация: M50 закрепляется обычными integration tests, а не benchmark regression suite.

8. Обновить план/документацию только в пределах M50.

   Не добавлять real PX4 CI, HIL, hardware support, calibration realism и новые алгоритмы. Это out of scope.

9. Проверить результат и закоммитить.

   Если были Rust test/code изменения:

   - `timeout 300s cargo fmt --all`;
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent portable_sitl_regression_smoke`;
   - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs` если docs sanity test добавлен;
   - `timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings` если бюджет позволяет.

   Если в M50 были только docs/test additions, live PX4 не запускать. PX4 остается manual verification.

## Testing Strategy

### 1. Tests That Need No Refactoring

Эти тесты можно добавить вместе с функциональными изменениями M50:

- `portable_sitl_regression_smoke` в `crates/swarm-examples/tests/sitl_agent.rs`:
  - temp scenario fixture;
  - dry-run success;
  - mock success;
  - expected waypoint/mission item count;
  - replay log summary count;
  - no external PX4.
- Valid safety pass test:
  - использовать existing safety config helper или temp config;
  - проверить `validate_pre_upload_safety` на valid scenario;
  - убедиться, что fail не маскируется отсутствием `mavlink-transport`.
- Docs sanity test:
  - `README.md` и `docs/SITL_SETUP.md` содержат обязательные anchors для mock/dry-run/PX4/manual boundary;
  - тест использует `include_str!`, а не абсолютные пути.
- Existing negative tests:
  - сохранить проверки unsafe mission, bad safety config, invalid connection lifecycle, no-feature gate;
  - не удалять и не ослаблять их ради нового consolidated smoke.

### 2. Tests That Need Light Refactoring

Эти тесты стоит делать, если новый smoke станет слишком длинным или начнет дублировать helpers:

- вынести общий fixture builder для SITL scenario/safety config внутри `sitl_agent.rs`;
- добавить маленький helper для запуска dry-run/mock commands с tempdir;
- добавить parser/helper для dry-run output, если substring assertions станут хрупкими;
- добавить shared assertion для expected two-waypoint fixture;
- выделить `sitl_portable.rs`, если `sitl_agent.rs` станет слишком большим.

### 3. Tests That Need Heavy Refactoring

Эти пункты не входят в M50, но важны для будущих milestone:

- first-class SITL suite inside `regression_runner` with dedicated suite type;
- CI-managed PX4 container or simulator job;
- ignored/manual integration tests that connect to real PX4 SITL;
- HIL/real hardware tests;
- production safety validation against real autopilot constraints;
- deterministic telemetry replay across real PX4 sessions.

## Risks And Tradeoffs

- Новый consolidated smoke может дублировать существующие granular tests. Нужно держать его как user-facing regression path, а не переносить в него все проверки.
- Docs sanity test может стать хрупким, если проверять точный текст. Лучше проверять стабильные anchors/section names/commands.
- No-feature connection tests не всегда исполняются под `--all-features`. Поэтому plan/safety checks лучше делать напрямую через library functions или отдельные cfg-aware tests.
- Добавление SITL в `regression_runner` сейчас может смешать benchmark metrics и CLI smoke. На M50 лучше использовать integration tests.
- Документация должна не переобещать: mock/dry-run regression-safe не означает production-ready PX4/hardware workflow.

## Open Questions

1. Где держать portable smoke: в существующем `sitl_agent.rs` или в новом `sitl_portable.rs`?
2. Должен ли README получить отдельную status row для M50, или достаточно обновить существующую Real PX4/SITL строку?
3. Нужен ли docs sanity test как обязательная automated guard, или достаточно ручной проверки документации? Рекомендация: добавить маленький test с anchors.
4. Должен ли CI запускать только targeted SITL tests или весь `cargo test -p swarm-examples`? Для M50 достаточно targeted checks, но README может показать оба варианта.
