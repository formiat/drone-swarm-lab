# Context

Планируем M76 - Synthetic Scenario Testbed из
`docs_raw/BEFORE_HARDWARE_A.23.md:814`. Цель M76 - убрать зависимость от
нескольких hand-written Urban fixtures и получить детерминированные семейства
малых synthetic scenarios для stress/degradation/regression tests.

Это support infrastructure, не новая mission family. Генератор должен создавать
обычные `ScenarioSuite` / `ScenarioSuiteEntry` для уже существующих mission
types (`urban-patrol`, `urban-search`, при необходимости generic failure/comms
overlays), а не добавлять новый runtime path.

Текущая база уже имеет:

- `ScenarioSuite` и `ScenarioSuiteEntry` в `crates/swarm-sim/src/dsl/types.rs:9`
  с `schema_version`, `name`, `description`, `scenarios`.
- DSL validation в `crates/swarm-sim/src/dsl/validate.rs:4` и mission-specific
  checks через `validate_entry`.
- JSON export/load helpers в `crates/swarm-sim/src/dsl/export.rs:3`.
- Urban profiles/builders в `crates/swarm-scenarios/src/urban.rs:15`, включая
  M74 blocked-route fixtures (`build_blocked_route_*`) и M75 moving bus /
  perimeter profiles.
- Runtime failure/comms fields in `RunConfig`: `failures`,
  `partition_events`, `packet_loss_rate`, `latency_ticks`, `latency_per_hop`,
  `comms_jitter_ticks` in `crates/swarm-sim/src/runner/types.rs:98`.
- Urban state fields для generated Urban input: `UrbanMap`, `UrbanTemporaryObstacle`,
  `UrbanBusRoute`, `UrbanPerimeterPatrol` in
  `crates/swarm-types/src/urban.rs:82` and
  `crates/swarm-sim/src/runner/types.rs:140`.
- Scenario catalog tests in `crates/swarm-sim/tests/scenario_catalog.rs:1`.
- Strategy-comparison dispatch for built-in profiles in
  `crates/swarm-examples/src/regression_lib.rs:143` and
  `crates/swarm-examples/src/strategy_comparison_runtime/missions.rs:165`.

Notion/GitLab context не использовался: prompt не содержит Notion task или
GitLab/MR target, `notion_policy=optional`; обязательные протоколы были
прочитаны.

# Investigation context

`INVESTIGATION.md` в workspace отсутствует, поэтому дополнительных findings из
investigation artifact нет.

# Affected components

- `crates/swarm-sim/src/dsl/types.rs`
  - Добавить optional generator manifest metadata к `ScenarioSuite`.
- `crates/swarm-sim/src/dsl/validate.rs`
  - Валидировать generator manifest, если он присутствует.
- `crates/swarm-sim/src/dsl/export.rs` и `crates/swarm-sim/src/dsl/tests.rs`
  - Проверить serde/export/load backward compatibility и new manifest fields.
- `crates/swarm-scenarios/src/generated.rs` (новый модуль)
  - Scenario generator API, configs, deterministic Urban generator,
    failure/comms overlays, category library.
- `crates/swarm-scenarios/src/lib.rs`
  - Export generated scenario API.
- `crates/swarm-scenarios/src/urban.rs`
  - Reuse helpers/patterns from existing Urban builders where practical, or add
    small shared helpers only when they remove duplication.
- `crates/swarm-examples/src/bin/generate_scenario_suite.rs` (новый binary)
  - Small CLI для explicit/manual regeneration of generated suites.
- `crates/swarm-examples/src/strategy_comparison_runtime/missions.rs` и/или
  `crates/swarm-examples/src/regression_lib.rs`
  - Подключить только regression-stable generated profile if needed; stress /
    experimental remain explicit/manual.
- `crates/swarm-sim/tests/scenario_catalog.rs`
  - Add generated-suite fixture validation if a checked-in generated JSON is
    added.
- `scenarios/generated.urban.tiny.json` or
  `scenarios/urban.generated.tiny.json`
  - Optional small checked-in generated fixture with manifest. Keep tiny and
    deterministic.
- Docs:
  - `README.md`
  - `docs/STATUS.md`
  - `docs/SCENARIO_DSL.md`
  - `docs/EXTENSION_GUIDE.md`
  - `docs/BENCHMARK_RESULTS.md`
  - `docs/REPLAY.md` only if replay/analysis claims change
  - `docs_raw/BEFORE_HARDWARE_A.23.md` only if milestone status text is updated

# Implementation steps

1. Add generator manifest metadata to Scenario DSL.
   - Files:
     - `crates/swarm-sim/src/dsl/types.rs:9`
     - `crates/swarm-sim/src/dsl/validate.rs:4`
     - `crates/swarm-sim/src/dsl/tests.rs:219`
   - Add backward-compatible optional field:
     ```rust
     #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
     pub struct ScenarioGeneratorManifest {
         pub schema_version: String,
         pub generator_name: String,
         pub generator_version: String,
         pub seed: u64,
         pub category: String,
         pub parameters: Vec<ScenarioGeneratorParameter>,
     }

     #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
     pub struct ScenarioGeneratorParameter {
         pub key: String,
         pub value: String,
     }

     pub struct ScenarioSuite {
         #[serde(default, skip_serializing_if = "Option::is_none")]
         pub generator_manifest: Option<ScenarioGeneratorManifest>,
         // existing fields
     }
     ```
   - Validation:
     - `schema_version` for manifest must be supported, e.g.
       `"scenario_generator_manifest.v1"`.
     - `generator_name`, `generator_version`, `category` must be non-empty.
     - `parameters` must not contain duplicate keys.
   - Expected result: old scenario JSON without `generator_manifest` still
     deserializes and validates; generated suites can record seed/config.

2. Create generator API module in `swarm-scenarios`.
   - Files:
     - `crates/swarm-scenarios/src/generated.rs` (new)
     - `crates/swarm-scenarios/src/lib.rs:1`
     - possibly `crates/swarm-scenarios/Cargo.toml` if typed serialization or
       `thiserror` is needed. Prefer existing dependencies first; `rand` already
       exists.
   - Add:
     ```rust
     pub trait ScenarioGenerator {
         type Config;

         fn generate(
             &self,
             config: &Self::Config,
         ) -> Result<GeneratedScenarioSuite, ScenarioGenerationError>;
     }

     pub struct GeneratedScenarioSuite {
         pub suite: swarm_sim::ScenarioSuite,
         pub manifest: swarm_sim::ScenarioGeneratorManifest,
     }
     ```
   - Add typed error enum, not `anyhow`:
     ```rust
     pub enum ScenarioGenerationError {
         InvalidConfig { field: String, message: String },
         ValidationFailed { errors: Vec<swarm_sim::ValidationError> },
     }
     ```
   - Expected result: a small public API for deterministic generated suites,
     separate from hand-written mission builders.

3. Implement config validation and categories.
   - Files:
     - `crates/swarm-scenarios/src/generated.rs`
   - Add:
     ```rust
     pub enum SyntheticScenarioCategory {
         Tiny,
         Small,
         Medium,
         Stress,
         RegressionStable,
         Experimental,
     }

     pub struct SyntheticUrbanConfig {
         pub seed: u64,
         pub category: SyntheticScenarioCategory,
         pub rows: usize,
         pub cols: usize,
         pub agent_count: usize,
         pub static_obstacle_density: f64,
         pub blocked_edge_count: usize,
         pub bus_mode: SyntheticBusMode,
         pub perimeter: bool,
         pub max_ticks: u64,
     }
     ```
   - Reject:
     - `rows < 2 || cols < 2`;
     - `agent_count == 0`;
     - non-finite or out-of-range densities/probabilities;
     - blocked edge count larger than available route edges;
     - `max_ticks == 0`;
     - stress category in default regression helper.
   - Expected result: invalid generator input fails before producing a scenario.

4. Implement deterministic seeded Urban grid/block generator.
   - Files:
     - `crates/swarm-scenarios/src/generated.rs`
     - Reuse Urban types from `crates/swarm-types/src/urban.rs:96`.
   - Algorithm contract:
     - Use `StdRng::seed_from_u64(config.seed)`.
     - Generate stable node ids like `g-r{row}-c{col}`.
     - Generate bidirectional road edges with stable ids like
       `g-e-r0-c0-r0-c1`.
     - Derive corridor widths from explicit range params or deterministic
       seed draw, but keep them finite and bounded.
     - Add AABB static obstacles from `static_obstacle_density` using bounded
       small rectangles near cells; generated obstacles must not invalidate the
       chosen route unless the config explicitly asks for a violation fixture.
     - Generate a route loop around the grid perimeter for `urban-patrol`.
     - Generate waypoint placeholder tasks matching existing Urban convention.
   - Pseudocode:
     ```rust
     let mut rng = StdRng::seed_from_u64(config.seed);
     let nodes = build_grid_nodes(config.rows, config.cols, spacing_m);
     let edges = build_bidirectional_edges(&nodes, &mut rng, corridor_width_range);
     let route_loop = perimeter_node_loop(config.rows, config.cols);
     let scenario_name = format!(
         "generated_urban_{}_r{}_c{}_seed{}",
         config.category.as_str(),
         config.rows,
         config.cols,
         config.seed,
     );
     ```
   - Expected result: same config/seed gives byte-stable JSON after
     `export_suite`; different seed changes at least one expected field
     (corridor width, blocked schedule, obstacle placement, bus route, etc.).

5. Generate M74-compatible blocked-edge schedules.
   - Files:
     - `crates/swarm-scenarios/src/generated.rs`
     - M74 reference builders:
       `crates/swarm-scenarios/src/urban.rs:412`,
       `crates/swarm-scenarios/src/urban.rs:474`,
       `crates/swarm-scenarios/src/urban.rs:541`
   - Add generated `temporary_obstacles: Vec<UrbanTemporaryObstacle>` into
     `UrbanState`.
   - Ensure at least one tiny profile creates a deterministic blocked edge that
     still leaves a valid run path under `UrbanBlockedPolicy::Wait` or
     `Replan`.
   - Expected result: generated Urban blocked-edge fixture can feed M74 runner
     tests without adding a new mission.

6. Add generated bus placement/route and optional perimeter.
   - Files:
     - `crates/swarm-scenarios/src/generated.rs`
     - Existing M75 types:
       `crates/swarm-types/src/urban.rs:82`,
       `crates/swarm-types/src/urban.rs:248`,
       `crates/swarm-sim/src/runner/types.rs:140`
   - `SyntheticBusMode` should support:
     - `None`
     - `Static`
     - `Route`
   - For `Route`, generate `UrbanBusRoute` stops on known grid nodes with
     strictly increasing `arrival_tick`.
   - For perimeter, set `UrbanState.perimeter_patrol` with a small polygon and
     spacing consistent with generated grid spacing.
   - Expected result: generated `urban-search` scenarios can exercise moving
     bus semantics; generated `urban-patrol` scenarios can exercise perimeter
     metrics.

7. Add failure and communication overlays.
   - Files:
     - `crates/swarm-scenarios/src/generated.rs`
     - Existing runtime contracts:
       `crates/swarm-sim/src/runner/types.rs:98`,
       `crates/swarm-sim/src/runner/types.rs:110`,
       `crates/swarm-sim/src/runner/types.rs:158`
   - Add config structs:
     ```rust
     pub struct SyntheticFailureConfig {
         pub agent_failure_tick: Option<u64>,
         pub failure_type: SyntheticFailureType,
         pub partial_completion_target: Option<u64>,
         pub replacement_policy: SyntheticReplacementPolicy,
     }

     pub struct SyntheticCommsConfig {
         pub packet_loss_rate: f64,
         pub latency_ticks: u64,
         pub latency_per_hop: u64,
         pub partitions: Vec<SyntheticPartitionConfig>,
     }
     ```
   - Map supported fields into existing `RunConfig.failures`,
     `RunConfig.packet_loss_rate`, `RunConfig.latency_ticks`,
     `RunConfig.latency_per_hop`, `RunConfig.partition_events`.
   - Record fields not directly represented in runtime, such as
     `partial_completion_target` or `replacement_policy`, in
     `generator_manifest.parameters`. Do not create opaque hidden behavior.
   - Expected result: generated degradation scenarios are reproducible and
     self-describing even when some degradation intent is metadata-only for
     later supervisor tests.

8. Add scenario library presets without putting large random runs in default CI.
   - Files:
     - `crates/swarm-scenarios/src/generated.rs`
     - `crates/swarm-examples/src/strategy_comparison_runtime/missions.rs:165`
     - `crates/swarm-examples/src/regression_lib.rs:143`
   - Add library helpers:
     ```rust
     pub struct SyntheticScenarioLibrary;

     impl SyntheticScenarioLibrary {
         pub fn urban_tiny_regression(seed: u64) -> SyntheticUrbanConfig;
         pub fn urban_small_exploratory(seed: u64) -> SyntheticUrbanConfig;
         pub fn urban_stress_manual(seed: u64) -> SyntheticUrbanConfig;
     }
     ```
   - Only `tiny` / `regression-stable` may be considered for automated smoke
     hooks. `small`, `medium`, `stress`, `experimental` remain explicit/manual.
   - Expected result: developers can choose stable generated fixtures without
     accidentally enabling large random default runs.

9. Add optional regeneration CLI.
   - Files:
     - `crates/swarm-examples/src/bin/generate_scenario_suite.rs` (new)
   - CLI contract:
     ```text
     generate_scenario_suite \
       --family urban \
       --category tiny \
       --seed 42 \
       --rows 3 \
       --cols 3 \
       --output scenarios/urban.generated.tiny.json
     ```
   - Use `swarm_sim::export_suite` for output.
   - Validate generated suite before writing.
   - Refuse overwrite unless `--force` exists, following existing artifact
     discipline.
   - Expected result: docs can tell exactly how to regenerate checked-in tiny
     fixtures.

10. Add a checked-in tiny generated fixture only if it remains small.
    - Files:
      - `scenarios/urban.generated.tiny.json` (new, optional but recommended)
      - `crates/swarm-sim/tests/scenario_catalog.rs:1`
    - Fixture constraints:
      - one or two entries max;
      - deterministic seed in manifest;
      - validates through `validate_scenario_suite`;
      - runs quickly under `ScenarioRunner`.
    - Expected result: catalog tests prove generated scenario JSON is portable.

11. Add unit/integration tests for generator behavior.
    - Files:
      - `crates/swarm-scenarios/src/generated.rs` unit tests
      - `crates/swarm-sim/src/dsl/tests.rs`
      - `crates/swarm-sim/tests/scenario_catalog.rs`
      - `crates/swarm-examples/tests/sitl_docs.rs` if docs smoke phrases are
        extended
    - Required tests:
      - same seed/config yields identical `ScenarioSuite` and manifest;
      - different seed changes an expected field;
      - generated Urban map validates;
      - generated blocked-edge schedule validates;
      - invalid generator config rejected;
      - generated suite passes `validate_scenario_suite`;
      - checked-in generated tiny fixture loads and runs if fixture is added.
    - Expected result: M76 done criteria are covered by automated tests.

12. Update docs and status.
    - Files:
      - `README.md`
      - `docs/STATUS.md`
      - `docs/SCENARIO_DSL.md`
      - `docs/EXTENSION_GUIDE.md`
      - `docs/BENCHMARK_RESULTS.md`
      - optionally `docs/REPLAY.md` if generated fixtures are documented as
        replay inputs
      - optionally `docs_raw/BEFORE_HARDWARE_A.23.md`
    - Required wording:
      - generated scenarios are deterministic synthetic fixtures, not
        real-world statistically representative distributions;
      - default regression uses only tiny/regression-stable generated profiles;
      - stress/medium/experimental generated suites are manual/explicit;
      - include exact regeneration commands and expected verification commands;
      - no hardware, lidar/raycast, physics or production safety claims.
    - Expected result: README and companion docs describe how to regenerate,
      which generated profiles are default regression, and which are
      exploratory/manual.

13. Run implementation-time checks and record them in docs/outbox.
    - Commands:
      ```bash
      cargo fmt --all
      timeout 300 cargo clippy --workspace --all-targets -- -D warnings
      timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-scenarios generated
      timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim dsl
      timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim --test scenario_catalog
      timeout 300 env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples scenario_generator
      git diff --check
      rg --files -g '*.proptest-regressions' -g '!target'
      ```
    - If a long manual generated suite is added, do not run it in default CI;
      document the exact manual command separately in docs.
    - Expected result: all default generated fixtures are portable and small;
      no proptest persistence files are left in the workspace.

# Testing strategy

## 1. Tests that need no refactoring

- `crates/swarm-scenarios/src/generated.rs`
  - `same_seed_yields_identical_urban_suite`
  - `different_seed_changes_corridor_or_obstacle_field`
  - `generated_urban_map_validates`
  - `generated_blocked_edge_schedule_validates`
  - `invalid_generated_config_is_rejected`
  - `generated_manifest_records_seed_category_and_parameters`
- `crates/swarm-sim/src/dsl/tests.rs`
  - `scenario_suite_generator_manifest_roundtrip`
  - `scenario_suite_generator_manifest_rejects_duplicate_parameters`
- `crates/swarm-sim/tests/scenario_catalog.rs`
  - If a checked-in generated JSON fixture is added:
    `generated_urban_tiny_scenario_loads_validates_and_runs`.

## 2. Tests that need light refactoring

- Shared generated-suite assertion helper:
  - compare exported JSON for same seed;
  - validate suite and manifest together.
- Small snapshot-style test over generated fixture:
  - not a large golden blob;
  - assert stable name, seed, category, node/edge counts, blocked schedule,
    bus/perimeter mode.
- CLI tests for `generate_scenario_suite`:
  - run with temp output path;
  - assert overwrite refusal;
  - assert generated JSON validates.

## 3. Tests that need heavy refactoring

- Property tests over many generated Urban maps.
- Cross-mission generated scenario framework beyond Urban.
- Long-run generated degradation suite with many seeds.
- Cross-version generator reproducibility tests that lock byte-identical output
  across future generator versions.
- Statistical representativeness tests are out of scope because M76 explicitly
  does not claim real-world distributions.

# Risks and tradeoffs

- **Schema growth:** adding optional `generator_manifest` to `ScenarioSuite` is
  backward-compatible for old JSON, but consumers that assume a fixed top-level
  schema may need to ignore unknown fields.
- **Determinism drift:** any future change to generator ordering, RNG draw
  order, or id naming can change generated fixtures. Mitigate with
  `generator_version` and stable tests for tiny/regression-stable profiles.
- **Overclaim risk:** generated scenarios can increase coverage but do not
  prove real-world distribution or hardware readiness. Docs must say this
  explicitly.
- **CI cost:** generated stress suites can grow quickly. Keep default tests tiny
  and make stress/manual suites opt-in.
- **Runtime mismatch:** `partial_completion_target` and replacement policy may
  not map cleanly to current generic `RunConfig`; record non-runtime intent in
  manifest until supervisor-specific generated fixtures are implemented.
- **Fixture duplication:** hand-written Urban fixtures and generated fixtures
  can diverge. Prefer generator tests that feed existing M74/M75 runner paths
  rather than adding new runtime semantics.

# Open questions

- Нужно ли checked-in `scenarios/urban.generated.tiny.json` сразу в M76, или
  достаточно builder-level generated tests плюс regeneration CLI? Рекомендация:
  добавить tiny checked-in fixture, если файл остаётся маленьким и хорошо
  валидируется.
- Должен ли generator manifest быть top-level optional field in `ScenarioSuite`
  или отдельным sidecar manifest рядом с generated JSON? Рекомендация:
  top-level optional field для portable single-file generated fixtures; sidecar
  можно добавить позже для больших generated packs.
- Нужно ли подключать generated tiny profile к `regression_runner` в M76?
  Рекомендация: только если прогон остаётся быстрым и deterministic; stress /
  experimental не подключать к default regression.
- Нужно ли делать cross-mission generator в M76? Рекомендация: нет, начать с
  Urban + generic failure/comms overlays; cross-mission framework оставить как
  heavy-refactoring future work.
