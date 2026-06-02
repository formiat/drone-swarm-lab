# План M73 - Fault Injection And Degraded Supervisor

## Context

M73 продолжает pre-hardware цепочку из
`docs_raw/BEFORE_HARDWARE_A.23.md` после M72 Artifact Validator + SITL Harness.
Цель этапа - сделать поведение supervisor при отказах явным и проверяемым до
появления железа:

```text
detect -> classify -> decide -> recover/abort -> report
```

Текущая база уже умеет часть этого:

- `run_live_supervisor_with_controllers` запускает live controllers stepwise и
  делает active-survivor mission replacement при `--reupload-on-failure`
  (`crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:123`).
- Runtime reallocation уже выпускает `agent_lost`, `task_released`,
  `task_reassigned`, `reallocation_completed` через `record_reallocation_output`
  (`crates/swarm-examples/src/sitl_supervisor/reallocation.rs:187`).
- `LiveAgentRun` хранит per-agent final status/error/completed tasks
  (`crates/swarm-examples/src/sitl_supervisor/config.rs:149`).
- M72 `artifact_validator` уже проверяет SITL pack consistency, final status,
  replay summary, replacement seq и safety report
  (`crates/swarm-examples/src/artifact_validator.rs:110`).
- Fake live controller tests уже покрывают happy path, partial failure,
  reallocation, replacement seq и отсутствие active survivor
  (`crates/swarm-examples/src/sitl_supervisor/tests_cases.rs:163`).

Главный gap: failure semantics сейчас не являются первым-class контрактом.
Отказ виден как строковый `final_status`/`error`, а report/replay не несут
структурированную классификацию вида `failure_mode`, `decision`,
`tasks_abandoned`, `recovery_started`, `recovery_failed`. M73 должен добавить
этот слой без claims про hardware, RF modeling или exhaustive PX4 failsafe
testing.

## Investigation context

`INVESTIGATION.md` отсутствует, поэтому отдельного investigation artifact нет.

Изученные входные файлы и выводы:

- `docs_raw/BEFORE_HARDWARE_A.23.md:460` - исходный M73 scope: failure modes,
  supervisor decisions, report fields, replay events, fake tests, manual-only
  SITL checks, failure matrix docs.
- `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:123` - live
  loop уже запускает всех controllers, poll-ит active agents и вызывает
  reallocation/replacement после failed run.
- `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:230` - место,
  где failed live run превращается в reallocation decision.
- `crates/swarm-examples/src/sitl_supervisor/reallocation.rs:36` - текущая
  функция live reallocation не возвращает явного outcome, только
  `MissionReplacementPlan`.
- `crates/swarm-examples/src/sitl_supervisor/reallocation.rs:299` - сборка
  replacement mission из recovered tasks; здесь нужен явный отказ для bad
  task/waypoint и unsafe replacement.
- `crates/swarm-examples/src/sitl_supervisor/live.rs:145` - PX4 controller
  стартует текущую миссию; upload/start failures сейчас схлопываются в
  `failed_run(error, completed_waypoints)`.
- `crates/swarm-examples/src/sitl_supervisor/live.rs:185` - no-progress и
  heartbeat timeout уже детектятся, но не классифицируются как отдельные M73
  modes в report/replay.
- `crates/swarm-examples/src/sitl_supervisor/live.rs:296` - replacement upload
  failure уже переводится в failed run, но без явного `recovery_failed` event.
- `crates/swarm-examples/src/sitl_supervisor/config.rs:42` - `SupervisorMetrics`
  считает recovery quantities, но не хранит failure-mode/decision counts.
- `crates/swarm-examples/src/sitl_report.rs:44` - `SitlMultiAgentRunReport`
  расширяем через `#[serde(default)]`, чтобы не ломать старые artifacts.
- `crates/swarm-examples/src/sitl_observability/events.rs:39` - SITL event enum
  расширяется additive variants без смены schema, если старые logs остаются
  meaningful.
- `crates/swarm-examples/src/sitl_supervisor/tests_support.rs:414` - текущий
  `FakeLiveAgentController` слишком простой; M73 требует scenario builder для
  deterministic failures.
- `docs/ARTIFACT_VALIDATION.md` и `crates/swarm-examples/src/artifact_validator.rs`
  - M72 validator нужно расширить M73 checks, а не создавать отдельный
  валидатор.

## Affected components

- `crates/swarm-examples/src/sitl_supervisor/degraded.rs` - новый модуль с
  типами failure mode / decision / degraded outcome.
- `crates/swarm-examples/src/sitl_supervisor/mod.rs` - подключение нового
  модуля и re-export внутри supervisor boundary.
- `crates/swarm-examples/src/sitl_supervisor/config.rs` - расширение
  `SupervisorMetrics`, `LiveAgentRun`, возможно `MissionReplacementPlan`.
- `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs` - явная
  классификация отказов и запись degraded events в live loop.
- `crates/swarm-examples/src/sitl_supervisor/reallocation.rs` - возврат
  typed recovery outcome, tracking abandoned/recovered tasks, failure reason
  для replacement planning.
- `crates/swarm-examples/src/sitl_supervisor/live.rs` - map PX4/controller
  failures в structured failure modes.
- `crates/swarm-examples/src/sitl_supervisor/artifacts.rs` - report population
  для degraded summary.
- `crates/swarm-examples/src/sitl_report.rs` - additive report fields для M73.
- `crates/swarm-examples/src/sitl_observability/events.rs` - additive replay
  events и summary counters.
- `crates/swarm-examples/src/artifact_validator.rs` - M73 artifact rules.
- `crates/swarm-examples/tests/artifact_validator.rs` - validator fixture tests
  для degraded reports/events.
- `crates/swarm-examples/src/sitl_supervisor/tests_support.rs` - fake
  controller builder для deterministic failure сценариев.
- `crates/swarm-examples/src/sitl_supervisor/tests_cases.rs` - M73 unit tests.
- `crates/swarm-examples/tests/sitl_docs.rs` - docs smoke anchors.
- `README.md`, `docs/STATUS.md`, `docs/SITL_SETUP.md`,
  `docs/HARDWARE_READINESS.md`, `docs/REPLAY.md`,
  `docs/ARTIFACT_VALIDATION.md`, `docs/PREFLIGHT_SAFETY.md`,
  возможно `docs/EXTENSION_GUIDE.md` - docs/status sync.

## Implementation steps

1. Добавить typed degraded model.

   Файл: `crates/swarm-examples/src/sitl_supervisor/degraded.rs`.

   Результат: единый набор serde-friendly типов для report/replay/tests:

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum SupervisorFailureMode {
       AgentLostBeforeUpload,
       UploadRejected,
       AgentLostAfterUploadBeforeMissionStart,
       NoProgressTimeout,
       HeartbeatLost,
       StaleTelemetry,
       PartialCompletionThenFailure,
       ReplacementMissionRejected,
       SurvivorFailedAfterReplacement,
       UnsafeReplacementRoute,
       BadWaypointOrMissionItem,
       Unknown,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum SupervisorDecision {
       Abort,
       Wait,
       ReassignUnfinishedTasks,
       ReleaseTasksToPool,
       MarkPartialSuccess,
       MarkTotalFailure,
       ContinueWithSurvivor,
       RefuseUnsafeReplacement,
   }

   #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
   pub struct DegradedRunRecord {
       pub failure_mode: SupervisorFailureMode,
       pub decision: SupervisorDecision,
       pub detected_tick: Option<u64>,
       pub detected_after_ms: Option<u64>,
       pub affected_agent_id: String,
       pub tasks_completed_before_failure: Vec<String>,
       pub tasks_recovered: Vec<String>,
       pub tasks_abandoned: Vec<String>,
       pub replacement_mission_id: Option<String>,
       pub recovery_latency_ticks: Option<u64>,
       pub final_status: String,
   }
   ```

   `Default` для enums лучше не делать неявным, кроме отдельного helper
   `unknown_record(...)`, чтобы accidental defaults не скрывали gaps.

2. Расширить `LiveAgentRun` и metrics для failure classification.

   Файл: `crates/swarm-examples/src/sitl_supervisor/config.rs:42`.

   Материализуемый результат:

   - `SupervisorMetrics` получает counters:
     `failure_mode_counts: BTreeMap<String, u64>`,
     `decision_counts: BTreeMap<String, u64>`,
     `tasks_abandoned: Vec<String>`,
     `recovery_failed_count: u64`.
   - `LiveAgentRun` получает optional поля с `#[serde(default)]`-friendly
     report projection:
     `failure_mode: Option<SupervisorFailureMode>`,
     `detected_after_ms: Option<u64>`,
     `tasks_abandoned: Vec<String>`.
   - `LiveAgentRun::report()` переносит failure mode в per-agent report после
     шага 4.

   Псевдокод:

   ```rust
   impl SupervisorMetrics {
       pub fn record_degraded(&mut self, record: &DegradedRunRecord) {
           *self.failure_mode_counts
               .entry(record.failure_mode.as_str().to_owned())
               .or_default() += 1;
           *self.decision_counts
               .entry(record.decision.as_str().to_owned())
               .or_default() += 1;
           self.tasks_abandoned.extend(record.tasks_abandoned.clone());
       }
   }
   ```

3. Добавить degraded report schema.

   Файл: `crates/swarm-examples/src/sitl_report.rs:44`.

   Результат:

   - `SitlMultiAgentRunReport` получает:
     `#[serde(default)] pub degraded: SitlDegradedRunReport`.
   - `SitlMultiAgentAgentReport` получает:
     `#[serde(default)] pub failure_mode: Option<String>`,
     `#[serde(default)] pub tasks_abandoned: Vec<String>`.
   - Новый `SitlDegradedRunReport`:

   ```rust
   #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
   pub struct SitlDegradedRunReport {
       pub records: Vec<DegradedRunRecord>,
       pub failure_mode_counts: BTreeMap<String, u64>,
       pub decision_counts: BTreeMap<String, u64>,
       pub tasks_abandoned: Vec<String>,
       pub recovery_failed_count: u64,
   }
   ```

   Все новые поля должны быть additive и `#[serde(default)]`, чтобы M58/M59/M72
   historical artifacts продолжали парситься.

4. Добавить replay events для M73.

   Файл: `crates/swarm-examples/src/sitl_observability/events.rs:39`.

   Additive variants:

   ```rust
   SupervisorFailureDetected {
       step: u64,
       agent_id: String,
       mode: String,
       completed_task_ids: Vec<String>,
   },
   SupervisorFailureClassified {
       step: u64,
       agent_id: String,
       mode: String,
       decision: String,
   },
   SupervisorRecoveryStarted {
       step: u64,
       agent_id: String,
       policy: String,
       task_ids: Vec<String>,
   },
   SupervisorReplacementUploaded {
       step: u64,
       agent_id: String,
       replacement_mission_id: String,
       mission_item_count: usize,
   },
   SupervisorRecoveryCompleted {
       step: u64,
       agent_id: String,
       recovered_task_ids: Vec<String>,
       latency_ticks: Option<u64>,
   },
   SupervisorRecoveryFailed {
       step: u64,
       agent_id: String,
       mode: String,
       reason: String,
   },
   SupervisorFinalStatus {
       step: u64,
       status: String,
       degraded: bool,
   },
   ```

   Также добавить `SitlEventRecorder::push_*` helpers и summary counters в
   `SitlEventLogSummary` рядом с существующими counters
   (`events.rs:621`). Старые event variants не удалять.

5. Реализовать классификацию отказов.

   Файлы:

   - `crates/swarm-examples/src/sitl_supervisor/degraded.rs`;
   - `crates/swarm-examples/src/sitl_supervisor/live.rs:92`;
   - `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:202`.

   Результат: helper `classify_live_run_failure(run, phase)` и explicit
   `SupervisorFailureMode`.

   Псевдокод:

   ```rust
   pub fn classify_live_failure(run: &LiveAgentRun, phase: LiveFailurePhase)
       -> SupervisorFailureMode
   {
       let error = run.error.as_deref().unwrap_or("");
       match phase {
           LiveFailurePhase::BeforeUpload => AgentLostBeforeUpload,
           LiveFailurePhase::Upload if error.contains("MISSION_ACK") => UploadRejected,
           LiveFailurePhase::AfterUploadBeforeStart => AgentLostAfterUploadBeforeMissionStart,
           LiveFailurePhase::Active if error.contains("no mission progress") => NoProgressTimeout,
           LiveFailurePhase::Active if error.contains("disconnected") => HeartbeatLost,
           LiveFailurePhase::Replacement if error.contains("replacement failed") =>
               ReplacementMissionRejected,
           _ if run.completed_task_count > 0 => PartialCompletionThenFailure,
           _ => Unknown,
       }
   }
   ```

   Для fake tests не полагаться только на string matching: builder должен уметь
   задавать expected failure mode напрямую. String fallback нужен только для
   live/PX4 errors.

6. Сделать decision/outcome явным в live supervisor loop.

   Файл: `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:230`.

   Результат: вместо неявного `if reupload_on_failure && failed` формируется
   `DegradedRunRecord`:

   - no survivor -> `decision=MarkPartialSuccess` или `MarkTotalFailure`
     согласно completed count / manifest completion;
   - `reupload_on_failure=false` -> `decision=MarkPartialSuccess` /
     `MarkTotalFailure`;
   - safe replacement -> `ReleaseTasksToPool`,
     `ReassignUnfinishedTasks`, `ContinueWithSurvivor`;
   - safety gate reject -> `RefuseUnsafeReplacement`,
     `recovery_failed_count += 1`, final status deterministic.

   Псевдокод:

   ```rust
   let mut record = DegradedRunRecord::from_failed_run(&run, mode);
   recorder.push_supervisor_failure_detected(&record);
   record.decision = decide_recovery(&run, config, &active_agent_ids);
   recorder.push_supervisor_failure_classified(&record);

   match record.decision {
       ContinueWithSurvivor | ReassignUnfinishedTasks => { ... replacement ... }
       RefuseUnsafeReplacement | Abort | MarkTotalFailure | MarkPartialSuccess => { ... }
       _ => {}
   }
   degraded_records.push(record);
   live_metrics.record_degraded(&record);
   ```

7. Безопасно обработать replacement failure.

   Файлы:

   - `crates/swarm-examples/src/sitl_supervisor/live.rs:296`;
   - `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:263`;
   - `crates/swarm-examples/src/sitl_supervisor/reallocation.rs:299`.

   Результат:

   - если `replace_mission` возвращает error или controller выставляет
     `finished_run` с replacement error, replay получает
     `SupervisorRecoveryFailed`;
   - report получает `failure_mode=ReplacementMissionRejected`;
   - tasks that cannot be recovered попадают в `tasks_abandoned`;
   - final status должен быть одним из документированных:
     `failed`, `partial_failed`, `completed_with_reallocation`,
     `failed_recovery`.

   Не вводить repeated-failure policy за пределами первого bounded recovery.

8. Обработать unsafe replacement route как explicit degraded path.

   Файл: `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:252`.

   Сейчас `safety_gate.validate_agent_task_subset(...) ?` просто выходит
   ошибкой. Для M73 нужно:

   - перехватить safety error;
   - записать `SupervisorFailureMode::UnsafeReplacementRoute`;
   - decision `RefuseUnsafeReplacement`;
   - `tasks_abandoned` = task ids из rejected replacement plan;
   - replay `SupervisorRecoveryFailed`;
   - report final status deterministic (`failed_recovery` или
     `partial_failed`, выбрать и задокументировать).

9. Добавить report assembly для degraded records.

   Файл: `crates/swarm-examples/src/sitl_supervisor/artifacts.rs:27`.

   Результат:

   - `LiveRunReportInput` получает `degraded_records: &'a [DegradedRunRecord]`.
   - `live_run_report` заполняет `SitlDegradedRunReport`.
   - `known_limitations` дополняются M73 boundary:
     "single bounded recovery attempt; repeated failures are reported but not
     recursively recovered".

10. Расширить M72 validator для degraded-mode packs.

    Файл: `crates/swarm-examples/src/artifact_validator.rs:139`.

    Результат:

    - новые rule ids:
      - `artifact.degraded_record_missing`;
      - `artifact.degraded_event_missing`;
      - `artifact.degraded_final_status_mismatch`;
      - `artifact.degraded_recovery_task_mismatch`;
      - `artifact.degraded_unsupported_path_unlabeled`.
    - если report `failed_agents > 0` или final status содержит
      `reallocation`/`failed`, validator требует degraded record или historical
      mode.
    - validator сверяет degraded records с replay events:
      `failure_detected`, `failure_classified`,
      `recovery_started/completed/failed`, final status.
    - M72 historical mode не должен ломаться на старых M58/M59 artifacts.

11. Рефактор fake live controller в scenario builder.

    Файл: `crates/swarm-examples/src/sitl_supervisor/tests_support.rs:414`.

    Результат: builder для deterministic failure scripts:

    ```rust
    FakeLiveAgentController::scripted(agent)
        .on_start(FakeStart::Ok)
        .on_poll(FakePoll::Pending)
        .on_poll(FakePoll::Failed {
            completed: 1,
            mode: SupervisorFailureMode::PartialCompletionThenFailure,
            error: "partial completion then disconnect",
        })
        .on_replace(FakeReplacement::Reject("fake replacement reject"));
    ```

    Старые helpers `completed`, `failed`, `completed_after_polls`,
    `failed_after_polls` оставить как thin wrappers, чтобы существующие тесты
    не стали шумными.

12. Добавить fake-controller tests для M73 supported paths.

    Файл: `crates/swarm-examples/src/sitl_supervisor/tests_cases.rs`.

    Конкретные tests:

    - `m73_fake_upload_rejection_reports_degraded_record`;
    - `m73_fake_no_progress_timeout_reports_abort_decision`;
    - `m73_fake_heartbeat_lost_reallocates_unfinished_tasks`;
    - `m73_fake_partial_completion_then_disconnect_abandons_completed_subset_correctly`;
    - `m73_fake_replacement_mission_rejected_reports_recovery_failed`;
    - `m73_fake_survivor_completes_recovered_tasks`;
    - `m73_fake_unsafe_replacement_route_is_refused`;
    - `m73_failure_metrics_aggregate_modes_and_decisions`.

    Каждый тест должен assert-ить:

    - `report.degraded.records`;
    - `report.degraded.failure_mode_counts`;
    - `report.degraded.decision_counts`;
    - `report.reallocation`;
    - relevant replay events;
    - M72 validator passes/fails as expected for generated temp artifact.

13. Добавить artifact validator fixture tests.

    Файл: `crates/swarm-examples/tests/artifact_validator.rs`.

    Новые tests:

    - valid degraded pack with failure/recovery events passes;
    - degraded report without degraded replay events fails
      `artifact.degraded_event_missing`;
    - degraded final status mismatch fails
      `artifact.degraded_final_status_mismatch`;
    - recovered tasks in degraded report not matching reallocation events fails;
    - historical M59 artifact still passes with `--allow-historical`.

14. Документировать failure matrix.

    Новый файл: `docs/DEGRADED_SUPERVISOR.md`.

    Обязательные разделы:

    - "Failure Matrix" table:
      `failure mode | detection source | decision | recovery attempt | final status | automated coverage | manual/SITL status`.
    - "Supported in fake tests".
    - "Experimental local SITL".
    - "Not tested / non-goals".
    - "Recovery Semantics".
    - "Report Fields".
    - "Replay Events".
    - "Manual Checks".

    Честно отметить:

    - no hardware failure testing;
    - no real RF/link-loss modeling;
    - no repeated-failure policy beyond one bounded recovery attempt;
    - local PX4/SIH fault injection stays manual-only and must be validated by
      M72 validator when artifacts are captured.

15. Обновить README и сопутствующие docs.

    Файлы:

    - `README.md`: current status row + milestone M73 + quick pointer to
      `docs/DEGRADED_SUPERVISOR.md`;
    - `docs/STATUS.md`: M73 row, limitations, readiness row;
    - `docs/SITL_SETUP.md`: manual fault-injection boundary and M72 validator
      requirement for any local artifacts;
    - `docs/HARDWARE_READINESS.md`: M73 is pre-hardware degraded behavior only,
      not real failsafe validation;
    - `docs/REPLAY.md`: list new degraded supervisor replay events;
    - `docs/ARTIFACT_VALIDATION.md`: new M73 rule ids and degraded pack
      consistency checks;
    - `docs/PREFLIGHT_SAFETY.md`: unsafe replacement route uses M71 gate and is
      reported as refused recovery, not certified safety;
    - `docs/EXTENSION_GUIDE.md`: if adding report/replay schema fields, note
      additive schema policy for degraded events.

16. Зафиксировать verification commands для implementation round.

    Обязательные быстрые проверки:

    ```bash
    cargo fmt --all

    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
      /home/formi/.local/bin/runlim \
      cargo test -p swarm-examples sitl_supervisor

    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
      /home/formi/.local/bin/runlim \
      cargo test -p swarm-examples --test artifact_validator

    timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
      /home/formi/.local/bin/runlim \
      cargo test -p swarm-examples --test sitl_docs

    timeout 300 cargo clippy --workspace --all-targets -- -D warnings
    ```

    `make clippy` можно пробовать первым, но в текущем репозитории нет такого
    target; repo-approved equivalent - прямой `cargo clippy` выше.

    Ручные/длинные проверки, которые не должны входить в default CI:

    - live `scripts/run_m58_local.sh` / `scripts/run_m59_local.sh` без
      `DRY_RUN=1`;
    - manual M73 local PX4/SIH fault-injection artifact capture;
    - repeated-failure soak tests;
    - stochastic communication failure sweeps;
    - любые 500/1000-seed benchmark refresh runs.

## Testing strategy

### 1. Tests that need no refactoring

- `m73_fake_upload_rejection_reports_degraded_record`.
- `m73_fake_no_progress_timeout_reports_abort_decision`.
- `m73_fake_heartbeat_lost_reallocates_unfinished_tasks`.
- `m73_fake_partial_completion_then_disconnect_abandons_completed_subset_correctly`.
- `m73_fake_replacement_mission_rejected_reports_recovery_failed`.
- `m73_fake_survivor_completes_recovered_tasks`.
- `m73_fake_unsafe_replacement_route_is_refused`.
- `m73_failure_metrics_aggregate_modes_and_decisions`.
- `artifact_validator` valid degraded pack passes.
- `artifact_validator` missing degraded event fails with stable rule id.
- `artifact_validator` degraded final status mismatch fails.
- `sitl_docs` anchors for `docs/DEGRADED_SUPERVISOR.md`, README, STATUS,
  REPLAY, ARTIFACT_VALIDATION, SITL_SETUP, HARDWARE_READINESS.

### 2. Tests that need light refactoring

- Fake live controller scenario builder with scripted start/poll/replace
  transitions.
- Shared assertion helpers:
  `assert_degraded_record(report, mode, decision)`,
  `assert_replay_event(log, event_name)`,
  `assert_validator_rule(report, rule_id)`.
- Shared final-status validation helper for supervisor reports and
  artifact-validator fixtures.
- Fixture helper that writes a complete M72 output-dir pack from fake
  supervisor result and immediately validates it.
- M72 validator helper to compare degraded report records against replay events.

### 3. Tests that need heavy refactoring

- Ignored/manual local PX4/SIH fault-injection harness for M73 representative
  failure paths.
- Repeated failure property tests where survivor can fail after replacement
  multiple times.
- Long-running supervisor soak test with synthetic failures.
- Stochastic communication failure sweeps / RF-like loss profiles.
- Full compatibility matrix over older M58/M59/M72 artifacts plus new M73
  degraded schema.

## Risks and tradeoffs

- Report/replay schema grows again. Keep changes additive with
  `#[serde(default)]`; old artifacts must remain parseable.
- Too much classification from free-form PX4 errors can be brittle. Prefer
  structured fake-controller modes in tests and use conservative `Unknown` for
  ambiguous live/PX4 strings.
- Final status taxonomy can drift. M73 should document and test a small set:
  `completed`, `completed_with_reallocation`, `partial_failed`,
  `failed`, `failed_recovery`.
- Unsafe replacement handling changes control flow from immediate error return
  to structured degraded report. This improves evidence but can affect callers
  that currently expect `Err` on safety rejection; document/test exit behavior.
- M72 validator strictness may fail old artifacts unless historical mode is
  retained. Do not remove `--allow-historical`.
- Fake tests can overfit to simplified behavior. Docs must clearly label what
  is fake-supported vs local SITL experimental vs not tested.
- No repeated-failure policy in M73: a survivor failing after replacement is
  reported deterministically, but recursive recovery remains future work.

## Open questions

- Какой final status выбрать для refused unsafe replacement:
  `failed_recovery` или `partial_failed`? Предлагаю `failed_recovery`, если
  recovery attempt был начат и отказан safety gate; `partial_failed`, если
  recovery не начинался.
- Нужно ли повышать `sitl_multi_agent_run_report.v1` schema version? Предлагаю
  оставить v1 с additive/default fields, потому что старые consumers всё ещё
  могут игнорировать новые поля.
- Делать ли новые degraded replay events generic `Supervisor*` или
  `MultiAgentSupervisor*`? Предлагаю `Supervisor*`, потому что это события
  управляющего слоя, а не отдельного агента.
- Должен ли `artifact_validator --strict` требовать degraded records для любых
  `failed_agents > 0` старых M59 artifacts? Предлагаю требовать только в
  current mode; historical mode должен предупреждать, но не падать.
- Нужно ли сейчас добавлять отдельный `scripts/run_m73_local.sh`? В M73 лучше
  запланировать docs-only manual recipe и M72 validation; отдельный script
  имеет смысл после стабилизации fake failure matrix.
