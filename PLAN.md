# План M78 - Benchmark Evidence Layer

## Context

Задача из inbox: подготовить план для M78 из `docs_raw/BEFORE_HARDWARE_A.23.md`, не делать 1000-seed rerun по умолчанию, а превратить уже имеющиеся benchmark artifacts в интерпретируемый evidence layer.

Текущий M78 описан в `docs_raw/BEFORE_HARDWARE_A.23.md:1001`: M69 уже дал полезный `1000`-seed artifact, а M78 должен улучшить reporting/interpretation вместо слепого повторения долгих прогонов (`docs_raw/BEFORE_HARDWARE_A.23.md:1007`). Done criteria требуют статистические поля, хотя бы один degradation sweep artifact, явную маркировку unsupported/caveat rows, явный Urban scope и различение simulation/SITL/hardware claims (`docs_raw/BEFORE_HARDWARE_A.23.md:1070`).

## Investigation context

- `INVESTIGATION.md` в репозитории сейчас отсутствует, поэтому confirmed findings/ruled-out hypotheses из отдельного investigation artifact нет.
- Прочитан источник M78 в `docs_raw/BEFORE_HARDWARE_A.23.md:1001`: цель, scope, non-goals, done criteria и категории automated tests.
- Прочитаны локальные Notion/GitLab protocol-файлы из `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs`; prompt не содержит Notion task id или GitLab MR, `notion_policy` равен `optional`, поэтому удаленный доступ не нужен и не использовался.
- Дополнительно проверены текущие user-facing docs: `README.md`, `docs/STATUS.md`, `docs/BENCHMARK_RESULTS.md`, `docs/SCENARIO_DSL.md`, `docs/EXTENSION_GUIDE.md`.

## Affected Components

- `crates/swarm-metrics/src/metrics/aggregate.rs:5` - `AggregateMetrics` сейчас хранит средние/rates, но не хранит `stddev`, `stderr`, доверительные интервалы, `min/max` и `failure_rate`.
- `crates/swarm-sim/src/benchmark/harness.rs:109` - benchmark harness агрегирует per-seed `RunMetrics` в `AggregateMetrics`; именно здесь нужно сохранить enough per-run values for statistics.
- `crates/swarm-sim/src/benchmark/markdown.rs:3` - markdown table benchmark report сейчас показывает только средние/rates; нужно компактно добавить evidence columns.
- `crates/swarm-sim/src/report_export/json.rs:121` и `crates/swarm-sim/src/report_export/csv.rs:8` - JSON/CSV export schema нужно расширить статистическими и support/evidence fields.
- `crates/swarm-sim/src/report_export/manifest.rs:4` - `BenchmarkManifest` уже содержит `git_commit`, command line, schema versions, jobs/build profile; нужно сделать current/historical/degradation artifact status machine-checkable.
- `crates/swarm-sim/src/support_matrix.rs:1` - support matrix сейчас имеет `Supported`, `KnownBug`, `Unsupported`; M78 требует явный `supported_with_caveats`.
- `crates/swarm-sim/src/runner/types.rs:157` и `crates/swarm-sim/src/runner/types.rs:257` - `RunConfig` и `compute_mission_success` уже имеют wildfire/inspection thresholds, но SAR success сейчас равен `GridState::all_targets_found()` (`crates/swarm-sim/src/runner/types.rs:278`).
- `crates/swarm-examples/src/strategy_comparison_runtime/cli.rs:26` - CLI знает `UrbanPatrol`/`UrbanSearch`, но `--mission all` их не включает (`crates/swarm-examples/src/strategy_comparison_runtime/cli.rs:37`).
- `crates/swarm-examples/src/strategy_comparison_runtime/missions.rs:33` - descriptors/profiles уже содержат Urban и SAR/Wildfire mission builders; сюда ляжет explicit Urban benchmark decision.
- `crates/swarm-examples/src/strategy_comparison_runtime/runs.rs:90` - основной benchmark flow пишет report/export/benchmark pack; сюда можно добавить lightweight degradation preset flow.
- `crates/swarm-sim/src/report_export/focused.rs:24` - focused report сейчас содержит статичный methodology/answers block; его нужно синхронизировать с M78 interpretation.
- `crates/swarm-examples/tests/benchmark_pack.rs:20` - уже есть manifest identity/output-dir tests; сюда добавить проверки новых manifest/export fields и Urban/degradation behavior.
- `crates/swarm-sim/src/benchmark/tests.rs:181` - уже есть determinism test jobs 1 vs 4; сюда добавить unit tests для stats helper.
- `docs/SCENARIO_DSL.md:152` - SAR DSL сейчас описывает `run_config.grid_state`, но не будущий `sar_success_threshold`; при добавлении поля нужно описать default/opt-in semantics.
- `docs/EXTENSION_GUIDE.md:190` - guide уже объясняет M77 extension knobs (`comms_penalty_weight`, `wildfire_priority_realloc_threshold`, `dynamic_belief_updates`); M78 должен добавить evidence/degradation metadata conventions для авторов новых profiles/strategies.
- `README.md`, `docs/STATUS.md`, `docs/BENCHMARK_RESULTS.md`, `docs/SCENARIO_DSL.md`, `docs/EXTENSION_GUIDE.md` - user-facing docs должны объяснять что является current/historical evidence, где simulation/SITL/hardware граница, что Urban не входит в старый M69 `--mission all`, и как новые DSL/evidence fields использовать без overclaiming.

## Implementation Steps

1. Ввести reusable статистику для benchmark metrics.
   - В `crates/swarm-metrics/src/metrics/aggregate.rs:5` добавить небольшой serializable struct, например `MetricStats { mean, stddev, stderr, ci95_low, ci95_high, min, max }`.
   - Добавить helper, который принимает `&[f64]`, считает sample stddev, stderr и нормальный 95% CI через `1.96 * stderr`.
   - Для пустого или одноэлементного массива поведение должно быть deterministic: `stddev = 0.0`, `stderr = 0.0`, `ci95_low = mean`, `ci95_high = mean`.
   - Расширить `AggregateMetrics` полями `success_stats`, `task_completion_stats`, `failure_rate`; остальные метрики добавлять только если они уже есть как per-run values и реально полезны. Не раздувать schema всеми существующими полями без необходимости.
   - Сохранить backwards compatibility через `#[serde(default)]` для новых полей.

2. Подключить статистику в benchmark aggregation.
   - В `crates/swarm-sim/src/benchmark/harness.rs:167` уже собирается `Vec<RunMetrics>` для каждой пары strategy/profile; `AggregateMetrics::from_runs` должен считать stats из этих values.
   - Для `success_stats` использовать per-run boolean success как `0.0/1.0`; `failure_rate = 1.0 - success_rate`.
   - Для `task_completion_stats` использовать `RunMetrics.task_completion_rate`.
   - Добавить unit tests в `crates/swarm-sim/src/benchmark/tests.rs:181`: стабильные значения для `[1, 0, 1, 1]`, deterministic behavior при одном seed, и equality для `jobs=1` vs `jobs=4` с новыми stats fields.

3. Расширить report exports и markdown без ломки старого формата сильнее необходимого.
   - В `crates/swarm-sim/src/benchmark/markdown.rs:3` добавить компактные columns: `Failure`, `Success stderr`, `Success 95% CI`, `Completion 95% CI`.
   - В `crates/swarm-sim/src/report_export/json.rs:121` добавить nested или flattened поля для `success_stats`, `task_completion_stats`, `failure_rate`.
   - В `crates/swarm-sim/src/report_export/csv.rs:8` добавить соответствующие CSV columns с явными именами: `success_stderr`, `success_ci95_low`, `success_ci95_high`, `task_completion_ci95_low`, `task_completion_ci95_high`, `failure_rate`.
   - Обновить compare/regression code только если новые поля попадают в сравнение. По умолчанию не делать новые stats release gates, чтобы не превратить M78 в regression-policy milestone.

4. Сделать artifact status/current-historical evidence проверяемым.
   - В `crates/swarm-sim/src/report_export/manifest.rs:4` добавить поля `artifact_kind` и `artifact_status_note` или эквивалентный enum-like string. Минимально нужные значения: `benchmark`, `degradation`, `smoke`.
   - Не пытаться автоматически утверждать "current" навсегда: `git_commit` уже есть в manifest; current/historical определяется сравнением `manifest.git_commit` с текущим `git rev-parse HEAD`.
   - В `crates/swarm-examples/tests/benchmark_pack.rs:20` добавить test helper, который читает manifest и проверяет, что старый `results/all_500_jobs14_m62_release/manifest.json` явно трактуется как historical evidence, а новый pack получает корректный `artifact_kind`.
   - В docs написать: M69 artifact полезен как benchmark evidence for its recorded commit, но не автоматически является fresh HEAD evidence после последующих кодовых изменений.

5. Уточнить SAR success semantics.
   - В `crates/swarm-sim/src/runner/types.rs:157` добавить `sar_success_threshold: Option<f64>` или `f64` с default, не ломая существующие сценарии.
   - В `compute_mission_success` (`crates/swarm-sim/src/runner/types.rs:257`) разделить:
     - strict SAR success: все targets found, как сейчас;
     - threshold SAR success: `targets_found / targets_total >= sar_success_threshold`, если threshold явно задан.
   - Контракт логики должен быть явным, например:
     ```rust
     let found_ratio = if total_targets == 0 {
         1.0
     } else {
         targets_found as f64 / total_targets as f64
     };
     let sar_goal_satisfied = match sar_success_threshold {
         Some(threshold) => found_ratio >= threshold,
         None => all_targets_found,
     };
     ```
   - В отчеты добавить текстовое объяснение: `probability_of_detection`/`targets_found` - mission-quality metrics; `success_rate` - binary predicate выбранной конфигурации.
   - В `docs/SCENARIO_DSL.md:152` описать новое поле: default/opt-in behavior, отличие strict "all targets found" от threshold success, связь с `probability_of_detection` и почему benchmark docs должны указывать выбранный predicate.
   - Добавить tests для small SAR fixture: all targets found, partial targets found below threshold, partial targets found above threshold. Тесты должны быть in-memory, без внешних файлов.

6. Обновить support matrix и связать ее с export/report.
   - В `crates/swarm-sim/src/support_matrix.rs:1` добавить `SupportedWithCaveats`.
   - Обновить `classify_support` (`crates/swarm-sim/src/support_matrix.rs:36`) так, чтобы rows с известными ограничениями не выглядели как fully supported.
   - В JSON/CSV report rows добавить `support_status` и `support_reason`, вычисленные по mission/profile/strategy identity.
   - В focused/markdown report добавить краткое пояснение, что unsupported/caveat rows нельзя читать как равнозначные сравнения алгоритмов.
   - Расширить tests в `crates/swarm-sim/src/support_matrix.rs:112`.

7. Сформировать явный Urban benchmark decision без смешивания со старым `--mission all`.
   - В `crates/swarm-examples/src/strategy_comparison_runtime/cli.rs:37` добавить explicit alias, например `--mission urban`, который разворачивается в `UrbanPatrol` + `UrbanSearch`.
   - Не включать Urban в старый `--mission all` в рамках M78, чтобы M69 baseline оставался сопоставимым.
   - В `crates/swarm-examples/src/strategy_comparison_runtime/missions.rs:263` добавить тест, что descriptors покрывают urban alias, и тест, что `all` исключает Urban осознанно.
   - В docs выбрать позицию: Urban benchmark is separate evidence track until comparable scenario matrix and degradation axes are defined.

8. Добавить минимальный degradation sweep preset и получить один короткий artifact.
   - Ввести небольшой модуль `crates/swarm-examples/src/strategy_comparison_runtime/degradation.rs`.
   - Минимальный preset: `coverage-packet-loss`, который запускает Coverage на существующих профилях с возрастающей сетевой деградацией и пишет отдельный `degradation.json`/`README.md`.
   - Подключить CLI флаг, например `--degradation coverage-packet-loss`, в `crates/swarm-examples/src/strategy_comparison_runtime/cli.rs:87` и flow в `runs.rs:90`.
   - Первый artifact сделать коротким release run, не 1000 seeds: достаточно 20-50 seeds, `jobs` по текущей машине, output `results/m78_degradation_coverage_packet_loss_YYYY-MM-DD/`.
   - Artifact README должен объяснять оси sweep, seed count, build profile, command line, и что это degradation evidence, а не publication benchmark.

9. Обновить benchmark interpretation docs.
   - `docs/BENCHMARK_RESULTS.md:1`: добавить M78 section со stats meaning, current/historical rule, degradation artifact summary, SAR success vs PoD, Urban separate-track decision.
   - `docs/STATUS.md:47`: обновить milestone status, ограничения и "not hardware evidence" wording.
   - `README.md`: обновить current capabilities/support matrix summary, добавить ссылку на M78 evidence layer и degradation artifact.
   - `docs/SCENARIO_DSL.md:152`: задокументировать `run_config.sar_success_threshold`, strict-vs-threshold SAR success semantics и пример minimal SAR config.
   - `docs/EXTENSION_GUIDE.md:202`: добавить guidance для новых benchmark/evidence extensions: как выбирать `artifact_kind`, когда заводить degradation preset, как маркировать experimental/unsupported/caveat rows, и почему extension artifacts не становятся hardware/PX4 evidence без отдельного прогона.
   - Если result README/manifest notes нужны, обновить `results/all_1000_jobs14_m69_release/README.md` и/или `results/all_500_jobs14_m62_release/README.md` только как historical/current wording, без переписывания результатов.

10. Проверить результат и зафиксировать.
    - Запустить targeted tests через `/home/formi/.local/bin/runlim`, каждый с hard timeout:
      - `timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-metrics`
      - `timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim benchmark`
      - `timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test benchmark_pack`
      - `timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test docs`
    - После Rust изменений обязательно:
      - `timeout 300 /home/formi/.local/bin/runlim cargo fmt --all`
      - `timeout 300 /home/formi/.local/bin/runlim cargo clippy --workspace --all-targets --all-features -- -D warnings`
    - Сделать release build перед degradation artifact:
      - `timeout 300 /home/formi/.local/bin/runlim cargo build --release --workspace`
    - Запустить только M78 degradation sweep, не 1000-seed benchmark, bounded form:
      - `timeout 300 /home/formi/.local/bin/runlim cargo run --release -p swarm-examples --bin strategy_comparison -- --degradation coverage-packet-loss --seeds 20 --jobs 4 --output-dir results/m78_degradation_coverage_packet_loss_YYYY-MM-DD/`
    - Если release build или degradation sweep не укладывается в `300s`, остановить команду, не расширять прогон молча, и документировать skipped/timeout в result README и финальном отчете.

## Testing Strategy

### Tests that need no refactoring

- Unit test for `MetricStats` on deterministic arrays: empty, one value, mixed binary success values, normal metric values.
- Benchmark harness test that new stats fields are identical for `jobs=1` and `jobs=4`, extending `crates/swarm-sim/src/benchmark/tests.rs:181`.
- JSON/CSV export tests that new stats/support fields exist and are numeric/parseable.
- Manifest identity/status test extending `crates/swarm-examples/tests/benchmark_pack.rs:20`.
- CLI parse tests: `--mission urban` expands to UrbanPatrol/UrbanSearch; `--mission all` remains old comparable suite.
- SAR success predicate tests for all targets found and threshold cases.
- Support matrix tests for `SupportedWithCaveats`.
- Docs smoke tests for required M78 phrases: historical evidence, no hardware claim, Urban separate track, SAR success vs PoD, `sar_success_threshold` in `docs/SCENARIO_DSL.md`, and extension evidence conventions in `docs/EXTENSION_GUIDE.md`.

### Tests that need light refactoring

- Benchmark-pack validation helper shared between manifest/current-historical tests and degradation artifact tests.
- Small in-memory degradation preset helper test that validates generated profile order and axis labels without running a long benchmark.
- Focused report snapshot/substring tests after adding interpretation text, avoiding brittle full-file golden snapshots.
- Shared support row assertion helper for JSON and CSV exports.

### Tests that need heavy refactoring

- Structured status manifest generated from code/docs instead of duplicated Markdown claims.
- Full historical/current benchmark classifier that scans all `results/*/manifest.json` and emits a machine-readable evidence index.
- Generic degradation sweep framework for many axes: latency, agent count, route length, obstacle density, blocked-edge frequency, bus detection probability, and failure count.
- Publication-grade statistical comparison tests with bootstrap or paired-seed significance, if future work needs stronger claims than CI columns.

## Risks And Tradeoffs

- Adding many stats fields can make reports noisy. Keep the first M78 implementation focused on success and task-completion stats; expand later only where interpretation needs it.
- SAR threshold can accidentally change old benchmark semantics. Prefer an explicit optional threshold or profile-level opt-in, and document the old strict behavior.
- `--mission all` should not silently change before comparing to M69. Urban must be explicit in M78.
- A degradation preset is useful, but a large generic sweep framework would delay the milestone. Implement one clean preset first, with extensible shape.
- Historical/current status cannot be a permanent property of an artifact because HEAD changes. It should be computed from manifest commit vs current commit, with docs wording reflecting that.
- CI fields over 20-50 seeds are interpretive, not publication-grade. Docs must avoid overclaiming.

## Open Questions

1. Нужно ли в M78 включать Urban degradation artifact сразу, или достаточно Coverage packet-loss sweep как первый degradation example?
2. Должен ли `sar_success_threshold` быть включен в стандартные SAR profiles, или только доступен в сценариях и явно описан в benchmark docs?
3. Сколько seeds выбрать для первого M78 degradation artifact: 20 для скорости или 50 для более устойчивой картинки?
4. Нужен ли отдельный `docs/EVIDENCE.md`, или достаточно обновить `docs/BENCHMARK_RESULTS.md` и `docs/STATUS.md`?
