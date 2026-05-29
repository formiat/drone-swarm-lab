# PLAN.md - M44 MAVLink Mission Upload Protocol

## Context

Планируем M44 из `docs_raw/DRONE_A.17.md`: заменить текущий debug/raw-message
path на настоящий PX4 mission upload protocol.

Текущий статус после M43:

- `sitl_agent` уже разделен на `--mock`, `--dry-run`, `--connection <addr>`.
- `--dry-run` строит portable mission upload plan без PX4.
- `crates/swarm-examples/src/sitl_plan.rs` фиксирует текущий coordinate contract:
  `Pose { x, y, z }` трактуется как local simulation coordinates, `z` - local
  altitude, а `x/y` не являются WGS84 latitude/longitude.
- `--connection` без `mavlink-transport` feature возвращает stable
  `FeatureMissing`.
- `--connection` с feature сейчас все еще идет через `MavlinkTransport::send()`,
  а `send()` в `crates/swarm-comms/src/mavlink.rs` отправляет `RAW_RPM`
  заглушку. Это не mission upload.
- `task_to_mavlink_waypoint()` существует только под `mavlink-transport`, но
  сейчас конвертирует `pose.x/y` как будто это global lat/lon и выставляет
  `z = 0.0`. Для M44 это нельзя оставлять как production path.

Цель M44:

```text
sitl_agent --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0
```

должен вызывать настоящий mission upload workflow:

1. дождаться PX4 heartbeat;
2. опционально очистить текущую mission;
3. отправить `MISSION_COUNT`;
4. отвечать на `MISSION_REQUEST_INT` / fallback `MISSION_REQUEST`;
5. отправлять `MISSION_ITEM_INT`;
6. дождаться final `MISSION_ACK`;
7. вернуть typed accepted/rejected/timeout result.

В scope M44 не входят:

- arm/takeoff/start;
- task completion telemetry;
- multi-agent SITL;
- hardware readiness;
- full safety gate из M45.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует.

Прочитанный локальный контекст:

- `.agent-io/inbox.txt`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`;
- `docs_raw/DRONE_A.17.md`;
- `crates/swarm-comms/src/mavlink.rs`;
- `crates/swarm-comms/src/lib.rs`;
- `crates/swarm-examples/src/bin/sitl_agent.rs`;
- `crates/swarm-examples/src/sitl_plan.rs`;
- `crates/swarm-examples/tests/sitl_agent.rs`;
- `docs/SITL_SETUP.md`;
- `README.md`.

Notion policy: `optional`. Notion task id в prompt не указан, поэтому Notion CLI
не вызывался. GitLab/MR target не указан, поэтому `glab` не вызывался.
Удаленные SSH/HTTP обращения не выполнялись.

## Affected components

- `crates/swarm-comms/src/mavlink.rs` - основной M44 код: mission upload API,
  mission protocol state machine, typed `MavlinkMissionError`, fake/test seam,
  conversion to `MISSION_ITEM_INT`.
- `crates/swarm-comms/src/lib.rs` - экспорт новых mission upload типов под
  `mavlink-transport`.
- `crates/swarm-examples/src/bin/sitl_agent.rs` - заменить connection path:
  вместо `Transport::send(RawMessage)` вызывать `MavlinkTransport::upload_mission`.
- `crates/swarm-examples/src/sitl_plan.rs` - при необходимости добавить helper
  для превращения `SitlPlan`/`SitlWaypointItem` в `swarm_comms::Waypoint` без
  дублирования в CLI.
- `crates/swarm-examples/tests/sitl_agent.rs` - обновить/расширить CLI tests,
  если меняется output/error contract connection path.
- `docs/SITL_SETUP.md` - обновить PX4 SITL mode: теперь это mission upload
  protocol, но все еще без arm/takeoff/execute.
- `README.md` - обязательно обновить Current Status / Quick Start / Known
  Limitations: M44 добавляет feature-gated mission upload, но проект все еще не
  выполняет mission и не готов к real hardware.

## Implementation steps

1. Зафиксировать public mission upload API в
   `crates/swarm-comms/src/mavlink.rs`.

   Добавить feature-gated типы:

   - `MissionUploadOptions`:
     - `target_system: u8`;
     - `target_component: u8`;
     - `timeout: std::time::Duration`;
     - `retry_count: u8`;
     - `clear_existing: bool`;
     - `home_origin` или эквивалентный global conversion config;
     - supported frame, на M44 рекомендуется `MAV_FRAME_GLOBAL_RELATIVE_ALT_INT`.
   - `MissionUploadReport`:
     - uploaded item count;
     - target system/component;
     - final ack result;
     - whether clear was requested.
   - `MavlinkMissionError`:
     - `ConnectionFailed`;
     - `HeartbeatTimeout`;
     - `MissionRequestTimeout`;
     - `UnexpectedRequestSeq`;
     - `MissionRejected`;
     - `UnsupportedFrame`;
     - `Conversion`;
     - optional `WriteFailed` / `ReadFailed`, если нужно разделить IO errors.

   API уровня transport:

   ```rust
   impl MavlinkTransport {
       pub fn upload_mission(
           &mut self,
           waypoints: &[Waypoint],
           options: MissionUploadOptions,
       ) -> Result<MissionUploadReport, MavlinkMissionError>;
   }
   ```

2. Ввести test seam для MAVLink protocol без PX4.

   В `crates/swarm-comms/src/mavlink.rs` добавить internal trait, например
   `MavlinkMissionConnection`, который умеет:

   - `send_message(MavMessage)`;
   - `try_recv_message()` или `recv_until(deadline)`;
   - возвращать `(MavHeader, MavMessage)` там, где нужен system/component id.

   Для production адаптировать текущий `mavlink::Connection<MavMessage>`.
   Для tests сделать scripted fake connection:

   - входящий script: HEARTBEAT, MISSION_REQUEST_INT seq=0, ..., MISSION_ACK;
   - запись исходящих messages для assertions;
   - возможность вернуть timeout/no message;
   - возможность вернуть wrong seq или rejected ACK.

   Если trait требует mavlink types, tests запускать с
   `--features mavlink-transport`. Это нормально: M44 protocol существует только
   под feature.

3. Реализовать mission upload state machine.

   Последовательность:

   1. Wait heartbeat до `options.timeout`.
      - сохранить `target_system`/`target_component` из options;
      - если в будущем захотим auto-detect target из heartbeat, это отдельное
        расширение, не обязательное для M44.
   2. Если `clear_existing = true`, отправить `MISSION_CLEAR_ALL`.
      - Для M44 достаточно отправки clear; ожидание отдельного ack можно не
        делать, если выбран стандартный workflow count/request/ack.
   3. Отправить `MISSION_COUNT`.
      - count = `waypoints.len()`;
      - zero waypoints вернуть typed error до handshake.
   4. Для каждого waypoint ждать request:
      - primary: `MISSION_REQUEST_INT`;
      - fallback: `MISSION_REQUEST`;
      - request `seq` должен совпадать с ожидаемым seq;
      - wrong seq -> `UnexpectedRequestSeq { expected, actual }`.
   5. На каждый request отправить `MISSION_ITEM_INT`.
   6. После последнего item ждать `MISSION_ACK`.
      - `MAV_MISSION_ACCEPTED` -> success report;
      - любой rejected result -> `MissionRejected`.
   7. Timeout на heartbeat/request/ack -> typed timeout error.
   8. Retry semantics:
      - на M44 сделать retries на уровне whole upload attempt;
      - retry должен очищать internal attempt state;
      - не делать бесконечные циклы.

4. Исправить waypoint -> MAVLink item conversion.

   Текущий M43 contract local, а PX4 mission upload должен получить конкретный
   MAVLink frame. Рекомендованный M44 путь:

   - ввести `MissionHomeOrigin { lat_deg, lon_deg, alt_m }`;
   - для PX4 SITL дать safe documented default или CLI options в `sitl_agent`;
   - конвертировать local metres offset `x/y` в approximate WGS84 lat/lon:
     - `lat = origin.lat + north_m / meters_per_degree_lat`;
     - `lon = origin.lon + east_m / meters_per_degree_lon(origin.lat)`;
     - `z = origin.alt_m + waypoint.z` или relative altitude, в зависимости от
       выбранного frame;
   - использовать `MAV_FRAME_GLOBAL_RELATIVE_ALT_INT`, если `z` трактуется как
     relative altitude;
   - явно reject unsupported frame через `MavlinkMissionError::UnsupportedFrame`.

   Важно: не использовать `pose.x * 1.0e-7` как в текущем
   `task_to_mavlink_waypoint()`. Это выглядит как обратный scale и дает
   некорректные координаты.

5. Заменить connection path в `crates/swarm-examples/src/bin/sitl_agent.rs`.

   Сейчас feature path строит debug string и вызывает `transport.send(raw)`.
   Нужно:

   - собрать `Vec<swarm_comms::Waypoint>` из `SitlPlan`;
   - создать `MissionUploadOptions` с documented defaults:
     - target system/component, например `1/1`, если не задаются CLI options;
     - timeout/retry defaults;
     - clear existing default;
     - home origin default for PX4 SITL или explicit CLI args;
   - вызвать `transport.upload_mission(&waypoints, options)`;
   - stdout/stderr должен показывать accepted/rejected summary;
   - no-feature path должен остаться как сейчас: stable `FeatureMissing`;
   - `--mock` и `--dry-run` не должны менять внешний контракт.

6. Не ломать существующий `Transport for MavlinkTransport` резко.

   M44 может оставить `Transport::send()` для обратной совместимости, но SITL
   real path больше не должен его использовать. Лучше:

   - либо оставить `send()` как legacy/debug transport и документировать, что
     mission upload идет через `upload_mission`;
   - либо вернуть typed `MavlinkError::NotConnected`/unsupported для raw mission
     misuse, если это безопасно.

   Главное требование M44: `sitl_agent --connection` не должен отправлять
   `RAW_RPM` заглушку и не должен оборачивать MAVLink packets в debug string.

7. Обновить docs.

   В `docs/SITL_SETUP.md`:

   - заменить "Not a full mission upload workflow yet" на описание M44
     feature-gated mission upload;
   - добавить предупреждение, что M44 только uploads mission, но не arm/takeoff;
   - описать required PX4 SITL endpoint;
   - описать coordinate origin/defaults and limitations;
   - добавить troubleshooting для heartbeat timeout, request timeout, rejected
     mission ack.

   В `README.md`:

   - обновить Current Status:
     - SITL Dry-Run остается stable M43;
     - Real PX4 / Mission Upload становится experimental M44, не execution;
   - Quick Start может оставить dry-run/mock, а PX4 upload дать ссылкой на
     `docs/SITL_SETUP.md`, чтобы README не выглядел как hardware-ready guide;
   - Known Limitations должен прямо говорить: upload != arm/takeoff/execute.

8. Финальная verification для реализации M44.

   Выполнить:

   ```bash
   cargo fmt --all
   make clippy
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms --features mavlink-transport mission
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --features mavlink-transport sitl
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent
   ```

   Если `make clippy` отсутствует, использовать:

   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

   Manual PX4 SITL command можно документировать, но не считать обязательным
   automated verification.

## Testing strategy

### 1. Tests that need no refactoring

Эти tests должны идти вместе с основной реализацией M44.

- `crates/swarm-comms/src/mavlink.rs`: waypoint conversion creates
  `MISSION_ITEM_INT` with expected seq/current/autocontinue/target ids/frame.
- `crates/swarm-comms/src/mavlink.rs`: local waypoint + configured home origin
  converts to stable expected lat/lon int and relative altitude.
- `crates/swarm-comms/src/mavlink.rs`: empty mission returns typed error before
  any messages are sent.
- `crates/swarm-comms/src/mavlink.rs`: unsupported frame returns
  `MavlinkMissionError::UnsupportedFrame`.
- `crates/swarm-comms/src/mavlink.rs`: fake connection happy path:
  HEARTBEAT -> MISSION_COUNT -> MISSION_REQUEST_INT seqs -> MISSION_ITEM_INT
  seqs -> MISSION_ACK accepted.
- `crates/swarm-comms/src/mavlink.rs`: fallback path accepts
  `MISSION_REQUEST` when `MISSION_REQUEST_INT` is not used.
- `crates/swarm-comms/src/mavlink.rs`: wrong requested seq returns
  `UnexpectedRequestSeq`.
- `crates/swarm-comms/src/mavlink.rs`: rejected final `MISSION_ACK` returns
  `MissionRejected`.
- `crates/swarm-comms/src/mavlink.rs`: heartbeat timeout returns
  `HeartbeatTimeout`.
- `crates/swarm-comms/src/mavlink.rs`: mission request timeout returns
  `MissionRequestTimeout`.
- `crates/swarm-comms/src/mavlink.rs`: ack timeout returns a typed timeout
  error.
- `crates/swarm-examples/src/bin/sitl_agent.rs` / tests: no-feature
  `--connection` path still returns `FeatureMissing`.
- `crates/swarm-examples/tests/sitl_agent.rs`: `--mock` and `--dry-run` output
  contracts still pass after adding upload API.

### 2. Tests that need light refactoring

- Introduce `MavlinkMissionConnection` trait or equivalent seam so protocol
  tests do not require PX4.
- Add scripted fake connection builder:
  - `with_heartbeat`;
  - `request_int(seq)`;
  - `request(seq)`;
  - `ack_accepted`;
  - `ack_rejected`;
  - `timeout`.
- Add helper assertions for outgoing message order:
  - first clear/count;
  - item seq order;
  - target system/component;
  - no `RAW_RPM`.
- Add helper in `swarm-examples` or `sitl_plan` for converting `SitlPlan` to
  `Vec<Waypoint>` so CLI tests do not duplicate mapping logic.
- Add feature-gated compile/test command in docs or CI notes if current CI does
  not run `mavlink-transport` tests by default.

### 3. Tests that need heavy refactoring

- Real PX4 SITL integration test.
  - It should be `#[ignore]` or manual by default.
  - It requires external PX4 process, UDP endpoint and simulator lifecycle.
  - It should verify actual accepted/rejected mission upload against PX4 SITL.
  - It must not be part of default portable CI until simulator orchestration is
    added.

Осознанные gaps M44:

- No arm/takeoff/execute automated test: это M46.
- No telemetry/task completion mapping: это M47.
- No multi-agent upload: это M52.
- No safety pre-upload validation beyond basic empty/unsupported frame errors:
  это M45.

## Risks and tradeoffs

- MAVLink mission protocol is stateful. Main risk: upload state machine can pass
  fake tests but still mismatch PX4 timing/ordering. Mitigation: keep fake tests
  close to MAVLink handshake and add an ignored/manual PX4 SITL script.
- Coordinate conversion is a real contract risk. M43 local coordinates must not
  be silently treated as global lat/lon. M44 must introduce explicit home origin
  and document default PX4 SITL assumptions.
- `MISSION_REQUEST` fallback can hide protocol ambiguity. Tests must cover both
  INT and non-INT request paths.
- Timeout/retry behavior can make tests slow or flaky. Use injectable clock/fake
  timeout behavior where possible; keep unit tests deterministic and fast.
- `MavlinkTransport::send()` currently sends `RAW_RPM`; removing or changing it
  may affect any hidden consumers. `rg` currently shows SITL as the main real
  path, but implementation should re-check before changing public behavior.
- `--connection` with feature will become more real. Error messages may change
  from "waypoints sent" to accepted/rejected upload summary; docs/tests must be
  updated together.
- README can overpromise. It must distinguish mission upload from mission
  execution and real hardware readiness.

## Open questions

1. Какой default home origin использовать для PX4 SITL?
   Рекомендация: выбрать documented PX4 SITL default only if confirmed locally
   from PX4 docs/setup; otherwise require explicit CLI/config origin and keep
   README conservative.
2. Должны ли target system/component быть CLI options в M44?
   Рекомендация: добавить defaults `1/1` plus optional CLI flags only if they do
   not bloat scope; internal options type должен поддерживать оба поля сразу.
3. Нужно ли ждать ack после `MISSION_CLEAR_ALL`?
   Рекомендация: для M44 можно отправлять clear opportunistically and proceed to
   `MISSION_COUNT`, но если fake/PX4 behavior показывает clear ack dependency,
   добавить typed clear timeout.
4. Где держать fake connection?
   Рекомендация: рядом с `mavlink.rs` test module, чтобы не экспортировать test
   API публично.
5. Что делать с `task_to_mavlink_waypoint()`?
   Рекомендация: либо заменить на новый conversion helper с explicit origin/frame,
   либо оставить deprecated wrapper только для tests/backward compatibility, но
   не использовать в upload path.
