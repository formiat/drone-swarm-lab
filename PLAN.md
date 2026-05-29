# PLAN.md - M43 SITL Contract & Dry-Run Foundation

## Context

Планируем M43 из `docs_raw/DRONE_A.17.md`: portable SITL contract and dry-run
foundation перед настоящим PX4/MAVLink workflow.

Текущий статус кода:

- `crates/swarm-examples/src/bin/sitl_agent.rs` уже поддерживает `--mock` и
  `--connection <addr>`, но режимы не разделены строго: можно не указать режим,
  неизвестные аргументы молча игнорируются, `--dry-run` отсутствует.
- CLI сам загружает scenario suite, фильтрует tasks with `pose`, конвертирует их
  в waypoint messages и печатает вывод. Это мешает unit-тестам на extraction,
  formatting и typed errors.
- `swarm_comms::task_to_waypoint()` в
  `crates/swarm-comms/src/mavlink.rs` берет `pose.x`/`pose.y`, но выставляет
  `z = 0.0`, поэтому altitude contract сейчас неочевиден.
- `docs/SITL_SETUP.md` описывает mock и experimental PX4 path, но не описывает
  dry-run mode и не фиксирует coordinate-frame limitations.
- `README.md` содержит mock SITL quick start и статус Real PX4 как
  experimental, но после M43 должен явно показать новую безопасную ступень:
  mock -> dry-run -> feature-gated PX4 SITL.

Цель M43: получить команду

```bash
cargo run --bin sitl_agent -- \
  --dry-run --scenario scenarios/sitl.waypoints.json --agent-id agent-0
```

которая печатает mission upload plan без подключения к PX4, включая agent id,
scenario path/name, task ids, waypoint sequence, coordinates, frame и altitude
interpretation.

В scope M43 не входят:

- настоящий MAVLink mission upload;
- arm/takeoff/execute;
- telemetry loop;
- multi-agent SITL;
- real hardware workflow.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует, поэтому отдельного investigation
artifact для этой задачи нет.

Прочитанный локальный контекст:

- `.agent-io/inbox.txt`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`;
- `docs_raw/DRONE_A.17.md`;
- `crates/swarm-examples/src/bin/sitl_agent.rs`;
- `crates/swarm-examples/src/lib.rs`;
- `crates/swarm-examples/Cargo.toml`;
- `crates/swarm-comms/src/mavlink.rs`;
- `crates/swarm-types/src/pose.rs`;
- `crates/swarm-sim/src/dsl.rs`;
- `docs/SITL_SETUP.md`;
- `README.md`;
- `scenarios/sitl.waypoints.json`;
- существующие CLI test patterns в `crates/swarm-examples/tests/*`.

Notion policy в inbox: `optional`. Notion task id в prompt не указан, поэтому
Notion CLI не вызывался. GitLab/MR target в prompt не указан, поэтому `glab` не
вызывался. Удаленные SSH/HTTP обращения не выполнялись.

## Affected components

- `crates/swarm-examples/src/bin/sitl_agent.rs` - строгий CLI mode parsing,
  dispatch `--mock` / `--dry-run` / `--connection <addr>`, стабильные exit
  errors.
- `crates/swarm-examples/src/sitl_plan.rs` - новый тестируемый helper для SITL
  planning: scenario loading, waypoint extraction, coordinate interpretation,
  dry-run formatting, typed errors.
- `crates/swarm-examples/src/lib.rs` - экспорт helper module для unit и
  integration tests.
- `crates/swarm-examples/Cargo.toml` - добавить `thiserror = { workspace = true }`,
  если typed errors будут жить в `swarm-examples`.
- `crates/swarm-comms/src/mavlink.rs` - проверить, нужно ли менять
  `task_to_waypoint()` на использование `pose.z`; если изменение общего helper
  увеличивает blast radius, оставить его совместимым и использовать новый
  M43-specific planning helper в `swarm-examples`.
- `crates/swarm-examples/tests/sitl_agent.rs` - новые CLI integration tests для
  dry-run and validation paths.
- `docs/SITL_SETUP.md` - обновить mode matrix: mock, dry-run, PX4 SITL,
  warning по real hardware.
- `README.md` - обязательно обновить quick start/current status, чтобы dry-run
  был видимым рекомендуемым шагом перед PX4 SITL.

## Implementation steps

1. Вынести SITL planning helper в `crates/swarm-examples/src/sitl_plan.rs`.

   Добавить типы:

   - `SitlMode` с вариантами `Mock`, `DryRun`, `Connection { addr: String }`;
   - `SitlCoordinateFrame`, на M43 поддержать только local simulation frame;
   - `SitlWaypointPlan` / `SitlWaypointItem` с `task_id`, `seq`, coordinates,
     frame, altitude source;
   - `SitlPlan` с `agent_id`, `scenario_path`, `suite_name`, `scenario_name`,
     `mission`, `profile`, `waypoints`;
   - `SitlError` через `thiserror`:
     `InvalidScenario`, `NoPoseTasks`, `FeatureMissing`, `BadConnectionString`,
     `UnsupportedCoordinateFrame`.

   Helper должен принимать `Path`/`PathBuf`, загружать suite через
   `swarm_sim::load_scenario_suite`, прогонять `validate_scenario_suite`,
   выбирать первую scenario entry как текущий CLI contract и возвращать typed
   error вместо `eprintln! + exit`.

2. Зафиксировать coordinate-frame contract в коде helper.

   На M43 контракт должен быть узким и честным:

   - `Pose { x, y, z }` для `sitl_agent` трактуется как local simulation
     coordinates;
   - `x` и `y` не являются WGS84 latitude/longitude;
   - `z` трактуется как altitude relative to local origin; если `z` отсутствует
     в JSON, serde default дает `0.0`;
   - dry-run печатает frame как local simulation frame и явно пишет, что real
     PX4/global conversion появится позже, в M44;
   - unsupported/global frame должен возвращать `UnsupportedCoordinateFrame`,
     если в рамках реализации появится явное поле/опция frame.

3. Переписать `crates/swarm-examples/src/bin/sitl_agent.rs` вокруг helper.

   Требования к CLI:

   - `--mock`, `--dry-run`, `--connection <addr>` должны быть mutually exclusive;
   - отсутствие режима должно давать стабильную ошибку;
   - неизвестный аргумент должен давать стабильную ошибку, а не игнорироваться;
   - `--scenario <path>` обязателен;
   - `--agent-id <id>` оставить с текущим explicit usage; если решим добавить
     default, это должно быть отражено в tests/docs;
   - `--dry-run` печатает mission upload plan в stdout и не создает MAVLink
     connection;
   - `--mock` остается portable and CI-friendly, использует тот же extracted
     waypoint plan, но по-прежнему отправляет waypoints в
     `MockMavlinkTransport`;
   - `--connection <addr>` без feature `mavlink-transport` возвращает
     `FeatureMissing` со стабильным текстом и build instruction;
   - `--connection <addr>` с заведомо плохим syntactic addr возвращает
     `BadConnectionString`; достаточно lightweight validation для форматов,
     которые планируется поддерживать (`udp:...`, позже `tcp:`/`serial:`), без
     сетевого подключения в M43.

4. Стабилизировать dry-run output format.

   Формат должен быть читаемым и достаточно стабильным для integration tests.
   Минимальные поля:

   - `mode: dry-run`;
   - `agent_id`;
   - `scenario_path`;
   - `suite_name`;
   - `scenario_name`;
   - `mission`;
   - `profile`;
   - `coordinate_frame`;
   - `altitude_source`;
   - список waypoint rows: `seq`, `task_id`, `x`, `y`, `z`.

   Лучше сделать formatting отдельной pure function, например
   `format_dry_run_plan(&SitlPlan) -> String`, чтобы unit tests не запускали
   бинарь ради проверки строк.

5. Уточнить взаимодействие с `swarm_comms::Waypoint`.

   Перед кодом еще раз поискать все uses `task_to_waypoint` и `Waypoint`.
   Если безопасно, поправить `task_to_waypoint()` так, чтобы `z = pose.z`, и
   обновить существующие unit tests в `crates/swarm-comms/src/mavlink.rs`.
   Если это может изменить semantics слишком широко, оставить shared helper как
   есть, а M43-specific altitude contract реализовать в `sitl_plan.rs`.

   Критерий выбора: M43 должен не размывать общий MAVLink API до полноценного
   mission upload. Настоящая MAVLink/global frame конверсия остается задачей M44.

6. Добавить tests без PX4.

   Новые tests должны быть self-contained and portable: inline scenarios или
   `tempfile`, без `$HOME`, абсолютных путей, внешнего simulator и сетевых
   подключений.

   Основной фокус:

   - pure helper tests в `sitl_plan.rs`;
   - integration tests через `env!("CARGO_BIN_EXE_sitl_agent")` в
     `crates/swarm-examples/tests/sitl_agent.rs`;
   - negative paths для typed errors and CLI validation.

7. Обновить `docs/SITL_SETUP.md`.

   Документ должен явно разделить:

   - mock mode: in-memory transport, CI-friendly;
   - dry-run mode: mission upload plan без PX4, рекомендуемый шаг перед PX4;
   - PX4 SITL mode: feature-gated experimental path;
   - real hardware warning: проект не готов к реальному железу, нет certified
     safety, нет hardware readiness до будущего M53.

   Также добавить coordinate-frame section: local simulation frame, `Pose.z` as
   altitude, global/PX4 conversion not implemented in M43.

8. Обновить `README.md`.

   Обязательное изменение README:

   - добавить dry-run в Quick Start рядом с mock SITL;
   - обновить Current Status строку SITL, чтобы было видно:
     mock stable, dry-run planned/implemented by M43, real PX4 remains
     experimental;
   - в Non-Goals/Known Limitations не обещать real hardware readiness;
   - добавить ссылку на `docs/SITL_SETUP.md` как canonical SITL workflow.

9. Verification перед commit реализации M43.

   Для реализации M43 выполнить:

   ```bash
   cargo fmt --all
   make clippy
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl
   PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-comms mavlink
   ```

   Если `make clippy` недоступен или в repo есть approved equivalent, использовать
   `cargo clippy --all-targets -- -D warnings` и явно указать это в итоговом
   отчете. Для manual smoke допускается дополнительно:

   ```bash
   /home/formi/.local/bin/runlim cargo run --bin sitl_agent -- \
     --dry-run --scenario scenarios/sitl.waypoints.json --agent-id agent-0
   ```

   Этот manual smoke не заменяет automated tests.

## Testing strategy

### 1. Tests that need no refactoring

Эти tests должны идти вместе с основной реализацией M43.

- `crates/swarm-examples/src/sitl_plan.rs`: helper extracts only tasks with
  `pose`, preserves task ids and assigns sequential `seq`.
- `crates/swarm-examples/src/sitl_plan.rs`: helper maps `Pose { x, y, z }` into
  dry-run waypoint coordinates and preserves `z` as altitude.
- `crates/swarm-examples/src/sitl_plan.rs`: missing `z` in inline JSON becomes
  altitude `0.0` and output says the altitude source is `pose.z` with serde
  default.
- `crates/swarm-examples/src/sitl_plan.rs`: scenario with zero pose tasks returns
  `SitlError::NoPoseTasks`.
- `crates/swarm-examples/src/sitl_plan.rs`: invalid scenario suite returns
  `SitlError::InvalidScenario` with validation details.
- `crates/swarm-examples/src/sitl_plan.rs`: `format_dry_run_plan()` includes
  agent id, scenario path/name, mission/profile, frame, altitude source, task ids
  and waypoint sequence.
- `crates/swarm-examples/tests/sitl_agent.rs`: CLI with no mode returns stable
  non-zero error.
- `crates/swarm-examples/tests/sitl_agent.rs`: CLI with both `--mock` and
  `--dry-run` returns stable non-zero error.
- `crates/swarm-examples/tests/sitl_agent.rs`: CLI `--connection udp:127.0.0.1:14550`
  without `mavlink-transport` feature returns stable `FeatureMissing` text.
- `crates/swarm-examples/tests/sitl_agent.rs`: CLI `--dry-run --scenario <tempfile>`
  succeeds and stdout contains the expected waypoint table.
- `crates/swarm-examples/tests/sitl_agent.rs`: CLI bad connection string returns
  stable `BadConnectionString` text and does not attempt network connection.
- `crates/swarm-comms/src/mavlink.rs`: if `task_to_waypoint()` is changed, update
  existing `task_to_waypoint_with_pose` test to assert `z == pose.z`.

### 2. Tests that need light refactoring

- Вынести reusable helper for invoking `sitl_agent` binary in
  `crates/swarm-examples/tests/sitl_agent.rs`, preferably через
  `env!("CARGO_BIN_EXE_sitl_agent")`, чтобы не запускать nested `cargo run`.
- Сделать shared inline SITL scenario fixture builder для helper tests and CLI
  tests. Fixture должен писать JSON во временный файл только внутри `tempfile`.
- Добавить reusable CLI error assertions: status non-zero, stderr contains
  stable error code/string, stdout empty for failure.
- Если понадобится проверять docs examples, добавить lightweight doctest-style
  command snippets только после того, как CLI output format стабилизирован.

### 3. Tests that need heavy refactoring

Heavy refactoring tests для M43 не нужны.

Осознанные gaps:

- Настоящий PX4/SITL e2e не покрывается в M43, потому что real MAVLink upload и
  simulator lifecycle входят в M44/M48.
- Arm/takeoff/execute/telemetry не покрываются, потому что это scope M46/M47.
- Multi-agent SITL не покрывается, потому что это scope M52.

## Risks and tradeoffs

- Строгий CLI parsing может сломать старые неявные сценарии, где пользователь
  случайно передавал лишние аргументы и они молча игнорировались. Это желаемое
  ужесточение, но его нужно отметить в docs.
- Если поменять `swarm_comms::task_to_waypoint()` на `z = pose.z`, может
  измениться поведение mock tests and future MAVLink conversion. Перед этим
  нужно проверить все uses и ограничить изменение, если оно выходит за M43.
- Coordinate-frame contract должен быть честным: current `Pose.x/y` нельзя
  выдавать за global lat/lon. Лучше явно назвать local simulation frame, чем
  обещать PX4-ready global conversion раньше M44.
- Dry-run output станет тестируемым контрактом. Если сделать его слишком
  verbose/случайным, будущие изменения будут чаще ломать tests. Поэтому формат
  должен быть простым и стабильным.
- Добавление `thiserror` в `swarm-examples` увеличивает dependency surface
  минимально, но workspace dependency уже существует.
- README/SITL docs могут создать ложное впечатление hardware readiness. Нужно
  явно писать, что M43 не подключает реальные дроны и не выполняет mission.

## Open questions

1. Нужно ли сделать `--agent-id` обязательным и дальше, или дать default
   `agent-0` для demo ergonomics? Рекомендация: оставить обязательным в M43,
   чтобы dry-run явно показывал, для какого agent строится plan.
2. Должен ли dry-run output идти в stdout, а diagnostics/errors в stderr?
   Рекомендация: да, stdout для mission upload plan, stderr для ошибок.
3. Валидировать ли `--connection` синтаксически до feature gate?
   Рекомендация: для валидной строки без feature возвращать `FeatureMissing`;
   для явно плохой строки возвращать `BadConnectionString` без network access.
4. Добавлять ли сейчас CLI option для frame, например `--frame local|global`?
   Рекомендация: не добавлять в M43. Поддержать только documented local frame,
   а global/PX4 conversion вынести в M44.
5. Нужно ли переносить SITL planning helper в `swarm-comms`, а не
   `swarm-examples`? Рекомендация: начать в `swarm-examples`, потому что M43
   описывает CLI contract. В `swarm-comms` переносить только reusable MAVLink
   protocol pieces на M44.
