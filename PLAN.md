# Context

Нужно реализовать M85 из `docs_raw/DRONE_A.25.md`: Urban Multi-Agent
Deconfliction. Цель M85 - разрешить нескольким дронам работать на одном Urban
road graph без одновременного владения одним и тем же route segment.

Важная граница: это mission-level deconfliction, не physical collision
avoidance. M85 не должен превращаться в 3D separation, lidar/raycast,
RF-coordination, PX4/Gazebo/HIL или hardware-readiness работу. Он должен
работать внутри текущей simulation/replay/metrics архитектуры и сохранять
одиночные Urban сценарии без изменений.

Исходный план M85 в `docs_raw/DRONE_A.25.md:558`-`643` требует:

- segment ownership registry;
- right-of-way policies `FirstCome`, `Priority`, `RoundRobin`;
- reserve-before-enter / release-after-complete поведение;
- wait/replan/abort на locked segment;
- replay events для lock/conflict/wait/replan/abort;
- metrics для conflict/wait/replan/abort/utilization/delay;
- детерминированные автотесты и обновление пользовательских docs.

Notion/GitLab в пользовательском prompt не упомянуты. По прочитанным
протоколам `notion_policy=optional`, дополнительных remote reads не требуется.
Удалённый доступ не использовался.

# Investigation context

`INVESTIGATION.md` в workspace отсутствует.

Релевантные находки по текущему коду:

- `crates/swarm-types/src/urban.rs:28`-`44` уже содержит `UrbanEdgeId`, а
  `UrbanRouteSegment` находится в `crates/swarm-types/src/urban.rs:152`-`160`.
  Это правильная основа для lock keys.
- `UrbanBlockedPolicy` уже есть в `crates/swarm-types/src/urban.rs:305`-`313`
  и покрывает Wait/Replan/Abort, но это policy для временно заблокированных
  edges, а не right-of-way между агентами.
- `UrbanState` сейчас лежит в `crates/swarm-sim/src/runner/types.rs:150`-`167`
  и содержит `blocked_route_policy`, но не содержит deconfliction config.
- `run_urban_patrol` в `crates/swarm-sim/src/runner/urban_patrol.rs:52`-`635`
  выбирает одного alive agent (`:93`-`:121`) как primary control path. Остальные
  агенты используются через `urban_analysis_agent_states` только для replay
  analysis (`crates/swarm-sim/src/runner/urban_helpers.rs:77`-`101`), то есть
  сейчас нет настоящего multi-agent Urban control.
- M74 blocked-route loop уже делает reserve-like decision point на границе
  сегмента: `distance_on_segment == 0.0` в
  `crates/swarm-sim/src/runner/urban_patrol.rs:367`-`385`. M85 должен
  встроиться в этот boundary перед `UrbanSegmentEntered`.
- Replan helper `try_replan` уже существует в
  `crates/swarm-sim/src/runner/urban_patrol.rs:655`-`711`; его можно
  обобщить для исключения locked edges, а не только temporary blocked edges.
- Replay event enum находится в `crates/swarm-replay/src/event_log.rs:22`-`266`.
  M74 events уже добавлены в `:216`-`:265`; M85 нужно добавить события
  рядом с ними с backward-compatible serde.
- Metrics `RunMetrics` находятся в `crates/swarm-metrics/src/metrics/run.rs:4`-`189`.
  Urban blocked-route metrics уже в `:171`-`:179`; M85 должен добавить новые
  поля с `#[serde(default)]`.
- Aggregate/report exports нужно протянуть через:
  `crates/swarm-metrics/src/metrics/aggregate.rs:150`-`193`,
  `crates/swarm-metrics/src/metrics/aggregate.rs:370`-`545`,
  `crates/swarm-sim/src/report_export/json.rs:99`-`127`,
  `crates/swarm-sim/src/report_export/csv.rs:78`-`104`,
  `crates/swarm-sim/src/report_export/focused.rs:80`-`135`.
- Urban analysis artifacts сейчас пишутся в
  `crates/swarm-examples/src/strategy_comparison_runtime/urban_artifacts.rs:38`-`108`;
  manifest содержит route trace / judge report / event counts / separation
  summary, но не ownership/deconfliction records.
- User-facing docs сейчас явно говорят, что Urban multi-agent deconfliction
  ещё не реализован: `README.md:799`-`806`, `README.md:838`-`840`,
  `docs/STATUS.md:138`-`157`, `docs/STATUS.md:196`.

Проверка текущей базы перед планированием:

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban_blocked_policy -- --nocapture
```

Результат: passed, 3 M74 blocked-route tests зелёные.

# Affected components

- `crates/swarm-types/src/urban.rs` - новые типы `UrbanRightOfWayPolicy`,
  `UrbanDeconflictionConfig`, `UrbanSegmentLock`, `UrbanSegmentConflictRecord`
  или их минимальные equivalents.
- `crates/swarm-sim/src/runner/types.rs` - новое поле
  `UrbanState.deconfliction`.
- `crates/swarm-sim/src/urban/deconfliction.rs` - новый runtime registry для
  segment locks и policy resolution.
- `crates/swarm-sim/src/urban/mod.rs` - экспорт нового deconfliction модуля.
- `crates/swarm-sim/src/runner/urban_patrol.rs` - переход от single-primary
  control к multi-agent Urban patrol state при включённом deconfliction.
- `crates/swarm-sim/src/runner/urban_helpers.rs` и
  `crates/swarm-sim/src/runner/urban_events.rs` - общие helper функции для
  per-agent route state и replay events.
- `crates/swarm-replay/src/event_log.rs` - M85 replay event variants и serde
  tests.
- `crates/swarm-replay/src/replay.rs` - summary/timeline rendering/counting для
  M85 events.
- `crates/swarm-sim/src/urban_analysis/events.rs` и
  `crates/swarm-sim/src/urban_analysis/trace.rs` - event counts/trace handling,
  чтобы M85 events попадали в Urban artifacts.
- `crates/swarm-metrics/src/metrics/run.rs`,
  `crates/swarm-metrics/src/metrics/aggregate.rs`,
  `crates/swarm-metrics/src/metrics/display.rs`,
  `crates/swarm-metrics/src/metrics/tests.rs` - новые metrics + aggregate tests.
- `crates/swarm-sim/src/report_export/{json,csv,focused,markdown,compare}.rs`
  и tests - экспорт/сравнение новых M85 metrics.
- `crates/swarm-scenarios/src/urban.rs` и, при необходимости,
  `scenarios/urban.multi-agent-deconflict.json` - deterministic fixture для
  конфликтующих route segments и всех policy variants.
- `crates/swarm-examples/src/strategy_comparison_runtime/urban_artifacts.rs` -
  ownership/deconfliction artifact records в `urban_analysis/manifest.json`.
- `crates/swarm-examples/src/artifact_validator.rs` и
  `crates/swarm-examples/tests/artifact_validator.rs` - лёгкий follow-up: если
  M85 artifacts пишутся в benchmark pack, валидировать отсутствие simultaneous
  duplicate ownership.
- Docs: `README.md`, `docs/STATUS.md`, `docs/REPLAY.md`,
  `docs/SCENARIO_DSL.md`, `docs/BENCHMARK_RESULTS.md`,
  `docs/EXTENSION_GUIDE.md`, `docs/OPERATIONAL_RUNBOOKS.md`,
  `docs/ARTIFACT_VALIDATION.md` и docs smoke tests
  `crates/swarm-examples/tests/sitl_docs.rs`.

# Implementation steps

1. Добавить M85 DSL/config типы.

   Файлы и anchors:

   - `crates/swarm-types/src/urban.rs:305` - рядом с `UrbanBlockedPolicy`
     добавить `UrbanRightOfWayPolicy`.
   - `crates/swarm-sim/src/runner/types.rs:150` - добавить
     `UrbanDeconflictionConfig` и поле `UrbanState.deconfliction`.

   Материализуемый результат:

   - JSON/serde config поддерживает:

     ```rust
     #[serde(rename_all = "snake_case")]
     enum UrbanRightOfWayPolicy {
         FirstCome,
         Priority,
         RoundRobin,
         MissionCriticalOverride,
     }

     struct UrbanDeconflictionConfig {
         enabled: bool,
         right_of_way_policy: UrbanRightOfWayPolicy,
         locked_segment_policy: UrbanBlockedPolicy,
         agent_priorities: BTreeMap<AgentId, u8>,
     }
     ```

   - `enabled=false` по умолчанию сохраняет single-agent/analysis-only
     поведение текущих Urban сценариев.
   - `Priority` использует `agent_priorities`; отсутствующий агент получает
     priority `0`.
   - `MissionCriticalOverride` добавляется как future hook, но в M85 должен
     возвращать typed unsupported/validation error либо вести себя как
     `Priority` только если явно покрыто тестом. Не оставлять silent behavior.

2. Реализовать segment ownership registry как отдельный модуль.

   Файлы и anchors:

   - новый `crates/swarm-sim/src/urban/deconfliction.rs`;
   - `crates/swarm-sim/src/urban/mod.rs` - экспорт public helper типов/функций.

   Материализуемый результат:

   - `UrbanSegmentLockRegistry` хранит:
     - `edge_id`;
     - `holder_agent_id`;
     - `acquired_at_tick`;
     - `planned_release` (`OnSegmentCompleted { segment_index }` достаточно
       для M85);
     - conflict history.
   - Методы:

     ```rust
     fn request_lock(&mut self, request: SegmentLockRequest, tick: u64)
         -> SegmentLockDecision;
     fn release(&mut self, edge_id: &UrbanEdgeId, agent_id: &AgentId, tick: u64)
         -> Option<UrbanSegmentLock>;
     fn active_locks(&self) -> impl Iterator<Item = &UrbanSegmentLock>;
     fn conflict_history(&self) -> &[UrbanSegmentConflictRecord];
     ```

   - `request_lock` не должен зависеть от порядка итерации `HashMap`; для
     simultaneous requests использовать stable сортировку по `(tick,
     request_order, agent_id)`.
   - Для policy:
     - `FirstCome`: текущий holder сохраняет lock, первый request получает
       lock, остальные получают conflict/lost decision.
     - `Priority`: выигрывает максимальный priority, tie-breaker стабильный по
       agent id.
     - `RoundRobin`: выбор между конфликтующими agent ids по deterministic
       cursor; cursor меняется только после принятого конфликта.

3. Добавить replay events M85.

   Файлы и anchors:

   - `crates/swarm-replay/src/event_log.rs:216` - добавить variants рядом с
     M74 Urban events.
   - `crates/swarm-replay/src/replay.rs` - summary/timeline counting/rendering.
   - `docs/REPLAY.md:34`-`50` и `docs/REPLAY.md:420`-`465` - описать события.

   Материализуемый результат:

   - Новые events:

     ```rust
     UrbanSegmentLockAcquired { agent_id, tick, edge_id, policy, reason }
     UrbanSegmentLockReleased { agent_id, tick, edge_id, held_ticks }
     UrbanSegmentConflict { tick, edge_id, holder_agent_id, requester_agent_id, policy, reason }
     UrbanDeconflictWait { agent_id, tick, edge_id, reason }
     UrbanDeconflictReplan { agent_id, tick, edge_id, edge_ids, route_length_m, reason }
     UrbanDeconflictAbort { agent_id, tick, edge_id, reason }
     ```

   - Serde tests аналогично существующим M74 tests
     `urban_wait_started_serde_roundtrip` и соседним tests в
     `crates/swarm-replay/src/event_log.rs:320`+.
   - Replay summary/timeline должен показывать эти события без ломки старых
     logs.

4. Выделить per-agent Urban runtime state.

   Файлы и anchors:

   - `crates/swarm-sim/src/runner/urban_helpers.rs:67`-`101` - заменить или
     расширить `UrbanAnalysisAgentState` до reusable `UrbanAgentRouteState`.
   - `crates/swarm-sim/src/runner/urban_patrol.rs:183`-`188` - перестать
     считать остальных агентов только analysis offsets при enabled M85.

   Материализуемый результат:

   - Для каждого alive agent хранить:
     - `agent_id`;
     - текущий route;
     - `segment_index`;
     - `distance_on_segment`;
     - `speed_m_per_tick`;
     - `completed`;
     - `aborted`;
     - `wait_start_tick`;
     - accumulated delay/wait metrics.
   - Для `deconfliction.enabled=false` оставить старую semantics: primary agent
     управляется как сейчас, остальные только analysis traces.
   - Для `enabled=true` все alive agents участвуют в route traversal и lock
     requests.

5. Встроить reserve-before-enter / release-after-complete в runner.

   Файлы и anchors:

   - `crates/swarm-sim/src/runner/urban_patrol.rs:306`-`573` - заменить
     single-agent movement loop на multi-agent tick loop при включённом M85.
   - `crates/swarm-sim/src/runner/urban_events.rs:38` - использовать
     `push_segment_entered` только после успешного lock acquire.
   - `crates/swarm-sim/src/runner/urban_patrol.rs:534`-`541` - release lock
     перед/после `UrbanSegmentCompleted`.

   Материализуемый результат:

   - На границе сегмента агент сначала вызывает registry:

     ```rust
     if state.distance_on_segment == 0.0 {
         match registry.request_lock(request, tick) {
             Acquired(lock) => push_lock_acquired(...),
             Conflict(decision) => apply_locked_segment_policy(...),
         }
     }
     ```

   - Агент не может emit `UrbanSegmentEntered` и не двигается по segment без
     active lock на `(edge_id, agent_id)`.
   - `release` происходит при `UrbanSegmentCompleted`; held time пишется в
     `UrbanSegmentLockReleased`.
   - В каждом tick invariant проверяет, что в `active_locks` нет двух holders
     на один `edge_id`. Нарушение превращать в typed unsupported/runtime failure
     и failing test, не замалчивать.

6. Реализовать wait/replan/abort на locked segment.

   Файлы и anchors:

   - `crates/swarm-sim/src/runner/urban_patrol.rs:385`-`510` - использовать
     существующий M74 pattern для policy action.
   - `crates/swarm-sim/src/runner/urban_patrol.rs:655`-`711` - обобщить
     `try_replan` так, чтобы он принимал `excluded_edges` из temporary
     obstacles + active locks.

   Материализуемый результат:

   - `locked_segment_policy=Wait`: loser не двигается, delay ticks
     увеличиваются, replay пишет `UrbanDeconflictWait`.
   - `locked_segment_policy=Replan`: runner строит route around locked edge;
     при успехе пишет `UrbanDeconflictReplan`, увеличивает replan metrics,
     сбрасывает segment progress для нового route.
   - `locked_segment_policy=Abort`: runner пишет `UrbanDeconflictAbort`,
     увеличивает abort metrics, агент больше не двигается.
   - Каждый wait/replan/abort содержит reason с edge id, holder/requester и
     right-of-way policy.

7. Добавить M85 metrics и протянуть их в reports.

   Файлы и anchors:

   - `crates/swarm-metrics/src/metrics/run.rs:171`-`179` - добавить
     `urban_deconflict_*` поля с `#[serde(default)]`.
   - `crates/swarm-metrics/src/metrics/aggregate.rs:150`-`193` и
     `:370`-`:545` - aggregate fields.
   - `crates/swarm-sim/src/runner/urban_metrics.rs:21`-`164` - передать
     новые значения из runner.
   - `crates/swarm-sim/src/report_export/json.rs:99`-`127`,
     `crates/swarm-sim/src/report_export/csv.rs:78`-`104`,
     `crates/swarm-sim/src/report_export/focused.rs:80`-`135`,
     `crates/swarm-sim/src/benchmark/markdown.rs` - экспортировать новые
     metrics.

   Материализуемый результат:

   - `RunMetrics`:
     - `urban_deconflict_conflict_count`;
     - `urban_deconflict_wait_ticks`;
     - `urban_deconflict_replan_count`;
     - `urban_deconflict_abort_count`;
     - `urban_segment_utilization`;
     - `urban_avg_delay_per_agent_ticks`.
   - `AggregateMetrics`:
     - соответствующие `avg_*` или rate fields.
   - Old JSON remains backward-compatible because all new fields have
     `serde(default)`.

8. Добавить deterministic scenarios/fixtures.

   Файлы и anchors:

   - `crates/swarm-scenarios/src/urban.rs:15`-`27` - добавить profiles:
     `deconflict-first-come`, `deconflict-priority`,
     `deconflict-round-robin`, `deconflict-replan`,
     `deconflict-abort`.
   - `crates/swarm-scenarios/src/urban.rs:227`-`250` - расширить
     multi-agent builder или добавить отдельный builder.
   - `scenarios/urban.multi-agent-deconflict.json` - canonical Scenario DSL
     fixture.

   Материализуемый результат:

   - Два агента стартуют так, чтобы запросить один и тот же edge в один
     deterministic tick.
   - Priority fixture задаёт разные `agent_priorities`.
   - RoundRobin fixture создаёт минимум два конфликта, чтобы проверить смену
     holder.
   - Replan fixture имеет alternate route; Abort fixture не имеет alternate
     route.

9. Добавить ownership/deconfliction artifacts в Urban analysis pack.

   Файлы и anchors:

   - `crates/swarm-sim/src/urban_analysis/events.rs` - считать M85 events.
   - Новый `crates/swarm-sim/src/urban_analysis/deconfliction.rs` или
     расширение существующего analysis модуля - построить ownership timeline.
   - `crates/swarm-examples/src/strategy_comparison_runtime/urban_artifacts.rs:18`-`36`
     и `:38`-`:108` - добавить artifact path в manifest и запись JSON/CSV.

   Материализуемый результат:

   - `urban_analysis/*.segment-ownership.json`;
   - `urban_analysis/*.segment-ownership.csv`;
   - `urban_analysis/manifest.json` содержит ссылки на новые файлы и counts.
   - Ownership artifact позволяет проверить invariant: для каждого
     `(edge_id, tick)` максимум один holder.

10. Добавить artifact-validator follow-up для route ownership records.

    Файлы и anchors:

    - `crates/swarm-examples/src/artifact_validator.rs:90`-`120` - расширить
      `ArtifactPackPaths` optional path для `urban_analysis/manifest.json`
      только если validator запускается на benchmark pack.
    - `crates/swarm-examples/tests/artifact_validator.rs` - negative fixture с
      duplicate holder для одного edge/tick.

    Материализуемый результат:

    - Новое правило, например
      `artifact.urban_deconfliction_duplicate_segment_owner`.
    - Validator не ломает существующие SITL supervisor/dry-run packs: проверка
      ownership artifacts включается только если `urban_analysis/manifest.json`
      существует.

11. Обновить docs и docs smoke tests.

    Файлы и anchors:

    - `README.md:799`-`806` и `README.md:838`-`840` - заменить старый
      "no multi-agent route deconfliction" на M85 boundary wording.
    - `docs/STATUS.md:138`-`157` и `docs/STATUS.md:196` - добавить M85 статус
      после реализации.
    - `docs/REPLAY.md:34`-`50` и `docs/REPLAY.md:420`-`465` - добавить M85
      replay event schema и примеры traces.
    - `docs/SCENARIO_DSL.md` - описать `urban_state.deconfliction`.
    - `docs/BENCHMARK_RESULTS.md` - объяснить новые metrics и что это не
      collision avoidance.
    - `docs/EXTENSION_GUIDE.md` - добавить extension guidance для new
      right-of-way policy.
    - `docs/OPERATIONAL_RUNBOOKS.md` - добавить runbook для M85 simulation
      artifact/validator flow.
    - `docs/ARTIFACT_VALIDATION.md` - описать new validator rule.
    - `crates/swarm-examples/tests/sitl_docs.rs` - добавить smoke test
      `m85_docs_describe_deconfliction_boundary`.

    Материализуемый результат:

    - Пользовательские docs честно говорят: M85 = mission-level segment
      ownership, не physical collision avoidance, не RF coordination, не
      hardware readiness.
    - Docs перечисляют concrete commands для проверки:

      ```bash
      PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban_deconfliction
      PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test benchmark_pack urban_deconfliction
      ```

12. Запустить проверки и не оставлять generated/proptest мусор.

    Материализуемый результат:

    - До commit:

      ```bash
      cargo fmt --all
      /home/formi/.local/bin/runlim cargo clippy --workspace --all-targets --all-features -- -D warnings
      PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-types urban
      PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay urban
      PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban_deconfliction
      PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-scenarios urban
      PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test benchmark_pack urban_deconfliction
      PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs m85
      PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test --workspace --all-targets
      ```

    - Если какой-то тест/прогон будет skipped, implementation outbox должен
      явно объяснить причину и риск.

# Testing strategy

## 1. Tests that need no refactoring - planned with main implementation

- `crates/swarm-sim/src/urban/deconfliction.rs` unit tests:
  - two agents requesting same segment produce exactly one holder;
  - first-come respects arrival order;
  - priority prefers higher `agent_priorities`;
  - priority tie-breaker deterministic by agent id;
  - round-robin rotates holder across repeated conflicts;
  - release after segment completion removes active lock;
  - duplicate active holder invariant cannot be constructed through public API.
- `crates/swarm-replay/src/event_log.rs` tests:
  - every new M85 replay event serde roundtrip;
  - legacy event logs without M85 fields still deserialize.
- `crates/swarm-sim/src/runner/tests.rs` tests:
  - two agents on overlapping route never simultaneously own same segment;
  - replay contains lock acquired/released/conflict/wait events;
  - replan policy emits `UrbanDeconflictReplan` and increments metrics;
  - abort policy emits `UrbanDeconflictAbort` and increments metrics;
  - single-agent scenario has zero M85 events and unchanged success/timing
    versus current `urban_patrol_completes_small_block_loop`.
- `crates/swarm-metrics/src/metrics/tests.rs`:
  - new run metrics default to zero for legacy JSON;
  - aggregate metrics average conflict/wait/replan/abort/utilization/delay.
- `crates/swarm-examples/tests/sitl_docs.rs`:
  - docs contain M85 boundary phrases and non-goals.

## 2. Tests that need light refactoring

- `crates/swarm-scenarios/src/urban.rs` scenario-builder tests:
  - canonical deconfliction profiles validate and produce expected conflicts.
- `crates/swarm-examples/tests/benchmark_pack.rs`:
  - scenario-suite run writes `urban_analysis/*.segment-ownership.json/csv`;
  - manifest contains ownership artifact paths and M85 event counts.
- `crates/swarm-examples/tests/artifact_validator.rs`:
  - benchmark pack with duplicate ownership record fails with
    `artifact.urban_deconfliction_duplicate_segment_owner`.
- Shared helper in `crates/swarm-sim/src/runner/tests.rs`:
  - `assert_no_duplicate_segment_ownership(log)` built from M85 lock events.

## 3. Tests that need heavy refactoring

- Property tests for N agents on random Urban topology:
  - generate route loops and speeds;
  - assert no simultaneous segment ownership;
  - assert all locks are eventually released or explained by abort/timeout.
- Stress test with 8 agents and all policy variants:
  - likely belongs in a later benchmark/degradation artifact, not basic M85
    implementation, because it can become flaky or slow.
- Temporal route reservation across command-plane execution:
  - should wait until M87/M89 command-plane/SITL evidence exists. M85 should
    expose enough event/artifact structure for that future test, but not block
    on command-plane execution now.

# Risks and tradeoffs

- **Runner complexity:** current Urban patrol path is intentionally simple and
  single-primary. True multi-agent route ownership adds per-agent state and
  can make `run_urban_patrol` too large. Keep registry and per-agent helpers in
  separate modules; do not hide all logic inside one function.
- **Sequential bias:** if agents are processed in vector order, `FirstCome`
  may accidentally mean "first in Vec". For simultaneous boundary requests,
  collect requests first, then resolve with deterministic policy.
- **Priority source ambiguity:** there is no existing agent-priority field.
  Use explicit `urban_state.deconfliction.agent_priorities` rather than
  overloading task priority or role.
- **Metrics naming conflict:** existing `urban_route_conflict_count` is an
  analysis/replay metric from M67, not M85 lock conflict count. Add explicit
  `urban_deconflict_conflict_count` to avoid semantic ambiguity.
- **Replan interaction with temporary obstacles:** locked edges and temporary
  blocked edges should be combined into one excluded set for route planning,
  but replay reasons must distinguish "blocked by obstacle" from "locked by
  agent".
- **Single-agent regression:** default `deconfliction.enabled=false` must keep
  existing Urban behavior, event streams, and metrics stable.
- **Artifact validator scope:** current validator is mostly SITL/dry-run
  oriented. Benchmark-pack ownership validation should be optional and
  activated only when `urban_analysis/manifest.json` exists.
- **Performance:** registry operations are small for current fixtures, but
  stress tests with many agents/segments should avoid O(agents * edges * ticks)
  scans where possible. Use edge-id keyed maps and per-tick request batches.

Что могло сломаться после реализации и как проверить:

- Urban single-agent timing/events could shift. Проверить
  `urban_patrol_completes_small_block_loop`,
  `urban_patrol_replay_records_ordered_route_events` и snapshot-like event
  assertions.
- Report schemas could miss new fields or break legacy JSON. Проверить
  metrics serde default tests, JSON/CSV/focused report tests and
  `cargo test --workspace --all-targets`.
- Replay summary/timeline could omit new events. Проверить
  `replay_cli_timeline_outputs_urban_events` plus new M85 timeline tests.
- Benchmark packs could write incomplete ownership artifacts. Проверить
  `benchmark_pack` tests and artifact validator negative duplicate-owner test.
- Deconfliction could be mistaken for safety/collision avoidance. Проверить
  docs smoke phrases in README/STATUS/REPLAY/BENCHMARK/OPERATIONAL docs.

# Open questions

- Should `MissionCriticalOverride` be accepted as a parsed enum with
  unsupported runtime error, or omitted until a concrete rule exists? The plan
  prefers adding it as an explicit future hook only if validation prevents
  silent use.
- Should M85 deconfliction apply only to `urban-patrol`, or also to
  `urban-search`? Minimal M85 should target `urban-patrol`; extending
  `urban-search` can follow once segment locks are stable.
- Should conflict resolution allow preemption of an already-held segment under
  `Priority`, or only decide between simultaneous requests before entry?
  Minimal M85 should avoid mid-segment preemption because that would imply
  physical/control semantics. Priority should choose among agents requesting a
  free/contended next segment before entry.
- Should `artifact_validator` grow a formal benchmark-pack mode now, or should
  ownership validation be a helper used by `benchmark_pack` tests first? The
  plan includes optional validation when `urban_analysis/manifest.json` exists,
  but executor may choose a smaller validator surface if the benchmark-pack
  schema would otherwise become too broad.
