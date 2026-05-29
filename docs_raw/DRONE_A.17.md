# DRONE_A.17 - итоговый план Ветки 6 Real SITL / PX4

Дата фиксации: 2026-05-28

## Контекст

Актуальный источник по веткам: `docs_raw/BRANCHES.md`.

Сравнивались:

- `docs_raw/DRONE_A.16.md`;
- `docs_raw/DRONE_B.16.md`.

Выбранный фокус: **Ветка 6 - Real SITL / PX4**.

Цель итогового плана - собрать более сильную версию из двух подходов:

- от `DRONE_A.16.md` взять осторожный engineering path: dry-run, typed errors,
  test seam, observability, hardware-readiness boundary;
- от `DRONE_B.16.md` взять прямую ориентацию на PX4 workflow: mission upload,
  arm/takeoff/execute/abort, telemetry loop, single-agent golden path;
- не включать лишние milestones из других веток, если они не помогают Real
  SITL / PX4 напрямую.

## Чем отличались A.16 и B.16

### Старт плана

`DRONE_A.16.md` начинается с `SITL Contract & Dry-Run Foundation`.

Плюсы:

- можно проверять scenario -> waypoint/mission-plan pipeline без PX4;
- легче тестировать CLI, typed errors and conversion helpers;
- меньше риск, что MAVLink protocol будет отлаживаться только руками.

Минусы:

- реальный PX4 upload откладывается на один milestone.

`DRONE_B.16.md` начинается сразу с `MAVLink Mission Upload Protocol`.

Плюсы:

- быстрее идет к главному technical risk;
- план короче и прямее.

Минусы:

- без dry-run/test seam возрастает риск ручной отладки;
- сложнее сохранить portable mock path как надежный CI baseline.

Итоговое решение: **начинать с dry-run/contract**, затем сразу переходить к
MAVLink upload.

### Нумерация

`DRONE_A.16.md` продолжает с M46, потому что раньше M43-M45 уже фигурировали в
линейном плане как Realism/Flood/Decision.

`DRONE_B.16.md` начинает с M43, считая M42 последним закрытым milestone.

Итоговое решение: **использовать M43-M53** для новой выбранной ветки. Это проще:
M42 закрывает общий ствол, после него начинается новый branch-focused roadmap.

### Safety

`DRONE_A.16.md` ставит safety после single-agent PX4 golden path.

`DRONE_B.16.md` ставит safety после upload/flight/telemetry.

Итоговое решение: **pre-upload safety должен идти до arm/takeoff/execute**.
Невалидную mission нельзя доводить до реального execution path даже в SITL.

### Realism calibration

`DRONE_B.16.md` добавляет отдельный milestone по realism calibration.

Итоговое решение: **не включать full realism calibration в этот план**.
Для Ветки 6 сейчас важнее получить надежный PX4 SITL workflow. Из realism/safety
можно взять только preflight constraints and metadata, но не full comparative
benchmark ideal/light/medium/heavy.

### Replay / observability

Оба плана предлагают replay/observability.

Итоговое решение: **оставить SITL observability and replay**, но без interactive
UI. Это нужно для отладки PX4 workflow и анализа failures.

### Hardware boundary

`DRONE_A.16.md` явно добавляет `Hardware Readiness Boundary`.

`DRONE_B.16.md` почти не фиксирует эту границу.

Итоговое решение: **оставить hardware-readiness milestone**. После PX4 SITL нельзя
неявно обещать готовность к реальным дронам.

## Итоговая последовательность

```text
M43 SITL Contract & Dry-Run Foundation
-> M44 MAVLink Mission Upload Protocol
-> M45 Pre-upload Safety Validation
-> M46 Flight Sequence: arm / takeoff / execute / abort
-> M47 Telemetry Loop & TaskStatus Mapping
-> M48 Single-Agent PX4 SITL Golden Path
-> M49 SITL Observability & Replay
-> M50 Mock Regression & Docs Hardening
-> M51 Dynamic Reallocation for Failed Agent
-> M52 Multi-Agent SITL Foundation
-> M53 Hardware Readiness Boundary
```

## M43 - SITL Contract & Dry-Run Foundation

Цель:

> зафиксировать portable SITL contract до подключения настоящего PX4 workflow.

Суть:

До реализации настоящего MAVLink upload нужно сделать testable and portable
foundation. `--dry-run` должен показывать, что именно будет отправлено в PX4, не
требуя внешнего simulator. Это снизит риск ручной отладки MAVLink protocol.

Что сделать:

1. Разделить режимы `sitl_agent`:
   - `--mock`;
   - `--dry-run`;
   - `--connection <addr>`.
2. Добавить dry-run output:
   - agent id;
   - scenario path/name;
   - task ids;
   - waypoint sequence;
   - coordinates;
   - frame/altitude interpretation.
3. Вынести waypoint extraction/conversion из CLI в тестируемый helper.
4. Зафиксировать coordinate-frame contract:
   - что значит `Pose { x, y, z }`;
   - local vs global coordinate interpretation;
   - altitude source;
   - current limitations.
5. Добавить typed errors:
   - invalid scenario;
   - no pose tasks;
   - feature missing;
   - bad connection string;
   - unsupported coordinate frame.
6. Обновить `docs/SITL_SETUP.md`:
   - mock mode;
   - dry-run mode;
   - PX4 SITL mode;
   - real hardware warning.

Ожидаемый результат:

- `sitl_agent --dry-run --scenario scenarios/sitl.waypoints.json` показывает
  mission upload plan без подключения к PX4;
- `--connection` без `mavlink-transport` feature дает стабильную ошибку;
- mock path остается portable and CI-friendly.

Не входит в scope:

- настоящий MAVLink mission upload;
- arm/takeoff/execute;
- telemetry loop;
- multi-agent SITL.

Tests that need no refactoring:

- waypoint extraction helper tests;
- dry-run formatting tests;
- CLI validation test for missing mode;
- CLI validation test for `--connection` without feature;
- scenario with zero pose tasks returns typed error.

Tests that need light refactoring:

- shared SITL scenario fixture;
- helper for invoking `sitl_agent` binary in tests;
- reusable CLI error assertions.

Tests that need heavy refactoring:

- none.

## M44 - MAVLink Mission Upload Protocol

Цель:

> заменить текущий debug/raw-message path на настоящий PX4 mission upload
> protocol.

Суть:

`MavlinkTransport::send()` сейчас не является mission upload mechanism. Нужно
реализовать отдельный upload workflow, который говорит с PX4 через стандартный
handshake.

Что сделать:

1. Ввести mission upload API:
   - `upload_mission(waypoints: &[Waypoint])`;
   - target system/component;
   - upload options: timeout, retry count.
2. Реализовать handshake:
   - wait heartbeat;
   - optional `MISSION_CLEAR_ALL`;
   - `MISSION_COUNT`;
   - wait `MISSION_REQUEST_INT`;
   - fallback на `MISSION_REQUEST`, если требуется;
   - send `MISSION_ITEM_INT`;
   - wait final `MISSION_ACK`.
3. Исправить current real path:
   - не отправлять `RAW_RPM` заглушку;
   - не оборачивать MAVLink packets в debug string для SITL logic;
   - `sitl_agent --connection` должен вызывать upload API.
4. Добавить typed `MavlinkMissionError`:
   - connection failed;
   - heartbeat timeout;
   - mission request timeout;
   - unexpected request seq;
   - mission rejected;
   - unsupported frame.
5. Добавить fake MAVLink connection для unit tests.

Ожидаемый результат:

- `sitl_agent --connection udp:127.0.0.1:14550 ...` загружает mission в PX4 SITL
  и получает accepted/rejected result;
- mock path не меняет внешний контракт;
- failure paths возвращают typed errors.

Не входит в scope:

- arm/takeoff/start;
- task completion telemetry;
- multi-agent.

Tests that need no refactoring:

- mission upload happy path with fake connection;
- `MISSION_REQUEST_INT` seq order test;
- wrong seq rejection test;
- rejected `MISSION_ACK` test;
- timeout test;
- task -> `MISSION_ITEM_INT` conversion test.

Tests that need light refactoring:

- introduce `MavlinkConnection` trait or equivalent test seam;
- fake connection script fixtures;
- typed error fixture helpers.

Tests that need heavy refactoring:

- real PX4 SITL integration test, manual/ignored by default.

## M45 - Pre-upload Safety Validation

Цель:

> не отправлять потенциально опасную или некорректную mission в transport.

Суть:

Safety validation должна идти до arm/takeoff/execute. Даже если речь пока о SITL,
connection path должен вести себя как будущий hardware-adjacent path.

Что сделать:

1. Ввести `SitlSafetyConfig`:
   - geofence bounds;
   - min/max altitude;
   - max distance between waypoints;
   - max mission radius from home;
   - no-fly zones;
   - required home/base point.
2. Валидировать перед upload:
   - empty mission;
   - duplicate waypoint ids;
   - missing pose;
   - invalid altitude;
   - outside geofence;
   - inside no-fly zone;
   - unsafe waypoint jump.
3. Ошибки должны содержать:
   - rule id;
   - task id or waypoint seq;
   - actual value;
   - allowed value/range.
4. Добавить `--safety-config <path>`.
5. Сделать safe defaults for SITL.
6. Обновить docs.

Ожидаемый результат:

- невалидная mission rejected before upload;
- ошибка actionable;
- safety validation covered by portable tests.

Не входит в scope:

- full realism calibration;
- hardware certification;
- runtime collision avoidance.

Tests that need no refactoring:

- geofence rejection test;
- altitude bounds test;
- no-fly zone test;
- max waypoint jump test;
- duplicate waypoint id test;
- valid mission passes test.

Tests that need light refactoring:

- safety config fixture builder;
- scenario mutation helpers.

Tests that need heavy refactoring:

- none initially.

## M46 - Flight Sequence: arm / takeoff / execute / abort

Цель:

> реализовать управляемый lifecycle после успешного mission upload.

Суть:

После upload нужен не только факт загрузки mission, но и последовательность
управления полетом в PX4 SITL: arm, takeoff/start mission, abort on failure.

Что сделать:

1. Добавить command helpers:
   - `arm`;
   - `disarm`, если нужен;
   - `takeoff`;
   - `set_auto_mode` / start mission;
   - `abort` / RTL or mission stop.
2. Добавить `wait_command_ack(command, timeout)`.
3. В `sitl_agent` добавить lifecycle options:
   - `--upload-only`;
   - `--execute`;
   - `--no-arm`;
   - `--abort-after <seconds>`;
   - `--timeout <seconds>`.
4. Определить failure behavior:
   - upload failed -> no arm;
   - arm failed -> exit with clear error;
   - takeoff/start failed -> abort;
   - telemetry timeout -> abort.
5. Обновить `docs/SITL_SETUP.md`.

Ожидаемый результат:

- один агент может пройти controlled PX4 lifecycle до telemetry monitoring;
- ошибка на любом шаге не выглядит как silent success;
- abort path testable through fake connection.

Не входит в scope:

- task completion mapping;
- multi-agent;
- hardware-specific failsafe tuning.

Tests that need no refactoring:

- `arm()` sends correct command id and param;
- `takeoff(altitude)` sends correct command and altitude;
- `abort()` sends RTL/stop command;
- command ack accepted/rejected tests;
- CLI option parsing tests.

Tests that need light refactoring:

- extend fake PX4 script to answer command acks;
- lifecycle command construction helper.

Tests that need heavy refactoring:

- real PX4 SITL lifecycle integration test.

## M47 - Telemetry Loop & TaskStatus Mapping

Цель:

> связать PX4 telemetry/progress с внутренним task lifecycle.

Суть:

`sitl_agent` не должен завершаться сразу после upload/start. Он должен ждать
telemetry, понимать текущий mission item and waypoint reached events, а затем
обновлять task statuses.

Что сделать:

1. Добавить telemetry events:
   - `Heartbeat`;
   - `MissionCurrent { seq }`;
   - `WaypointReached { seq }`;
   - `MissionComplete`;
   - `Disconnected`;
   - `MissionRejected`.
2. Добавить mapping:
   - mission item seq -> task id;
   - task id -> `TaskStatus`;
   - final mission status -> process exit code.
3. Реализовать progress loop:
   - current seq;
   - completed count;
   - last heartbeat time;
   - no-progress timeout;
   - disconnect timeout.
4. Добавить human-readable progress output.
5. Добавить `TaskStatus::Failed` mapping for rejected/aborted mission.

Ожидаемый результат:

- fake telemetry seq 0/1/2 turns tasks completed;
- mission failure marks tasks/run failed;
- disconnect triggers abort + non-zero exit.

Не входит в scope:

- multi-agent telemetry merge;
- replay UI;
- hardware failsafe tuning.

Tests that need no refactoring:

- `MISSION_CURRENT` -> current task test;
- waypoint reached -> completed task test;
- rejected mission -> failed status test;
- no heartbeat -> disconnected test;
- all waypoints reached -> exit success test.

Tests that need light refactoring:

- telemetry parser helper;
- fake telemetry stream;
- task-status assertion helpers.

Tests that need heavy refactoring:

- real PX4 telemetry integration test.

## M48 - Single-Agent PX4 SITL Golden Path

Цель:

> получить первый настоящий end-to-end PX4 SITL workflow для одного агента.

Суть:

Это первый milestone, где проект должен пройти полный путь:

```text
scenario -> safety validation -> mission upload -> arm/takeoff/start -> telemetry
-> task completion -> final report
```

Что сделать:

1. Документировать tested PX4 setup:
   - PX4 version;
   - simulator backend;
   - startup command;
   - connection string;
   - expected ports;
   - troubleshooting.
2. Собрать single-agent golden command.
3. Добавить final run report:
   - scenario;
   - agent id;
   - mission item count;
   - completed count;
   - final status;
   - error if any.
4. Проверить вручную на PX4 SITL.
5. Описать результат in docs.

Ожидаемый результат:

- один агент проходит `scenarios/sitl.waypoints.json` in PX4 SITL;
- mock/dry-run remain portable;
- docs clearly separate mock, dry-run, PX4 SITL, real hardware.

Не входит в scope:

- multi-agent SITL;
- real hardware support;
- complex non-waypoint mission families.

Tests that need no refactoring:

- final report serialization test;
- fake golden path test;
- CLI success/failure exit-code tests with fake connection.

Tests that need light refactoring:

- dry-run lifecycle plan fixture;
- fake PX4 script for golden path.

Tests that need heavy refactoring:

- real PX4 SITL integration test, ignored/manual by default.

## M49 - SITL Observability & Replay

Цель:

> сделать SITL behavior inspectable after a run.

Суть:

PX4 workflow без event log трудно отлаживать. Нужно писать компактный SITL run log
и уметь получать summary через replay tooling.

Что сделать:

1. Добавить SITL event log:
   - connection opened;
   - heartbeat seen;
   - mission clear sent;
   - mission count sent;
   - mission item requested;
   - mission item sent;
   - mission ack received;
   - arm/takeoff/start command sent;
   - current seq changed;
   - waypoint reached;
   - task completed;
   - abort/disconnect/failure.
2. Добавить `--replay-log <path>` for `sitl_agent`.
3. Расширить replay CLI:
   - `replay --sitl-summary <log>`;
   - compact text summary.
4. Документировать log schema.

Ожидаемый результат:

- after SITL run there is JSON log;
- replay summary explains mission progress and failures;
- mock/fake tests cover event log.

Не входит в scope:

- interactive UI;
- map overlay;
- long-term historical telemetry store.

Tests that need no refactoring:

- event log serialization roundtrip;
- summary counts mission upload events;
- summary counts waypoint reached events;
- failure event summary test;
- mock run writes expected events.

Tests that need light refactoring:

- event log builder fixture;
- replay fixture by event type.

Tests that need heavy refactoring:

- interactive visualization tests if UI is introduced later.

## M50 - Mock Regression & Docs Hardening

Цель:

> закрепить portable SITL path as regression-safe and documented.

Суть:

Real PX4 SITL cannot be default CI gate. Mock and dry-run paths can. После
single-agent golden path нужно зафиксировать portable regression coverage and docs.

Что сделать:

1. Добавить mock/dry-run regression smoke:
   - scenario loads;
   - waypoint extraction works;
   - safety validation passes;
   - mission plan has expected item count;
   - no external PX4 needed.
2. Обновить `docs/SITL_SETUP.md`:
   - mock mode;
   - dry-run mode;
   - PX4 SITL mode;
   - real hardware warning;
   - troubleshooting.
3. Добавить docs for CI/manual boundary:
   - what is automated;
   - what is manual;
   - what requires PX4.
4. Optionally add experimental regression suite only if it is portable.

Ожидаемый результат:

- mock/dry-run path is default regression-safe;
- docs do not imply hardware readiness;
- reviewers can verify SITL foundation without external simulator.

Не входит в scope:

- real PX4 CI setup;
- hardware-in-the-loop;
- full realism calibration.

Tests that need no refactoring:

- mock SITL CLI smoke;
- dry-run CLI smoke;
- docs command sanity test if available;
- no-feature `--connection` error test.

Tests that need light refactoring:

- binary invocation fixture;
- tempdir output fixture.

Tests that need heavy refactoring:

- CI-managed PX4 container.

## M51 - Dynamic Reallocation for Failed Agent

Цель:

> добавить минимальный failure/reallocation behavior, нужный для multi-agent SITL.

Суть:

Это точечное заимствование из Ветки 1. Не нужно брать весь Algorithm Depth. Нужно
только обработать потерю агента и вернуть его незавершенные tasks в pool.

Что сделать:

1. Heartbeat timeout -> agent lost.
2. Незавершенные tasks lost агента возвращаются в unassigned pool.
3. Оставшиеся агенты получают reallocated tasks.
4. Добавить metrics or report fields:
   - `reassignment_count`;
   - reallocation latency;
   - tasks recovered.
5. Отразить reallocation in event log.
6. Сначала покрыть mock/fake transport, затем optional SITL check.

Ожидаемый результат:

- deterministic test: agent lost -> tasks reassigned;
- event log shows failure and reallocation;
- task ownership remains unique.

Не входит в scope:

- hierarchical coordination;
- communication-aware scoring;
- broad CBBA rewrite.

Tests that need no refactoring:

- lost agent returns tasks to pool;
- reallocation assigns tasks to surviving agent;
- duplicate assignment invariant;
- event log contains reallocation event.

Tests that need light refactoring:

- fake heartbeat stream;
- deterministic failure scenario fixture.

Tests that need heavy refactoring:

- multi-agent SITL failure integration test.

## M52 - Multi-Agent SITL Foundation

Цель:

> перейти от single-agent SITL к нескольким агентам без преждевременной
> алгоритмической сложности.

Суть:

Сначала нужен foundation: mapping agents to connections, task subset split,
multi-agent dry-run and no ownership conflicts. Сложную swarm coordination лучше
добавлять позже.

Что сделать:

1. Описать mapping:
   - `agent_id` -> MAVLink system id;
   - `agent_id` -> component id;
   - `agent_id` -> connection string;
   - `agent_id` -> assigned task subset.
2. Поддержать config:
   - agent connection map;
   - per-agent start delay;
   - per-agent upload-only/execute flags.
3. Добавить multi-agent dry-run:
   - tasks per agent;
   - connection strings;
   - ownership summary.
4. Поддержать два режима:
   - several `sitl_agent` processes;
   - supervisor process.
5. Проверять no duplicate task ownership before upload.

Ожидаемый результат:

- two mock/SITL agents get different waypoint subsets;
- multi-agent dry-run manifest exists;
- duplicate ownership rejected before upload.

Не входит в scope:

- robust distributed coordination;
- real multi-agent hardware;
- automatic swarm safety certification.

Tests that need no refactoring:

- agent connection config parse test;
- task split test;
- duplicate ownership rejection test;
- multi-agent dry-run output test.

Tests that need light refactoring:

- agent config fixture;
- supervisor fake transport.

Tests that need heavy refactoring:

- real multi-agent PX4 SITL integration test.

## M53 - Hardware Readiness Boundary

Цель:

> явно отделить tested SITL workflow от real hardware claims.

Суть:

Даже после PX4 SITL проект нельзя считать готовым к реальным дронам. Нужно
зафиксировать assumptions, operator checklist and unsupported areas.

Что сделать:

1. Добавить `docs/HARDWARE_READINESS.md`:
   - what is verified in mock;
   - what is verified in dry-run;
   - what is verified in PX4 SITL;
   - what is not verified on hardware;
   - safety assumptions;
   - operator checklist.
2. Добавить explicit CLI warning for hardware-looking connections.
3. Разделить connection classes:
   - mock;
   - dry-run;
   - local PX4 SITL UDP;
   - remote/serial hardware candidate.
4. Не обещать production safety.
5. Зафиксировать requirements before any hardware experiment:
   - physical kill switch;
   - geofence;
   - manual pilot override;
   - low-risk environment;
   - no autonomous flight outside controlled test.

Ожидаемый результат:

- users understand SITL vs hardware boundary;
- real hardware path cannot be enabled accidentally;
- checklist exists before any hardware experiments.

Не входит в scope:

- hardware-in-the-loop CI;
- flight certification;
- production safety guarantees.

Tests that need no refactoring:

- connection classifier tests;
- hardware warning output test;
- docs checklist existence check, if useful.

Tests that need light refactoring:

- CLI warning helper.

Tests that need heavy refactoring:

- hardware-in-the-loop tests. Not for default CI.

## Что не включаем сейчас

Не включаем как milestones:

- full Research Benchmark из Ветки 3;
- full Realism Calibration из Ветки 4;
- interactive visualization/UI из Ветки 5;
- flood/new mission work из Веток 2/8;
- broad Algorithm Depth из Ветки 1;
- public API stabilization из Ветки 7.

Причина: это отвлекает от главной ценности Ветки 6 - получить реальный,
воспроизводимый, наблюдаемый PX4 SITL workflow.

## Рекомендуемый старт

Начинать с **M43 SITL Contract & Dry-Run Foundation**.

Почему:

- дает быстрый полезный результат без PX4;
- делает CLI behavior testable;
- фиксирует coordinate-frame assumptions;
- подготавливает fake/test seam для MAVLink mission upload;
- сохраняет mock path как portable regression baseline.

После M43 нужно идти в M44. Главный technical risk всей Ветки 6 находится именно
там: правильная реализация PX4 mission upload protocol.
