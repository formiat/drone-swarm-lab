# План исправлений после аудита DRONE_A.17 / DRONE_B.17

## Context

Работа идет по ветке Real SITL / PX4 из `docs_raw/DRONE_A.17.md`.
Большая часть M43-M53 уже реализована, но из ревью и локальной проверки остался
набор долгов, который лучше закрыть перед live PX4 SITL прогоном и дальнейшим
развитием multi-agent SITL.

Главный принцип плана: сначала закрыть небольшие расхождения между планом,
кодом и публичной документацией, затем стабилизировать regression harness, и
только после этого считать M48/M51/M52 достаточно хорошо оформленными для
следующего шага.

Notion/GitLab контекст не использовался: в задаче нет Notion task id, MR или
GitLab review target; политика Notion указана как optional.

## Investigation context

`INVESTIGATION.md` отсутствует.

Локальная проверка подтвердила:

- `docs_raw/DRONE_A.17.md` требует убрать RAW_RPM-заглушку в M44.
- `docs_raw/DRONE_B.17.md` дополнительно требует публичный
  `scenarios/sitl.multi-agent.json`.
- `crates/swarm-comms/src/mavlink.rs` все еще содержит
  `MavlinkTransport::send()` с отправкой `RAW_RPM_DATA::default()`.
- `scenarios/` содержит только `sitl.waypoints.json`; публичного multi-agent
  SITL scenario fixture нет.
- `scenarios/sitl.waypoints.json` не задает `z`, поэтому live PX4 golden path
  получит relative altitude `0.0`.
- `docs/STATUS.md` устарел: Last audit M39b, старые next steps M40-M45, нет
  актуального статуса M43-M53.
- `docs/REPLAY.md` уже описывает часть SITL schema, но reallocation events
  нужно явно сверить и довести текст до текущей реализации/границ.
- В real connection upload observer `mission_item_sent` сейчас записывает
  `task_id: null`, потому `SitlMavlinkObserver` получает только `seq`; telemetry
  events уже умеют восстанавливать `task_id` через `SitlPlan`.
- `strategy_comparison --regression` / `regression_runner --jobs 1` проявляют
  flake: default regression может падать на SAR smoke suite, при этом отдельный
  suite проходит.
- `inspection_perimeter_battery_constraint_no_exhaustion` требует отдельной
  проверки: если тест действительно падает, надо либо чинить модель/fixture, либо
  честно пересмотреть assertion.

## Affected components

- `crates/swarm-comms/src/mavlink.rs` - убрать misleading RAW_RPM generic
  transport path.
- `crates/swarm-examples/src/bin/sitl_agent.rs` - дописать `task_id` в real
  connection `mission_item_sent` events.
- `crates/swarm-examples/src/sitl_observability.rs` - при необходимости
  уточнить event schema/tests для task_id и reallocation events.
- `crates/swarm-examples/tests/sitl_agent.rs` - добавить integration tests для
  публичных fixtures, task_id в replay log и CLI docs contracts.
- `crates/swarm-comms` tests - добавить тест на отсутствие RAW_RPM-заглушки или
  явную unsupported ошибку generic send path.
- `crates/swarm-scenarios/src/inspection.rs` - проверить/починить perimeter
  battery constraint test или скорректировать его критерий.
- `crates/swarm-sim/src/regression.rs`,
  `crates/swarm-examples/src/bin/regression_runner.rs`,
  `crates/swarm-examples/src/bin/strategy_comparison.rs` - расследовать
  default regression flake.
- `scenarios/sitl.px4-golden.json` - новый single-agent PX4 golden fixture с
  явными `z`.
- `scenarios/sitl.multi-agent.json` - новый public multi-agent SITL fixture.
- `scenarios/sitl.multi-agent.config.json` - рекомендуемый paired config для
  `sitl_supervisor`/`sitl_agent --multi-agent-config`.
- `docs/SITL_SETUP.md`, `docs/REPLAY.md`, `docs/STATUS.md`,
  `docs/HARDWARE_READINESS.md`, `README.md` - актуализация статуса, команд,
  границ и fixtures.

## Implementation steps

1. Убрать RAW_RPM-заглушку из generic MAVLink transport path.
   - Файл: `crates/swarm-comms/src/mavlink.rs`.
   - Заменить поведение `impl Transport for MavlinkTransport::send()` так, чтобы
     generic `RawMessage` path не отправлял фальшивый `RAW_RPM`.
   - Предпочтительный вариант: вернуть явную typed ошибку вроде
     `MavlinkError::UnsupportedRawTransportSend`, объясняющую, что PX4 SITL
     должен использовать `upload_mission*` / lifecycle API, а не generic
     `Transport::send`.
   - Проверить, что `MockMavlinkTransport` и in-memory/UDP transports не меняют
     поведение.

2. Добавить публичные SITL fixtures.
   - Файл: `scenarios/sitl.px4-golden.json`.
   - Сделать single-agent scenario для M48 live PX4 SITL с 3 waypoint tasks и
     явными `pose.z`, например `5.0`, `6.0`, `5.0`, чтобы execute path не
     загружал relative altitude `0.0`.
   - Файл: `scenarios/sitl.multi-agent.json`.
   - Сделать 2-agent scenario с разными waypoint tasks, явными `pose.z`, и
     минимальным run_config.
   - Файл: `scenarios/sitl.multi-agent.config.json`.
   - Добавить paired `multi_sitl.v1` config с loopback UDP ports, distinct
     `system_id`, `component_id`, lifecycle и task subsets.
   - Не менять `scenarios/sitl.waypoints.json` без необходимости: он уже
     используется как portable/mock fixture и часть тестов ожидает `z=0.0` для
     missing-z fallback.

3. Довести M49 task_id в real connection mission item events.
   - Файл: `crates/swarm-examples/src/bin/sitl_agent.rs`.
   - Расширить `SitlMavlinkObserver`, чтобы он имел доступ к mapping
     `seq -> task_id` из `SitlPlan`.
   - В `MissionItemSent { seq }` записывать
     `recorder.push_mission_item_sent(seq, Some(task_id))`, если seq известен.
   - Для неизвестного seq оставить `None`, чтобы failure/debug path не падал.

4. Актуализировать replay/reallocation документацию.
   - Файл: `docs/REPLAY.md`.
   - Добавить/сверить описание `agent_lost`, `task_released`,
     `task_reassigned`, `reallocation_completed`.
   - Явно указать текущую границу: reallocation events покрыты schema/API и
     mock/runtime tests; live multi-agent PX4 supervisor flow пока не обязан
     эмитить эти events.
   - Файл: `docs/SITL_SETUP.md`.
   - Обновить команды M48 на `scenarios/sitl.px4-golden.json`.
   - Добавить multi-agent public fixture/config examples.
   - Оставить честную пометку, что live PX4 verification pending, пока прогон
     реально не выполнен.

5. Обновить `docs/STATUS.md`.
   - Заменить старый M39b audit на актуальный статус после M43-M53.
   - Отдельно отметить:
     - M43-M47, M49-M50, M53 реализованы и покрыты portable/fake tests;
     - M48 code path реализован, но live PX4 run pending;
     - M51 реализован на runtime/mock/schema boundary, но не как live
       multi-agent PX4 flow;
     - M52 является foundation, не full orchestration.
   - Убрать старые next steps M40-M45 как текущий roadmap; оставить их только
     как historical context, если вообще нужно.
   - Зафиксировать найденный regression flake как актуальный blocker для больших
     benchmark/regression прогонов.

6. Обязательно актуализировать README.
   - Файл: `README.md`.
   - Добавить ссылки/команды на `scenarios/sitl.px4-golden.json`,
     `scenarios/sitl.multi-agent.json` и
     `scenarios/sitl.multi-agent.config.json`.
   - Синхронизировать current status с `docs/STATUS.md`.
   - Не обещать, что M48 live PX4 уже пройден, пока нет фактического report/log.
   - Уточнить, что 1000-seed benchmark не закрывает M48; для M48 нужен live PX4
     SITL run, а для benchmark confidence сначала нужен regression determinism
     fix.

7. Разобраться с regression flake.
   - Файлы: `crates/swarm-sim/src/regression.rs`,
     `crates/swarm-examples/src/bin/regression_runner.rs`,
     `crates/swarm-examples/src/bin/strategy_comparison.rs`.
   - Воспроизвести отдельно:
     - `regression_runner --jobs 1` несколько раз подряд;
     - `strategy_comparison --regression --jobs 1` несколько раз подряд;
     - конкретные SAR suites по одному и в составе default regression.
   - Проверить, есть ли shared mutable state между suites, порядок factory reuse,
     недетерминированная инициализация RNG, зависимость от parallel iterator или
     reuse builder/factory между SAR profiles.
   - Исправить корень, если это баг.
   - Если это честная variance single-seed smoke, пересмотреть threshold/suite
     classification и документацию: smoke не должен флапать при `jobs=1`.

8. Проверить `inspection_perimeter_battery_constraint_no_exhaustion`.
   - Файл: `crates/swarm-scenarios/src/inspection.rs`.
   - Запустить тест отдельно и вместе с crate tests.
   - Если падает из-за реальной модели батареи/маршрута, починить scenario
     builder или assertion.
   - Если текущий assertion слишком сильный для constrained perimeter profile,
     переименовать/переформулировать тест так, чтобы он проверял стабильный
     контракт, а не желаемое поведение.

9. Решить по M51 live event log wiring.
   - Минимально сейчас: привести docs к фактической границе "schema/API/runtime
     covered, live supervisor flow not wired".
   - Если нужен полноценный flow: планировать отдельный milestone, где
     `sitl_supervisor`/`agent_process` получает failure injection или heartbeat
     timeout simulation и пишет `agent_lost`/`task_released`/`task_reassigned`/
     `reallocation_completed` в общий SITL log.
   - Не смешивать этот большой шаг с мелкими M44/M48/M49/docs fixes.

10. Подготовить M48 live PX4 verification после кодовых/docs исправлений.
    - Сначала автоматические checks и portable smoke.
    - Затем manual/local PX4 SITL:
      `sitl_agent --features mavlink-transport --connection udp:127.0.0.1:14550 --scenario scenarios/sitl.px4-golden.json --agent-id agent-0 --execute --run-report ... --replay-log ...`.
    - По итогам записать в docs фактический PX4 version/backend/command/report.
    - Это ручной verification gap, не replacement для автотестов.

## Testing strategy

### 1. Tests that need no refactoring

- `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms --features mavlink-transport`
  - Проверить, что mission upload/lifecycle tests остаются зелеными после
    удаления RAW_RPM path.
- `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --features mavlink-transport --test sitl_agent`
  - Добавить тест, что real connection replay observer пишет `task_id` для
    `mission_item_sent` через fake/golden path seam.
  - Добавить тесты, что новые public fixtures проходят dry-run/mock/multi-agent
    manifest без tempfile-only сценариев.
- `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_plan`
  - Проверить z-bearing fixture parsing and dry-run formatting.
- `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_multi_agent`
  - Проверить public multi-agent fixture/config.
- `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_observability`
  - Проверить replay event schema, task_id fields and reallocation summary.
- `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`
  - Обновить docs anchor test под README/SITL_SETUP/REPLAY/STATUS/HARDWARE docs.
- `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-scenarios inspection_perimeter_battery_constraint_no_exhaustion`
  - Проверить конкретный perimeter battery test после решения.
- `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test regression`
  - Проверить regression tests после flake fix.

### 2. Tests that need light refactoring

- Добавить helper в `crates/swarm-examples/tests/sitl_agent.rs` для запуска
  public fixtures из `scenarios/`, чтобы тесты не зависели только от inline
  tempfile fixtures.
- Добавить небольшой test seam/helper для `SitlMavlinkObserver`, чтобы без real
  PX4 проверить `MissionItemSent { seq } -> task_id`.
- Если `MavlinkTransport::send()` нельзя протестировать без реального endpoint,
  вынести unsupported generic send behavior в маленький helper или добавить fake
  connection constructor под `#[cfg(test)]`.
- Добавить повторяемый regression flake test/helper, который гоняет default
  regression несколько раз с `jobs=1`; держать его не слишком дорогим для обычной
  suite или пометить как targeted test, если runtime слишком большой.

### 3. Tests that need heavy refactoring

- Property test для reallocation under arbitrary failures: полезен, но требует
  генератора валидных agents/tasks/failure streams и четкого ограничения
  "каждая assignable задача eventually assigned". Не блокирует маленькие fixes.
- Full live multi-agent SITL failure/reallocation integration test: нужен
  supervisor-level failure injection и общий event log; это отдельный milestone.
- Real PX4 SITL integration test: должен быть ignored/manual или запускаться
  только в окружении с PX4 SITL. В default CI не включать.

## Что могло сломаться

- Поведение generic `Transport::send()` для `MavlinkTransport`: если где-то
  внешний код реально использовал этот path, после замены RAW_RPM на explicit
  error он начнет получать ошибку. Проверка: `rg "MavlinkTransport"`, compile
  all targets, `swarm-comms --features mavlink-transport`.
- SITL replay schema: добавление `task_id` в real upload events может поменять
  golden expectations. Проверка: `sitl_observability`, `replay_cli`,
  `sitl_agent` tests.
- Public scenario fixtures: новые `z` и multi-agent fixtures могут выявить
  safety/geofence/radius ограничения. Проверка: dry-run/mock/multi-agent tests и
  safety validation tests.
- Docs consistency: README/STATUS/SITL_SETUP/REPLAY могут разъехаться по статусу
  M48/M51/M52. Проверка: расширить `sitl_docs` на обязательные фразы/ссылки.
- Regression thresholds: исправление flake может потребовать изменить threshold
  или suite grouping. Проверка: repeated `regression_runner --jobs 1`,
  `strategy_comparison --regression --jobs 1`, затем workspace tests.
- Performance/resources: repeated regression checks могут стать дорогими для
  обычного test run. Проверка: держать дорогой repeat как targeted command или
  документированный pre-release check.
- Integration behavior: M51 live wiring, если будет выбран, может смешать
  runtime metrics и SITL event log semantics. Проверка: отдельные tests на
  ordering, duplicate ownership, recovered task count and summary.

## Risks and tradeoffs

- Удаление RAW_RPM через explicit unsupported error честнее, чем пытаться
  преобразовать arbitrary `RawMessage` в MAVLink packet. Минус: generic
  `Transport` для real MAVLink останется compile-compatible, но runtime path
  станет явно unsupported.
- Добавление нового `sitl.px4-golden.json` безопаснее, чем изменение
  `sitl.waypoints.json`: старые portable tests остаются стабильными, а M48
  получает корректный altitude-bearing fixture.
- M51 live supervisor wiring лучше не делать в этом же маленьком patch set: это
  отдельная архитектурная работа, а документацию можно быстро сделать честной.
- Regression flake может оказаться не SITL-related, но он блокирует доверие к
  большим прогонам и статусу "green workspace tests".
- Manual PX4 прогон не заменяет автотесты: он закрывает только live M48
  verification, а не regression determinism.

## Open questions

1. По M51 выбираем ли сейчас только честное сужение документации до
   "schema/API/runtime covered", или сразу планируем отдельную реализацию live
   supervisor reallocation log?
2. Нужно ли добавлять `scenarios/sitl.multi-agent.config.json` как публичный
   пример рядом со сценарием, или достаточно scenario fixture и inline config в
   docs/tests?
3. Должен ли `Transport::send()` для `MavlinkTransport` возвращать новый
   `MavlinkError::UnsupportedRawTransportSend`, или лучше убрать этот impl
   полностью, если downstream generic code не зависит от него?
4. Regression flake надо чинить до M48 live PX4 прогона или можно закрыть
   маленькие SITL/docs fixes сначала? Рекомендация: маленькие SITL/docs fixes
   сначала, regression flake сразу после них.
5. Property test для reallocation включаем в ближайший scope или оставляем как
   отдельную heavy-refactoring задачу?
