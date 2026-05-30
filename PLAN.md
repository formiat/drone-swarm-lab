# План M58 - Live Multi-Agent PX4/SIH Execute Orchestration

## Context

Планируем этап **M58 - Live Multi-Agent PX4/SIH Execute Orchestration** из
`docs_raw/DRONE_A.19.md`.

Текущий выбранный scope остается локальным:

- local PX4 SITL / SIH only;
- mock/fake tests;
- manual/ignored local integration checks;
- no physical hardware;
- no HIL/Gazebo as required gate;
- no failure/reallocation in M58;
- no distributed onboard coordination.

Что уже есть по локальному коду:

- `sitl_agent --connection --execute` умеет single-agent PX4/SIH golden path:
  mission upload, arm/takeoff/start, telemetry progress, optional run report and
  replay log.
- `sitl_agent --multi-agent-config` умеет выбрать task subset and connection
  settings for one agent from `multi_sitl.v1`.
- `sitl_supervisor --dry-run` / `--mock` уже строит multi-agent manifest,
  проверяет duplicate ownership and runs portable mock supervisor flow.
- M57 уже вынес supervisor/controller boundary:
  `AgentController`, `MockAgentController`, shared supervisor loop, assertable
  `SupervisorMetrics`, fake-controller tests.
- M55/M52 доказали upload-only основу: две local PX4 SIH instance can accept
  distinct waypoint subsets, но supervisor пока не умеет вести их как один live
  execute run.

Главная цель M58: получить первый local PX4/SIH multi-agent execute workflow
under `sitl_supervisor`, где два агента с разными MAVLink endpoints/system ids
исполняют disjoint task subsets, а supervisor пишет общий event log and common
final report.

Ключевое архитектурное требование: **не копировать большую часть
`sitl_agent.rs` в `sitl_supervisor.rs`**. Сначала нужно вынести reusable
connection/execute lifecycle из binary в library module, затем подключить его
через `Px4AgentController`.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует.

Обязательные локальные протоколы прочитаны:

- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/notion_access_protocol.md`;
- `/home/formi/Documents/RustProjects/multi-agent-orchestrator-rs/docs/gitlab_access_protocol.md`.

Notion/GitLab context не читался: в prompt нет Notion task id, GitLab MR,
review comments or discussions; `notion_policy` указан как optional.

Локально проверено для планирования:

- `docs_raw/DRONE_A.19.md`;
- текущий `crates/swarm-examples/src/bin/sitl_supervisor.rs`;
- текущий `crates/swarm-examples/src/sitl_supervisor.rs`;
- `crates/swarm-examples/src/sitl_multi_agent.rs`;
- `crates/swarm-examples/src/bin/sitl_agent.rs`;
- `crates/swarm-examples/src/sitl_observability.rs`;
- `crates/swarm-examples/src/sitl_report.rs`;
- `README.md`, `docs/STATUS.md`, `docs/SITL_SETUP.md`, `docs/REPLAY.md`,
  `docs/HARDWARE_READINESS.md`;
- relevant tests under `crates/swarm-examples/tests/`.

## Affected components

- `crates/swarm-examples/src/bin/sitl_agent.rs`
  - current source of live PX4/SIH execute logic;
  - should become thinner after extracting reusable lifecycle code;
  - must keep existing CLI behavior and tests.
- `crates/swarm-examples/src/sitl_connection.rs` or
  `crates/swarm-examples/src/sitl_execute.rs`
  - new preferred library module for reusable single-agent connection lifecycle:
    upload-only, execute, telemetry progress, run report mapping, event
    recording hooks, feature-gated MAVLink driver.
- `crates/swarm-examples/src/sitl_supervisor.rs`
  - add `Px4AgentController` behind `mavlink-transport`;
  - add fake live controller tests over M58 lifecycle;
  - add multi-agent live execute supervisor/report aggregation.
- `crates/swarm-examples/src/bin/sitl_supervisor.rs`
  - extend CLI with explicit live mode, likely `--connection`;
  - add `--execute`, `--run-report`, `--timeout`,
    `--telemetry-timeout`, `--no-progress-timeout`;
  - reject conflicting `--mock` / `--dry-run` / `--connection` modes.
- `crates/swarm-examples/src/sitl_multi_agent.rs`
  - keep manifest/config as the source for agent endpoint/system/component,
    lifecycle and task subset;
  - optionally add helper for validating all live agents use
    `MultiAgentLifecycle::Execute` in M58 live mode.
- `crates/swarm-examples/src/sitl_observability.rs`
  - add or emulate multi-agent run-level events:
    `multi_agent_run_started` and `multi_agent_run_finished`;
  - ensure per-agent emitted events retain `agent_id` semantics in the common
    supervisor log.
- `crates/swarm-examples/src/sitl_report.rs`
  - add a multi-agent report type, for example
    `SitlMultiAgentRunReport`;
  - keep existing single-agent `SitlRunReport` backwards compatible.
- `crates/swarm-examples/src/lib.rs`
  - export any new internal modules needed by tests and binaries.
- `crates/swarm-examples/tests/sitl_agent.rs`
  - keep existing `sitl_agent` and `sitl_supervisor` subprocess coverage;
  - add M58 CLI negative tests.
- `crates/swarm-examples/tests/sitl_docs.rs`
  - update docs assertion coverage for M58 wording and command examples.
- `crates/swarm-examples/tests/replay_cli.rs`
  - add synthetic multi-agent execute log summary coverage if replay output
    changes.
- `README.md`
  - add M58 section and command examples;
  - state local PX4/SIH execute scope and non-hardware boundary.
- `docs/STATUS.md`
  - mark M58 as planned/experimental during implementation and complete only
    after artifact capture.
- `docs/SITL_SETUP.md`
  - add how to run two local PX4 SIH instances and supervisor execute mode;
  - document endpoint/system id assumptions and troubleshooting.
- `docs/REPLAY.md`
  - document common multi-agent execute event log and replay summary.
- `docs/HARDWARE_READINESS.md`
  - explicitly state M58 is local PX4/SIH validation, not hardware readiness.
- `results/m58_multi_agent_px4_sih_execute_YYYY-MM-DD/`
  - final captured artifact directory, created only after a real local PX4/SIH
    run.

## Implementation steps

1. **Зафиксировать current M57/M52 behavior before live refactor**
   - Paths:
     - `crates/swarm-examples/src/bin/sitl_supervisor.rs`;
     - `crates/swarm-examples/src/sitl_supervisor.rs`;
     - `crates/swarm-examples/tests/sitl_agent.rs`.
   - Run quick portable baseline before code changes:
     - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_supervisor`
     - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent multi_agent_sitl_supervisor`
   - Цель: убедиться, что dry-run/mock paths green before adding live mode.

2. **Extract reusable single-agent connection lifecycle from `sitl_agent`**
   - Preferred new path:
     - `crates/swarm-examples/src/sitl_connection.rs`
   - Move or wrap from `src/bin/sitl_agent.rs`:
     - live connection config;
     - upload-only flow;
     - execute flow;
     - `SitlGoldenPathDriver`-style abstraction;
     - telemetry progress loop;
     - progress/failure mapping into `SitlRunReport`;
     - event recorder integration.
   - Keep binary parser and user-facing `sitl_agent` CLI in the binary.
   - Add a library API shaped for both `sitl_agent` and future
     `Px4AgentController`, for example:
     - `SitlConnectionRunConfig`;
     - `SitlConnectionLifecycle`;
     - `run_sitl_connection(plan, connection, lifecycle, report/log hooks)`;
     - `MissionExecutor` / `MissionDriver` trait for fake tests.
   - Important: preserve existing `sitl_agent --connection --execute` output,
     reports and replay behavior unless a change is explicitly required.

3. **Add supervisor live mode CLI**
   - Path:
     - `crates/swarm-examples/src/bin/sitl_supervisor.rs`.
   - Extend `SupervisorMode`:
     - `DryRun`;
     - `Mock`;
     - `Connection`.
   - Proposed CLI:
     ```bash
     cargo run -p swarm-examples --features mavlink-transport --bin sitl_supervisor -- \
       --connection \
       --scenario scenarios/sitl.multi-agent.json \
       --config scenarios/sitl.multi-agent.config.json \
       --execute \
       --replay-log results/m58_multi_agent_px4_sih_execute_YYYY-MM-DD/run.sitl-log.json \
       --run-report results/m58_multi_agent_px4_sih_execute_YYYY-MM-DD/report.json \
       --timeout 120 \
       --telemetry-timeout 30 \
       --no-progress-timeout 45
     ```
   - Validation rules:
     - exactly one of `--dry-run`, `--mock`, `--connection`;
     - `--execute` required for M58 `--connection`;
     - `--run-report` valid only with `--connection --execute`;
     - `--fail-agent` / `--fail-after-ticks` remain mock-only in M58;
     - `--heartbeat-timeout-ticks` remains mock-only unless reused under an
       explicit live failure milestone later;
     - missing values and invalid numeric durations produce typed/actionable
       errors.
   - Without `mavlink-transport`, live `--connection` must return
     `SitlError::FeatureMissing { feature: "mavlink-transport" }` or an
     equally typed/actionable error.

4. **Enforce local PX4/SIH hardware boundary for supervisor live mode**
   - Paths:
     - `crates/swarm-examples/src/bin/sitl_supervisor.rs`;
     - `crates/swarm-examples/src/sitl_supervisor.rs`;
     - possibly `crates/swarm-examples/src/sitl_plan.rs`.
   - Use `classify_connection_string` for every agent connection in the
     multi-agent config.
   - Default M58 live supervisor should accept only local PX4/SITL UDP classes.
   - Do not silently accept remote UDP, wildcard UDP, TCP or serial hardware
     candidates.
   - If `--allow-hardware-candidate` is added for consistency, keep it
     explicitly documented as out-of-scope for M58 and do not use it in the
     captured artifact.

5. **Implement `Px4AgentController`**
   - Path:
     - `crates/swarm-examples/src/sitl_supervisor.rs`, or split to
       `crates/swarm-examples/src/sitl_px4_controller.rs` if the module grows.
   - Behind `#[cfg(feature = "mavlink-transport")]`:
     - construct controller from `MultiAgentSitlManifestAgent`;
     - build per-agent `SitlPlan` / waypoint list from manifest data;
     - use per-agent connection string, `system_id`, `component_id`;
     - call reusable connection lifecycle extracted in step 2;
     - map lifecycle into supervisor-level states.
   - Without the feature:
     - compile a stub path that returns typed feature-missing error.
   - Minimal states:
     - `Pending`;
     - `Uploaded`;
     - `Started`;
     - `InProgress`;
     - `Completed`;
     - `Failed`;
     - `Aborted`.
   - M58 should not implement failure/reallocation. On one agent failure, final
     overall status should be failed/partial, not reallocated.

6. **Define launch policy**
   - Path:
     - `crates/swarm-examples/src/sitl_supervisor.rs`.
   - Default: sequential launch in manifest order.
   - Respect `start_delay_ms` from `scenarios/sitl.multi-agent.config.json`.
   - Do not make parallel launch required in M58.
   - Optional only if very low risk:
     - add `--parallel` as experimental;
     - otherwise document it as future work.
   - Sequential default is chosen because local SIH debugging is easier and
     safer than racing two MAVLink lifecycle starts.

7. **Add per-agent telemetry aggregation**
   - Paths:
     - `crates/swarm-examples/src/sitl_supervisor.rs`;
     - `crates/swarm-examples/src/sitl_connection.rs`.
   - Track:
     - `(agent_id, seq) -> task_id`;
     - heartbeat/progress counts per agent;
     - completed task ids per agent;
     - failed/aborted status per agent;
     - no-progress timeout per agent;
     - mission item count per agent;
     - final status per agent.
   - The report aggregation must not rely on parsing stderr.

8. **Write common multi-agent event log**
   - Path:
     - `crates/swarm-examples/src/sitl_observability.rs`.
   - Add event variants or equivalent existing-event representation for:
     - `MultiAgentRunStarted { agent_count, scenario }`;
     - `MultiAgentRunFinished { overall_status }`.
   - Preserve existing event schema compatibility where practical:
     - do not rename existing variants;
     - update replay summary for new variants;
     - keep per-agent events attributable via `agent_id` in common log.
   - Add synthetic replay tests before relying on a live PX4 run.

9. **Add final multi-agent report**
   - Path:
     - `crates/swarm-examples/src/sitl_report.rs`.
   - Suggested report fields:
     - `schema_version`;
     - `run_id`;
     - `scenario`;
     - `config`;
     - `mode`;
     - per-agent connection/system/component;
     - per-agent lifecycle;
     - per-agent mission item count;
     - per-agent completed task count;
     - total completed tasks;
     - failed/aborted agents;
     - overall status;
     - event log path;
     - known limitations.
   - Add writer helper that creates parent directories like existing
     `write_sitl_run_report`.

10. **Preserve mock/dry-run portability**
    - Paths:
      - `crates/swarm-examples/src/bin/sitl_supervisor.rs`;
      - `crates/swarm-examples/src/sitl_supervisor.rs`;
      - `crates/swarm-examples/tests/sitl_agent.rs`.
    - Existing `--dry-run` and `--mock` must continue to compile and test
      without `mavlink-transport` and without PX4.
    - Any `mavlink-transport` import must be feature-gated so portable CI does
      not start requiring MAVLink dependencies beyond existing optional feature
      behavior.

11. **Create fake-controller automated coverage for M58 logic**
    - Paths:
      - `crates/swarm-examples/src/sitl_supervisor.rs`, module tests; or
      - `crates/swarm-examples/tests/sitl_supervisor.rs` if public test API is
        cleaner.
    - Cover:
      - two fake live agents upload/start/progress/complete;
      - one fake live agent fails and overall status becomes failed/partial
        without reallocation;
      - `start_delay_ms` ordering is honored via fake scheduler/time provider,
        not real sleeps;
      - report aggregation from two controllers;
      - event log has run started/finished and per-agent attribution.

12. **Add CLI tests**
    - Path:
      - `crates/swarm-examples/tests/sitl_agent.rs`.
    - Add subprocess tests for:
      - `--mock` + `--connection` conflict;
      - `--dry-run` + `--connection` conflict;
      - `--connection` without `--execute`;
      - `--run-report` without live execute mode;
      - missing `--timeout`, `--telemetry-timeout`,
        `--no-progress-timeout` values if these flags are used;
      - invalid numeric values;
      - live mode without `mavlink-transport` returns actionable feature error;
      - hardware-candidate endpoint rejected unless explicitly allowed.

13. **Add optional ignored/manual PX4/SIH integration test**
    - Path:
      - `crates/swarm-examples/tests/sitl_live_multi_agent.rs` or existing
        `sitl_agent.rs` if keeping all SITL CLI tests together.
    - Mark as `#[ignore]` and require explicit env vars such as:
      - `PX4_DIR`;
      - `PX4_AGENT0_ENDPOINT`;
      - `PX4_AGENT1_ENDPOINT`.
    - It should never run in default CI.
    - It may take longer than 5 minutes depending on PX4 startup and must be
      documented as a manual/local validation, not an automated gate.

14. **Capture M58 artifact**
    - Path:
      - `results/m58_multi_agent_px4_sih_execute_YYYY-MM-DD/`.
    - Contents:
      - `README.md`;
      - exact commands;
      - PX4 path/version/commit if available;
      - endpoints and system ids;
      - stdout/stderr snippets;
      - `report.json`;
      - `run.sitl-log.json`;
      - replay summary.
    - This is the only step that requires real local PX4/SIH processes.
    - If implementation workflow has a strict total runtime limit, do not run
      this step automatically; document the exact manual command and leave the
      artifact capture as pending until the user explicitly allows the live
      local run.

15. **Update README and all companion Markdown docs**
    - Required:
      - `README.md`;
      - `docs/STATUS.md`;
      - `docs/SITL_SETUP.md`;
      - `docs/REPLAY.md`;
      - `docs/HARDWARE_READINESS.md`.
    - Update docs to state:
      - M58 is local PX4/SIH execute orchestration;
      - no real hardware readiness is claimed;
      - mock/dry-run remain portable;
      - failure/reallocation remains M59;
      - manual/ignored live runs may exceed 5 minutes and are not default CI.
    - Update `crates/swarm-examples/tests/sitl_docs.rs` with required wording.

16. **Final verification before commit**
    - Because Rust files will change:
      - `timeout 300s cargo fmt --all`
      - `timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings`
    - Portable targeted tests:
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_supervisor`
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent`
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs`
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test replay_cli`
    - Feature-gated targeted tests:
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --features mavlink-transport sitl_connection`
      - `timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --features mavlink-transport sitl_supervisor`
    - Check no proptest persistence files:
      - `rg --files -g '*.proptest-regressions'`

## Testing strategy

### 1. Tests that need no refactoring

- Existing multi-agent config parse/validation tests in
  `crates/swarm-examples/src/sitl_multi_agent.rs`.
- Existing duplicate ownership rejection tests:
  - config-level;
  - `sitl_supervisor --dry-run` subprocess.
- Existing standalone command generation tests for per-agent `sitl_agent`
  commands.
- Existing `sitl_supervisor --mock` tests:
  - distinct subsets;
  - mock reallocation;
  - manifest output;
  - missing/invalid CLI args.
- New CLI negative tests can be added with existing subprocess helpers:
  - `--mock` / `--connection` conflict;
  - `--dry-run` / `--connection` conflict;
  - `--connection` without `--execute`;
  - `--run-report` outside live execute mode;
  - missing/invalid timeout values;
  - no-feature actionable error for live mode.
- Synthetic replay roundtrip can be added without PX4 by constructing a
  multi-agent execute event log in memory.
- Docs assertions in `sitl_docs` can be updated without refactoring.

### 2. Tests that need light refactoring

- Extracted single-agent connection lifecycle with fake driver:
  - upload success;
  - start success;
  - telemetry progress to completion;
  - upload failure maps to failed report;
  - telemetry no-progress timeout maps to failed/aborted report.
- Fake `Px4AgentController` execute lifecycle:
  - `Pending -> Uploaded -> Started -> InProgress -> Completed`;
  - failure maps to `Failed` without calling runtime reallocation in M58.
- Per-agent telemetry mapping:
  - `(agent_id, seq) -> task_id`;
  - duplicate seq across agents is valid because agent id is part of key.
- Supervisor report aggregation:
  - two fake controllers complete;
  - one fake controller fails;
  - aggregate totals and overall status are correct.
- Event log ordering and attribution:
  - multi-agent run started before per-agent events;
  - multi-agent run finished after terminal agent states;
  - per-agent event attribution is not lost.
- Hardware boundary tests:
  - local UDP endpoints accepted;
  - remote/wildcard/serial hardware candidates rejected by default.

### 3. Tests that need heavy refactoring

- Manual/ignored two-instance PX4/SIH execute integration.
  - Requires local PX4 build/processes and endpoint coordination.
  - Not suitable for default CI.
  - May exceed 5 minutes; must be documented and run only with explicit user
    approval in constrained workflows.
- Parallel launch smoke.
  - Optional/future unless `--parallel` is added in M58.
  - Requires concurrency-safe controller orchestration and deterministic fake
    scheduler tests before any live run.
- Time-bounded live SITL smoke.
  - Could become a local release checklist item later, but not a default
    automated test.
- Full cross-check between common event log and final PX4 telemetry traces.
  - Useful after M58, but heavy if it requires replaying real telemetry logs.

## Risks and tradeoffs

- **Scope risk:** extracting `sitl_agent` live lifecycle into a reusable module
  can be larger than adding a subprocess wrapper, but it avoids poor metrics and
  event merging in M58/M59.
- **Subprocess fallback:** spawning multiple `sitl_agent` processes is faster to
  implement but weaker for common report, abort handling, and future
  reallocation. Use only if library extraction proves too risky in one pass, and
  document it as technical debt.
- **Feature gating:** live controller code must not make portable tests require
  `mavlink-transport` or PX4.
- **Hardware safety boundary:** connection strings from multi-agent config can
  accidentally point at remote/hardware candidates. M58 must reject those by
  default.
- **Timing flakiness:** local SIH execute depends on PX4 process readiness,
  endpoints and arming state. Keep automated tests fake/mock; keep live
  validation manual/ignored.
- **Event schema churn:** adding multi-agent run events should not break replay
  of existing single-agent and mock logs.
- **Report semantics:** if one agent completes and one fails, define
  `overall_status` clearly (`completed`, `failed`, `partial_failed`, or similar)
  before writing artifacts.
- **Runtime limits:** implementation workflows may forbid runs longer than 5
  minutes. The live PX4/SIH artifact can exceed that; plan and docs must mark it
  as manual unless explicit permission is given.

## Open questions

1. Exact live supervisor CLI shape:
   - use bare `--connection` as mode flag with per-agent connections from
     config;
   - or use a more explicit name like `--px4-sih` / `--live`.
   The current recommendation is `--connection` for consistency with
   `sitl_agent`, but parser errors must make it clear that supervisor
   connections come from the multi-agent config.
2. Should M58 add `--parallel`?
   - Recommendation: no, keep sequential default only unless it is trivial after
     the fake controller abstraction.
3. Should live supervisor support `upload_only` lifecycle values from config?
   - M58 done criteria says lifecycle execute, so recommendation is to reject
     non-`execute` agents in live supervisor mode for now.
4. Where should multi-agent report schema live?
   - Recommendation: `sitl_report.rs`, next to single-agent report, with a new
     schema version such as `sitl_multi_agent_run_report.v1`.
5. Should new event variants include per-agent nested event streams or one flat
   event stream?
   - Recommendation: one flat common event stream for replay simplicity, plus
     explicit `agent_id` where attribution matters.
6. What is the exact `overall_status` enum?
   - Recommendation: start with `completed`, `failed`, `partial_failed`,
     `aborted`, `timed_out`; document mapping from per-agent final statuses.
7. How to capture PX4 version reliably?
   - Possible sources: local PX4 git commit in `PX4_DIR`, command output, or a
     manually recorded version line in the result README.
