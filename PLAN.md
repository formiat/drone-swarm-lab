# PLAN: M74 — Urban Blocked-Route Decision Logic

## Контекст

M74 добавляет mission-level реакцию на заблокированные рёбра графа в Urban-симуляции,
без физики, без реального сенсора, без имитации реального obstacle avoidance.

Цепочка поведения:
```
ребро блокируется → детектор смотрит вперёд → политика (Wait/Replan/Abort) →
агент ждёт или перепланирует → судья фиксирует нарушение если агент
пытается въехать → replay + метрики отражают решение
```

M74 — четвёртый milestone в цепочке M70→M79 (см. `docs_raw/BEFORE_HARDWARE_A.23.md`).
M71 safety gate (`judge_route` + `SafetyValidationReport`) уже реализован и будет
использован для проверки маршрута перед принятием нового плана.

## Результаты исследования кодовой базы

| Что | Файл : строка | Статус |
|-----|--------------|--------|
| `UrbanEdge.blocked: bool` | `swarm-types/src/urban.rs:100` | уже есть |
| `UrbanViolation::BlockedEdge` | `swarm-types/src/urban.rs:152` | уже есть |
| `UrbanViolation` replay event | `swarm-replay/src/event_log.rs:151` | уже есть |
| `urban_replan_count` в `RunMetrics` | `swarm-metrics/src/metrics/run.rs:150` | уже есть, всегда 0 |
| Planner фильтрует `edge.blocked` | `swarm-sim/src/urban/planner.rs:113` | уже есть |
| Static violation check при старте patrol | `swarm-sim/src/runner/urban_patrol.rs:147` | уже есть |
| `judge_route` — M71 gate | `swarm-sim/src/urban/judge.rs` | уже есть |
| `SafetyValidationReport` | `swarm-safety/src/preflight.rs:20` | уже есть |
| Runtime replanning / waiting | — | **отсутствует** |
| `UrbanTemporaryObstacle` | — | **отсутствует** |
| Effective blocked-set по tick | — | **отсутствует** |
| Политики Wait / Replan / Abort | — | **отсутствует** |
| 8 новых M74 replay events | — | **отсутствует** |
| Новые метрики wait_time_ticks и т.д. | — | **отсутствует** |

Ключевые файлы runner'а: `swarm-sim/src/runner/urban_patrol.rs` (298 строк);
маршрут планируется один раз в начале (`plan` → `route`), затем пробегается
без изменений. Никакой логики реакции на заблокированные рёбра во время
выполнения нет.

Плановщик `plan_route_with_mode` умеет обходить blocked-рёбра через фильтр
`!edge.blocked` — его можно расширить параметром `extra_blocked`.

## Затронутые компоненты

- `crates/swarm-types` — новые типы и валидация
- `crates/swarm-replay` — 8 новых событий
- `crates/swarm-metrics` — 4 новые метрики
- `crates/swarm-sim` — detektор, политики, runtime patrol, preflight, DSL-валидация
- `crates/swarm-scenarios` — 3 новых профиля сценариев
- `docs/`, `README.md` — обновление документации

## Implementation steps

### Шаг 1. `UrbanTemporaryObstacle` и `UrbanBlockedPolicy` в swarm-types

**Файл:** `crates/swarm-types/src/urban.rs`

Добавить после `UrbanSearchState` (≈ строка 187):

```rust
/// A time-gated edge blockage for Urban scenarios.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UrbanTemporaryObstacle {
    pub edge_id: UrbanEdgeId,
    pub appears_at_tick: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disappears_at_tick: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
}

impl UrbanTemporaryObstacle {
    /// Returns true if the obstacle is active at `tick`.
    pub fn is_active(&self, tick: u64) -> bool {
        tick >= self.appears_at_tick
            && self.disappears_at_tick.map_or(true, |d| tick < d)
    }
}

/// Policy applied when the next segment on the route is blocked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UrbanBlockedPolicy {
    #[default]
    Wait,
    Replan,
    Abort,
}
```

Добавить в `impl UrbanMap`:
```rust
/// Validate temporary obstacles: edge_id must exist, appears ≤ disappears.
pub fn validate_temporary_obstacles(
    &self,
    obstacles: &[UrbanTemporaryObstacle],
) -> Vec<UrbanMapValidationError>
```

Добавить unit-тесты в `urban.rs::tests`:
- `temporary_obstacle_is_active_within_window`
- `temporary_obstacle_no_disappears_stays_forever`
- `temporary_obstacle_inactive_before_appears`
- `validate_temporary_obstacles_rejects_unknown_edge`
- `validate_temporary_obstacles_rejects_inverted_window`

**Ожидаемый результат:** `cargo test -p swarm-types` проходит.

---

### Шаг 2. Добавить поля в `UrbanState`

**Файл:** `crates/swarm-sim/src/runner/types.rs`

В `UrbanState` (строки 142–149) добавить:
```rust
    #[serde(default)]
    pub temporary_obstacles: Vec<UrbanTemporaryObstacle>,
    #[serde(default)]
    pub blocked_route_policy: UrbanBlockedPolicy,
```

`#[serde(default)]` обеспечивает обратную совместимость со старыми сценариями.

**Ожидаемый результат:** `cargo check -p swarm-sim` проходит, старые сценарии
десериализуются без изменений.

---

### Шаг 3. Новый модуль `obstacle.rs` в `swarm-sim/src/urban/`

**Файл:** `crates/swarm-sim/src/urban/obstacle.rs` (новый)

```rust
use std::collections::HashSet;
use swarm_types::{UrbanEdgeId, UrbanMap, UrbanPlannedRoute, UrbanTemporaryObstacle};

/// Number of route segments looked ahead by the mock detector.
pub const URBAN_BLOCKED_LOOKAHEAD_SEGMENTS: usize = 3;

/// Returns the set of edge IDs blocked at `tick`.
/// Combines static map `blocked` flags with active temporary obstacles.
pub fn effective_blocked_edges(
    map: &UrbanMap,
    obstacles: &[UrbanTemporaryObstacle],
    tick: u64,
) -> HashSet<UrbanEdgeId>

/// Returns the first (segment_index, edge_id) found blocked within
/// `lookahead` segments ahead of `from_segment`.
pub fn detect_blocked_ahead(
    route: &UrbanPlannedRoute,
    from_segment: usize,
    blocked: &HashSet<UrbanEdgeId>,
    lookahead: usize,
) -> Option<(usize, UrbanEdgeId)>
```

Зарегистрировать в `crates/swarm-sim/src/urban/mod.rs`:
```rust
pub mod obstacle;
pub use obstacle::{effective_blocked_edges, detect_blocked_ahead, URBAN_BLOCKED_LOOKAHEAD_SEGMENTS};
```

Unit-тесты в `obstacle.rs` или вынести в `crates/swarm-sim/src/urban/tests.rs`:
- `effective_blocked_edges_includes_static_blocked`
- `effective_blocked_edges_includes_active_obstacle`
- `effective_blocked_edges_excludes_inactive_obstacle`
- `detect_blocked_ahead_finds_blocked_within_lookahead`
- `detect_blocked_ahead_returns_none_when_clear`
- `detect_blocked_ahead_respects_lookahead_limit`

**Ожидаемый результат:** `cargo test -p swarm-sim obstacle` проходит.

---

### Шаг 4. Расширить planner — `plan_route_excluding`

**Файл:** `crates/swarm-sim/src/urban/planner.rs`

Добавить функцию (без изменения существующих):
```rust
/// Plan a route treating `extra_blocked` edge IDs as blocked
/// in addition to the map's static `blocked` flag.
pub fn plan_route_excluding(
    map: &UrbanMap,
    from: &UrbanNodeId,
    to: &UrbanNodeId,
    extra_blocked: &HashSet<UrbanEdgeId>,
    planner: UrbanPlannerMode,
) -> Result<UrbanPlannedRoute, UrbanRouteError>
```

Реализация: копирует логику `plan_route_with_mode`, меняя фильтр на
`!edge.blocked && !extra_blocked.contains(&edge.id)`.

Unit-тесты:
- `plan_route_excluding_finds_alternate_path`
- `plan_route_excluding_returns_no_route_if_all_blocked`

**Ожидаемый результат:** `cargo test -p swarm-sim planner` проходит.

---

### Шаг 5. Добавить 8 новых M74 replay events

**Файл:** `crates/swarm-replay/src/event_log.rs`

Добавить в `Event` после `UrbanSearchCompleted` (≈ строка 197):

```rust
// M74: Urban Blocked-Route Decision Logic
UrbanEdgeBlocked {
    agent_id: AgentId,
    tick: u64,
    edge_id: UrbanEdgeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
},
UrbanEdgeUnblocked {
    agent_id: AgentId,
    tick: u64,
    edge_id: UrbanEdgeId,
},
UrbanObstacleDetected {
    agent_id: AgentId,
    tick: u64,
    edge_id: UrbanEdgeId,
    lookahead_segments: usize,
},
UrbanPolicyDecision {
    agent_id: AgentId,
    tick: u64,
    edge_id: UrbanEdgeId,
    policy: String,  // "wait" | "replan" | "abort"
},
UrbanRouteReplanned {
    agent_id: AgentId,
    tick: u64,
    edge_ids: Vec<UrbanEdgeId>,
    route_length_m: f64,
},
UrbanWaitStarted {
    agent_id: AgentId,
    tick: u64,
    edge_id: UrbanEdgeId,
},
UrbanWaitCompleted {
    agent_id: AgentId,
    tick: u64,
    edge_id: UrbanEdgeId,
    waited_ticks: u64,
},
UrbanNoRouteAvailable {
    agent_id: AgentId,
    tick: u64,
    from: UrbanNodeId,
    to: UrbanNodeId,
    reason: String,
},
```

Добавить serde roundtrip тесты для каждого нового события в `event_log.rs` или
в тестовый модуль `swarm-replay`.

**Ожидаемый результат:** `cargo test -p swarm-replay` проходит, новые события
сериализуются/десериализуются без потерь.

---

### Шаг 6. Добавить 4 новые метрики в `RunMetrics`

**Файл:** `crates/swarm-metrics/src/metrics/run.rs`

После `urban_route_conflict_count` (строка 168):
```rust
// v0.74 Urban Blocked-Route Decision Logic
#[serde(default)]
pub urban_wait_time_ticks: u64,
#[serde(default)]
pub urban_blocked_edge_count: u64,
#[serde(default)]
pub urban_replan_success_rate: f64,
#[serde(default)]
pub urban_unresolved_blockage_count: u64,
```

`#[serde(default)]` сохраняет прямую и обратную совместимость.

Обновить `urban_patrol_metrics` и `urban_search_metrics` в
`crates/swarm-sim/src/runner/urban_metrics.rs` — добавить параметры и
прокинуть значения. Во всех вызовах этих функций передавать нули пока
не подключён runtime.

**Ожидаемый результат:** `cargo check -p swarm-metrics` и `cargo check -p swarm-sim`
проходят.

---

### Шаг 7. Runtime decision logic в `urban_patrol.rs`

**Файл:** `crates/swarm-sim/src/runner/urban_patrol.rs`

Добавить состояние ожидания перед тик-циклом:
```rust
struct BlockedRouteState {
    waiting_for: Option<UrbanEdgeId>,
    wait_start_tick: u64,
    wait_ticks: u64,
    replan_count: u64,
    replan_successes: u64,
    blocked_edge_detections: u64,
    unresolved_blockages: u64,
}
```

В начале каждого тика (перед движением):
1. `effective_blocked = effective_blocked_edges(map, &temporary_obstacles, tick)`
2. Если `brs.waiting_for` установлен:
   - если ребро больше не в `effective_blocked`: записать `UrbanEdgeUnblocked` +
     `UrbanWaitCompleted`, сбросить `waiting_for`, продолжить движение
   - иначе: пропустить движение, инкрементировать `brs.wait_ticks`
3. Иначе: `detect_blocked_ahead(&route, segment_index, &effective_blocked, LOOKAHEAD)`
   - если `Some((seg_idx, edge_id))`:
     - `brs.blocked_edge_detections += 1`
     - записать `UrbanObstacleDetected`
     - применить политику по `urban_state.blocked_route_policy`:
       - **Wait**: записать `UrbanEdgeBlocked` + `UrbanWaitStarted` + `UrbanPolicyDecision("wait")`;
         установить `brs.waiting_for = Some(edge_id)`; пропустить движение
       - **Replan**: вызвать `plan_route_excluding(...)` с remaining route goal;
         если Ok → заменить `route` оставшимися сегментами, записать `UrbanRouteReplanned` + `UrbanPolicyDecision("replan")`, `brs.replan_count += 1`, `brs.replan_successes += 1`;
         если Err → записать `UrbanNoRouteAvailable` + `UrbanPolicyDecision("abort")`,
         `brs.unresolved_blockages += 1`, выйти с failure
       - **Abort**: записать `UrbanNoRouteAvailable` + `UrbanPolicyDecision("abort")`,
         `brs.unresolved_blockages += 1`, выйти с failure

Перед принятием нового маршрута (Replan) — M71 gate check:
```rust
let violations = judge_route(map, &new_route);
// + check no segment in effective_blocked
let has_effective_violations = new_route.segments.iter()
    .any(|s| effective_blocked.contains(&s.edge_id));
if !violations.is_empty() || has_effective_violations {
    // refuse, fall back to abort
}
```

Если агент пытается войти в сегмент при `is_waiting == false` но ребро оказалось
в `effective_blocked` (случай отсутствия lookahead): записать `UrbanViolation::BlockedEdge`
(уже существующий тип события).

Обнаружение `UrbanEdgeBlocked` при first occurrence: при первом тике когда
ребро переходит в blocked (было не blocked → стало blocked), если агент ещё не
знает — детектор поймает это при следующем обходе. Для simple implementation:
детектор опрашивается каждый тик.

Передать `brs` данные в `urban_patrol_metrics` при финальном вызове.

**Ожидаемый результат:** интеграционные тесты сценариев `BlockedRouteWait`,
`BlockedRouteReplan`, `BlockedRouteNoAlternative` (шаг 9) проходят.

---

### Шаг 8. Обновить preflight и DSL-валидацию

**Файл:** `crates/swarm-sim/src/preflight.rs`

В `check_urban_safety()` (≈ строка 174) добавить:
```rust
let obstacle_errors = urban_state.map
    .validate_temporary_obstacles(&urban_state.temporary_obstacles);
for error in obstacle_errors {
    report.violations.push(SafetyViolation {
        rule_id: "urban.invalid_temporary_obstacle".to_owned(),
        severity: ViolationSeverity::Error,
        affected_id: None,
        reason: error.to_string(),
    });
}
```

**Файл:** `crates/swarm-sim/src/dsl/urban_validate.rs`

Добавить вызов `validate_temporary_obstacles` при валидации DSL-сценария.

Добавить тест в `preflight.rs`:
- `urban_invalid_temporary_obstacle_fails_preflight` — obstacle с несуществующим edge_id

**Ожидаемый результат:** `cargo test -p swarm-sim preflight` проходит.

---

### Шаг 9. Добавить сценарии с временными препятствиями

**Файл:** `crates/swarm-scenarios/src/urban.rs`

Добавить три профиля:

**`BlockedRouteWaitAndContinue`**:
- Карта: 4 узла, линейный маршрут A→B→C→D
- Obstacle: ребро B→C blocked с тика 5, unblocked с тика 15
- Политика: Wait
- Ожидание: агент доходит до B, ждёт, ребро освобождается, агент продолжает,
  маршрут завершается успешно

**`BlockedRouteReplan`**:
- Карта: 4 узла, два пути: прямой A→C (через B) и обходной A→D→C
- Obstacle: ребро A→B blocked с тика 0, без disappears
- Политика: Replan
- Ожидание: агент перепланирует через D, маршрут завершается по-новому

**`BlockedRouteNoAlternative`**:
- Карта: 3 узла, единственный путь A→B→C
- Obstacles: оба ребра A→B и B→C blocked с тика 0
- Политика: Replan (→ Abort при отсутствии пути)
- Ожидание: `UrbanNoRouteAvailable`, success=false, явная причина

**Ожидаемый результат:** `cargo test -p swarm-scenarios` проходит.

---

### Шаг 10. Обновить urban_analysis event counters

**Файл:** `crates/swarm-sim/src/urban_analysis/events.rs`

Если там есть `EventCounters` или подобная структура — добавить подсчёт для
`UrbanEdgeBlocked`, `UrbanRouteReplanned`, `UrbanNoRouteAvailable`,
`UrbanWaitStarted`, `UrbanWaitCompleted`.

**Ожидаемый результат:** `cargo check -p swarm-sim` проходит.

---

### Шаг 11. Документация

**`docs/SCENARIO_DSL.md`** — добавить секцию о полях `temporary_obstacles` и
`blocked_route_policy` в Urban сценарии; примеры JSON/YAML для каждого поля.

**`docs/REPLAY.md`** — добавить секцию M74 с описанием 8 новых событий:
поля, семантика, порядок появления в типичном blocked-route trace.

**`docs/PREFLIGHT_SAFETY.md`** — добавить правило `urban.invalid_temporary_obstacle`.

**`docs/STATUS.md`** — отметить M74 как реализованный, обновить текущее состояние
проекта.

**`README.md`** — обновить описание Urban-функциональности: добавить упоминание
blocked-route decision logic и временных препятствий.

**Ожидаемый результат:** изменения в docs коммитятся вместе с кодом.

---

### Шаг 12. Финальные проверки

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --all
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-types
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-replay
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-metrics
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-scenarios
```

**Ожидаемый результат:** все тесты зелёные, clippy без warnings.

## Testing strategy

### Категория 1 — тесты без рефакторинга (реализовать вместе с функциональностью)

**swarm-types:**
- `temporary_obstacle_is_active_within_window` — `is_active(tick)` возвращает true внутри окна
- `temporary_obstacle_no_disappears_stays_forever` — без `disappears_at_tick` всегда active
- `temporary_obstacle_inactive_before_appears` — до `appears_at_tick` inactive
- `validate_temporary_obstacles_rejects_unknown_edge`
- `validate_temporary_obstacles_rejects_inverted_window`

**swarm-sim (urban/obstacle.rs):**
- `effective_blocked_edges_includes_static_blocked`
- `effective_blocked_edges_includes_active_obstacle`
- `effective_blocked_edges_excludes_inactive_obstacle`
- `detect_blocked_ahead_finds_blocked_within_lookahead`
- `detect_blocked_ahead_returns_none_when_clear`
- `detect_blocked_ahead_respects_lookahead_limit`

**swarm-sim (urban/planner.rs):**
- `plan_route_excluding_finds_alternate_path`
- `plan_route_excluding_returns_no_route_if_all_blocked`

**swarm-replay:**
- Serde roundtrip для каждого из 8 новых событий (8 тестов)

**swarm-sim (интеграционные):**
- Blocked edge before arrival triggers Wait policy (через сценарий `BlockedRouteWaitAndContinue`)
- Wait policy completes after edge unblocks
- Replan policy chooses alternate route (через `BlockedRouteReplan`)
- No alternate route fails with explicit reason (через `BlockedRouteNoAlternative`)
- Replay содержит `UrbanObstacleDetected`, `UrbanPolicyDecision`, `UrbanWaitStarted/Completed`
  или `UrbanRouteReplanned` или `UrbanNoRouteAvailable` — по профилю сценария
- M71 gate отклоняет замену маршрута если новый маршрут проходит через заблокированное ребро
- `urban_invalid_temporary_obstacle_fails_preflight`

**Покрытие gap:** `near_miss_count` не планируется — нет точного определения
(см. Open questions).

### Категория 2 — тесты с лёгким рефакторингом

- Blocked-edge scenario builder helper: переиспользуемая функция для построения
  линейного/разветвлённого графа с одним obstacle — упростит тесты из Категории 1
- Route policy assertion helper: утилита, которая проверяет наличие нужных
  событий в replay log по сценарию
- Urban replay event fixture helper: расширение существующих helpers в
  `swarm-sim/src/urban/tests.rs` для M74 событий
- Effective blocked-set helper для тест-ассертов

### Категория 3 — тесты с тяжёлым рефакторингом (отложено)

- Multi-agent yield policy tests — требует стабильного single-agent decision logic
- Dynamic obstacle schedule property tests — property-based генерация расписаний
- Larger generated-map stress tests — зависит от M76 (Synthetic Scenario Testbed)

## Что могло сломаться

| Риск | Как проверить |
|------|--------------|
| Старые сценарии без `temporary_obstacles` перестают десериализоваться | `#[serde(default)]` защищает; запустить `cargo test -p swarm-scenarios` |
| `urban_patrol_metrics` сигнатура меняется — все вызывающие не скомпилируются | `cargo check --all-targets` после шага 6–7 |
| `RunMetrics` serde forward/back compat нарушена | `#[serde(default)]` защищает; проверить existing benchmark JSON fixtures |
| `Event` enum добавляет варианты — existing serde-based replay files могут не десериализоваться | `tag = "type"` + unknown variants через `#[serde(other)]` если он задан; иначе добавление вариантов не ломает читателей, только писателей |
| `plan_route_excluding` имеет логическую ошибку → неверный alternate route | Тесты `plan_route_excluding_*` + сценарий `BlockedRouteReplan` |
| M71 gate не проверяет effective_blocked → опасный маршрут принимается | Тест `m71_gate_rejects_route_with_effective_blocked_edge` |
| Бесконечный Wait если `disappears_at_tick` не задан и агент ждёт вечно | Нет встроенного timeout в M74; поведение корректно по контракту (агент никогда не движется); документировать в Open questions |
| Счётчик `urban_replan_count` в `RunMetrics` теперь начнёт быть ненулевым | Ожидаемое изменение; существующие тесты на `urban_replan_count == 0` нужно обновить |

## Risks and tradeoffs

**Мутация маршрута в mid-run.** При Replan маршрут заменяется на лету. Текущая
структура runner'а использует индекс сегмента и дистанцию — при замене хвоста маршрута
нужно аккуратно сохранить уже пройденный префикс. Предлагаемый подход: при Replan
составлять новый маршрут от текущего узла (конец последнего завершённого сегмента)
до конечной точки маршрута. Начало — `route.segments[segment_index - 1].to` если
`segment_index > 0`, иначе стартовый узел.

**Отсутствие timeout для Wait.** Если obstacle без `disappears_at_tick`, агент
ждёт до конца симуляции. Это корректное поведение по спецификации M74. Добавление
timeout_ticks не входит в M74 scope.

**`near_miss_count` отложен.** Нет точного определения — метрика не добавляется.

**Yield-политика отложена.** Single-agent logic должен стабилизироваться перед
добавлением multi-agent yield.

**`UrbanBlockedPolicy` в swarm-types.** Полиси-enum нужен в scenario DSL (который
живёт в swarm-sim), но сам тип логичнее держать в swarm-types чтобы избежать
cyclic deps. Альтернатива: держать enum в swarm-sim и не экспортировать в swarm-types.
Предпочтительный вариант: в swarm-types (как `UrbanBlockedPolicy`) — это data-тип,
не поведение.

## Open questions

1. **Timeout для Wait-политики.** Нужен ли `max_wait_ticks: Option<u64>` в M74 или
   в M75? Без него агент с `Wait` и вечным obstacle висит до конца симуляции.
   Предложение: задокументировать ограничение в M74, добавить timeout в M75.

2. **Yield-политика.** Спецификация допускает yield только если "simple deterministic
   rule is ready". Правило пока не определено. Оставляем на M75 или позже.

3. **`near_miss_count`.** Нет точного определения. Что считается near-miss в
   graph-based симуляции без физики? Оставить undefined, не добавлять метрику.

4. **Lookahead range.** Константа `URBAN_BLOCKED_LOOKAHEAD_SEGMENTS = 3` выбрана
   произвольно. Нужно ли делать её конфигурируемой через `UrbanState`? Предложение:
   оставить константой в M74, сделать конфигурируемой в M75 если понадобится.

5. **Позиция агента при Replan.** Если агент находится на середине сегмента в момент
   обнаружения, нужно ли ждать окончания сегмента или реплан происходит немедленно?
   Предложение: Replan происходит на ближайшей узловой границе (по завершении
   текущего сегмента) — это упрощает стыковку маршрутов.
