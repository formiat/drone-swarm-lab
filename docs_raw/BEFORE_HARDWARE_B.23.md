# BEFORE_HARDWARE_B.23 — Итоговые майлстоуны до железа

Дата фиксации: 2026-06-01

Источник: синтез A.22, B.22 и C.22. Этот документ — финальный план, заменяющий
три предыдущих версии.

## Контекст и цель

Проект — исследовательский симулятор с доказанным PX4/SIH workflow (TRL 3–4).
Железа нет и в ближайшее время не будет. Цель этого плана:

```text
Поднять проект до hardware-integration candidate:
когда железо появится — интеграция начинается с контролируемой,
документированной и протестированной основы, а не с ad-hoc экспериментов.
```

Архитектурная граница остаётся прежней:

- PX4/autopilot owns: stabilization, attitude/rate control, motor physics,
  low-level waypoint execution, flight failsafes.
- This project owns: mission planning, route export, task allocation/reallocation,
  safety/invariant validation, replay, metrics, benchmark evidence, SITL workflows.

---

## Цепочка майлстоунов

```text
M70 Benchmark Credibility
  -> M71 Urban Route Export to SITL/PX4
    -> M72 Preflight Safety Contract
      -> M73 Artifact Validator and SITL Harness
        -> M74 Fault Injection Matrix
          -> M75 Algorithm Differentiation
            -> M76 Urban Blocked-Route Decision Logic
              -> M77 Synthetic Scenario Testbed
                -> M78 Benchmark and Degradation Evidence
                  -> M79 Operational Runbooks and Hardware Entry Gate
```

M70 делается первым: без читаемого benchmark нечего мерить после M75. M73 и M75
независимы и могут идти параллельно после M72.

---

## M70 — Benchmark Credibility

### Цель

Исправить известные проблемы, из-за которых текущий benchmark нечитаем:
SAR success ≈ 0 у всех стратегий, CBBA аномалии не объяснены, нет confidence
intervals. Внешний человек, увидев таблицы, решит что проект не работает.

### M70.1 — SAR success threshold

**Проблема:** `sar_success` в `runner.rs` использует `gs.all_targets_found()`,
что требует 100% обнаружения при `detection_probability < 1.0`. В текущих
профилях `success ≈ 0` у всех стратегий. `probability_of_detection` уже
вычисляется корректно в `RunMetrics` — нужно использовать её в предикате.

**Изменения:**

1. Добавить `sar_success_threshold: f64` в `RunConfig` рядом с
   `wildfire_success_threshold` (default: 1.0 для обратной совместимости).
2. В `compute_mission_success` заменить:
   ```rust
   let sar_success = gs.all_targets_found()
       && all_expected_failures_detected
       && max_task_unassigned_ticks <= max_unassigned_ticks_config;
   ```
   на:
   ```rust
   let pod = gs.targets_found as f64 / gs.targets.len().max(1) as f64;
   let sar_success = pod >= sar_success_threshold
       && all_expected_failures_detected
       && max_task_unassigned_ticks <= max_unassigned_ticks_config;
   ```
3. В SAR-профилях (`ideal`, `standard`) установить `sar_success_threshold: 0.8`.
4. Doc comment: SAR success = probability-of-detection metric, не "все цели найдены".

**Затронутые файлы:**
- `crates/swarm-sim/src/runner.rs` — `compute_mission_success`, `RunConfig`
- `crates/swarm-scenarios/src/sar_scenario.rs` — threshold в профилях

### M70.2 — Confidence intervals в экспортах

**Проблема:** `AggregateMetrics` содержит только mean. В JSON/CSV/Markdown нет
stddev, stderr, min, max. 1000-seed run без интервалов выглядит анекдотически.

**Изменения:**

1. Добавить в `AggregateMetrics`:
   ```rust
   pub stddev_success_rate: f64,
   pub stderr_success_rate: f64,
   pub min_success_rate: f64,
   pub max_success_rate: f64,
   ```
2. В `AggregateMetrics::from_runs` вычислять:
   ```rust
   let n = runs.len() as f64;
   let mean = total_success / n;
   let variance = runs.iter()
       .map(|r| (r.success as f64 - mean).powi(2))
       .sum::<f64>() / n;
   let stddev = variance.sqrt();
   let stderr = stddev / n.sqrt();
   ```
3. В Markdown export: `{mean:.3} ±{stderr:.3}` рядом с `success_rate`.
4. В JSON/CSV: поля `stddev_success_rate`, `stderr_success_rate`, `min_success_rate`,
   `max_success_rate`.

**Затронутые файлы:**
- `crates/swarm-metrics/src/metrics.rs`
- `crates/swarm-sim/src/report_export.rs`

### M70.3 — CBBA convergence диагностика

**Проблема:** 6 coverage профилей CBBA: `success=0.000, completion=1.000`.
Причина неизвестна. `CbbaBundleUpdated` не содержит `conflicting_tasks`.

**Изменения:**

1. Добавить в `CbbaBundleUpdated` опциональное поле
   `conflicting_task_count: u64` — количество задач с конфликтом в этом тике.
   Вычислять в `cbba.rs` при merge bids.
2. Прогнать `replay --timeline --category generic` на heavy-loss профиле.
   Проверить гипотезу: CBBA не переконвергирует за `gossip_interval_ticks`
   после `AgentFailed` event.
3. Если гипотеза подтверждается: при `AgentFailed` в coordinator — один
   внеплановый gossip round для CBBA.
4. Если нет: задокументировать как inherent limitation в support matrix
   с replay evidence.

**Затронутые файлы:**
- `crates/swarm-replay/src/event_log.rs` — поле в `CbbaBundleUpdated`
- `crates/swarm-alloc/src/cbba.rs` — подсчёт `conflicting_task_count`
- `crates/swarm-runtime/src/coordinator.rs` — gossip burst при agent failure

### Done criteria M70

- SAR строки показывают non-zero success при `threshold=0.8`.
- JSON/CSV/Markdown содержат `stddev_success_rate`, `stderr_success_rate`.
- Причина `success=0` у CBBA heavy-loss профилей задокументирована или исправлена.
- `cargo test --workspace` green.

### Тесты M70

#### Без рефакторинга

- `sar_threshold_0_8_succeeds_with_partial_detection`: при 80% найденных и
  `threshold=0.8` → `success=true`.
- `sar_threshold_1_0_requires_all_found`: при `threshold=1.0` и 80% →
  `success=false` (regression guard).
- `aggregate_stderr_is_zero_for_uniform_runs`: все runs с одним результатом
  → `stderr=0.0`.
- `aggregate_stderr_matches_formula`: 4 runs с known values → `stderr = stddev /
  sqrt(4)` численно.
- `report_export_markdown_contains_stderr`: Markdown вывод содержит `±`.
- `cbba_bundle_updated_has_conflict_count`: при конфликте двух агентов →
  `conflicting_task_count >= 1`.

#### Лёгкий рефакторинг

- Переписать `support_matrix_sar_greedy` с явным threshold.
- CBBA fixture с explicit agent failure mid-run для проверки gossip burst.

#### Тяжёлый рефакторинг

- Property tests над `AggregateMetrics::from_runs` с generated runs.
- Cross-run statistical consistency suite.

---

## M71 — Urban Route Export to SITL/PX4

### Цель

Соединить существующий Urban simulation layer с PX4/SIH waypoint workflow.
Добавить configurable `geo_origin`, чтобы убрать hardcode Цюриха из кода.

```text
Urban planned route -> ordered waypoint mission -> dry-run/SITL-compatible plan
```

Это не hardware execution. Это детерминированная конвертация и validation path,
которая позже используется для SITL или hardware-adjacent экспериментов.

### Scope

1. Route conversion:
   - конвертировать `UrbanPlannedRoute` segments в ordered waypoint items;
   - сохранять stable task/segment ids между runs;
   - явно сохранять altitude assumptions;
   - определить waypoint spacing rule для длинных Urban segments.

2. Configurable `geo_origin` (prerequisite для корректных координат):
   ```rust
   pub struct GeoOrigin {
       pub lat_deg: f64,
       pub lon_deg: f64,
       pub alt_m: f64,
   }
   // в Scenario:
   pub geo_origin: Option<GeoOrigin>,
   ```
   - При наличии `scenario.geo_origin` передавать в `MissionUploadOptions.home_origin`
     вместо hardcode.
   - Добавить явный `geo_origin` в `scenarios/sitl.px4-golden.json` и
     `scenarios/sitl.multi-agent.json` (поведение не меняется, поле становится видимым).

3. Export metadata artifact:
   - source scenario path;
   - planner name;
   - route length;
   - waypoint count;
   - altitude;
   - safety validation result (passed/failed + reasons);
   - git commit и command identity где практично.

4. Dry-run integration:
   - produce `sitl_supervisor`/`sitl_agent` compatible waypoint plan;
   - run existing dry-run path без PX4;
   - write output в explicit `--output-dir`;
   - include manifest, run id, config snapshot.

5. Docs scope:
   - local SITL/PX4-compatible export only;
   - no hardware;
   - no real obstacle avoidance;
   - no real perception.

### Non-goals

- No hardware run.
- No Gazebo/HIL.
- No new PX4 protocol work if existing waypoint path suffices.
- No arbitrary polygon/navmesh.

### Done criteria M71

- Urban patrol route exports в детерминированный waypoint список.
- Exported waypoints проходят dry-run validation.
- Dry-run с `geo_origin { lat=55.75, lon=37.62 }` выводит корректные глобальные
  координаты без PX4.
- `scenarios/sitl.px4-golden.json` содержит явный `geo_origin`.
- Export artifact включает route/source metadata.

### Тесты M71

#### Без рефакторинга

- `urban_route_exports_ordered_waypoints`: простой square route → ordered waypoint items.
- `urban_route_export_stable_ids`: repeated export → identical task/segment ids.
- `urban_route_altitude_explicit`: altitude preserved or defaulted deterministically.
- `geo_origin_overrides_default_in_dry_run`: waypoint lat/lon совпадают с
  origin + local offset, не с hardcode Цюрихом.
- `geo_origin_absent_uses_sitl_default`: без поля — текущее поведение.
- `geo_origin_roundtrip_json`: сериализация/десериализация без потерь.
- Dry-run smoke с committed small Urban route fixture.

#### Лёгкий рефакторинг

- Shared route-to-waypoint helper fixture.
- Export metadata assertion helper.
- Safety-validation wrapper для exported waypoints.

#### Тяжёлый рефакторинг

- Manual/ignored local PX4/SIH upload для exported Urban route.
- Cross-run export artifact comparison.
- Route densification property tests.

---

## M72 — Preflight Safety Contract

### Цель

Сделать так чтобы unsafe mission inputs падали до execution. Это важнее для
будущего hardware, чем добавление новых mission features. BH2 превращает
разрозненные safety validation pieces в явный preflight contract.

Это не flight certification. Это детерминированные gates, которые останавливают
known-bad missions до SITL или hardware bench tests.

### Scope

1. Mission-level safety checks:
   - geofence bounds;
   - no-fly zone intersection;
   - max altitude;
   - min altitude где релевантно;
   - max route length;
   - max estimated mission duration;
   - minimum battery reserve estimate;
   - duplicate task ownership;
   - missing waypoint/task ids;
   - invalid or non-finite coordinates.

2. Urban-specific safety checks:
   - route uses known graph edges;
   - route avoids blocked edges;
   - route avoids static AABB obstacles;
   - exported waypoint route stays inside declared Urban assumptions;
   - route planner и export metadata agree.

3. Ownership invariants:
   - no duplicate task ownership;
   - released tasks reassigned or explicitly abandoned;
   - unsupported strategy/mission pairs не могут silently claim success.

4. `SafetyValidationReport` schema:
   ```rust
   pub struct SafetyViolation {
       pub rule_id: String,
       pub severity: ViolationSeverity,
       pub affected_id: Option<String>,
       pub reason: String,
   }
   pub enum ViolationSeverity { Fatal, Warning }
   pub struct SafetyValidationReport {
       pub passed: bool,
       pub violations: Vec<SafetyViolation>,
   }
   ```

5. CLI behavior:
   - unsafe mission → non-zero exit;
   - error message называет failed rule ids;
   - output artifact записывает safety result при `--output-dir`.
   - стабильные exit codes: validation=2, runtime=3, artifact=4, env=5.

6. Docs:
   - список каждого preflight rule и его ограничения;
   - явно отделить simulation-level invariants от real flight safety.

### Non-goals

- No certified safety.
- No real obstacle avoidance.
- No hardware failsafe.
- No regulatory claim.

### Done criteria M72

- Safety failures структурированы и assertable в тестах.
- Exported Urban routes используют тот же safety contract.
- Unsafe dry-run fixtures падают детерминированно.
- Docs перечисляют каждое правило и его ограничение.

### Тесты M72

#### Без рефакторинга

- `safety_geofence_violation_fails`: waypoint за geofence → `violations` содержит
  `rule_id="geofence_bounds"`.
- `safety_no_fly_aabb_violation`: route через AABB obstacle → fatal violation.
- `safety_duplicate_ownership_rejected`: две задачи с одним agent → fatal.
- `safety_non_finite_coordinate_rejected`: `NaN` в waypoint → fatal.
- `safety_blocked_edge_urban_fails`: exported Urban route через blocked edge → fatal.
- `safety_unsupported_pair_cannot_succeed`: unsupported strategy/mission pair →
  fatal before execution.

#### Лёгкий рефакторинг

- Shared `SafetyValidationReport` assertion helper.
- Small fixture builder для valid/invalid route plans.
- CLI output helper для rule-id assertions.

#### Тяжёлый рефакторинг

- Property tests для generated waypoints vs geofence/no-fly rules.
- Cross-mission preflight compatibility suite.
- Battery reserve estimator tests с mission-duration model.

---

## M73 — Artifact Validator and SITL Harness

### Цель

Сделать run artifacts machine-checkable. Будущая hardware работа будет зависеть
от artifacts больше чем от informal console output. M73 добавляет evidence contract
для simulation, SITL dry-run и local PX4/SIH artifacts.

### Scope

1. Validator inputs:
   - manifest (command/git commit/build profile/run id);
   - run report;
   - event log;
   - replay summary;
   - safety validation result;
   - benchmark/result table где релевантно.

2. Validator checks:
   - manifest: command, git commit, build profile, run id — все присутствуют;
   - run id и output dir consistent;
   - event log final status совпадает с run report final status;
   - completed tasks в report существуют в event log;
   - replacement mission completion seq использует active mission seq;
   - replay summary counts consistent с event log;
   - no accidental overwrite без `--force`;
   - limitations section существует для SITL/PX4 artifacts.

3. CLI/tooling:
   - `validate-artifact --dir <path>` (или library function);
   - readable error list с rule ids;
   - exit code 0 = valid, non-zero = invalid;
   - portable тесты с committed tiny fixtures или inline temp fixtures.

4. Local harness scripts (из B.22 B3.2):
   - `scripts/run_m58_local.sh`:
     - запускает два PX4 SIH instance (0 и 1) в background с known PIDs;
     - ждёт MAVLink ports 14550/14560 (`nc -z localhost PORT`);
     - запускает `sitl_supervisor` с `--output-dir`;
     - `trap cleanup EXIT` — убивает PX4 процессы по завершению;
     - артефакт в `results/local_m58_YYYY-MM-DD/`.
   - `scripts/run_m59_local.sh`:
     - то же плюс kill первого PX4 через N секунд для agent failure.
   - Секция "Локальное воспроизведение M58/M59" в `docs/SITL_SETUP.md`.

5. Documentation:
   - определить что считается acceptable evidence;
   - различать simulation, dry-run, local PX4/SIH и hardware evidence.

### Non-goals

- No remote artifact store.
- No CI dependency on local PX4.
- No hardware artifact claim.

### Done criteria M73

- Committed small valid artifact fixture проходит validator.
- Deliberately inconsistent fixture падает с clear rule ids.
- M58/M59-style reports покрыты validator logic.
- Developer воспроизводит M58/M59 workflow из скриптов.
- Docs описывают evidence contract.

### Тесты M73

#### Без рефакторинга

- `validator_valid_tiny_artifact_passes`: minimal valid fixture → exit 0.
- `validator_missing_manifest_field_fails`: manifest без `git_commit` → rule id.
- `validator_final_status_mismatch_fails`: event log "success", report "failure" →
  violation.
- `validator_task_not_in_event_log_fails`: completed task без event → violation.
- `validator_replay_count_mismatch_fails`: summary counts не совпадают с event log.

#### Лёгкий рефакторинг

- Shared artifact fixture builder.
- Validator rule-id assertion helper.
- Event-log/report consistency helper.

#### Тяжёлый рефакторинг

- Validator над full committed M58/M59 artifacts.
- Multi-artifact pack validator для benchmark directories.
- Schema-version compatibility matrix.

---

## M74 — Fault Injection Matrix

### Цель

Систематически тестировать failure paths до появления hardware. Успешные golden
paths недостаточны. M74 превращает failure handling в матрицу known scenarios и
expected outcomes.

```text
detect -> classify -> decide -> recover/abort -> report
```

### Scope

1. Failure modes:
   - agent lost before upload;
   - upload rejected;
   - agent lost after upload before start;
   - no-progress timeout;
   - heartbeat lost;
   - partial completion then failure;
   - replacement mission rejected;
   - survivor fails after replacement;
   - stale telemetry;
   - bad waypoint/mission item;
   - duplicate ownership mid-run;
   - unsupported strategy selected.

2. Supervisor decisions для каждого mode:
   - abort;
   - wait;
   - reassign unfinished tasks;
   - mark partial success;
   - mark total failure;
   - continue with survivor;
   - refuse unsafe replacement.

3. Report fields для degraded runs:
   - `failure_mode: String`;
   - `detected_at_tick: u64`;
   - `affected_agent_id`;
   - `tasks_completed_before_failure: Vec<TaskId>`;
   - `tasks_recovered: Vec<TaskId>`;
   - `tasks_abandoned: Vec<TaskId>`;
   - `replacement_mission_id: Option<MissionId>`;
   - `final_status`.

4. Replay events:
   - `FailureDetected { agent_id, tick, failure_mode }`;
   - `FailureClassified { agent_id, tick, classification }`;
   - `RecoveryStarted { tick, tasks }`;
   - `RecoveryCompleted { tick, recovered, abandoned }`;
   - `RecoveryFailed { tick, reason }`.

5. Metrics:
   - `failure_detected_count`;
   - `tasks_released_count`;
   - `tasks_reassigned_count`;
   - `recovery_latency_ticks: Option<u64>`;
   - `survivor_completion_rate`;
   - `unrecovered_task_count`.

6. Local SITL/manual (где практично):
   - один или два representative failure paths;
   - артефакты валидируются M73 validator.

7. Documentation:
   - failure matrix table: failure mode / behavior / supported / status;
   - явные unsupported paths;
   - exact recovery semantics.

### Non-goals

- No hardware failure testing.
- No physical failsafe validation.
- No RF/link-loss modeling beyond deterministic simulation profiles.

### Done criteria M74

- Каждый supported failure mode имеет fake-controller тест.
- Supervisor final status детерминирован и объясним.
- Recovered task ownership valid.
- Failure matrix существует в docs.
- Artifact validator может верифицировать degraded-mode runs.

### Тесты M74

#### Без рефакторинга

- `fake_upload_rejection_handled`: upload rejected → supervisor marks failure,
  tasks released.
- `fake_no_progress_timeout`: агент не продвигается N тиков → supervisor
  reassigns tasks.
- `fake_partial_completion_then_disconnect`: выполнено 50% tasks, затем потеря →
  оставшиеся reassigned survivor.
- `fake_replacement_mission_rejected`: replacement upload rejected → partial success
  с сохранёнными completed tasks.
- `fake_survivor_completes_recovered_tasks`: survivor принимает recovered tasks →
  mission completion.
- `failure_metrics_aggregation`: correct counts после multi-failure scenario.

#### Лёгкий рефакторинг

- Fake controller scenario builder (configurable failure tick, type).
- Failure-mode assertion helper.
- Shared final-status validation helper.

#### Тяжёлый рефакторинг

- Manual/ignored local PX4/SIH fault-injection harness.
- Repeated failure property tests.
- Long-running supervisor soak с synthetic failures.

---

## M75 — Algorithm Differentiation

### Цель

Сделать так чтобы стратегии давали измеримо разные результаты в подходящих
условиях. Сейчас greedy ≈ auction ≈ CBBA — непонятно зачем использовать
что-то сложнее. M75 должен быть после M70, чтобы benchmark был читаемым до
снятия delta.

### M75.1 — Communication-aware allocation scoring

**Проблема:** `AllocationAgent.comms_range` хранится, но ни один allocator не
использует его в scoring. В тестах `comms_range = f64::INFINITY`.

**Изменения:**

1. Добавить `comms_penalty_weight: f64` в `RunConfig` (default 0.0 — off).
2. В greedy и auction scoring:
   ```rust
   let dist = agent.pose.distance_to(&task_pose);
   let comms_penalty = if dist > agent.comms_range {
       comms_penalty_weight * (dist - agent.comms_range)
   } else {
       0.0
   };
   let score = base_score - comms_penalty;
   ```
3. Benchmark delta: heavy-loss-* и partition-prone-* с/без
   `comms_penalty_weight=1.0`. Ожидание: connectivity-aware получает advantage.
4. Добавить `comms_penalty_weight` в manifest/report.

**Затронутые файлы:**
- `crates/swarm-alloc/src/allocator.rs` — `AuctionAllocator.cost()`,
  `GreedyAllocator.allocate()`
- `crates/swarm-sim/src/runner.rs` — `RunConfig`

### M75.2 — Wildfire priority-triggered reallocation

**Проблема:** `push_wildfire_priority_update` эмитирует `TaskPriorityUpdated`,
но не тригерит reallocation. Агент продолжает лететь к `priority=2` задаче
даже если другая зона стала `priority=9`.

**Изменения:**

1. Добавить `wildfire_priority_realloc_threshold: u8` в `RunConfig` (default 8).
2. При wildfire priority update: если `new_priority >= threshold` → добавить
   `task.id` в `force_realloc_queue: HashSet<TaskId>`.
3. В начале следующего тика coordinator: освободить assignment агента к этой
   задаче, вернуть задачу в `Unassigned`, очистить очередь.
4. Benchmark delta: medium-dynamic wildfire с/без priority realloc.

**Затронутые файлы:**
- `crates/swarm-sim/src/runner.rs` — wildfire tick loop, `RunConfig`
- `crates/swarm-runtime/src/coordinator.rs` — force realloc mechanism

### M75.3 — SAR belief-entropy ordering

**Проблема:** `sar_task_priority` статичен. Агент не учитывает что клетка уже
частично обследована.

**Изменения:**

1. Добавить `dynamic_belief_updates: bool` в `SarRunConfig` (default false).
2. При `dynamic_belief_updates=true`: после каждого scan event пересчитать
   posterior belief. Ранжировать по убыванию remaining uncertainty:
   ```rust
   let remaining_uncertainty = prior * (1.0 - detection_prob);
   task.priority = (remaining_uncertainty * 10.0) as u8;
   ```
3. Обновлять `task.priority` в `task_registry` — coordinator учтёт при
   следующем allocation round.

**Затронутые файлы:**
- `crates/swarm-scenarios/src/sar_scenario.rs` — `SarRunConfig`
- `crates/swarm-sim/src/runner.rs` — SAR scan tick loop

### M75.4 — Benchmark delta

После M75.1–M75.3 прогнать targeted benchmark:
- coverage heavy-loss с/без `comms_penalty_weight`.
- wildfire medium-dynamic с/без priority realloc.
- SAR с/без dynamic belief.

Зафиксировать delta в `docs/BENCHMARK_RESULTS.md`: где сложные алгоритмы
выигрывают и почему.

### Done criteria M75

- `comms_penalty_reduces_score_beyond_range`: score ниже при `dist > comms_range`.
- `wildfire_priority_reallocates_agent_above_threshold`: детерминированный тест.
- `sar_dynamic_belief_changes_task_order`: при `dynamic=true` порядок меняется.
- Benchmark delta committed с интерпретацией.
- `cargo test --workspace` green.

### Тесты M75

#### Без рефакторинга

- `comms_penalty_reduces_score_beyond_range`: агент `comms_range=10`, задача
  на dist=20 → score ниже чем при `comms_range=∞`.
- `comms_penalty_zero_no_effect`: при `weight=0.0` поведение идентично текущему.
- `comms_penalty_infinite_range_no_effect`: `comms_range=∞` → penalty=0.
- `wildfire_priority_reallocates_above_threshold`: `priority >= threshold` →
  агент переходит к высокоприоритетной задаче.
- `wildfire_priority_below_threshold_no_realloc`: `priority < threshold` →
  реаллокации нет.
- `sar_dynamic_belief_changes_task_order`: при `dynamic=true` порядок назначений
  отличается от статичного.
- `sar_static_belief_unchanged_with_flag_false`: при `dynamic=false` — текущее
  поведение.

#### Лёгкий рефакторинг

- Shared scoring delta assertion helper.
- Benchmark profile builder с explicit comms/priority/belief settings.

#### Тяжёлый рефакторинг

- Property tests над allocation scoring consistency.
- Multi-agent comms partition scenario.
- Wildfire repeated-threshold sweep.

---

## M76 — Urban Blocked-Route Decision Logic

### Цель

Добавить mission-level reactivity без претензий на real obstacle avoidance.
M76 расширяет Urban от static route following до детерминированных route decisions:

```text
edge becomes blocked -> detector/policy notices -> wait or replan -> judge/report
```

### Scope

1. Dynamic blocked route state:
   ```rust
   pub struct UrbanTemporaryObstacle {
       pub edge_id: UrbanEdgeId,
       pub appears_at_tick: u64,
       pub disappears_at_tick: Option<u64>,
       pub reason: String,
       pub severity: ObstacleSeverity,
   }
   pub enum ObstacleSeverity { Hard, Soft }
   // в RunConfig/UrbanState:
   pub temporary_obstacles: Vec<UrbanTemporaryObstacle>,
   ```

2. Effective blocked set per tick:
   ```rust
   let blocked_edges: HashSet<UrbanEdgeId> = map.edges.iter()
       .filter(|e| e.blocked)
       .map(|e| &e.id)
       .chain(urban_state.temporary_obstacles.iter()
           .filter(|o| o.is_active(tick))
           .map(|o| &o.edge_id))
       .cloned()
       .collect();
   ```

3. Mock obstacle detector:
   - graph lookahead N hops (configurable);
   - детерминированный результат;
   - no real lidar/raycast;
   - optional detection range in graph distance.

4. Policies:
   - `Wait`: стоять пока edge не unblock;
   - `Replan`: искать alternate route вокруг blocked edge;
   - `Abort`: нет альтернативы → fail safely;
   - `Yield`: другой агент имеет priority на этот edge.

5. Metrics:
   - `urban_replan_count: u64`;
   - `urban_wait_time_ticks: u64`;
   - `urban_blocked_edge_count: u64`;
   - `urban_replan_success_rate: f64`;
   - `urban_unresolved_blockage_count: u64`.

6. Replay events:
   - `UrbanEdgeBlocked { edge_id, tick }`;
   - `UrbanEdgeUnblocked { edge_id, tick }`;
   - `UrbanPolicyDecision { agent_id, tick, policy, blocked_edge_id }`;
   - `UrbanRoutePlanned { agent_id, tick, waypoint_count }`;
   - `UrbanWaitStarted { agent_id, tick, edge_id }`;
   - `UrbanWaitCompleted { agent_id, tick }`;
   - `UrbanAbortReason { agent_id, tick, reason }`.

### Non-goals

- No certified obstacle avoidance.
- No real sensor stream.
- No physics.
- No arbitrary polygon geometry beyond small helpers.

### Done criteria M76

- Один детерминированный blocked-edge сценарий recovers через wait или replan.
- Один no-route сценарий fails safely с explicit reason.
- Replay объясняет decision.
- Metrics разделяют route following от replan/wait behavior.

### Тесты M76

#### Без рефакторинга

- `temporary_obstacle_active_within_window`: `is_active(tick)` = true между
  appears и disappears.
- `temporary_obstacle_no_disappears_stays_forever`: `disappears_at_tick=None`
  → active до конца run.
- `runner_emits_edge_blocked_event`: при появлении obstacle → `UrbanEdgeBlocked`
  в replay.
- `runner_emits_edge_unblocked_event`: при disappears → `UrbanEdgeUnblocked`.
- `wait_policy_completes_after_unblock`: агент ждёт, edge открывается → продолжает.
- `replan_policy_finds_alternate_route`: blocked edge → новый маршрут через
  alternate edges.
- `abort_policy_on_no_route`: нет альтернативы → `UrbanAbortReason` в replay.
- `replay_contains_policy_decision_event`: decision эмитируется в replay.

#### Лёгкий рефакторинг

- Blocked-edge scenario builder.
- Route policy assertion helper.
- Urban replay event fixture helper.

#### Тяжёлый рефакторинг

- Multi-agent yield policy тесты.
- Dynamic obstacle schedule property tests.
- Generated map stress tests.

---

## M77 — Synthetic Scenario Testbed

### Цель

Заменить ad-hoc hand-picked сценарии воспроизводимыми scenario families.
Без hardware, реалистичное давление приходит от детерминированной вариации:
maps, blocked edges, bus schedules, packet loss, failures, obstacle density.

### Scope

1. Seeded Urban generator:
   - `seed: u64`;
   - grid/block road graph (rows, cols, corridor width);
   - static obstacle density;
   - blocked edge schedule;
   - bus placement или route.

2. Failure generator:
   - agent failure tick;
   - failure type;
   - partial completion amount;
   - replacement acceptance/rejection.

3. Communication generator:
   - packet loss profile;
   - latency distribution;
   - partition events;
   - agent count sweep.

4. Scenario manifest:
   ```rust
   pub struct GeneratedScenarioManifest {
       pub generator_name: String,
       pub seed: u64,
       pub parameters: serde_json::Value,
       pub schema_version: String,
       pub git_commit: Option<String>,
       pub generated_at: DateTime<Utc>,
   }
   ```

5. Категории:
   - `tiny` — для unit тестов, < 10 nodes;
   - `small` — быстрый regression, < 50 nodes;
   - `medium` — default CI smoke;
   - `stress` — большие runs, не default CI;
   - `regression-stable` — pinned seed, нельзя менять без migration;
   - `experimental` — seeded но не regression-stable.

6. Test usage:
   - tiny/small generated fixtures в unit тестах;
   - нет зависимости от local absolute paths;
   - generated data остаётся маленьким в CI.

### Non-goals

- No large random tests в default CI.
- No opaque random failures.
- No generated scenario без reproducible seed/manifest.

### Done criteria M77

- Same seed → identical scenario на repeated runs.
- Generated scenario проходит DSL validation.
- Хотя бы один generated Urban blocked-edge fixture питает M76 тесты.
- Generator parameters записаны в manifest.

### Тесты M77

#### Без рефакторинга

- `same_seed_same_scenario`: identical scenario при repeated calls.
- `different_seed_different_scenario`: хотя бы одно поле меняется.
- `generated_urban_map_validates`: passes DSL validator.
- `generated_blocked_edge_schedule_validates`: временные препятствия valid.
- `invalid_generator_config_rejected`: rows=0 или cols=0 → structured error.

#### Лёгкий рефакторинг

- Scenario generator trait/interface.
- Manifest assertion helper.
- Small generated-fixture snapshot test.

#### Тяжёлый рефакторинг

- Property tests над many generated maps.
- Cross-mission generated scenario framework.
- Long-run generated degradation suite.

---

## M78 — Benchmark and Degradation Evidence

### Цель

Превратить "оно прошло сценарий" в "мы знаем где работает, где деградирует,
где не поддерживается". M78 должен идти после хотя бы одного нового behavior
из M75–M77 (есть что мерить).

### Scope

1. Statistical layer в `AggregateMetrics` (extends M70.2):
   - mean, stddev, stderr, 95% CI, min, max;
   - failure rate;
   - N runs.

2. Degradation curves — sweeps по параметрам:
   - packet loss: 0%, 10%, 30%, 50%;
   - latency: 0, 50ms, 200ms;
   - agent count: 1, 2, 4, 8;
   - route length / task density;
   - obstacle density (Urban);
   - blocked-edge frequency (Urban);
   - bus detection probability (Urban);
   - failure count (M74 profiles).

3. Support matrix integration:
   - `supported` — стабильные результаты, тесты green;
   - `experimental` — работает, но есть known gaps;
   - `unsupported` — известные failure modes;
   - `supported_with_caveats` — работает только при условиях;
   - `not_evaluated` — нет evidence.

4. Urban benchmark decision (явно выбрать одно):
   - добавить Urban в `--mission all`; или
   - создать `--mission urban`; или
   - держать Urban как отдельную scenario-suite evidence с documented reason.

5. Current vs historical artifacts:
   - classify artifact по code commit и schema version;
   - docs не представляют stale packs как current evidence;
   - benchmark result README объясняет scope и дату.

6. Interpretation docs в `docs/BENCHMARK_RESULTS.md`:
   - SAR success semantics (PoD vs all-found);
   - wildfire success vs completion;
   - CBBA weak rows — причина из M70.3;
   - algorithm differentiation delta из M75;
   - Urban route-risk/replan tradeoffs из M76.

### Non-goals

- No publication paper.
- No 1000-seed rerun если existing evidence достаточно.
- No unsupported pair как success claim.
- No hardware evidence claim.

### Done criteria M78

- Хотя бы один degradation sweep артефакт существует.
- Reports включают statistical fields для key metrics.
- Unsupported rows явно помечены в support matrix.
- Docs различают simulation, SITL и future hardware evidence.
- Urban benchmark scope явный.

### Тесты M78

#### Без рефакторинга

- `confidence_interval_helper_correct`: known data → correct 95% CI boundaries.
- `report_export_includes_statistical_fields`: JSON содержит `stddev_success_rate`,
  `stderr_success_rate`, `min_success_rate`, `max_success_rate`.
- `unsupported_pair_not_in_success_claims`: unsupported strategy/mission →
  marked в support matrix, не in results as success.
- `manifest_records_seed_range_and_profile`: degradation suite manifest полный.

#### Лёгкий рефакторинг

- Benchmark pack validator helper.
- Degradation suite runner helper.
- Summary table consistency assertions.

#### Тяжёлый рефакторинг

- Statistical delta report validation.
- Multi-pack comparison tooling.
- Long-run reproducibility harness.

---

## M79 — Operational Runbooks and Hardware Entry Gate

### Цель

Подготовить human и procedural сторону будущего hardware experiment. Готовность
к hardware — не только код. Go/no-go gates и runbooks определяют когда
интеграция допустима.

### Scope

1. Runbooks (в `docs/`):
   - simulation runbook: команды, expected artifacts, stop/abort;
   - SITL dry-run runbook: команды, validation steps;
   - local PX4/SIH runbook: M58/M59 scripts, preconditions, cleanup;
   - hardware candidate runbook: preflight, observation, post-run, abort.

2. Operational checklist (pre-run):
   - mission file validated (M72 safety contract passed);
   - artifact output dir unique (нет overwrite без `--force`);
   - manual override assumption recorded;
   - geofence/no-fly assumptions recorded;
   - expected failure behavior recorded;
   - geo_origin matches intended location.

3. Go/no-go gates (явные, не "best effort"):
   - **no hardware** if simulation fails;
   - **no hardware** if dry-run fails;
   - **no hardware** if artifact validator fails;
   - **no hardware** if mission has unclassified safety violations;
   - **no hardware** without external safety process (separate from this project);
   - **no multi-drone hardware** without separate safety review after single-drone.

4. Post-run inspection checklist:
   - validate artifacts (M73 validator);
   - inspect replay timeline;
   - compare run report и event log;
   - record known limitations;
   - решить допускается ли rerun.

5. Error handling:
   - structured CLI errors (M72);
   - stable exit codes: 0=ok, 2=validation, 3=runtime, 4=artifact, 5=env;
   - actionable messages для missing PX4, bad scenario, unsafe mission,
     artifact mismatch.

6. Documentation `docs/HARDWARE_READINESS.md`:
   - что готово к simulation;
   - что готово к local SITL;
   - что **не** готово к hardware;
   - что должно произойти когда hardware появится.

   Явно добавить фразы:
   - "first hardware experiment is still not product readiness";
   - "multi-agent hardware requires separate safety review";
   - "no regulatory or certified safety claim".

### Non-goals

- No real hardware checklist pretending to be complete without hardware.
- No legal/regulatory certification.
- No public product-readiness claim.
- No semver commitment unless API branch explicitly chosen.

### Done criteria M79

- Новый developer может запустить simulation и SITL dry-run из docs.
- Go/no-go gates явны и machine-checkable где возможно.
- Error messages actionable.
- `docs/HARDWARE_READINESS.md` содержит обязательные boundary phrases.
- Hardware boundary остаётся консервативной.

### Тесты M79

#### Без рефакторинга

- `docs_smoke_hardware_readiness_not_product`: файл содержит "not product readiness".
- `docs_smoke_multi_agent_safety_review`: файл содержит "separate safety review".
- `docs_smoke_no_regulatory_claim`: файл содержит "no regulatory" или "no certified".
- `cli_error_missing_scenario_file`: отсутствующий сценарий → exit=5, actionable msg.
- `cli_error_unsafe_mission`: safety violation → exit=2, named rule ids.
- `schema_compatibility_smoke_existing_fixtures`: existing fixtures parse без ошибок.

#### Лёгкий рефакторинг

- Shared docs phrase assertion helper.
- CLI error assertion helper.
- Runbook command fixture validation.

#### Тяжёлый рефакторинг

- End-to-end scripted dry-run following the runbook.
- Artifact validator integration над runbook-generated output.
- Manual/ignored local PX4/SIH rehearsal.

---

## Итоговый уровень проекта

После M70–M79 без железа проект будет:

```text
hardware-integration candidate / hardware-ready research platform
```

Это всё ещё не:

- production drone system;
- certified safety stack;
- real perception system;
- hardware-proven swarm controller;
- ready for uncontrolled field use.

Когда железо появится, следующий этап начинается отдельным планом:

```text
bench without propellers
  -> MAVLink connectivity verification
    -> mission upload only (no execute)
      -> telemetry mapping
        -> abort/failsafe validation
          -> single-drone constrained flight
            -> multi-drone only after separate safety review
```

Ценность pre-hardware milestones в том, что этот этап начинается с
контролируемой, evidence-backed основы вместо improvised scripts и unclear claims.

---

## Что не делать в этом плане

- **UI/visualizer** — не приближает к железу. Отдельная задача.
- **Hierarchical coordination (8+ агентов)** — нет benchmark evidence о необходимости.
- **Polygon geometry / lidar raycast** — риск превратиться в geometry engine.
- **Published API / semver** — пока нет внешних пользователей.
- **Logistics/Delivery mission** — интересна, но не "боевая" без working
  precedence constraints в allocator.
- **1000-seed rerun без нового behavior** — делать после M75, когда алгоритмы
  дифференцированы.
- **Hardware-specific code paths** — beyond existing boundary guards.
- **Real HIL, lidar/CV/SLAM** — вне scope этого проекта до появления железа.
