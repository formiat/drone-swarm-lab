# Context

Планируется milestone **M53 - Hardware Readiness Boundary** из
`docs_raw/DRONE_A.17.md`.

Цель M53: явно отделить уже проверенный `mock` / `dry-run` / PX4 SITL workflow
от любых заявлений о готовности к реальным дронам. После M43-M52 в проекте уже
есть portable SITL path, экспериментальный single-agent PX4 SITL connection
path, telemetry/report/replay plumbing, dynamic reallocation foundation и
multi-agent SITL config/manifest foundation. Но это всё еще не production
flight-control system и не hardware-ready продукт.

Главное решение для реализации: сделать не только документационное
предупреждение, но и небольшой кодовый safety/UX boundary. Для
`hardware-looking` connections нужно добавить явную классификацию и защиту от
случайного запуска. Рекомендуемый контракт: локальный PX4 SITL UDP
(`udp:127.0.0.1:*`, `udp:localhost:*`, `udp:[::1]:*`) остается обычным
экспериментальным SITL path; `serial:*`, `tcp:*` и `udp:*` с non-loopback host
считаются hardware candidate и требуют явного opt-in флага вроде
`--allow-hardware-candidate`.

README нужно обязательно актуализировать: текущий README уже говорит, что проект
research prototype, но после M53 должен прямо ссылаться на
`docs/HARDWARE_READINESS.md`, показывать новый boundary в Current Status /
Known Limitations и документировать CLI guard для hardware candidate.

# Investigation context

`INVESTIGATION.md` в корне workspace отсутствует, дополнительных investigation
constraints нет.

Прочитанные обязательные протоколы:

- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`.

Notion/GitLab чтение не выполнялось: текущий prompt не содержит Notion task ID,
GitLab MR или review target, а `notion_policy=optional`.

Кодовая и документационная картина:

- `crates/swarm-examples/src/sitl_plan.rs` сейчас содержит
  `validate_connection_string`, принимает `udp`, `tcp`, `serial`, но не
  классифицирует connection по hardware risk.
- `crates/swarm-examples/src/bin/sitl_agent.rs` валидирует `--connection`,
  загружает safety config и затем идет в `run_connection`; отдельного warning /
  guard для remote/serial нет.
- `crates/swarm-examples/tests/sitl_agent.rs` уже содержит CLI tests для
  `--connection`, safety validation, bad connection string и no-feature
  `mavlink-transport` path; туда естественно добавить hardware boundary
  regression tests.
- `crates/swarm-examples/tests/sitl_docs.rs` уже проверяет README и
  `docs/SITL_SETUP.md` anchors; туда можно добавить `docs/HARDWARE_READINESS.md`
  и обязательные hardware-readiness anchors.
- `docs/SITL_SETUP.md` уже содержит `Real Hardware Warning`, но это короткий
  warning без operator checklist и без connection classes.
- `README.md` уже имеет честный `research prototype` статус и Known
  Limitations, но M53 должен сделать hardware boundary более явным и
  discoverable.

# Affected components

- `crates/swarm-examples/src/sitl_plan.rs`:
  добавить типы и функции классификации connection string, переиспользуя
  существующий parsing/validation path вместо параллельного ad hoc parser.
- `crates/swarm-examples/src/bin/sitl_agent.rs`:
  добавить CLI flag для explicit opt-in, встроить connection classification в
  connection path, печатать warning для hardware candidate при явном opt-in и
  возвращать typed error без opt-in.
- `crates/swarm-examples/src/sitl_multi_agent.rs`:
  проверить, что multi-agent config validation продолжает использовать общий
  connection validation; при необходимости явно не дублировать classifier в
  manifest builder, а использовать shared helper из `sitl_plan.rs`.
- `crates/swarm-examples/tests/sitl_agent.rs`:
  добавить CLI regression tests для local SITL connection, remote/serial
  hardware candidate без opt-in, hardware candidate с opt-in, и multi-agent
  config-implied connection.
- `crates/swarm-examples/tests/sitl_docs.rs`:
  расширить docs tests на `docs/HARDWARE_READINESS.md`, README anchors и
  `docs/SITL_SETUP.md` anchors.
- `docs/HARDWARE_READINESS.md`:
  новый основной документ M53.
- `docs/SITL_SETUP.md`:
  обновить Real Hardware Warning, troubleshooting и connection classes.
- `README.md`:
  обновить Quick Start / Current Status / Known Limitations / portable SITL
  checks, добавить ссылку на hardware-readiness boundary.

# Implementation steps

1. В `crates/swarm-examples/src/sitl_plan.rs` выделить shared parsing helper для
   connection string.
   - Сейчас `validate_connection_string` валидирует строку, но не возвращает
     scheme/host/port/path.
   - Добавить компактный внутренний parsed representation, например
     `ParsedSitlConnection`.
   - `validate_connection_string` оставить публично совместимым и реализовать
     через новый parser.

2. В `crates/swarm-examples/src/sitl_plan.rs` добавить публичную классификацию
   connection boundary.
   - Например:
     - `SitlConnectionClass::LocalPx4SitlUdp`;
     - `SitlConnectionClass::HardwareCandidate`.
   - Добавить `classify_connection_string(addr: &str) -> Result<SitlConnectionClass, SitlError>`.
   - Правила:
     - `udp` + loopback host (`127.0.0.1`, `localhost`, `[::1]`, `::1`) =
       `LocalPx4SitlUdp`;
     - `serial:*` = `HardwareCandidate`;
     - `tcp:*` = `HardwareCandidate`;
     - `udp:*` с non-loopback host = `HardwareCandidate`;
     - malformed input остается `BadConnectionString`.
   - Не менять acceptance существующих valid connection strings: `udp`, `tcp`,
     `serial` остаются syntactically valid; меняется только hardware boundary
     policy в CLI.

3. В `crates/swarm-examples/src/sitl_plan.rs` добавить typed error для
   hardware candidate без explicit opt-in.
   - Например `SitlError::HardwareCandidateRequiresExplicitAllow { addr, class }`.
   - Error message должен быть action-oriented: объяснить, что connection похож
     на real hardware / remote endpoint и требует
     `--allow-hardware-candidate`.

4. В `crates/swarm-examples/src/bin/sitl_agent.rs` добавить CLI flag
   `--allow-hardware-candidate`.
   - Добавить поле в `CliArgs`.
   - Обновить usage string.
   - Для `SitlMode::Connection { addr }` после `validate_connection_string` и
     до попытки создать MAVLink transport выполнить classification.
   - Если class = `HardwareCandidate` и flag не задан: вернуть typed error.
   - Если class = `HardwareCandidate` и flag задан: напечатать explicit warning
     в stderr. Warning должен говорить, что это не production hardware workflow,
     что оператор обязан использовать checklist из `docs/HARDWARE_READINESS.md`,
     и что проект не предоставляет certified safety layer.
   - Local PX4 SITL UDP не должен требовать flag и не должен получать hardware
     warning.

5. Проверить multi-agent path в `crates/swarm-examples/src/bin/sitl_agent.rs`.
   - В multi-agent режиме `mode` может браться из `agent.connection_string`,
     если CLI `--connection` не задан.
   - Тот же hardware boundary guard должен срабатывать для config-implied
     connection.
   - `--allow-hardware-candidate` должен применяться и к explicit CLI
     connection override, и к connection из multi-agent config.

6. Обновить `docs/HARDWARE_READINESS.md`.
   Документ должен содержать:
   - status summary: проект не hardware-ready;
   - verified matrix: mock, dry-run, portable regression, single-agent PX4 SITL,
     multi-agent SITL foundation;
   - not verified on hardware: airframe-specific failsafes, radio/link loss,
     GNSS/estimator behavior, real battery model, real obstacle avoidance,
     pilot handoff, certified geofence, flight termination, hardware-in-the-loop
     CI;
   - safety assumptions;
   - operator checklist before any hardware experiment:
     physical kill switch, geofence, manual pilot override, low-risk controlled
     environment, no autonomous flight outside controlled test, propeller/bench
     safety where applicable, PX4 params reviewed, logs enabled, emergency RTL /
     disarm procedure rehearsed;
   - explicit statement that this document is not flight certification.

7. Обновить `docs/SITL_SETUP.md`.
   - Расширить `Real Hardware Warning`.
   - Добавить subsection про connection classes:
     `mock`, `dry-run`, `local PX4 SITL UDP`, `hardware candidate`.
   - Добавить пример expected CLI error для hardware candidate без
     `--allow-hardware-candidate`.
   - Добавить troubleshooting entry для hardware boundary error.

8. Обновить `README.md`.
   - В Quick Start рядом с PX4 SITL добавить ссылку на
     `docs/HARDWARE_READINESS.md`.
   - В Current Status добавить строку M53 или обновить Real PX4 notes так, чтобы
     было явно: hardware readiness boundary documented, hardware execution
     remains unsupported / guarded.
   - В Known Limitations уточнить, что remote/serial hardware candidate требует
     explicit opt-in и не означает hardware readiness.
   - Проверить, что README не обещает production safety или real swarm hardware
     readiness.

9. Обновить `crates/swarm-examples/tests/sitl_docs.rs`.
   - Добавить `const HARDWARE_READINESS: &str = include_str!(...)`.
   - Проверять наличие anchors/фраз:
     `Hardware Readiness`, `operator checklist`, `physical kill switch`,
     `manual pilot override`, `low-risk`, `not flight certification`,
     `--allow-hardware-candidate`.
   - Проверить, что README и `docs/SITL_SETUP.md` ссылаются на
     `docs/HARDWARE_READINESS.md`.

10. Добавить unit tests в `crates/swarm-examples/src/sitl_plan.rs`.
    - `classifies_loopback_udp_as_local_px4_sitl`.
    - `classifies_remote_udp_as_hardware_candidate`.
    - `classifies_tcp_as_hardware_candidate`.
    - `classifies_serial_as_hardware_candidate`.
    - malformed strings still return `BadConnectionString`.

11. Добавить integration tests в `crates/swarm-examples/tests/sitl_agent.rs`.
    - Remote UDP without `--allow-hardware-candidate` fails before feature error
      and includes the flag name.
    - Serial without `--allow-hardware-candidate` fails before feature error.
    - Local `udp:127.0.0.1:14550` keeps existing no-feature behavior when
      `mavlink-transport` is disabled and does not mention hardware candidate.
    - Remote UDP with `--allow-hardware-candidate` emits warning and then, when
      built without `mavlink-transport`, reaches the existing feature-missing
      error.
    - Multi-agent config-implied remote/serial connection follows the same
      guard.

12. Run formatting and verification after implementation.
    - `cargo fmt --all`.
    - `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
    - Targeted tests listed below.
    - `git diff --check`.
    - `find . -name '*.proptest-regressions' -print`.

# Testing strategy

## 1. Tests that need no refactoring - planned with implementation

- Unit: `crates/swarm-examples/src/sitl_plan.rs`
  - local loopback UDP classification:
    `udp:127.0.0.1:14550`, `udp:localhost:14550`, optionally
    `udp:[::1]:14550`;
  - remote UDP classification:
    `udp:192.168.1.10:14550`, `udp:10.0.0.5:14550`;
  - TCP classification:
    `tcp:localhost:5760`, `tcp:192.168.1.10:5760`;
  - serial classification:
    `serial:/dev/ttyUSB0:57600`;
  - malformed strings still return `SitlError::BadConnectionString`.

- Integration: `crates/swarm-examples/tests/sitl_agent.rs`
  - happy path / compatibility: local PX4 SITL UDP still reaches existing
    `feature missing` path when `mavlink-transport` is disabled;
  - negative path: remote UDP without `--allow-hardware-candidate` fails with
    hardware boundary error before feature missing;
  - negative path: serial without `--allow-hardware-candidate` fails with
    hardware boundary error before feature missing;
  - opt-in path: remote UDP with `--allow-hardware-candidate` prints warning and
    then reaches existing feature-missing path when feature is disabled;
  - config path: multi-agent config-implied hardware candidate is also guarded.

- Docs: `crates/swarm-examples/tests/sitl_docs.rs`
  - `docs/HARDWARE_READINESS.md` exists and contains operator checklist anchors;
  - README links to `docs/HARDWARE_READINESS.md`;
  - `docs/SITL_SETUP.md` documents connection classes and
    `--allow-hardware-candidate`;
  - docs still contain existing portable/manual boundary anchors.

Recommended commands:

```bash
timeout 300s cargo fmt --all
timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_connection_class
timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent hardware_candidate
timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs
timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings
timeout 300s git diff --check
timeout 300s find . -name '*.proptest-regressions' -print
```

## 2. Tests that need light refactoring

- CLI warning helper tests if the warning text is extracted from `run()` into a
  helper function, for example `hardware_candidate_warning(addr, class)`.
- Shared parser/classifier tests if `validate_connection_string` is refactored
  to return a parsed representation internally; the public API should remain
  stable, but tests may need to move from generic validation names to
  classifier-focused names.
- Multi-agent config helper fixture if the current JSON fixture helpers in
  `tests/sitl_agent.rs` need small extension to generate remote/serial
  connection strings without duplicating large JSON blocks.

## 3. Tests that need heavy refactoring

- Hardware-in-the-loop tests with real autopilot, serial device, bench setup,
  kill switch, PX4 params and safety operator loop. Do not add to default CI.
- Real remote PX4/SITL network tests against non-loopback endpoints. These need
  external simulator orchestration and should remain manual/ignored until there
  is a controlled test harness.
- End-to-end validation that `--allow-hardware-candidate` can safely operate on
  real hardware. This is outside M53 because M53 is a boundary/guard milestone,
  not certification.

Autotest gaps:

- Real hardware behavior is intentionally not covered by automated tests in
  M53. The gap is expected: the milestone prevents accidental claims and
  accidental enablement; it does not certify hardware behavior.
- Live PX4 SITL upload/execute is not part of the default test suite. Existing
  code keeps live PX4 verification manual/local.

# Risks and tradeoffs

- CLI compatibility: adding a hard gate for `serial`, `tcp`, and remote `udp`
  changes behavior for users who were already experimenting with those
  connection strings. This is intentional for M53, but the error must be clear
  and provide the opt-in flag.
- False positives in classifier: a non-loopback UDP endpoint might still be a
  local lab SITL VM/container. Treating it as hardware candidate is conservative
  and acceptable because the user can opt in explicitly.
- False negatives in classifier: loopback UDP could theoretically be forwarded
  to real hardware by external tooling. The docs should state that connection
  classification is a guardrail, not a safety guarantee.
- Documentation drift: if README, SITL setup, and hardware readiness docs repeat
  the same boundary in inconsistent language, users may misunderstand the
  status. Prefer one canonical `docs/HARDWARE_READINESS.md` and link to it.
- Error ordering: new hardware boundary errors must not mask earlier syntax
  validation errors. Malformed connection strings should still return
  `BadConnectionString`.
- Feature gating: tests without `mavlink-transport` should still be able to
  verify warning/guard behavior before transport creation.
- Multi-agent config path: because `--multi-agent-config` can imply connection
  mode, the guard must cover config-derived connection strings, not only CLI
  `--connection`.
- Performance/resources: classifier adds negligible overhead; no data/DB
  migrations are involved.

# Open questions

- Exact opt-in flag name is proposed as `--allow-hardware-candidate`. If the
  project prefers shorter wording, use `--allow-hardware`, but keep the docs and
  tests aligned.
- Whether `tcp:localhost:*` should be considered local SITL or hardware
  candidate. This plan treats all `tcp:*` as hardware candidate because
  `DRONE_A.17.md` names only local PX4 SITL UDP as the supported local class.
- Whether the opt-in should be warning-only or hard gate. This plan chooses hard
  gate because the expected result says real hardware path cannot be enabled
  accidentally; warning-only is weaker and easier to miss.
