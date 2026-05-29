# DRONE_A.16 - план развития с упором на Ветку 6

Дата фиксации: 2026-05-28

## Контекст

Актуальный источник по веткам: `docs_raw/BRANCHES.md`.

Выбранный фокус: **Ветка 6 - Real SITL / PX4**.

Основная цель этого направления - сделать проект не только headless simulator /
benchmark harness, а системой, которая может взять DSL-сценарий с waypoint tasks,
загрузить mission в PX4 SITL, наблюдать прогресс и связать telemetry обратно с
`TaskStatus`.

Важно: этот план не означает немедленный переход к реальным дронам. Правильная
траектория - сначала воспроизводимый single-agent PX4 SITL workflow, затем safety
and observability, затем multi-agent SITL, и только потом осторожная граница
hardware readiness.

## Текущий статус

Что уже есть:

- mock SITL path: `MockMavlinkTransport`;
- CLI: `sitl_agent --mock`;
- сценарий: `scenarios/sitl.waypoints.json`;
- feature-gated `MavlinkTransport`;
- базовые conversion helpers: `task_to_waypoint`, `task_to_mavlink_waypoint`;
- документация `docs/SITL_SETUP.md`;
- DSL validation для `sitl` tasks with `pose`.

Что еще не является полноценным workflow:

- `sitl_agent --connection` не реализует полноценный PX4 mission protocol;
- нет нормального `MISSION_COUNT` / `MISSION_ITEM_INT` / request / ack flow;
- telemetry не связывается обратно с `TaskStatus`;
- нет arm/takeoff/start/abort lifecycle;
- нет строгого preflight safety gate перед upload;
- нет полноценного SITL event log / replay summary;
- multi-agent SITL пока отсутствует.

## Принцип плана

Акцент остается на Ветке 6. Подзадачи из других веток добавляются только там, где
они прямо помогают real SITL / PX4 workflow:

- из Ветки 1 - только failure handling and reallocation после multi-agent
  foundation;
- из Ветки 4 - только safety/preflight constraints, без full realism calibration;
- из Ветки 5 - SITL observability and replay summary;
- из Ветки 7 - минимальный transport/agent extension contract;
- из Ветки 3 - не брать research benchmark как milestone;
- из Веток 2 и 8 - пока ничего не брать.

## Линейный план

```text
M46 SITL Contract & Dry-Run Foundation
-> M47 MAVLink Mission Upload Protocol
-> M48 PX4 Telemetry to TaskStatus
-> M49 Single-Agent PX4 SITL Golden Path
-> M50 Safety Preflight Gate
-> M51 SITL Observability & Replay
-> M52 Multi-Agent SITL Foundation
-> M53 Failure Handling & Reallocation
-> M54 Hardware Readiness Boundary
```

## M46 - SITL Contract & Dry-Run Foundation

Цель:

> зафиксировать portable SITL contract до подключения настоящего PX4 workflow.

Суть:

Нужно разделить mock, dry-run and real connection modes так, чтобы разработка real
MAVLink path не ломала CI-friendly mock path. `--dry-run` должен стать способом
проверять весь scenario -> waypoint/mission-plan pipeline без внешнего PX4.

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
   - координаты;
   - frame/altitude interpretation.
3. Вынести waypoint extraction/conversion из CLI в тестируемый helper.
4. Зафиксировать coordinate-frame contract:
   - что сейчас значит `Pose { x, y, z }`;
   - как это преобразуется в local/global MAVLink coordinates;
   - какие ограничения пока существуют.
5. Добавить typed errors:
   - invalid scenario;
   - no pose tasks;
   - feature missing;
   - bad connection string;
   - unsupported coordinate frame.
6. Обновить `docs/SITL_SETUP.md`:
   - mock mode;
   - dry-run mode;
   - real PX4 mode;
   - known limitations.

Ожидаемый результат:

- `sitl_agent --dry-run --scenario scenarios/sitl.waypoints.json` показывает
  mission upload plan без подключения к PX4;
- `--connection` без feature дает стабильную понятную ошибку;
- mock path остается полностью portable.

Не входит в scope:

- настоящий PX4 mission upload;
- telemetry tracking;
- multi-agent SITL.

Tests that need no refactoring:

- waypoint extraction helper tests;
- dry-run formatting tests;
- CLI validation test for missing mode;
- CLI validation test for `--connection` without `mavlink-transport`;
- scenario with zero pose tasks returns typed error.

Tests that need light refactoring:

- shared SITL scenario fixture;
- helper for invoking `sitl_agent` binary in tests;
- reusable CLI error assertions.

Tests that need heavy refactoring:

- none for this milestone.

## M47 - MAVLink Mission Upload Protocol

Цель:

> заменить текущий debug/raw-message real path на настоящий PX4 mission upload
> protocol.

Суть:

Сейчас real `MavlinkTransport` существует как scaffold, но `sitl_agent --connection`
не выполняет полноценный PX4 mission protocol. Нужно реализовать state machine,
которая разговаривает с PX4 через стандартный mission upload flow.

Что сделать:

1. Реализовать mission upload state machine:
   - wait heartbeat;
   - `MISSION_CLEAR_ALL`;
   - `MISSION_COUNT`;
   - обработка `MISSION_REQUEST_INT`;
   - fallback на `MISSION_REQUEST`, если нужен;
   - отправка `MISSION_ITEM_INT`;
   - обработка `MISSION_ACK`.
2. Добавить timeout/retry policy:
   - retry count;
   - per-step timeout;
   - clear error on timeout;
   - abort on wrong sequence.
3. Добавить typed `MavlinkMissionError`:
   - connection failed;
   - heartbeat timeout;
   - mission request timeout;
   - unexpected request seq;
   - mission rejected;
   - unsupported frame/coordinate conversion.
4. Сделать fake MAVLink connection для unit tests без PX4.
5. Убрать real path, который отправляет waypoint как debug `RawMessage`.
6. Сохранить mock path без внешних зависимостей.

Ожидаемый результат:

- `sitl_agent --connection` реально вызывает mission upload protocol;
- happy path upload покрыт unit tests;
- failure paths покрыты typed errors.

Не входит в scope:

- arm/takeoff/start;
- telemetry -> task completion;
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

- real PX4 SITL integration test.

## M48 - PX4 Telemetry to TaskStatus

Цель:

> связать PX4 telemetry/progress с внутренним task lifecycle.

Суть:

После mission upload нужно понимать, что происходит. Нужно получать telemetry,
понимать текущий waypoint/mission seq и обновлять `TaskStatus` для исходных tasks.

Что сделать:

1. Обработать основные MAVLink messages:
   - `HEARTBEAT`;
   - `MISSION_CURRENT`;
   - `MISSION_ITEM_REACHED`, если доступен в dialect/stream;
   - `MISSION_ACK`;
   - disconnect/timeout.
2. Добавить mapping:
   - mission item seq -> task id;
   - task id -> status;
   - final mission status -> run status.
3. Ввести progress loop:
   - current seq;
   - completed waypoint count;
   - last telemetry timestamp;
   - timeout on no progress.
4. Добавить task status transitions:
   - `Unassigned` / planned;
   - `InProgress`;
   - `Completed`;
   - `Failed`.
5. Добавить human-readable progress output.

Связь с Веткой 5:

Минимально полезно взять replay/event summary, но не interactive UI.

Ожидаемый результат:

- fake telemetry seq 0/1/2 превращается в completed task statuses;
- CLI показывает progress;
- mission failure превращается в failed status.

Не входит в scope:

- full replay UI;
- multi-agent telemetry merge;
- hardware-specific failsafe logic.

Tests that need no refactoring:

- telemetry `MISSION_CURRENT` -> current task test;
- waypoint reached -> completed task test;
- mission rejected -> failed task/run test;
- disconnect timeout test.

Tests that need light refactoring:

- telemetry parser helper;
- fake telemetry stream;
- task-status assertion helpers.

Tests that need heavy refactoring:

- real PX4 telemetry integration test.

## M49 - Single-Agent PX4 SITL Golden Path

Цель:

> получить первый настоящий end-to-end PX4 SITL workflow для одного агента.

Суть:

Это первый milestone, где проект перестает быть только headless simulation. Scope
намеренно узкий: один агент, waypoint scenario, PX4 SITL, upload and execute.

Что сделать:

1. Документировать tested PX4 setup:
   - PX4 version/command;
   - simulator backend;
   - connection string;
   - expected ports;
   - troubleshooting.
2. Реализовать CLI lifecycle options:
   - `--upload-only`;
   - `--execute`;
   - `--no-arm`;
   - `--abort-after <seconds/ticks>`;
   - `--timeout <seconds>`.
3. Добавить sequence:
   - connect;
   - wait heartbeat;
   - upload mission;
   - arm;
   - takeoff or mission start;
   - monitor progress;
   - finish with clear status.
4. Добавить abort behavior:
   - user interrupt;
   - timeout;
   - failed ack;
   - telemetry loss.
5. Обновить `docs/SITL_SETUP.md` with exact golden path.

Ожидаемый результат:

- один агент проходит `scenarios/sitl.waypoints.json` в PX4 SITL;
- mock/dry-run остаются portable;
- docs четко разделяют mock, dry-run, PX4 SITL and real hardware.

Не входит в scope:

- multi-agent SITL;
- real hardware support;
- complex mission families beyond waypoint tasks.

Tests that need no refactoring:

- CLI option parsing tests;
- lifecycle command construction tests;
- abort condition tests with fake connection.

Tests that need light refactoring:

- dry-run lifecycle plan fixture;
- fake PX4 script for golden path.

Tests that need heavy refactoring:

- real PX4 SITL integration test.

## M50 - Safety Preflight Gate

Цель:

> не отправлять потенциально опасную или некорректную mission в transport.

Суть:

Даже в SITL real connection path должен иметь explicit preflight validation. Это
особенно важно перед любыми будущими hardware experiments.

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
   - task id / waypoint seq;
   - actual value;
   - allowed value/range.
4. Добавить `--safety-config <path>`.
5. Сделать safe defaults for SITL.
6. Не добавлять silent override. Если нужен override, он должен быть explicit and
   documented, но лучше отложить.

Связь с Веткой 4:

Берем только safety constraints. Full realism calibration сюда не включается.

Ожидаемый результат:

- невалидная mission не уходит в MAVLink transport;
- ошибки actionable;
- safety config portable and testable.

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

## M51 - SITL Observability & Replay

Цель:

> сделать SITL behavior inspectable and reproducible after a run.

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
   - task completed;
   - abort/disconnect/failure.
2. Добавить machine-readable SITL run report:
   - scenario;
   - agent id;
   - connection mode;
   - mission item count;
   - completed item count;
   - final status;
   - duration;
   - error if any.
3. Расширить replay CLI:
   - `replay --sitl-summary <log>`;
   - compact text summary.
4. Документировать log schema.

Связь с Веткой 5:

Это минимальная observability часть Ветки 5. Interactive UI пока не нужен.

Ожидаемый результат:

- после SITL run есть JSON log;
- replay summary объясняет, что произошло;
- mock/fake transport tests покрывают event log.

Tests that need no refactoring:

- event log serialization roundtrip;
- summary counts mission upload events;
- failure event summary test;
- mock run writes expected events.

Tests that need light refactoring:

- event log builder fixture;
- replay fixture by event type.

Tests that need heavy refactoring:

- interactive visualization tests, если когда-нибудь появится UI.

## M52 - Multi-Agent SITL Foundation

Цель:

> перейти от single-agent SITL к нескольким агентам без усложнения алгоритмов.

Суть:

Сначала нужен foundation: mapping agents to connections, task subset split,
multi-agent dry-run and no ownership conflicts. Не нужно сразу делать сложную
swarm coordination на PX4.

Что сделать:

1. Описать mapping:
   - `agent_id` -> MAVLink system id;
   - `agent_id` -> component id;
   - `agent_id` -> connection string;
   - `agent_id` -> assigned task subset.
2. Поддержать config:
   - JSON/YAML/TOML agent connection map;
   - per-agent start delay;
   - per-agent upload-only/execute flags.
3. Добавить multi-agent dry-run:
   - какие tasks кому уходят;
   - какие connection strings используются;
   - ownership summary.
4. Поддержать два режима запуска:
   - несколько `sitl_agent` процессов;
   - один supervisor process.
5. Проверять no duplicate task ownership before upload.

Связь с Веткой 7:

Берем минимальный transport/agent extension contract, но не стабилизируем весь
public API.

Ожидаемый результат:

- два mock/SITL agents получают разные waypoint subsets;
- есть multi-agent dry-run manifest;
- duplicate ownership rejected before upload.

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

## M53 - Failure Handling & Reallocation

Цель:

> добавить минимальный failure/reallocation behavior, нужный именно для
> multi-agent SITL.

Суть:

Это точечное заимствование из Ветки 1. Не нужно брать весь Algorithm Depth. Нужно
только обработать потерю агента и вернуть его незавершенные tasks в pool.

Что сделать:

1. Heartbeat timeout -> agent lost.
2. Незавершенные tasks lost агента возвращаются в unassigned pool.
3. Оставшиеся агенты получают reallocated tasks.
4. Добавить metrics:
   - `reassignment_count`;
   - `avg_reallocation_ticks` или SITL-specific equivalent.
5. Отразить reallocation в event log.
6. Сначала покрыть mock/fake transport, затем optional SITL check.

Связь с Веткой 1:

Берется только dynamic reallocation при отказе агента. Не включаем hierarchical
coordination, communication-aware scoring and broad algorithm work.

Ожидаемый результат:

- deterministic test: агент теряется, его задачи получает другой агент;
- event log показывает failure and reallocation;
- task ownership remains unique.

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

## M54 - Hardware Readiness Boundary

Цель:

> явно отделить tested SITL workflow от real hardware claims.

Суть:

Даже после PX4 SITL нельзя утверждать, что проект готов к реальным дронам. Нужно
оформить границу готовности, assumptions and checklist.

Что сделать:

1. Добавить `docs/HARDWARE_READINESS.md`:
   - что проверено в mock;
   - что проверено в dry-run;
   - что проверено в PX4 SITL;
   - что не проверено на hardware;
   - safety assumptions;
   - operator checklist.
2. Добавить explicit CLI warning для hardware-looking connections.
3. Разделить connection classes:
   - mock;
   - dry-run;
   - local PX4 SITL UDP;
   - remote/serial hardware candidate.
4. Не обещать production safety.
5. Зафиксировать требования перед любым hardware experiment:
   - physical kill switch;
   - geofence;
   - manual pilot override;
   - low-risk environment;
   - no autonomous flight outside controlled test.

Ожидаемый результат:

- понятно, где заканчивается research/SITL workflow;
- real hardware path не выглядит случайно включаемым;
- есть checklist before hardware experiments.

Tests that need no refactoring:

- connection classifier tests;
- hardware warning output test;
- docs checklist existence check, если в repo есть doc tests.

Tests that need light refactoring:

- CLI warning helper.

Tests that need heavy refactoring:

- hardware-in-the-loop tests. Не делать в обычном CI.

## Что не делать сейчас

Не стоит сейчас делать:

- full Research Benchmark из Ветки 3;
- interactive UI из Ветки 5;
- full Realism Calibration из Ветки 4;
- flood/new mission work из Веток 2/8;
- broad Algorithm Depth из Ветки 1;
- public API stabilization из Ветки 7.

Причина: все это отвлекает от главной ценности выбранной Ветки 6 - получить
реальный, воспроизводимый PX4 SITL path.

## Короткая рекомендация

Начинать с M46. Это небольшой, безопасный этап, который быстро даст пользу:

- dry-run без PX4;
- понятный contract для waypoint mission;
- перенос логики из CLI в тестируемые helpers;
- стабильные ошибки;
- подготовка к настоящему MAVLink upload.

После M46 можно переходить к M47, где находится главный технический риск Ветки 6:
правильная реализация PX4 mission upload protocol.
