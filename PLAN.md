# План M89 - SITL Dual-Stack Evidence Pack

## Context

Нужно реализовать M89 из `docs_raw/DRONE_A.25.md`: подготовить дисциплинированный
evidence path для PX4 и ArduPilot без требования железа. M89 не должен
становиться hardware milestone: обязательная часть должна оставаться portable,
быстрой и проверяемой локальными Rust-тестами без установленного PX4,
ArduPilot, Gazebo, HIL или реального транспорта.

Текущий HEAD уже содержит фундамент M80-M88:

- M80 command IR: dry-run artifact хранит `command_ir_summary`.
- M81 MAVLink Common compiler: `compile_mavlink_common_plan` строит
  `MavlinkCommonPlan` с ACK/telemetry/IR hash.
- M82 capability profiles: `MavlinkCapabilityProfileId` уже поддерживает
  `mavlink_common_generic`, `px4`, `ardupilot`.
- M83 primitive missions: `takeoff-hold-land`, `orbit`, `waypoint-square`
  уже компилируются в dry-run artifacts.
- M86 FC contract: geofence/parameter contract уже может жить внутри
  `MavlinkCommonPlan`.
- M87/M88 command plane/topology уже валидируются в supervisor artifacts.

Поэтому M89 не должен заново изобретать profile support. Главный недостающий
слой: единый dual-stack evidence pack, который показывает, что одна и та же
command IR mission может быть скомпилирована, сохранена и провалидирована под
PX4 и ArduPilot профили, с честными caveats и без claims о hardware readiness.

Найденные anchors:

- `crates/swarm-examples/src/sitl_plan.rs:233` - `SitlDryRunArtifact`.
- `crates/swarm-examples/src/sitl_plan.rs:894` -
  `dry_run_artifact_with_mavlink_profile`.
- `crates/swarm-examples/src/sitl_plan.rs:1221` -
  `write_dry_run_artifact_with_mavlink_profile`.
- `crates/swarm-examples/src/sitl_agent_runtime/runtime.rs:96` - dry-run branch
  writes profile-specific artifact.
- `crates/swarm-examples/src/sitl_agent_runtime/cli.rs:162` -
  `--mavlink-profile` parsing.
- `crates/swarm-comms/src/mavlink_capability_profile.rs:20` -
  `MavlinkCapabilityProfileId`.
- `crates/swarm-comms/src/mavlink_capability_profile.rs:831` - PX4 profile.
- `crates/swarm-comms/src/mavlink_capability_profile.rs:848` - ArduPilot profile.
- `crates/swarm-comms/src/mavlink_common_plan.rs:29` -
  `compile_mavlink_common_plan`.
- `crates/swarm-comms/src/mavlink_common_plan.rs:108` -
  `MavlinkCommonPlan`.
- `crates/swarm-comms/src/mavlink_common_plan.rs:125` - `fence_summary`.
- `crates/swarm-comms/src/mavlink_common_plan.rs:129` -
  `fc_contract_result`.
- `crates/swarm-examples/src/artifact_validator.rs:390` -
  `validate_dry_run_artifact`.
- `crates/swarm-examples/src/artifact_validator.rs:558` -
  `validate_mavlink_common_plan`.
- `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs:455` -
  current primitive dry-run coverage.
- `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs:537` -
  current PX4 profile dry-run test.
- `crates/swarm-examples/tests/artifact_validator.rs:552` - dry-run artifact
  validator happy path.
- `docs/SITL_SETUP.md:202` - existing dry-run artifact description.
- `docs/SITL_SETUP.md:248` - existing M83 primitive dry-run section.
- `docs/MAVLINK_CAPABILITY_PROFILES.md:115` - profile CLI docs.
- `docs/OPERATIONAL_RUNBOOKS.md:305` - primitive dry-run runbook.
- `README.md:688` - M80-M88 status table area.
- `docs/STATUS.md:58` - M80-M88 status table area.
- `docs/HARDWARE_READINESS.md:34` - portable evidence boundary table.

## Investigation context

`INVESTIGATION.md` отсутствует. Дополнительного investigation artifact для M89
нет.

Проверка протоколов:

- Notion protocol прочитан. В prompt нет Notion task id, policy `optional`,
  чтение Notion не требуется.
- GitLab protocol прочитан. В prompt нет MR/GitLab target, чтение GitLab не
  требуется.

## Affected components

- `crates/swarm-examples/src/sitl_plan.rs` - расширить dry-run/evidence модели
  или добавить M89-specific DTO рядом с `SitlDryRunArtifact`.
- `crates/swarm-examples/src/sitl_dual_stack_evidence.rs` - новый модуль для
  генерации dual-stack evidence pack из двух профильных dry-run artifacts,
  включая явные `abort_replacement` и `fc_safety_contract` sections.
- `crates/swarm-examples/src/lib.rs` - экспорт нового модуля.
- `crates/swarm-examples/src/bin/sitl_dual_stack_evidence.rs` - новый portable
  CLI для генерации evidence pack без запуска SITL.
- `crates/swarm-examples/Cargo.toml` - зарегистрировать новый binary.
- `crates/swarm-examples/src/artifact_validator.rs` - добавить поддержку нового
  `dual-stack-evidence` artifact mode или расширить dry-run mode так, чтобы он
  валидировал `sitl_dual_stack_evidence_pack.v1.json`.
- `crates/swarm-examples/tests/artifact_validator.rs` - happy/negative tests для
  evidence pack validation.
- `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs` - покрыть
  PX4+ArduPilot primitive dry-run profiles без внешних SITL dependencies.
- `crates/swarm-examples/tests/sitl_docs.rs` - docs smoke tests для M89 docs.
- `docs/SITL_SETUP.md` - добавить раздел M89 dual-stack dry-run evidence и
  optional manual PX4/ArduPilot SITL notes.
- `docs/ARTIFACT_VALIDATION.md` - описать schema/rules для M89 evidence pack.
- `docs/MAVLINK_CAPABILITY_PROFILES.md` - уточнить, что M89 сравнивает PX4 и
  ArduPilot profiles через common IR, но не доказывает equivalence.
- `docs/OPERATIONAL_RUNBOOKS.md` - добавить runbook для M89 dual-stack evidence.
- `docs/HARDWARE_READINESS.md` - добавить M89 как pre-hardware evidence layer.
- `docs/STATUS.md` и `README.md` - добавить M89 status/usage/limitations.
- `docs/ARDUPILOT_SITL.md` - новый optional runbook для локального ArduPilot
  SITL, без обязательной установки в automated tests.
- `scripts/run_ardupilot_local.sh` - optional manual harness только в dry-run
  safe mode по умолчанию, если dependencies manageable. Если реализация
  полноценного harness окажется рискованной, оставить только docs/runbook и
  явно зафиксировать optional gap.

## Implementation steps

1. Добавить M89 evidence DTO и генератор.
   - Файлы:
     - `crates/swarm-examples/src/sitl_dual_stack_evidence.rs` (новый).
     - `crates/swarm-examples/src/lib.rs:1`.
   - Результат:
     - Новый serializable artifact `sitl_dual_stack_evidence_pack.v1`.
     - Pack содержит один source mission и две profile записи: `px4`,
       `ardupilot`.
     - Каждая запись ссылается на профильный `SitlDryRunArtifact` shape и
       summary поля из `MavlinkCommonPlan`.
   - Контракт DTO:
     ```rust
     pub const SITL_DUAL_STACK_EVIDENCE_SCHEMA_VERSION: &str =
         "sitl_dual_stack_evidence_pack.v1";

     pub struct SitlDualStackEvidencePack {
         pub schema_version: String,
         pub source_scenario_path: PathBuf,
         pub mission: String,
         pub profile: String,
         pub agent_id: String,
         pub command_ir_hash: String,
         pub abort_replacement: DualStackAbortReplacementEvidence,
         pub profiles: Vec<SitlDualStackProfileEvidence>,
         pub limitations: Vec<String>,
     }

     pub struct SitlDualStackProfileEvidence {
         pub mavlink_profile: MavlinkCapabilityProfileId,
         pub stack_name: String,
         pub dry_run_artifact_path: PathBuf,
         pub backend_profile: String,
         pub overall_classification: MavlinkCompatibilityClass,
         pub hardware_facing_allowed: bool,
         pub expected_ack_count: usize,
         pub telemetry_milestone_count: usize,
         pub command_prelude_count: usize,
         pub mission_item_count: usize,
         pub command_postlude_count: usize,
         pub safety_passed: bool,
         pub abort_replacement: ProfileAbortReplacementEvidence,
         pub fc_safety_contract: ProfileFcSafetyContractEvidence,
         pub caveats: Vec<String>,
     }

     pub struct DualStackAbortReplacementEvidence {
         pub timeout_policy: TimeoutPolicy,
         pub expected_terminal_state: TerminalState,
         pub replacement_policy: ReplacementEvidenceStatus,
         pub evidence_status: String,
         pub caveats: Vec<String>,
      }

     pub struct ProfileAbortReplacementEvidence {
         pub timeout_on_timeout: TimeoutAction,
         pub expected_terminal_state: TerminalState,
         pub abort_command: Option<MavlinkCommonCommandName>,
         pub rtl_available: MavlinkCompatibilityClass,
         pub replacement_policy: ReplacementEvidenceStatus,
         pub caveats: Vec<String>,
      }

     pub struct ProfileFcSafetyContractEvidence {
         pub safety_report_passed: bool,
         pub fence_summary_present: bool,
         pub fc_contract_result_present: bool,
         pub fc_contract_passed: Option<bool>,
         pub geofence_support: MavlinkCompatibilityClass,
         pub parameter_support: MavlinkCompatibilityClass,
         pub unsupported_or_unknown_claims: Vec<String>,
         pub caveats: Vec<String>,
      }
     ```
   - Нетривиальная логика:
     ```rust
     // Both entries must be compiled from the same command IR.
     ensure(px4.command_ir_hash == ardupilot.command_ir_hash);
     ensure(profiles == ["px4", "ardupilot"]);
     ensure(!ardupilot.hardware_facing_allowed || ardupilot has explicit SITL evidence flag);

     // Primitive single-agent evidence still records abort/replacement explicitly.
     ensure(pack.abort_replacement.timeout_policy.on_timeout == TimeoutAction::Abort);
     ensure(pack.abort_replacement.expected_terminal_state == TerminalState::Landed);
     ensure(pack.abort_replacement.replacement_policy
         == ReplacementEvidenceStatus::NotApplicableSingleAgentPrimitive);

     // Do not reduce FC evidence to a boolean.
     ensure(profile.fc_safety_contract.safety_report_passed == artifact.safety_report.passed);
     ensure(profile.fc_safety_contract.fc_contract_result_present
         == artifact.mavlink_common_plan.fc_contract_result.is_some());
     ensure(unknown_or_unsupported_safety_claims are visible in caveats);
     ```
   - Для M89 primitive `takeoff-hold-land` replacement обычно не применяется,
     но это должно быть сериализовано как
     `not_applicable_single_agent_primitive`, а не пропущено. Если позже M89
     evidence pack строится по multi-agent command-plane artifact, тот же DTO
     должен уметь явно фиксировать `command_plane_replacement_supported`.

2. Добавить portable CLI для генерации dual-stack evidence.
   - Файлы:
     - `crates/swarm-examples/src/bin/sitl_dual_stack_evidence.rs` (новый).
     - `crates/swarm-examples/Cargo.toml:23`.
   - Результат:
     - Команда генерирует:
       - `<output-dir>/px4/sitl_dry_run_artifact.v1.json`;
       - `<output-dir>/ardupilot/sitl_dry_run_artifact.v1.json`;
       - `<output-dir>/sitl_dual_stack_evidence_pack.v1.json`.
   - CLI contract:
     ```text
     sitl_dual_stack_evidence
       --scenario <path>
       --agent-id <id>
       --output-dir <path>
       [--force]
     ```
   - Реализация должна переиспользовать существующий path:
     - `load_sitl_suite` / `build_sitl_plan` из `sitl_plan.rs`;
     - `dry_run_artifact_with_mavlink_profile` из `sitl_plan.rs:894`;
     - `MavlinkCapabilityProfileId::Px4` и `MavlinkCapabilityProfileId::ArduPilot`.
   - Не запускать `sitl_agent` как subprocess внутри Rust-кода: лучше вызвать
     library functions, чтобы тесты были быстрыми и deterministic.
   - `--force=false` должен отказать, если output files already exist.

3. Расширить artifact validator под M89 evidence pack.
   - Файлы:
     - `crates/swarm-examples/src/artifact_validator.rs:88` -
       `ArtifactValidationMode`.
     - `crates/swarm-examples/src/bin/artifact_validator.rs:120` -
       `parse_mode`.
     - `docs/ARTIFACT_VALIDATION.md:73`.
   - Результат:
     - Новый mode: `dual-stack-evidence`.
     - Validator читает `sitl_dual_stack_evidence_pack.v1.json`, проверяет обе
       referenced dry-run artifacts и сверяет summary fields.
   - Минимальные stable rule ids:
     - `artifact.dual_stack_evidence_missing`;
     - `artifact.dual_stack_profile_missing`;
     - `artifact.dual_stack_profile_mismatch`;
     - `artifact.dual_stack_ir_hash_mismatch`;
     - `artifact.dual_stack_hardware_claim_unsafe`.
     - `artifact.dual_stack_abort_replacement_missing`;
     - `artifact.dual_stack_abort_policy_mismatch`;
     - `artifact.dual_stack_replacement_policy_mismatch`;
     - `artifact.dual_stack_fc_contract_missing`;
     - `artifact.dual_stack_fc_contract_hidden_caveat`;
     - `artifact.dual_stack_fc_contract_claim_unsafe`.
   - Нетривиальная проверка:
     ```rust
     let profiles = pack.profiles.iter().map(|p| p.mavlink_profile).collect();
     require(profiles == {Px4, ArduPilot});
     require(all referenced dry-run artifacts exist);
     require(all referenced artifacts validate with ArtifactValidationMode::DryRun);
     require(all command_ir_hash values are equal);
     require(ardupilot.hardware_facing_allowed == false unless explicit manual SITL evidence exists);
     require(pack.abort_replacement exists);
     require(pack.abort_replacement.timeout_policy == source command_ir_summary.timeout_policy);
     require(pack.abort_replacement.expected_terminal_state
         == source command_ir_summary.expected_terminal_state);
     require(profile.abort_replacement.replacement_policy is explicit, even when not_applicable);
     require(profile.fc_safety_contract exists);
     require(profile.fc_safety_contract.safety_report_passed == dry_run.safety_report.passed);
     require(profile.fc_safety_contract mirrors mavlink_common_plan.fence_summary/fc_contract_result);
     require(UnknownUntilSitlOrHardware safety/fence/parameter classifications have visible caveats);
     ```
   - Validator не должен требовать live replacement для single-agent primitive
     pack. Он должен требовать, чтобы отсутствие replacement было
     объяснено explicit status/reason.

4. Усилить/параметризовать dry-run profile tests для PX4 и ArduPilot.
   - Файлы:
     - `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs:455`.
     - `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs:537`.
   - Результат:
     - Existing PX4 tests остаются.
     - Добавить ArduPilot variants для `takeoff-hold-land` как minimum и для
       `orbit` / `waypoint-square` как broader coverage.
     - Проверить, что ArduPilot artifact содержит:
       - `mavlink_common_plan.backend_profile == "ardupilot"`;
       - `compatibility.profile == "ardupilot"`;
       - caveat содержит `ArduPilot`;
       - hardware-facing unknown/caveat не скрыт.
       - `command_ir_summary.timeout_policy.on_timeout == "abort"`;
       - `command_ir_summary.expected_terminal_state == "landed"`.
   - Tests не должны запускать PX4/ArduPilot.

5. Добавить tests для M89 evidence generator.
   - Файлы:
     - `crates/swarm-examples/src/sitl_dual_stack_evidence.rs` unit tests.
     - или `crates/swarm-examples/tests/dual_stack_evidence.rs` integration tests.
   - Результат:
     - Happy path генерирует pack для
       `scenarios/primitive.takeoff-hold-land.json`.
     - Negative path:
       - missing PX4 artifact;
       - mismatched `command_ir_hash`;
       - duplicate profile;
       - missing ArduPilot profile;
       - unsafe `hardware_facing_allowed=true` for unknown ArduPilot behavior.
       - missing `abort_replacement`;
       - mismatched timeout policy between `command_ir_summary` and
         `abort_replacement`;
       - missing `fc_safety_contract`;
       - hidden `UnknownUntilSitlOrHardware` FC/fence/parameter caveat.

6. Добавить artifact-validator tests для M89.
   - Файлы:
     - `crates/swarm-examples/tests/artifact_validator.rs:552`.
   - Результат:
     - `dual_stack_evidence_pack_passes`.
     - `dual_stack_evidence_missing_profile_fails`.
     - `dual_stack_evidence_mismatched_ir_hash_fails`.
     - `dual_stack_evidence_missing_referenced_dry_run_fails`.
     - `dual_stack_evidence_unsafe_hardware_claim_fails`.
     - `dual_stack_evidence_missing_abort_replacement_fails`.
     - `dual_stack_evidence_mismatched_abort_policy_fails`.
     - `dual_stack_evidence_missing_fc_contract_section_fails`.
     - `dual_stack_evidence_hidden_fc_contract_caveat_fails`.

7. Добавить optional ArduPilot SITL docs/runbook.
   - Файлы:
     - `docs/ARDUPILOT_SITL.md` (новый).
     - `docs/SITL_SETUP.md:248`.
     - `docs/OPERATIONAL_RUNBOOKS.md:344`.
   - Результат:
     - Документ объясняет local-only ArduPilot SITL path, expected endpoints,
       profile limitations and non-goals.
     - Не требовать установки ArduPilot в tests.
     - Если concrete command unknown/too environment-specific, фиксировать как
       operator-provided command placeholder, а не выдавать непроверенную
       инструкцию как готовый факт.
   - Обязательный текст границы:
     ```text
     ArduPilot SITL evidence is optional/manual. Dry-run dual-stack evidence does
     not prove ArduPilot command acceptance, mode behavior, failsafe behavior, or
     hardware readiness.
     ```

8. Добавить optional manual harness только если он остаётся безопасным.
   - Файлы:
     - `scripts/run_ardupilot_local.sh` (новый, optional).
     - `docs/ARDUPILOT_SITL.md`.
   - Результат:
     - По умолчанию `DRY_RUN=1` только печатает команды/expected env vars.
     - Без `DRY_RUN=0` не стартует внешний simulator.
     - Не делает SSH/HTTP.
     - Если implementation окажется слишком speculative, не добавлять script и
       явно записать в docs, что M89 ограничивается dry-run evidence + runbook.

9. Обновить README и сопутствующие docs.
   - Файлы:
     - `README.md:688`.
     - `docs/STATUS.md:58`.
     - `docs/HARDWARE_READINESS.md:34`.
     - `docs/MAVLINK_CAPABILITY_PROFILES.md:115`.
     - `docs/SITL_SETUP.md:202`.
     - `docs/ARTIFACT_VALIDATION.md:73`.
     - `docs/OPERATIONAL_RUNBOOKS.md:305`.
   - Результат:
     - M89 отображается как complete только после code/tests.
     - Docs объясняют:
       - current vs historical PX4 evidence;
       - ArduPilot dry-run evidence vs missing live SITL evidence;
       - no hardware claim;
       - no PX4/ArduPilot equivalence claim;
       - abort/replacement evidence boundary: primitive single-agent pack records
         timeout abort policy and terminal state, but replacement is
         `not_applicable_single_agent_primitive` unless a command-plane/multi-agent
         artifact is used;
       - FC/safety contract boundary: `safety_report`, `fence_summary`,
         `fc_contract_result`, geofence support, parameter support and profile
         caveats are evidence fields, not certified flight safety;
       - какие команды запускать для dry-run evidence pack.

10. Зафиксировать быстрые generated evidence artifacts, если они небольшие и
    stable.
    - Файлы:
      - `results/m89_dual_stack_evidence/README.md` (новый).
      - `results/m89_dual_stack_evidence/sitl_dual_stack_evidence_pack.v1.json`
        (если размер разумный).
      - `results/m89_dual_stack_evidence/px4/sitl_dry_run_artifact.v1.json`
        (если размер разумный).
      - `results/m89_dual_stack_evidence/ardupilot/sitl_dry_run_artifact.v1.json`
        (если размер разумный).
    - Результат:
      - Committed portable dry-run evidence для `takeoff-hold-land`.
      - README указывает commit hash, commands, validation result, limitations.
    - Если artifacts слишком шумные или нестабильные, не коммитить JSON; вместо
      этого коммитить README с command transcript и сохранять JSON generation as
      documented reproducible output.

11. Запустить обязательные быстрые проверки и записать результаты.
    - Форматирование:
      - `timeout 300 cargo fmt --all`.
    - Lint:
      - `timeout 300 /home/formi/.local/bin/runlim cargo clippy --workspace --all-targets -- -D warnings`.
    - Tests:
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-examples dual_stack -- --nocapture`.
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test artifact_validator dual_stack -- --nocapture`.
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent primitive -- --nocapture`.
      - `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs m89 -- --nocapture`.
    - Optional generation run:
      - `timeout 300 /home/formi/.local/bin/runlim cargo run -p swarm-examples --bin sitl_dual_stack_evidence -- --scenario scenarios/primitive.takeoff-hold-land.json --agent-id agent-0 --output-dir target/m89-dual-stack --force`.
      - `timeout 300 /home/formi/.local/bin/runlim cargo run -p swarm-examples --bin artifact_validator -- --output-dir target/m89-dual-stack --mode dual-stack-evidence --strict`.
    - Не запускать реальные PX4/ArduPilot SITL execute runs в automated stage.
      Если такой прогон нужен, он должен быть optional/manual и отдельно
      описан в docs/results.

## Testing strategy

### 1. Tests that need no refactoring

- `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs`:
  - добавить ArduPilot variant рядом с
    `primitive_canonical_dry_run_artifacts_compile_to_mavlink_plans`;
  - добавить explicit PX4+ArduPilot profile dry-run check для
    `takeoff-hold-land`.
- `crates/swarm-examples/src/sitl_dual_stack_evidence.rs`:
  - unit test: pack generation from two valid dry-run artifacts;
  - unit test: profile set must be exactly PX4 + ArduPilot;
  - unit test: `command_ir_hash` mismatch fails.
  - unit test: pack contains `abort_replacement` with
    `timeout_policy.on_timeout=Abort`,
    `expected_terminal_state=Landed`, and
    `replacement_policy=not_applicable_single_agent_primitive` for primitive
    single-agent evidence.
  - unit test: pack contains `fc_safety_contract` for each profile and mirrors
    `safety_report.passed`, `fence_summary`, `fc_contract_result`,
    `geofence_support`, and `parameter_support`.
- `crates/swarm-examples/tests/artifact_validator.rs`:
  - valid dual-stack pack passes;
  - missing ArduPilot profile fails;
  - missing referenced dry-run artifact fails;
  - unsafe hardware-facing ArduPilot claim fails.
  - missing `abort_replacement` section fails;
  - mismatched timeout/abort policy fails;
  - missing `fc_safety_contract` section fails;
  - unsafe hidden FC/fence/parameter claim fails.
- `crates/swarm-examples/tests/sitl_docs.rs`:
  - M89 docs mention `sitl_dual_stack_evidence_pack.v1`;
  - docs mention `--mavlink-profile px4`;
  - docs mention `--mavlink-profile ardupilot`;
  - docs mention abort/replacement evidence boundary;
  - docs mention FC/safety contract evidence boundary;
  - docs mention no hardware readiness / no equivalence claim.

### 2. Tests that need light refactoring

- Parameterize existing helper in
  `crates/swarm-examples/tests/sitl_agent/report_and_boundary_tests.rs:348`
  so test helper can pass `--mavlink-profile`.
- Extract small fixture builder in
  `crates/swarm-examples/tests/artifact_validator.rs:1062` so dry-run artifact
  fixtures can be built for PX4 and ArduPilot without copy-paste.
- Extract shared assertion helper for dual-stack evidence sections:
  - `assert_abort_replacement_evidence_matches_ir(pack, dry_run)`;
  - `assert_fc_safety_contract_evidence_matches_plan(profile, dry_run)`.
- Add `ArtifactValidationMode::DualStackEvidence` parsing in
  `crates/swarm-examples/src/bin/artifact_validator.rs:120` and shared path
  discovery in `ArtifactPackPaths`.
- Add reusable validator helper for command ACK/telemetry plus
  abort/replacement and FC/safety sections, so M89 validation does not duplicate
  all dry-run checks by string matching.
- Add docs smoke helper for M89 anchors in
  `crates/swarm-examples/tests/sitl_docs.rs`.

### 3. Tests that need heavy refactoring

- Automated ArduPilot SITL harness with a real simulator process. Не включать в
  обязательный M89 automated scope.
- Dual-stack SITL execute comparison runner that uploads to live PX4 and
  ArduPilot. Это отдельный future milestone unless local dependencies are
  already stable.
- Backend abstraction shared with real connection code and hardware serial
  transport. Для M89 достаточно dry-run evidence + optional manual runbook.
- Manual evidence pack generator for real SITL execute logs. Планировать как
  follow-up, не блокировать M89.

## Risks and tradeoffs

- **Risk: M89 может раздуться до полноценной ArduPilot integration.**
  Mitigation: обязательный scope ограничить dry-run evidence, validator и docs;
  real ArduPilot SITL оставить optional/manual.

- **Risk: новый evidence pack дублирует `SitlDryRunArtifact`.**
  Mitigation: pack должен быть summary/manifest поверх существующих dry-run
  artifacts, а не заменой `sitl_dry_run_artifact.v1`.

- **Risk: downstream tools ожидают только `artifact_validator --mode dry-run`.**
  Mitigation: добавить новый mode additive; существующий `dry-run` mode не
  менять семантически.

- **Risk: ArduPilot profile имеет много `unknown_until_sitl_or_hardware`, и
  validator может ошибочно блокировать весь pack.**
  Mitigation: разрешить unknown classifications для dry-run evidence, если
  `hardware_facing_allowed=false` и caveats видны. Блокировать только скрытые
  unsafe claims.

- **Risk: abort/replacement section выглядит как обещание live failover.**
  Mitigation: для primitive single-agent pack сериализовать explicit
  `not_applicable_single_agent_primitive`; live replacement разрешать только
  при наличии command-plane/multi-agent artifact evidence.

- **Risk: FC/safety contract будет снова сведён к `safety_passed=true`.**
  Mitigation: отдельный `fc_safety_contract` DTO и validator rules должны
  проверять `safety_report`, `fence_summary`, `fc_contract_result`,
  geofence/parameter support class и видимые caveats.

- **Risk: generated JSON artifacts будут шумными из-за `git_commit` или command
  args.**
  Mitigation: если artifacts unstable, коммитить только README/commands и
  держать JSON generated under `target/`; если stable and small, коммитить
  `results/m89_dual_stack_evidence`.

- **Risk: optional script для ArduPilot будет выглядеть как проверенная команда,
  хотя окружение не проверялось.**
  Mitigation: script по умолчанию `DRY_RUN=1`; docs используют placeholders and
  explicit operator responsibility.

### Что могло сломаться

- Dry-run artifact schema consumers: если M89 добавит новые optional fields, старые
  readers должны продолжить читать `sitl_dry_run_artifact.v1`.
- Artifact validator CLI: новый mode не должен менять поведение `dry-run`,
  `supervisor-run`, `historical`, `benchmark-pack`.
- Docs/status claims: нельзя случайно написать, что ArduPilot SITL или hardware
  уже подтверждены.
- Existing PX4 path: `sitl_agent --connection` and M48/M58/M59 docs/scripts не
  должны начать требовать dual-stack artifacts.
- Resource use: generation/tests должны оставаться быстрыми, без запуска
  simulator processes.

## Open questions

- Нужно ли коммитить generated JSON evidence в `results/m89_dual_stack_evidence`
  или достаточно reproducible commands + validator tests? Рекомендация:
  коммитить JSON только если он небольшой, stable и не содержит machine-specific
  absolute paths.
- Добавлять ли `scripts/run_ardupilot_local.sh` в M89? Рекомендация: только если
  он будет безопасным `DRY_RUN=1` harness с operator-provided command; иначе
  ограничиться `docs/ARDUPILOT_SITL.md`.
- Должен ли `artifact_validator --mode dual-stack-evidence` рекурсивно
  валидировать referenced dry-run artifacts или только сверять summary fields?
  Рекомендация: рекурсивно вызывать существующую dry-run validation logic, но
  без subprocess.
- Нужно ли включать Urban geo mission в M89 evidence pack? Рекомендация:
  minimum done criteria закрывать primitive `takeoff-hold-land`; Urban оставить
  optional extra, чтобы M89 не смешивался с M84/M85.
