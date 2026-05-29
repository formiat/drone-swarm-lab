# PLAN.md - M45 Pre-upload Safety Validation

## Context

Идем по ветке 6 Real SITL / PX4 из `docs_raw/DRONE_A.17.md`.

Текущий статус:

- M43 уже дал portable `sitl_agent --dry-run`, mock path, typed CLI errors and
  coordinate-frame contract.
- M44 уже добавил feature-gated MAVLink mission upload:
  `MavlinkTransport::upload_mission`, fake MAVLink tests, `sitl_agent --connection`
  без старой `RAW_RPM` заглушки.
- M45 должен закрыть следующий safety boundary: connection path не должен
  отправлять потенциально опасную или некорректную mission в transport.

Важный текущий нюанс кода: `crates/swarm-examples/src/sitl_plan.rs` сейчас строит
`SitlPlan` через фильтрацию только задач с `pose`. Поэтому проверки M45 нельзя
делать только по готовому `SitlPlan`: иначе `missing pose` уже будет потерян.
Safety validation для upload должна работать на уровне исходного
`ScenarioSuiteEntry`/`Scenario` до waypoint extraction.

Цель M45:

> не отправлять потенциально опасную или некорректную mission в transport.

Не входит в scope:

- full realism calibration;
- hardware certification;
- runtime collision avoidance;
- arm/takeoff/execute lifecycle, это M46;
- live PX4 SITL integration test as default CI gate.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует.

Дополнительный локальный контекст:

- `docs_raw/DRONE_A.17.md` описывает M45 как milestone после M44 и до M46.
- `crates/swarm-examples/src/bin/sitl_agent.rs` сейчас парсит только
  `--mock`, `--dry-run`, `--connection`, `--scenario`, `--agent-id`.
- `crates/swarm-examples/src/sitl_plan.rs` содержит `SitlPlan`,
  `SitlWaypointItem`, `SitlError`, `load_sitl_plan`, `build_sitl_plan` and
  `format_dry_run_plan`.
- `crates/swarm-safety/src/lib.rs` уже содержит reusable 2D `Aabb`, `Geofence`,
  `NoFlyZone`, `SafetyConfig`, but those types target runtime agent checks.
  M45 needs pre-upload mission validation with richer actionable errors, so it
  can reuse `Aabb` shape or dependency, but should not force-fit runtime
  `SafetyViolation`.
- `scenarios/sitl.waypoints.json` has `base_station: null`, but has agent
  `agent-0` at `(0, 0)`. Safe defaults should therefore resolve home from
  explicit config, scenario base station, or selected agent pose before
  rejecting missing home.

## Affected components

- `crates/swarm-examples/Cargo.toml`
  - likely add direct `swarm-safety = { workspace = true }` dependency if reusing
    `Aabb`.
- `crates/swarm-examples/src/lib.rs`
  - export new `sitl_safety` module.
- `crates/swarm-examples/src/sitl_safety.rs`
  - new module for config, validation, error reporting, file loading and tests.
- `crates/swarm-examples/src/sitl_plan.rs`
  - add small loader/refactor helpers so CLI can load `ScenarioSuite` once and run
    safety validation before `build_sitl_plan`.
  - extend `SitlError` with safety config/validation variants.
- `crates/swarm-examples/src/bin/sitl_agent.rs`
  - parse `--safety-config <path>`.
  - run pre-upload safety validation before `MavlinkTransport::new` and before
    `upload_mission`.
- `crates/swarm-examples/tests/sitl_agent.rs`
  - CLI integration coverage for `--safety-config`, invalid mission rejection and
    interaction with no-feature connection path.
- `docs/SITL_SETUP.md`
  - document safety defaults, config JSON example, rejection behavior and
    hardware boundary.
- `README.md`
  - update current Real PX4/SITL status and quick usage examples with
    `--safety-config`.

## Implementation steps

1. Add M45 safety model in `crates/swarm-examples/src/sitl_safety.rs`.

   Define:

   - `SitlSafetyConfig`:
     - `geofence: Option<Aabb>`;
     - `min_altitude_m: f64`;
     - `max_altitude_m: f64`;
     - `max_waypoint_jump_m: f64`;
     - `max_mission_radius_m: f64`;
     - `no_fly_zones: Vec<SitlNoFlyZone>`;
     - `home: Option<Pose>`;
     - `require_home: bool`.
   - `SitlNoFlyZone { id: String, bounds: Aabb }`.
   - `SitlSafetyRuleId` enum or stable string constants:
     - `empty_mission`;
     - `duplicate_waypoint_id`;
     - `missing_pose`;
     - `invalid_altitude`;
     - `outside_geofence`;
     - `inside_no_fly_zone`;
     - `unsafe_waypoint_jump`;
     - `mission_radius_exceeded`;
     - `missing_home`.
   - `SitlSafetyViolation`:
     - `rule_id`;
     - `task_id: Option<String>`;
     - `seq: Option<u16>`;
     - `actual: String`;
     - `allowed: String`.
   - `SitlSafetyValidationReport` or `Result<(), Vec<SitlSafetyViolation>>`.

   Safe defaults for SITL should be conservative but compatible with
   `scenarios/sitl.waypoints.json`:

   - geofence around local simulation space, for example `[-1000, 1000]` in x/y;
   - altitude range `0.0..=120.0`;
   - max waypoint jump above current sample jumps, for example `500.0m`;
   - max mission radius above current sample radius, for example `1000.0m`;
   - empty no-fly zones;
   - `require_home = true`;
   - home resolution fallback: explicit config `home`, else `scenario.base_station`,
     else selected agent pose.

2. Implement config loading in `crates/swarm-examples/src/sitl_safety.rs`.

   - Add `load_sitl_safety_config(path: Option<&Path>) -> Result<SitlSafetyConfig, SitlError>`
     or module-local error converted into `SitlError`.
   - Use JSON via existing `serde_json`; do not add a new YAML/TOML dependency for
     M45 unless implementation discovers a strong reason.
   - Apply `#[serde(default)]` so partial config files can override safe defaults.
   - Validate config sanity before mission validation:
     - finite numeric values;
     - `min_altitude_m <= max_altitude_m`;
     - positive max jump/radius;
     - valid AABB min/max ordering.

3. Refactor `crates/swarm-examples/src/sitl_plan.rs` enough to validate before
   waypoint extraction.

   Keep existing `load_sitl_plan` and `build_sitl_plan` behavior for mock/dry-run
   compatibility, but add helper(s), for example:

   - `load_sitl_suite(path) -> Result<ScenarioSuite, SitlError>`;
   - `first_sitl_entry(suite) -> Result<&ScenarioSuiteEntry, SitlError>`.

   `sitl_agent` should be able to:

   ```text
   load suite -> select entry -> validate safety for connection mode -> build SitlPlan
   ```

   This avoids losing tasks without `pose` before the `missing_pose` rule runs.

4. Implement `validate_pre_upload_safety(entry, agent_id, config)` in
   `crates/swarm-examples/src/sitl_safety.rs`.

   Validation rules:

   - `empty_mission`: no tasks intended as SITL waypoints.
   - `duplicate_waypoint_id`: duplicate task ids in `entry.scenario.tasks`.
   - `missing_pose`: any SITL waypoint task without `pose`.
   - `invalid_altitude`: non-finite z, z below min, or z above max.
   - `outside_geofence`: pose x/y outside configured geofence.
   - `inside_no_fly_zone`: pose x/y inside any configured no-fly zone.
   - `unsafe_waypoint_jump`: distance between consecutive extracted waypoint poses
     exceeds `max_waypoint_jump_m`.
   - `mission_radius_exceeded`: distance from resolved home to waypoint exceeds
     `max_mission_radius_m`.
   - `missing_home`: no explicit config home, no `scenario.base_station`, and no
     selected agent pose when `require_home = true`.

   Error formatting must be actionable:

   - include `rule_id`;
   - include task id or waypoint seq when applicable;
   - include actual value, e.g. `z=-5.0`, `distance=842.1m`,
     `point=(1200.0, 20.0)`;
   - include allowed value/range, e.g. `0.0..=120.0`,
     `<= 500.0m`, `geofence=[-1000..1000, -1000..1000]`.

   Prefer collecting all violations and returning them together instead of
   failing on the first violation. CLI can print the joined list through
   `SitlError::SafetyValidationFailed`.

5. Wire safety into `crates/swarm-examples/src/bin/sitl_agent.rs`.

   - Extend `CliArgs` with `safety_config: Option<String>`.
   - Parse `--safety-config <path>`.
   - For `SitlMode::Connection`, run safety validation before:
     - creating `MavlinkTransport`;
     - converting `SitlWaypointItem` into `swarm_comms::Waypoint`;
     - calling `upload_mission`.
   - Preserve existing behavior for `--mock` and `--dry-run` unless a clear reason
     appears during implementation. Dry-run may print safety config only if that
     stays simple, but M45 acceptance should be about pre-upload rejection.
   - Ensure invalid safety config or invalid mission exits non-zero before any
     transport operation.

6. Add/extend typed errors in `crates/swarm-examples/src/sitl_plan.rs` or
   a shared error module.

   Required user-facing errors:

   - config file read/parse error with path;
   - config value error with field and reason;
   - safety validation failed with one or more violations;
   - missing `--safety-config` value.

   Keep errors stable enough for integration tests to assert substrings such as:

   - `safety validation failed`;
   - `rule_id=outside_geofence`;
   - `task_id=wp-0`;
   - `actual=...`;
   - `allowed=...`.

7. Update documentation.

   In `docs/SITL_SETUP.md`:

   - explain that safety validation runs before upload in `--connection` mode;
   - list safe defaults;
   - document `--safety-config <path>`;
   - include compact JSON config example;
   - document violation output format;
   - clarify that this is not hardware certification or runtime collision
     avoidance.

   In `README.md`:

   - update the PX4 SITL command example to include optional `--safety-config`;
   - update current status / limitations to mention pre-upload safety validation;
   - keep the non-goal boundary explicit.

8. Keep M45 scoped.

   Do not implement:

   - MAVLink arm/takeoff/start/abort;
   - telemetry loop;
   - live PX4 test automation;
   - runtime collision avoidance;
   - full safety calibration against real vehicles.

## Testing strategy

### 1. Tests that need no refactoring

Add unit tests in `crates/swarm-examples/src/sitl_safety.rs`:

- `valid_mission_passes_with_safe_defaults`
  - sample `sitl` scenario from helper passes validation.
- `geofence_rejection_test`
  - waypoint outside configured AABB returns `rule_id=outside_geofence`,
    task id/seq, actual point and allowed geofence.
- `altitude_bounds_test`
  - waypoint below min and above max return `rule_id=invalid_altitude` with
    actual z and allowed range.
- `no_fly_zone_test`
  - waypoint inside configured no-fly AABB returns `rule_id=inside_no_fly_zone`
    and zone id.
- `max_waypoint_jump_test`
  - two consecutive waypoint poses farther than `max_waypoint_jump_m` return
    `rule_id=unsafe_waypoint_jump`.
- `duplicate_waypoint_id_test`
  - duplicate task ids return `rule_id=duplicate_waypoint_id`.
- `missing_pose_test`
  - SITL task without pose returns `rule_id=missing_pose`.
- `mission_radius_test`
  - waypoint farther than `max_mission_radius_m` from resolved home returns
    `rule_id=mission_radius_exceeded`.
- `missing_home_test`
  - no config home, no base station and no matching agent pose returns
    `rule_id=missing_home`.
- `config_rejects_invalid_ranges`
  - invalid min/max altitude, invalid AABB, non-positive jump/radius rejected.

Add CLI integration tests in `crates/swarm-examples/tests/sitl_agent.rs`:

- `connection_rejects_unsafe_mission_before_feature_error_or_upload`
  - run `sitl_agent --connection udp:127.0.0.1:14550 --safety-config <bad>`
    without `mavlink-transport`; assert safety error appears and no feature-missing
    fallback masks it.
- `connection_accepts_valid_safety_config_then_hits_existing_no_feature_error`
  - with valid config and no feature, validation passes and existing
    `feature missing` behavior remains.
- `bad_safety_config_path_is_typed_error`.
- `bad_safety_config_json_is_typed_error`.

These CLI tests remain portable because they do not require PX4 and can run
without `mavlink-transport`.

### 2. Tests that need light refactoring

- Extract a small scenario fixture builder from existing
  `crates/swarm-examples/tests/sitl_agent.rs` / `sitl_plan.rs` tests, or keep it
  local if shared extraction would be larger than the benefit.
- Add scenario mutation helpers:
  - move waypoint outside geofence;
  - set waypoint altitude;
  - duplicate task id;
  - remove pose;
  - add no-fly zone config.
- Add safety config fixture builder:
  - default compatible config;
  - config with narrow geofence;
  - config with no-fly zone;
  - config with tight jump/radius.

These refactors should stay test-local unless production code also benefits.

### 3. Tests that need heavy refactoring

- None initially.

Explicit gaps:

- No default live PX4 SITL automated test in M45. Reason: external simulator,
  timing and environment dependency conflict with portable CI expectations.
- No hardware-in-the-loop test. Reason: M45 explicitly excludes hardware
  certification and real hardware readiness.
- No runtime collision avoidance tests. Reason: M45 validates static pre-upload
  mission constraints only; runtime avoidance belongs to a later safety/runtime
  milestone.

Recommended verification commands for implementation:

```bash
cargo fmt --all
timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_safety
timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent
timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings
```

If implementation touches `swarm-safety` directly, also run the relevant
`swarm-safety` unit tests with `runlim`.

## Risks and tradeoffs

- **Source-of-truth risk:** `SitlPlan` currently drops no-pose tasks. M45 must
  validate the original scenario before extraction, otherwise `missing_pose` and
  duplicate id checks will be incomplete.
- **Config format risk:** JSON is pragmatic because `serde_json` already exists.
  YAML/RON can be added later, but adding a new config format now increases
  scope.
- **Home resolution risk:** existing SITL scenario has no `base_station`.
  Rejecting it outright would break the golden sample. The plan therefore allows
  fallback to selected agent pose while still enforcing that a home point must be
  resolvable.
- **Safety semantics risk:** static pre-upload checks are not equivalent to
  runtime collision avoidance or certified hardware safety. Docs and README must
  keep that boundary explicit.
- **No-fly zone dimensionality:** existing `Aabb` is 2D; altitude is validated by
  separate min/max rules. This is sufficient for M45 but may need 3D zones later.
- **CLI ordering risk:** safety validation should happen before transport upload,
  but bad connection string validation can still happen first. Tests should
  clarify expected ordering for invalid safety vs invalid connection inputs.
- **MAVLink feature interaction:** invalid mission should be rejected even without
  `mavlink-transport` so portable tests can verify pre-upload safety without
  opening a connection.

## Open questions

- Should `--dry-run` also run safety validation and print violations, or should
  M45 keep validation strictly on `--connection` as a pre-upload gate? Suggested
  default: gate `--connection` first; optionally add dry-run safety output only
  if implementation stays simple.
- Should `--safety-config` override all defaults or merge partially with safe
  defaults? Suggested default: partial JSON config merges with `SitlSafetyConfig::default()`.
- Should no-fly zones be pure 2D for M45 or include altitude ranges immediately?
  Suggested default: keep 2D AABB + separate altitude bounds for M45.
- Should `home_origin` in `MissionUploadOptions` and `SitlSafetyConfig.home` be
  unified now? Suggested default: do not couple them in M45; safety home is a
  local simulation point for radius checks, MAVLink home origin is WGS84
  conversion metadata.
