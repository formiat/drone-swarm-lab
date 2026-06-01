# BEFORE_HARDWARE_A.22 - milestones before real hardware

Дата фиксации: 2026-05-31

Цель документа: описать, что можно сделать без физического железа, чтобы
приблизить проект к real/product-like readiness. Это не план превращения проекта
в production flight stack. Без HIL, bench tests, field tests, safety process and
real hardware project cannot honestly claim production readiness.

Правильная формулировка цели:

```text
Reach hardware-ready research platform level:
when hardware appears, integration starts from a controlled, documented,
tested foundation instead of ad-hoc experiments.
```

Архитектурная граница остается прежней:

- PX4/autopilot owns stabilization, attitude/rate control, motor physics,
  low-level waypoint execution and flight failsafes.
- This project owns mission-level planning, route export, task allocation,
  reallocation, safety/invariant validation, replay, metrics, benchmark evidence
  and local SITL workflows.

## Summary

Recommended pre-hardware milestone chain:

```text
M70 Urban Route Export to SITL/PX4
  -> M71 PX4/SITL Harness and Artifact Validator
    -> M72 Safety and Invariant Gate
      -> M73 Failure Matrix in Mock/SITL
        -> M74 Scenario Generation and Stress Library
          -> M75 Benchmark Evidence Layer
            -> M76 Operational Runbooks and API Boundary
```

This sequence moves the project from "research simulation with SITL artifacts"
to "hardware integration candidate". Real hardware work begins only after these
milestones and still requires separate bench/HIL/flight safety planning.

---

## M70 - Urban Route Export to SITL/PX4

### Goal

Turn Urban planned routes into PX4/SITL waypoint missions.

This is the most direct pre-hardware step: Urban scenarios stop being only
headless simulation routes and become exportable local SITL missions.

### Scope

1. Route-to-waypoint conversion:
   - convert `UrbanPlannedRoute` segments into ordered waypoint items;
   - preserve node/edge/task identity where possible;
   - keep deterministic ordering;
   - include altitude/default mission parameters explicitly.

2. Dry-run export:
   - produce a readable plan without requiring PX4;
   - show route length, waypoint count, start/end, and safety validation status;
   - include mission/run id for artifact traceability.

3. Safety validation reuse:
   - geofence;
   - no-fly/static obstacle checks;
   - basic waypoint sanity;
   - no duplicate ownership for multi-agent exports when applicable.

4. Optional local PX4/SIH artifact:
   - upload-only first;
   - execute only if local environment is stable;
   - store artifact under `results/...`;
   - include manifest, command, config snapshot, SITL log, replay summary.

5. Documentation:
   - clarify that export means local waypoint workflow;
   - no hardware readiness claim;
   - no real obstacle avoidance claim.

### Non-goals

- No hardware.
- No Gazebo gate by default.
- No certified obstacle avoidance.
- No low-level flight control.
- No promise that Urban route constraints remain safe in the real world.

### Done criteria

- Urban route can be converted to SITL waypoint items deterministically.
- Exported mission passes existing safety validation or fails with structured
  reasons.
- Dry-run artifact is readable and reproducible.
- Optional PX4/SIH artifact, if produced, is clearly marked local/manual.
- Docs separate simulation route validity from SITL waypoint execution.

### Tests

#### Tests that need no refactoring

- Unit test: simple Urban route -> expected waypoint order.
- Unit test: route waypoint altitude/defaults are explicit.
- Safety test: invalid exported route is rejected before upload.
- Snapshot-like dry-run text/JSON fixture.

#### Tests that need light refactoring

- Shared route-to-waypoint conversion helper.
- Urban export fixture builder.
- SITL plan assertion helper.

#### Tests that need heavy refactoring

- Manual/ignored PX4/SIH upload integration harness.
- Multi-agent Urban route export to distinct PX4 endpoints.

---

## M71 - PX4/SITL Harness and Artifact Validator

### Goal

Make local PX4/SIH workflows repeatable and inspectable without relying on
manual note-taking.

M58/M59 artifacts exist, but repeating them still depends too much on local
manual procedure. This milestone improves reproducibility before hardware.

### Scope

1. Local harness scripts:
   - start one or two PX4/SIH instances;
   - wait for MAVLink endpoints;
   - run `sitl_supervisor`;
   - collect logs;
   - stop/cleanup processes;
   - write artifacts to a deterministic output directory.

2. Artifact validator:
   - manifest exists and has required fields;
   - run report exists and parses;
   - event log exists and parses;
   - replay summary expected categories are present;
   - agent ids and task ids are consistent;
   - replacement/reallocation events are semantically consistent where expected.

3. Clear manual-only boundary:
   - scripts are not default CI;
   - docs state local assumptions;
   - missing PX4 produces clear failure message.

4. Result discipline:
   - output directory naming;
   - `--run-id`;
   - `--force` semantics;
   - command line capture;
   - config snapshot.

### Non-goals

- No CI-managed PX4 container unless explicitly planned later.
- No hardware.
- No broad PX4 version certification.

### Done criteria

- A developer can rerun M58-like/M59-like local workflows from documented
  scripts.
- Validator can distinguish a complete artifact from a malformed one.
- Failed local setup produces actionable error messages.
- Docs show exact manual commands and expected outputs.

### Tests

#### Tests that need no refactoring

- Artifact validator unit tests with small inline fixtures.
- Missing-file validator tests.
- Manifest required-field tests.
- Replay/event category validation tests.

#### Tests that need light refactoring

- Shared artifact fixture builder.
- Harness dry-run mode that does not launch PX4.
- Portable process cleanup helper for tests.

#### Tests that need heavy refactoring

- Ignored/manual two-PX4 harness integration test.
- PX4 version matrix runner.

---

## M72 - Safety and Invariant Gate

### Goal

Add a stronger simulation-level safety/invariant layer before any future
hardware integration.

This is not flight certification. It is a set of deterministic gates that stop
known-bad missions and catch inconsistent artifacts before they reach SITL or
future hardware bench tests.

### Scope

1. Ownership invariants:
   - no duplicate task ownership;
   - no duplicate route segment ownership where exclusive ownership is required;
   - released tasks must be either reassigned or explicitly abandoned.

2. Mission validity invariants:
   - route does not cross forbidden/static obstacle zones;
   - blocked edges are not used unless policy explicitly allows wait/replan;
   - mission completion requires the documented predicate;
   - unsupported strategy/mission pairs cannot silently claim supported success.

3. Replay/report invariants:
   - completion event references an existing mission item;
   - replacement mission sequence ids remain consistent;
   - replay summary and run report agree on completed/failed/lost agents;
   - artifact manifest commit/build/run metadata is present.

4. CLI/operation gates:
   - invalid mission exits before upload;
   - structured error categories;
   - stable exit codes for validation, runtime, artifact and environment errors.

5. Documentation:
   - clearly separate simulation-level invariants from real flight safety;
   - list what is checked and what is not checked.

### Non-goals

- No certified safety.
- No real collision avoidance guarantee.
- No hardware failsafe validation.
- No regulatory claims.

### Done criteria

- Invariant checks can run without PX4/hardware.
- Invalid scenarios fail before SITL upload/export.
- Reports identify which invariant failed.
- Existing valid scenarios remain green.
- Docs explain that these are pre-hardware gates, not real-world guarantees.

### Tests

#### Tests that need no refactoring

- Duplicate ownership rejection.
- Route through forbidden obstacle rejection.
- Completion-without-predicate rejection.
- Unsupported pair cannot be marked as supported.
- Replay completion seq consistency fixture.

#### Tests that need light refactoring

- Shared invariant assertion helper.
- Small artifact consistency fixtures.
- Shared structured error test helper.

#### Tests that need heavy refactoring

- Property tests over generated scenarios.
- Cross-artifact validator for full result directories.
- Versioned schema compatibility tests.

---

## M73 - Failure Matrix in Mock/SITL

### Goal

Systematically test failure paths before hardware exists.

Successful golden paths are not enough for product-like readiness. This
milestone turns failure handling into a matrix of known scenarios and expected
outcomes.

### Scope

1. Failure categories:
   - agent lost/disconnected;
   - no-progress timeout;
   - mission upload rejected;
   - partial completion then failure;
   - stale telemetry;
   - unsafe route after environment change;
   - duplicate ownership;
   - unsupported strategy selected.

2. Mock/fake tests:
   - deterministic controller failures;
   - deterministic timeout;
   - deterministic partial completion;
   - deterministic mission rejection.

3. Local SITL/manual checks where practical:
   - one or two representative failure paths;
   - no default CI dependency;
   - artifacts validated by M71 validator.

4. Metrics:
   - failure detected;
   - tasks released;
   - tasks reassigned;
   - recovery latency;
   - survivor completion status;
   - unrecovered tasks.

5. Documentation:
   - failure matrix table;
   - supported / experimental / not-tested status;
   - exact recovery semantics.

### Non-goals

- No hardware failure testing.
- No exhaustive physical failsafe validation.
- No real RF/link-loss modeling beyond deterministic simulation profiles.

### Done criteria

- Failure matrix exists in docs.
- Mock/fake tests cover core failure outcomes.
- At least one representative local SITL failure artifact remains valid if
  environment is available.
- Known unsupported paths are explicitly labeled.

### Tests

#### Tests that need no refactoring

- Fake controller lost-agent recovery.
- No-progress timeout fake.
- Upload rejected fake.
- Partial completion then reassignment.
- Failure metrics aggregation test.

#### Tests that need light refactoring

- Reusable fake failure scenario builder.
- Shared failure matrix assertion helper.
- Artifact validator integration with failure reports.

#### Tests that need heavy refactoring

- Manual/ignored local SITL failure harness.
- Multi-failure/repeated failure integration tests.
- Stochastic communication failure sweeps.

---

## M74 - Scenario Generation and Stress Library

### Goal

Replace ad-hoc hand-picked scenarios with reproducible scenario families.

Without hardware, realistic pressure comes from deterministic scenario
variation: maps, blocked edges, bus schedules, packet loss, failures, obstacle
density and task density.

### Scope

1. Scenario generator API:
   - deterministic seed;
   - explicit generator parameters;
   - manifest records generator settings;
   - stable scenario names.

2. Initial generators:
   - Urban block/grid maps;
   - blocked edge schedules;
   - bus route schedules;
   - packet-loss profiles;
   - agent failure profiles;
   - wildfire threat patterns.

3. Scenario library categories:
   - tiny;
   - small;
   - medium;
   - stress;
   - regression-stable;
   - experimental.

4. Validation:
   - generated scenario is valid;
   - generated scenario is deterministic for the same seed;
   - invalid parameter combinations fail clearly.

5. Documentation:
   - how to regenerate;
   - which profiles are default regression;
   - which profiles are exploratory.

### Non-goals

- No random benchmark without pinned seeds.
- No huge generated suites in default CI.
- No claim that generated scenarios match real-world distributions.

### Done criteria

- At least one Urban generator is committed.
- Generated scenarios include manifest metadata.
- Deterministic tests prove seed stability.
- Generated scenarios can be used by benchmark/regression tooling.

### Tests

#### Tests that need no refactoring

- Same seed -> same scenario.
- Different seed -> changed but valid scenario.
- Invalid generator config rejected.
- Generated Urban map passes DSL validation.

#### Tests that need light refactoring

- Scenario generator trait/helper.
- Shared manifest metadata assertion helper.
- Small generated scenario fixture.

#### Tests that need heavy refactoring

- Property tests over generated maps.
- Large scenario stress runner.
- Cross-version generator reproducibility tests.

---

## M75 - Benchmark Evidence Layer

### Goal

Upgrade benchmark output from "we ran many seeds" to interpretable evidence.

The current 1000-seed artifact is useful, but product-like readiness requires
knowing where the system is stable, where it degrades, and which claims are
unsupported.

### Scope

1. Statistical summary:
   - mean;
   - stddev;
   - stderr;
   - confidence interval;
   - min/max;
   - failure rate.

2. Degradation curves:
   - packet loss;
   - latency;
   - number of agents;
   - map size;
   - task density;
   - urban obstacle density;
   - bus detection probability.

3. Support matrix integration:
   - supported;
   - experimental;
   - unsupported;
   - known bug;
   - not evaluated.

4. Current vs historical artifacts:
   - classify artifact by code commit and schema version;
   - docs do not present stale packs as current evidence;
   - benchmark result README explains scope.

5. Urban benchmark entrypoint:
   - either include Urban in an explicit benchmark mode;
   - or keep Urban as separate scenario-suite evidence with clear docs.

### Non-goals

- No publication paper unless explicitly chosen.
- No 1000-seed rerun by default if existing evidence is enough.
- No unsupported pair success claims.

### Done criteria

- Reports include statistical uncertainty for key metrics.
- At least one degradation sweep artifact exists.
- Support matrix is visible in benchmark/report docs.
- Urban benchmark scope is explicit.
- Current/historical distinction is machine-checkable or at least documented in
  artifact metadata.

### Tests

#### Tests that need no refactoring

- Confidence interval helper tests.
- Benchmark export includes statistical fields.
- Support matrix report tests.
- Manifest identity tests.

#### Tests that need light refactoring

- Benchmark pack validation helper.
- Multi-pack comparison helper.
- Degradation suite runner helper.

#### Tests that need heavy refactoring

- Statistical delta validation.
- Historical artifact database.
- Long-run reproducibility harness.

---

## M76 - Operational Runbooks and API Boundary

### Goal

Make the project easier to operate and extend before hardware appears.

This is not cosmetic polish. For any future bench/HIL/hardware attempt,
operators need repeatable procedures, failure handling notes, structured logs
and extension boundaries.

### Scope

1. Runbooks:
   - simulation run;
   - Urban scenario run;
   - SITL dry-run/export;
   - local PX4/SIH manual run;
   - failure recovery procedure;
   - artifact validation procedure.

2. Operational checklist:
   - pre-run validation;
   - environment assumptions;
   - expected artifacts;
   - stop/abort procedure;
   - post-run checks.

3. Error handling:
   - structured CLI errors;
   - stable exit codes;
   - actionable messages for missing PX4, bad scenario, unsafe mission,
     artifact mismatch.

4. API/platform boundary:
   - external-style mission example;
   - schema compatibility smoke;
   - report/replay schema policy;
   - "no public semver promise yet" unless explicitly chosen.

5. Documentation:
   - what is ready for simulation;
   - what is ready for local SITL;
   - what is not ready for hardware;
   - what must happen when hardware appears.

### Non-goals

- No public release polish unless chosen separately.
- No semver commitment unless API branch is selected.
- No hardware procedure beyond future-prep checklist.

### Done criteria

- A new developer can run simulation and local SITL dry-run from docs.
- Artifacts can be validated after a run.
- Error messages are actionable.
- Extension boundary is documented with at least one real mission example.
- Docs clearly state the project is hardware-integration-ready candidate, not
  hardware-ready product.

### Tests

#### Tests that need no refactoring

- Docs smoke tests for required limitation phrases.
- CLI error tests for missing/invalid arguments.
- Schema compatibility smoke for existing fixtures.
- Artifact validation command tests with fixtures.

#### Tests that need light refactoring

- Shared CLI assertion helper.
- Shared docs phrase assertion helper.
- External-style extension fixture.

#### Tests that need heavy refactoring

- Public API compatibility checks.
- Versioned schema migration tests.
- End-to-end local SITL runbook smoke with ignored/manual marker.

---

## Resulting project level

After M70-M76, without hardware, the project would be:

```text
hardware-ready research platform / hardware-integration candidate
```

It still would not be:

- a production drone system;
- a certified safety stack;
- a real perception system;
- a hardware-proven swarm controller;
- ready for uncontrolled field use.

When hardware appears, the next stage should start with a separate plan:

```text
bench without propellers
  -> MAVLink connectivity
    -> mission upload only
      -> telemetry mapping
        -> abort/failsafe validation
          -> single-drone constrained flight
            -> multi-drone only after separate safety review
```

The value of the pre-hardware milestones is that this later stage begins from a
controlled, evidence-backed foundation instead of improvised scripts and unclear
claims.
