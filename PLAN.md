# M35 — Dynamic Mission Correctness

## Context

M34 закрыт. Planner Correctness v2 реализован:
- `BatteryAwarePlanner::order` корректно drop задачи из ordered subset;
- `is_feasible` поддерживает battery model v2;
- runner вычисляет meaningful route metrics;
- regression baseline обновлён.

Следующий шаг по линейному плану DRONE_A.14.linear.md — M35 Dynamic Mission Correctness.

## Investigation context

`INVESTIGATION.md` отсутствует. Анализ кода показал:

### 1. Generic success semantics не подходят для dynamic missions

**Файл:** `crates/swarm-sim/src/runner.rs:1160-1162`

```rust
let success = all_tasks_assigned
    && all_expected_failures_detected
    && max_task_unassigned_ticks <= config.max_unassigned_ticks;
```

`success` — единый для всех миссий. Но dynamic missions имеют разные критерии завершения:
- **SAR**: success = все цели найдены (`grid_state.all_targets_found()`), а не все задачи assigned. Задачи (`SarScan`) release-ятся после сканирования и должны быть переназначены.
- **Inspection**: success = достаточное покрытие рёбер (`edge_coverage_rate > threshold`), не обязательно 100%. Battery/time constraints делают 100% покрытие невозможным для perimeter.
- **Wildfire**: success = все зоны с приоритетом ≥ N mapped. Dynamic threat меняет приоритеты, но already-mapped зоны остаются completed.

### 2. SAR release/replan создаёт stale reassignment loop

**Файл:** `crates/swarm-sim/src/runner.rs:790-793`

```rust
for task_id in scanned_task_ids {
    node.coordinator.registry.release_task(&task_id);
}
```

- CBBA: после `release_task()` требуется re-convergence. Если `max_unassigned_ticks` мал (10), CBBA не успевает reconverge → `success = false`.
- Centralized: static pre-plan, не видит released tasks. Агенты revisiting stale cell assignments.
- README отмечает SAR + CBBA/centralized как unsupported, но причины не протестированы в коде.

### 3. Wildfire medium-dynamic completion/success mismatch

**Файл:** `crates/swarm-sim/src/runner.rs:1128-1129`

```rust
let adapter_complete =
    Self::adapter_driven_complete(&live_tasks, &run_state, &adapter_registry);
```

- `WildfireAdapter::is_completed` проверяет `state.mapped_zones.contains(&task.id.to_string())`.
- Но `task.id` — это zone id. Если dynamic threat обновляет приоритет задачи, `is_completed` всё равно возвращает `true` (зона уже mapped).
- Однако `all_tasks_assigned` требует, чтобы ВСЕ задачи были assigned или completed. Если dynamic threat создаёт новые задачи (сейчас не создаёт), или если агенты не успевают дойти до зон, `all_tasks_assigned = false`.
- Для `medium-dynamic` с `enable_dynamic_threat = true` приоритеты меняются, но задачи не добавляются/удаляются. mismatch проявляется в том, что `adapter_complete = true` (все зоны mapped), но `all_tasks_assigned = false` (из-за timeout или battery).

### 4. Inspection perimeter: high coverage, low success

**Файл:** `crates/swarm-scenarios/src/inspection.rs:45-52`

Perimeter profile имеет `battery_constraint: 0.3`, что ограничивает battery агентов. Agents exhaust before covering all edges. `all_tasks_assigned` остаётся `false`, хотя `edge_coverage_rate` может быть высоким (~0.8). README отмечает success rate ~0–0.4.

### 5. Support matrix не протестирована в коде

**Файл:** `README.md:158-174`

Support matrix описана в README, но:
- Нет автоматических тестов, которые проверяют unsupported combinations;
- Нет тестов на причины unsupported (static pre-plan, delayed reconvergence);
- При изменении кода support matrix может устареть без предупреждения.

## Affected components

| Компонент | Путь | Что меняется |
|---|---|---|
| Runner success semantics | `crates/swarm-sim/src/runner.rs` | Mission-specific success determination |
| SAR scenario builder | `crates/swarm-scenarios/src/sar_scenario.rs` | Document unsupported strategies with tests |
| Wildfire scenario builder | `crates/swarm-scenarios/src/wildfire.rs` | Align success/completion for medium-dynamic |
| Inspection scenario builder | `crates/swarm-scenarios/src/inspection.rs` | Perimeter success threshold |
| Adapter completion | `crates/swarm-types/src/adapter.rs` | WildfireAdapter priority-aware completion |
| Support matrix tests | `crates/swarm-examples/tests/` | New tests for documented support matrix |
| README | `README.md` | Update support matrix with M35 findings |

## Implementation steps

### 1. Define mission-specific success semantics

**Файл:** `crates/swarm-sim/src/runner.rs`

**Текущий код:**
```rust
let success = all_tasks_assigned
    && all_expected_failures_detected
    && max_task_unassigned_ticks <= config.max_unassigned_ticks;
```

**Исправление:**
- Добавить `mission_success` field в `RunMetrics` (или переименовать логику):
  - **SAR**: `success = grid_state.as_ref().is_none_or(|g| g.all_targets_found()) && max_task_unassigned_ticks <= config.max_unassigned_ticks`
  - **Inspection**: `success = edge_coverage_rate >= coverage_threshold` (threshold из конфигурации, default 0.8)
  - **Wildfire**: `success = adapter_complete && all_high_priority_zones_mapped`
  - **Coverage**: `success = all_tasks_assigned` (текущее поведение)
- Оставить `all_tasks_assigned` как отдельный флаг (не заменять на `success`).

### 2. Fix wildfire medium-dynamic success/completion alignment

**Файлы:** `crates/swarm-sim/src/runner.rs`, `crates/swarm-types/src/adapter.rs`, `crates/swarm-scenarios/src/wildfire.rs`

**Проблема:** `adapter_complete = true` (все зоны mapped), но `success = false` (battery/timeout).

**Исправление:**
- В `WildfireConfig` добавить `success_threshold: f64` (default 0.8) — доля зон, которую нужно mapped для success.
- В `RunMetrics` добавить `wildfire_success_threshold_met: bool`.
- В runner вычислять:
  ```rust
  let wildfire_success = wildfire_state.as_ref().is_none_or(|w| {
      if w.zones.is_empty() { true }
      else {
          let mapped = w.mapped_zone_ids.len() as f64;
          let total = w.zones.len() as f64;
          mapped / total >= success_threshold
      }
  });
  ```
- Обновить `WildfireAdapter::is_completed` для учёта dynamic threat: если `enable_dynamic_threat = true`, high-priority zones должны быть mapped для completion.

### 3. Revisit SAR release/replan and unsupported statuses

**Файлы:** `crates/swarm-sim/src/runner.rs`, `crates/swarm-scenarios/src/sar_scenario.rs`, `crates/swarm-examples/tests/`

**Проблема:** SAR + CBBA/centralized дают 0% success, но причины не протестированы.

**Исправление:**
- Добавить `SarUnsupportedReason` enum:
  ```rust
  pub enum UnsupportedReason {
      StaticPrePlan,       // centralized: static pre-plan incompatible with dynamic release
      DelayedReconvergence, // cbba: re-convergence exceeds max_unassigned_ticks
      PhysicallyConstrained, // battery/time makes mission impossible
  }
  ```
- В `build_sar_scenario` добавить unsupported markers:
  ```rust
  if strategy == "cbba" || strategy == "centralized" {
      config.unsupported_reason = Some(UnsupportedReason::...);
  }
  ```
- Добавить тест `sar_cbba_is_unsupported_with_reason`.
- Добавить тест `sar_centralized_is_unsupported_with_reason`.

### 4. Fix inspection perimeter success semantics

**Файлы:** `crates/swarm-scenarios/src/inspection.rs`, `crates/swarm-sim/src/runner.rs`

**Проблема:** Perimeter с `battery_constraint = 0.3` даёт `edge_coverage_rate > 0.8`, но `success = false` из-за `all_tasks_assigned = false`.

**Исправление:**
- В `InspectionConfig` добавить `coverage_threshold: f64` (default 0.8 для perimeter).
- В runner для inspection missions:
  ```rust
  let inspection_success = inspection_state.as_ref().is_none_or(|s| {
      let total = s.graph.edges.len() as f64;
      let covered = s.covered.len() as f64;
      total == 0.0 || covered / total >= coverage_threshold
  });
  ```
- `success` для inspection = `inspection_success && all_expected_failures_detected && max_task_unassigned_ticks <= config.max_unassigned_ticks`.

### 5. Add support matrix tests

**Файл:** `crates/swarm-examples/tests/support_matrix.rs` (новый)

**Что тестировать:**
- SAR + greedy → supported
- SAR + auction → supported
- SAR + cbba → unsupported (DelayedReconvergence)
- SAR + centralized → unsupported (StaticPrePlan)
- Inspection linear + all → supported
- Inspection perimeter + greedy → experimental
- Inspection perimeter + cbba → experimental
- Wildfire small-static + all → supported
- Wildfire medium-dynamic + all → experimental (dynamic threat)

**Формат теста:**
```rust
#[test]
fn support_matrix_sar_cbba_is_unsupported() {
    let (scenario, config) = build_sar_scenario(&SarProfile::Ideal.config(42));
    let metrics = ScenarioRunner::run_with(&scenario, config, CbbaAllocator::default());
    assert!(!metrics.success, "SAR + CBBA should be unsupported");
    assert_eq!(metrics.unsupported_reason, Some(UnsupportedReason::DelayedReconvergence));
}
```

### 6. Update README

**Файл:** `README.md`

- Обновить Strategy Support Matrix с M35 findings;
- Убрать устаревшие ссылки на M27 для SAR CBBA (теперь M35);
- Добавить статус dynamic missions: SAR (stable для greedy/auction), wildfire (experimental для medium-dynamic), inspection perimeter (experimental).

## Testing strategy

### Категория 1 — без рефакторинга

- **wildfire success/completion consistency test**:
  ```rust
  let metrics = run_wildfire_medium_dynamic();
  assert!(metrics.adapter_complete == metrics.wildfire_success_threshold_met || metrics.total_ticks >= config.max_ticks);
  ```
- **SAR release/replan deterministic test**:
  ```rust
  let metrics = run_sar_with_releases();
  assert!(metrics.targets_found > 0);
  assert!(metrics.max_task_unassigned_ticks <= config.max_unassigned_ticks);
  ```
- **inspection perimeter edge coverage vs success test**:
  ```rust
  let metrics = run_inspection_perimeter();
  if metrics.edge_coverage_rate > 0.8 { assert!(metrics.success); }
  ```
- **support matrix test** (SAR cbba/centralized unsupported with reason).

### Категория 2 — лёгкий рефакторинг

- **dynamic mission fixture builders**:
  - `fn wildfire_medium_dynamic_fixture() -> (Scenario, RunConfig)`
  - `fn sar_with_release_fixture() -> (Scenario, RunConfig)`
  - `fn inspection_perimeter_battery_constrained_fixture() -> (Scenario, RunConfig)`
- **shared mission outcome assertions**:
  - `fn assert_mission_success(metrics: &RunMetrics, mission: &str)`
  - `fn assert_unsupported(metrics: &RunMetrics, reason: UnsupportedReason)`

### Категория 3 — тяжёлый рефакторинг

- **dynamic replanning property test**: для random task releases, greedy allocator должен переназначать задачи без превышения `max_unassigned_ticks`.
- **multi-seed dynamic mission regression**: `--quick --mission wildfire --profile medium-dynamic` across 10 seeds, проверка consistency `success == adapter_complete`.
- **strategy cross-product support matrix generation**: скрипт, который запускает все (mission, strategy) pairs и генерирует support matrix markdown.

## Risks and tradeoffs

| Риск | Вероятность | Влияние | Митигация |
|---|---|---|---|
| Изменение `success` definition ломает regression baseline | Средняя | Высокое | Обновить baseline после fix; оставить `all_tasks_assigned` как отдельный флаг |
| SAR CBBA/centralized unsupported тесты flaky | Низкая | Среднее | Использовать deterministic fixtures; фиксировать seed |
| Wildfire threshold субъективен | Средняя | Среднее | Сделать threshold configurable; default 0.8 основан на README |
| Inspection perimeter threshold меняет смысл success | Средняя | Высокое | Документировать threshold; оставить `edge_coverage_rate` в metrics |

## Open questions

1. **Как определять success для wildfire medium-dynamic?**
   - Вариант A: `mapped / total >= threshold` (просто)
   - Вариант B: `high_priority_mapped / high_priority_total >= threshold` (учитывает dynamic threat)
   - Вариант C: `time_to_map_all <= time_limit` (временной критерий)
   - Рекомендуется A для простоты + B как optional enhancement

2. **Как обрабатывать SAR task release в CBBA?**
   - Вариант A: увеличить `max_unassigned_ticks` для SAR CBBA (workaround)
   - Вариант B: добавить fast-reconvergence mode для released tasks
   - Вариант C: оставить unsupported с explicit reason
   - Рекомендуется C (unsupported + reason) в M35; A/B — будущие milestones

3. **Нужен ли `UnsupportedReason` в `RunMetrics`?**
   - Вариант A: добавить `unsupported_reason: Option<String>` в `RunMetrics`
   - Вариант B: добавить `mission_outcome: MissionOutcome` enum (Success, Failure, Unsupported)
   - Рекомендуется A для простоты, B для M36+

4. **Как интегрировать с regression?**
   - Regression thresholds для dynamic missions должны учитывать новые success semantics
   - Рекомендуется обновить thresholds в M36 (Regression Harness v2)

## Что могло сломаться

- **Поведение**: `success` для SAR/inspection/wildfire теперь определяется mission-specific rules. Старые тесты, которые проверяли `metrics.success == metrics.all_tasks_assigned`, могут сломаться.
- **API/контракты**: `RunMetrics` получает новые поля (`wildfire_success_threshold_met`, `unsupported_reason`). Старые JSON десериализуются (serde default).
- **Данные**: `AggregateMetrics.success_rate` изменится для dynamic missions. Старые benchmark reports несовместимы по смыслу (но не по schema).
- **Интеграции**: Regression baseline для SAR/inspection/wildfire изменится. Нужно обновить baseline.
- **Производительность**: Дополнительные проверки в runner — negligible overhead.

## Критерии готовности

- [ ] `cargo test --workspace` проходит (включая новые support matrix tests).
- [ ] `cargo clippy --all-targets -- -D warnings` проходит.
- [ ] `cargo fmt --all` не меняет код.
- [ ] SAR CBBA/centralized имеют explicit unsupported reason с тестами.
- [ ] Wildfire medium-dynamic имеет explainable success/completion behavior.
- [ ] Inspection perimeter success aligns с coverage threshold.
- [ ] README support matrix обновлён.
- [ ] Локальный commit сделан.
