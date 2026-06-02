# План M72 - Artifact Validator + SITL Harness

## Context

M72 продолжает цепочку `docs_raw/BEFORE_HARDWARE_A.23.md` после M70
Urban route export и M71 preflight safety contract. Цель этапа - не новый
алгоритм и не новый PX4-прогон, а машинно-проверяемый контракт артефактов:
валидатор должен уметь проверить output-dir от `sitl_supervisor`, а локальные
harness-скрипты должны сделать повтор M58/M59 воспроизводимым для разработчика,
у которого установлен PX4/SIH.

Текущая база уже даёт большую часть сырья:

- `sitl_supervisor --output-dir` создаёт `manifest.json`,
  `events.sitl-log.json`, `run-report.json`, `replay-summary.txt` и
  `safety_validation_report.v1.json`.
- В `results/m58_multi_agent_px4_sih_execute_2026-05-31/` и
  `results/m59_px4_sih_failure_reallocation_2026-05-31/` уже есть реальные
  M58/M59-style артефакты.
- В `crates/swarm-examples/tests/replay_cli.rs` уже есть частные проверки:
  M58/M59 replay logs парсятся, ожидаемые категории событий присутствуют, а
  M59 completion events используют seq replacement mission.

M72 должен превратить эти разрозненные проверки в отдельный валидатор с
понятными rule id, стабильным exit code и portable тестами на inline/temp
fixtures. Реальные PX4/SIH запуски остаются manual-only.

## Investigation context

`INVESTIGATION.md` отсутствует, поэтому отдельный investigation artifact не
используется.

Изученные точки кода:

- `docs_raw/BEFORE_HARDWARE_A.23.md` - исходный scope M72: validator inputs,
  checks, local M58/M59 harness, manual-only boundary, done criteria и тестовые
  категории.
- `crates/swarm-examples/src/sitl_supervisor_cli/output.rs:10` - текущий
  `OutputPaths`: manifest, replay log, run report, replay summary, safety
  report и run id.
- `crates/swarm-examples/src/sitl_supervisor_cli/output.rs:62` - текущая
  проверка overwrite policy для известных output-файлов.
- `crates/swarm-examples/src/sitl_supervisor_cli/output.rs:124` - replay
  summary пишется через `summarize_sitl_event_log` и `format_sitl_summary`.
- `crates/swarm-examples/src/sitl_supervisor_cli/run.rs:20` - единый CLI flow:
  load suite/config, build manifest, resolve output paths, preflight, then
  dry-run/mock/connection.
- `crates/swarm-examples/src/sitl_multi_agent.rs:48` - текущий
  `MultiAgentSitlManifest` хранит scenario/mission/profile/agents/ownership,
  но не хранит command line, git commit, build profile и config snapshot.
- `crates/swarm-examples/src/sitl_report.rs:44` - текущий
  `SitlMultiAgentRunReport` хранит `run_id`, `final_status`,
  `events_summary`, `reallocation`, `limitations` и `known_limitations`.
- `crates/swarm-examples/src/sitl_observability/events.rs:39` - event log
  variants для multi-agent lifecycle, task completion, failure и reallocation.
- `crates/swarm-examples/src/sitl_observability/events.rs:621` - summary
  counters, которые можно использовать для replay summary consistency.
- `crates/swarm-examples/src/sitl_supervisor/artifacts.rs:27` - report
  limitations для live supervisor.
- `crates/swarm-examples/tests/replay_cli.rs:437` и `:504` - уже существующие
  M58/M59 sanity checks, которые нужно вынести/продублировать в validator
  coverage.
- `docs/SITL_SETUP.md`, `docs/REPLAY.md`, `docs/HARDWARE_READINESS.md`,
  `docs/STATUS.md`, `docs/PREFLIGHT_SAFETY.md`, `docs/BENCHMARK_RESULTS.md` и
  `README.md` - пользовательские docs, которые должны быть синхронизированы.

## Affected components

- `crates/swarm-examples/src/artifact_validator.rs` - новый публичный модуль
  валидатора.
- `crates/swarm-examples/src/lib.rs` - экспорт `pub mod artifact_validator;`.
- `crates/swarm-examples/src/bin/artifact_validator.rs` или
  `crates/swarm-examples/src/bin/sitl_artifact_validator.rs` - новый CLI.
- `crates/swarm-examples/src/sitl_supervisor_cli/output.rs` - расширение
  output-dir contract: command/config/scenario snapshots, manifest metadata,
  overwrite policy для новых файлов.
- `crates/swarm-examples/src/sitl_supervisor_cli/run.rs` - заполнение metadata
  в manifest/report/output-dir перед запуском mode-specific flow.
- `crates/swarm-examples/src/sitl_multi_agent.rs` - добавление versioned
  metadata в `MultiAgentSitlManifest`.
- `crates/swarm-examples/src/sitl_report.rs` - при необходимости
  compatibility/default поля для validator metadata и явных limitations.
- `crates/swarm-examples/src/sitl_observability/events.rs` - reuse summary
  logic; менять event schema только если без этого нельзя получить стабильную
  машинную проверку.
- `scripts/run_m58_local.sh` и `scripts/run_m59_local.sh` - новые manual-only
  harness-скрипты.
- `docs/ARTIFACT_VALIDATION.md` - новый пользовательский документ по validator
  contract.
- `README.md`, `docs/STATUS.md`, `docs/SITL_SETUP.md`,
  `docs/HARDWARE_READINESS.md`, `docs/REPLAY.md`,
  `docs/PREFLIGHT_SAFETY.md`, `docs/BENCHMARK_RESULTS.md` - docs/status sync.
- `crates/swarm-examples/tests/artifact_validator.rs` - основные portable
  tests валидатора.
- `crates/swarm-examples/tests/sitl_docs.rs` - anchors/smoke tests для новых
  docs.

## Implementation steps

1. Добавить доменную модель валидатора в
   `crates/swarm-examples/src/artifact_validator.rs`.

   Минимальный API:

   ```rust
   pub struct ArtifactPackPaths {
       pub output_dir: PathBuf,
       pub manifest: PathBuf,
       pub event_log: Option<PathBuf>,
       pub run_report: Option<PathBuf>,
       pub replay_summary: Option<PathBuf>,
       pub safety_report: Option<PathBuf>,
       pub scenario_snapshot: Option<PathBuf>,
       pub config_snapshot: Option<PathBuf>,
   }

   pub struct ArtifactValidationReport {
       pub schema_version: String,
       pub output_dir: PathBuf,
       pub passed: bool,
       pub violations: Vec<ArtifactValidationViolation>,
   }

   pub struct ArtifactValidationViolation {
       pub rule_id: String,
       pub severity: ArtifactValidationSeverity,
       pub path: Option<PathBuf>,
       pub reason: String,
   }

   pub fn validate_artifact_pack(
       paths: &ArtifactPackPaths,
       options: ArtifactValidationOptions,
   ) -> ArtifactValidationReport;
   ```

   `ArtifactValidationReport::passed` должен быть `true` только если нет
   `error` violations. Warning-уровень можно оставить для compatibility notices
   по старым historical artifacts.

2. Зафиксировать стабильные rule id.

   Базовый набор M72:

   - `artifact.manifest_missing`
   - `artifact.manifest_schema_unsupported`
   - `artifact.manifest_command_missing`
   - `artifact.git_commit_missing`
   - `artifact.build_profile_missing`
   - `artifact.run_id_mismatch`
   - `artifact.output_dir_mismatch`
   - `artifact.final_status_mismatch`
   - `artifact.completed_task_missing_event`
   - `artifact.replay_summary_count_mismatch`
   - `artifact.replacement_seq_mismatch`
   - `artifact.safety_report_missing`
   - `artifact.limitations_missing`
   - `artifact.overwrite_policy_missing`
   - `artifact.parse_failed`

   Rule id должны печататься в human-readable CLI output и попадать в JSON
   report, чтобы ошибки можно было ссылать из docs/CI/manual runbook.

3. Добавить pack loader.

   Реализовать `ArtifactPackPaths::from_output_dir(output_dir)`:

   - ожидает `manifest.json`;
   - если есть `events.sitl-log.json`, читает его через
     `read_sitl_event_log`;
   - если есть `run-report.json`, парсит `SitlMultiAgentRunReport`;
   - если есть `replay-summary.txt`, читает текст;
   - если есть `safety_validation_report.v1.json`, парсит
     `SafetyValidationReport`;
   - если будут добавлены `scenario.snapshot.json`,
     `config.snapshot.json`, `command.txt` или metadata JSON, включает их в
     проверку.

   Парсинг не должен падать через panic. Любая ошибка чтения/serde должна
   превращаться в violation `artifact.parse_failed` с path и причиной.

4. Расширить output-dir contract в `sitl_supervisor`.

   В `crates/swarm-examples/src/sitl_supervisor_cli/output.rs` добавить новые
   optional paths:

   - `scenario_snapshot`;
   - `config_snapshot`;
   - `command_capture` или `run_metadata`;
   - при необходимости `artifact_validation_report`.

   В `resolve_output_paths` эти файлы должны жить внутри
   `<output-dir>/<run-id>/`. В `ensure_output_paths_available` они должны
   участвовать в той же `--force` политике, что manifest/report/log/summary.

   В `crates/swarm-examples/src/sitl_supervisor_cli/run.rs` после загрузки
   scenario/config, но до mode-specific исполнения, записывать snapshot-файлы
   через `write_checked_file`. Это делает M58/M59 pack воспроизводимым без
   поиска исходного локального файла.

5. Добавить metadata в manifest/report без ломки старых fixtures.

   В `crates/swarm-examples/src/sitl_multi_agent.rs` добавить versioned metadata
   к `MultiAgentSitlManifest`, например:

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   pub struct SitlArtifactMetadata {
       pub command: Vec<String>,
       pub git_commit: Option<String>,
       pub build_profile: String,
       pub run_id: Option<String>,
       pub scenario_snapshot_path: Option<PathBuf>,
       pub config_snapshot_path: Option<PathBuf>,
   }
   ```

   Для backward compatibility использовать `#[serde(default)]` на новом поле.
   Валидатор должен различать:

   - current M72 output-dir: metadata обязательна;
   - historical committed artifacts: metadata может быть warning, если режим
     validator явно `--allow-historical`.

   Git commit получать через best-effort helper:

   ```rust
   fn current_git_commit() -> Option<String> {
       std::process::Command::new("git")
           .args(["rev-parse", "HEAD"])
           .output()
           .ok()
           .filter(|out| out.status.success())
           .and_then(|out| String::from_utf8(out.stdout).ok())
           .map(|s| s.trim().to_owned())
           .filter(|s| !s.is_empty())
   }
   ```

   Если `git` недоступен, CLI должен явно писать `unknown`, а validator должен
   решать это через rule `artifact.git_commit_missing` в strict mode.

6. Реализовать consistency checks.

   Конкретные проверки:

   - `run_id`: `run-report.run_id == event-log.run_id == manifest.metadata.run_id`
     и, если используется canonical output-dir, basename директории совпадает с
     run id.
   - final status: `run-report.final_status` и/или
     `run-report.overall_status` совпадает с
     `summarize_sitl_event_log(&log).final_status`.
   - completed tasks: каждый `SitlEvent::MultiAgentTaskCompleted { task_id }`
     должен соответствовать task id из manifest task ownership, а report
     `total_completed_tasks` должен совпадать с количеством completion events.
   - event summary: `run-report.events_summary` должен совпадать с
     `summarize_sitl_event_log(&event_log)` по ключевым counters:
     mission item sent, task completed, failures, reallocation, survivor mission
     updates, final status.
   - replay summary: `replay-summary.txt` должен совпадать с
     `format_sitl_summary(&summarize_sitl_event_log(&event_log))` или хотя бы с
     теми же ключевыми строками, если формат оставляем текстовым.
   - limitations: для `connection_execute` report должен иметь непустые
     `limitations`/`known_limitations`.
   - safety: для supervisor output-dir должен быть
     `safety_validation_report.v1.json`, и в strict mode он должен быть valid
     JSON с pass/fail состоянием.

7. Вынести M59 replacement seq проверку в reusable validator helper.

   Сейчас логика есть как тестовая проверка в
   `crates/swarm-examples/tests/replay_cli.rs:504`. Нужен helper:

   ```rust
   fn validate_replacement_completion_seq(log: &SitlEventLog) -> Vec<Violation> {
       let mut active_seq_by_agent_task = HashMap::new();
       for event in &log.events {
           match event {
               SitlEvent::MultiAgentMissionItemSent {
                   agent_id,
                   seq,
                   task_id: Some(task_id),
                   ..
               } => {
                   active_seq_by_agent_task
                       .insert((agent_id.clone(), task_id.clone()), *seq);
               }
               SitlEvent::MultiAgentTaskCompleted {
                   agent_id,
                   seq,
                   task_id,
                   ..
               } => {
                   if active_seq_by_agent_task
                       .get(&(agent_id.clone(), task_id.clone()))
                       .is_some_and(|expected| expected != seq)
                   {
                       // artifact.replacement_seq_mismatch
                   }
               }
               _ => {}
           }
       }
   }
   ```

   Важно: map намеренно обновляется на каждый
   `MultiAgentMissionItemSent`, поэтому replacement mission переопределяет
   active `(agent_id, task_id) -> seq`.

8. Добавить CLI.

   Новый binary:

   ```bash
   cargo run -p swarm-examples --bin artifact_validator -- \
     --output-dir results/m59_px4_sih_failure_reallocation_2026-05-31/m59-px4-sih-failure-reallocation \
     --mode supervisor-run
   ```

   CLI flags:

   - `--output-dir <path>` - required;
   - `--mode supervisor-run|dry-run|historical` - default `supervisor-run`;
   - `--allow-historical` - downgrade metadata gaps for old committed packs;
   - `--json` - print `ArtifactValidationReport` JSON;
   - `--strict` - treat warnings as errors.

   Exit codes:

   - `0` - valid;
   - `2` - validation failed;
   - `3` - CLI usage error;
   - `4` - unreadable/unparseable artifact root before rules could run.

9. Добавить portable tests в `crates/swarm-examples/tests/artifact_validator.rs`.

   Использовать `tempfile::tempdir()` и inline JSON fixtures. Тесты не должны
   зависеть от `$HOME`, абсолютных локальных путей, установленного PX4 или
   текущего содержимого `results/`.

   Минимальная fixture должна содержать:

   - `manifest.json` с двумя агентами и task ids;
   - `events.sitl-log.json` с `MultiAgentRunStarted`,
     `MultiAgentMissionItemSent`, `MultiAgentTaskCompleted`,
     `MultiAgentRunFinished`;
   - `run-report.json` с matching `final_status`, `events_summary`,
     `limitations`;
   - `replay-summary.txt`, сгенерированный через production
     `format_sitl_summary`;
   - `safety_validation_report.v1.json`.

   Negative fixtures должны менять ровно одно поле, чтобы rule id был
   однозначным.

10. Добавить harness scripts.

    `scripts/run_m58_local.sh`:

    - `set -euo pipefail`;
    - проверяет env/config: `PX4_BIN` или `PX4_ROOT`, endpoints, build profile;
    - создаёт deterministic output dir, например
      `results/m58_multi_agent_px4_sih_local/${RUN_ID}`;
    - стартует два PX4/SIH процесса только если они ещё не предоставлены
      пользователем;
    - ждёт endpoints;
    - запускает `sitl_supervisor --connection --execute --output-dir ...`;
    - запускает `artifact_validator --output-dir ...`;
    - по `trap` останавливает только PID, которые сам запустил.

    `scripts/run_m59_local.sh`:

    - использует тот же baseline;
    - включает `--reupload-on-failure`;
    - inject controlled first-agent loss через kill собственного PID
      agent-0/SIH или через уже существующий documented fail hook, если он
      есть;
    - после run вызывает validator и проверяет reallocation-specific rules.

    У обоих скриптов должен быть `--dry-run` или `DRY_RUN=1`, который печатает
    команды, проверяет наличие бинарей/paths и не запускает PX4. Это нужно для
    portable shell tests.

11. Обновить docs.

    Новый `docs/ARTIFACT_VALIDATION.md`:

    - artifact pack layout;
    - обязательные и optional файлы;
    - validator command examples;
    - rule id table;
    - strict vs historical mode;
    - M58/M59 harness usage;
    - manual-only boundary;
    - failure examples.

    Обновить:

    - `README.md`: milestone table M72, quick command для validator, статус
      manual harness.
    - `docs/STATUS.md`: M72 status и ограничения.
    - `docs/SITL_SETUP.md`: раздел "Artifact validation and local harness".
    - `docs/HARDWARE_READINESS.md`: artifact validator как gate перед будущими
      hardware-candidate работами, но не hardware readiness.
    - `docs/REPLAY.md`: validator проверяет replay summary/event log
      consistency и replacement seq semantics.
    - `docs/PREFLIGHT_SAFETY.md`: safety report становится входом validator.
    - `docs/BENCHMARK_RESULTS.md`: отметить, что M72 валидирует SITL packs; full
      benchmark-pack validator остаётся follow-up, если не будет реализован в
      этом этапе.

12. Зафиксировать verification commands для реализации M72.

    Обязательные быстрые проверки:

    ```bash
    cargo fmt --all

    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
      /home/formi/.local/bin/runlim \
      cargo test -p swarm-examples --test artifact_validator

    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
      /home/formi/.local/bin/runlim \
      cargo test -p swarm-examples --test sitl_docs

    bash -n scripts/run_m58_local.sh scripts/run_m59_local.sh

    DRY_RUN=1 scripts/run_m58_local.sh
    DRY_RUN=1 scripts/run_m59_local.sh
    ```

    Если затрагивается публичный API или shared crates:

    ```bash
    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
      /home/formi/.local/bin/runlim \
      cargo test -p swarm-examples sitl_observability

    PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 \
      /home/formi/.local/bin/runlim \
      cargo test -p swarm-examples --test sitl_agent
    ```

    Long/manual boundary:

    - live `scripts/run_m58_local.sh` и `scripts/run_m59_local.sh` без
      `DRY_RUN=1` запускать только вручную на машине с PX4/SIH;
    - не добавлять live PX4/SIH в default CI;
    - 500/1000-seed benchmark rerun для M72 не нужен.

## Testing strategy

### 1. Tests that need no refactoring

- Valid tiny supervisor artifact pack passes validator.
- Missing `manifest.json` fails with `artifact.manifest_missing`.
- Missing manifest command/git/build metadata fails in strict mode.
- `run-report.final_status` vs event-log final status mismatch fails with
  `artifact.final_status_mismatch`.
- Report completed task count vs event log completion count mismatch fails with
  `artifact.completed_task_missing_event` or a dedicated count rule.
- `run-report.events_summary` vs recomputed `summarize_sitl_event_log` mismatch
  fails.
- `replay-summary.txt` vs recomputed `format_sitl_summary` mismatch fails.
- Replacement mission completion seq mismatch fails with
  `artifact.replacement_seq_mismatch`.
- Missing safety report in supervisor-run mode fails with
  `artifact.safety_report_missing`.
- Missing `limitations`/`known_limitations` in connection execute report fails
  with `artifact.limitations_missing`.
- CLI exits `0` on valid pack and non-zero on invalid pack.
- `bash -n scripts/run_m58_local.sh scripts/run_m59_local.sh`.
- Docs smoke tests verify `docs/ARTIFACT_VALIDATION.md`, README and SITL setup
  mention manual-only PX4/SIH boundary and validator command.

### 2. Tests that need light refactoring

- Shared artifact fixture builder to avoid duplicating manifest/report/event
  JSON in every validator test.
- Rule-id assertion helper:
  `assert_has_rule(&report, "artifact.final_status_mismatch")`.
- Shared event-log/report consistency helper reused by validator and existing
  replay tests.
- Harness dry-run mode tests that execute scripts without PX4 and assert clear
  missing-PX4/actionable messages.
- Helper to build current M72 metadata from CLI args, git commit and build
  profile without relying on local absolute paths.

### 3. Tests that need heavy refactoring

- Validator over full committed M58/M59 artifact directories as stable fixtures.
  This may require normalizing historical metadata gaps or adding
  `--allow-historical`.
- Multi-artifact benchmark pack validator for `results/all_*` directories.
  Useful later, but not necessary for the core M72 SITL contract.
- Schema-version compatibility matrix for old/new manifest/report/event-log
  versions.
- Ignored/manual two-PX4 harness integration test that actually starts PX4/SIH.
  This must stay opt-in and out of default CI.

## Risks and tradeoffs

- Existing M58/M59 committed artifacts likely lack the new command/git/build
  metadata. Strict M72 should validate new packs, while historical artifacts
  should either warn or require `--allow-historical`.
- Parsing `replay-summary.txt` as text is brittle. Prefer recomputing expected
  text from event log and comparing exact output, or add a future
  machine-readable summary artifact if text drift becomes painful.
- Adding manifest metadata changes serialized JSON. Use `#[serde(default)]` for
  compatibility and update tests/docs explicitly.
- Harness scripts can accidentally kill unrelated PX4 processes if implemented
  carelessly. They must track only PIDs they started and document externally
  managed endpoint mode separately.
- Git commit/build profile collection is best-effort outside Cargo/CI. The
  validator should make this visible instead of silently accepting unknown
  metadata in strict mode.
- M72 improves artifact discipline, but does not prove Gazebo/HIL/hardware
  readiness and does not replace manual PX4/SIH operator judgement.

## Open questions

- Название CLI: `artifact_validator` короче, `sitl_artifact_validator` точнее.
  Предлагаю `artifact_validator`, если валидатор сразу проектировать как
  расширяемый за пределы SITL.
- Нужен ли `--allow-historical` по умолчанию для committed M58/M59 artifacts,
  или старые артефакты должны быть явно помечены historical в docs?
- Достаточно ли text `replay-summary.txt`, или в M72 стоит сразу добавить
  `replay-summary.json`?
- Как лучше фиксировать `build_profile`: env `PROFILE`, Cargo compile-time
  helper или CLI flag `--build-profile`?
- Harness должен сам стартовать PX4/SIH или по умолчанию требовать уже
  поднятые endpoints, а старт PX4 делать только через явные env-переменные?
