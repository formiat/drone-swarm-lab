# DRONE_B.20 — Векторы развития после M57–M62

Дата: 2026-05-31

## Контекст

Завершены все плановые milestones M57–M62 из `DRONE_A.19.md`:

- **M57** Supervisor Controller Boundary — `AgentController` trait, `MockAgentController`,
  fake-controller тесты, кодовая граница для live PX4 path.
- **M58** Live Multi-Agent PX4/SIH Execute — `Px4AgentController`, stepwise polling loop,
  два агента под одним supervisor, артефакт в `results/m58_multi_agent_px4_sih_execute_2026-05-31/`.
- **M59** Live PX4/SIH Failure & Reallocation — live lost-agent detection, abort/replace
  у активного survivor, fake-контроллер тесты, артефакт в
  `results/m59_px4_sih_failure_reallocation_2026-05-31/`.
- **M60** Hardening — `--output-dir`, `--run-id`, `--force`, stable exit codes, schema hardening.
- **M61** Extension Guide — `docs/EXTENSION_GUIDE.md`, crate boundaries, schema version policy,
  test-only extension fixtures.
- **M62** Benchmark Refresh — 500-seed release baseline в `results/all_500_jobs14_m62_release/`,
  `docs/BENCHMARK_RESULTS.md` обновлён.

Из `DRONE_B.19.md` не реализованы:

- M61 (B.19) Disaster Mapping v2 — частично. Flood задокументирован как out-of-scope.
  Priority → allocation в wildfire уже работает (`task.priority * 20.0 + threat_urgency`).
  Wildfire success semantics не задокументированы тестом.
- M63 (B.19) New Mission — не начата.
- M65 (B.19) Algorithm Depth Decision Point — стратегическая развилка, не реализация.

Текущая позиция проекта: simulation foundation + deterministic regression + single-agent и
multi-agent PX4/SIH execute + controlled failure/reallocation + extension guide + 500-seed
benchmark baseline. Это хорошая точка для осознанного выбора следующего вектора.

---

## Вектор A — Алгоритмическая глубина

### Суть

Улучшить сами алгоритмы координации, которые уже работают, но не дифференцированы достаточно.

Центральный вопрос, который задаёт этот вектор: **в каких сценариях какая стратегия
действительно лучше и почему?** Сейчас benchmark (M62) показывает, что в большинстве профилей
greedy не хуже auction и connectivity-aware. Это либо означает, что greedy достаточно хорош,
либо что стратегии недостаточно дифференцированы сценариями и метриками.

### Пункт A1 — Communication-aware allocation

**Текущее состояние:** `ConnectivityAwareAllocator` существует, но его connectivity-логика
ограничена: она разделяет задачи на relay и scout и оптимизирует размещение relay-агентов.
Scout-задачи всегда уходят в greedy без учёта `comms_range`. Все остальные стратегии
(greedy, auction, CBBA, centralized) игнорируют `comms_range` полностью — он хранится в
`AllocationAgent`, но в scoring не используется.

**Что можно сделать:**

1. Добавить `comms_penalty_weight: f64` как параметр scoring-функций. Если расстояние между
   агентом и назначенной задачей превышает `comms_range`, score снижается пропорционально.
   Это приближает поведение к реальному: агент за пределами зоны связи — ненадёжный исполнитель.

2. Benchmark delta: сравнить coverage/SAR/wildfire с и без `comms_penalty_weight` по метрике
   `agent_availability` при разных профилях packet loss (heavy-loss-*, partition-prone-*).

3. Ожидаемый результат: явная разница между greedy и connectivity-aware в profles с
   ограниченной дальностью связи.

**Сложность:** средняя. Изменения в `swarm-alloc/src/allocator.rs` (scoring),
`swarm-types/src/adapter.rs` (score signature), тесты, benchmark delta.

### Пункт A2 — Mission-specific planners

**Текущее состояние:** все миссии используют одинаковый scoring pattern:
`base_score - distance + battery_factor + priority`. Разница только в коэффициентах.
2-opt route planning существует в `swarm-alloc/src/route_planner.rs` и используется в
`CentralizedPlanner`, но для greedy/auction не применяется.

**Конкретные gaps:**

- **SAR:** задачи должны упорядочиваться по убыванию belief entropy, а не только по расстоянию.
  Сейчас `sar_task_priority` вычисляется статически при генерации сценария и не обновляется
  при изменении belief. Dynamic priority update есть в `WildfireState`, аналога для SAR нет.

- **Wildfire:** dynamic priority updates пишут события в replay log и обновляют `priority`
  поле задачи, но re-allocation при изменении приоритета не происходит — агент продолжает
  выполнять старое назначение даже если другая зона стала критичнее. Нужен механизм
  priority-triggered reallocation.

- **Inspection:** 2-opt route planning для `CentralizedPlanner` уже работает (M34),
  но greedy и auction не используют route optimization. Разница в metrics
  (`total_route_cost`, `task_conflicts`) должна быть измеримой.

**Сложность:** средняя–высокая. Изменения в `swarm-runtime` (dynamic reallocation trigger),
`swarm-scenarios` (dynamic SAR belief updates), benchmark delta.

### Пункт A3 — CBBA convergence

**Текущее состояние:** SAR + CBBA явно unsupported (`delayed_reconvergence`). Coverage + CBBA
имеет 6 профилей с `success = 0.000` при `completion = 1.000` — что означает, что
задачи выполняются, но success predicate не срабатывает. Emergency mesh + CBBA даёт
`success = 0.427` при `conflicts = 4.2` (vs centralized `success = 0.828`, `conflicts = 0.0`).

**Что стоит исследовать:**

1. Почему coverage CBBA fails в heavy-loss и high-latency профилях? Hypothesis: при потере
   агентов CBBA не успевает переконвергировать за `gossip_interval_ticks`. Проверяется
   изменением `gossip_interval_ticks` и replay анализом.

2. SAR + CBBA: delayed reconvergence = агенты не получают сообщения о выбывших задачах
   достаточно быстро и продолжают "торговать" уже выполненными. Возможный fix: более частый
   gossip при agent loss event.

3. Emergency mesh конфликты: `connectivity-aware` и другие стратегии дают `conflicts ≈ 2`.
   Centralized даёт 0. Это связано с тем, что relay placement требует глобального view —
   что является inherent преимуществом centralized. Стоит задокументировать это явно
   в support matrix.

**Сложность:** средняя. Преимущественно анализ и параметрический эксперимент, возможно
небольшой patch к gossip policy.

### Пункт A4 — Hierarchical coordination (8+ агентов)

**Текущее состояние:** все тесты и benchmark профили ограничены малым числом агентов (2–5).
`Coordinator` и `AgentNode` не имеют ограничений на число агентов, но поведение при 8+
агентах не проверено.

**Что можно сделать:** добавить сценарии с 8 и 16 агентами для coverage и wildfire,
измерить scaling по `time_to_completion`, `agent_availability`, `task_conflicts`.
Если scaling плохой — ввести hierarchical grouping: кластеры из 3–4 агентов с
локальным coordinator.

**Сложность:** высокая. Затрагивает `swarm-runtime` и требует нового дизайна.

---

## Вектор B — Новая миссия

### Суть

Добавить новый класс миссий через уже готовый extension path (M61/EXTENSION_GUIDE.md).
Это подтвердит что extension path работает на реальной миссии, а не только на
test-only fixtures.

Три кандидата с разной сложностью и разной ценностью для проекта.

### Кандидат B1 — Multi-target Pursuit (движущиеся цели)

**Domain model:**

```rust
TaskKind::Pursuit { target_id: TargetId, mode: PursuitMode, proximity_radius: f64 }

enum PursuitMode { Intercept, Escort }

struct PursuitTarget { id: TargetId, trajectory: Vec<(Pose, u64)>, speed: f64 }

// RunState extension
active_targets: HashMap<TargetId, Pose>,
captured_targets: HashSet<TargetId>,
```

**Механика:** цели движутся по заданным траекториям (или по простой модели уклонения).
Задача completed когда агент входит в `proximity_radius` от цели. Задачи динамически
появляются при обнаружении новой цели.

**Метрики:** `capture_rate`, `time_to_intercept`, `targets_lost`, `total_pursuit_distance`,
`interception_efficiency` (расстояние до цели в момент capture).

**Что проверяет:** алгоритмическую реактивность при динамическом появлении/исчезновении
задач. Принципиально отличается от существующих миссий: задачи не статические waypoints,
а движущиеся объекты. Это stress-test для auction и CBBA.

**Extension point:** требует `RunState::active_targets` — новое поле в `swarm-runtime`.
Это единственный breaking change; в остальном — чистое расширение через `MissionAdapter`.

**Сложность:** высокая. Новые типы в `swarm-types`, `swarm-runtime`, `swarm-scenarios`,
сложная completion semantics, движущиеся цели требуют tick-driven state update.

### Кандидат B2 — Logistics / Delivery (precedence constraints)

**Domain model:**

```rust
TaskKind::Pickup { item_id: ItemId, location: Pose }
TaskKind::Dropoff { item_id: ItemId, location: Pose, requires_pickup: TaskId }

// AgentState extension
cargo: Vec<ItemId>,
cargo_capacity: usize,

// RunState extension
delivered_items: HashSet<ItemId>,
failed_deliveries: Vec<ItemId>,
```

**Механика:** нельзя выполнить Dropoff без предшествующего Pickup того же `item_id`.
Агент несёт ограниченное число грузов. Опционально: time window (дедлайн доставки).

**Метрики:** `delivery_rate`, `late_deliveries`, `capacity_violations`,
`total_route_cost`, `unserved_deliveries`, `precedence_violations`.

**Что проверяет:** обобщаемость DSL на задачи с зависимостями. Ни одна из существующих
миссий не имеет task dependencies. Это проверяет allocator корректность: не должен
назначать Dropoff без Pickup.

**Сложность:** высокая. Требует dependency tracking в `swarm-runtime/src/registry.rs`,
validation в allocators, новые типы в `swarm-types`.

### Кандидат B3 — Perimeter Patrol (периметр полигона)

**Domain model:**

```rust
// Scenario DSL extension
PerimeterZone { polygon: Vec<Pose>, waypoint_spacing_m: f64 }

// Mission builder: polygon -> Vec<Task> (waypoints along perimeter)
fn perimeter_waypoints(polygon: &[Pose], spacing: f64) -> Vec<Pose>
```

**Механика:** полигон задаёт периметр квартала или объекта. Миссия = обойти периметр,
посетив все waypoints вдоль периметра. Дополнительно: `forbidden_zones: Vec<Polygon>` —
зоны, которые нельзя пересекать.

**Что проверяет:** geographic mission type без движущихся целей и без task dependencies.
Наиболее близко к реальной прикладной задаче (охрана периметра, мониторинг объекта).
При multi-agent: как разделить периметр между агентами (disjoint segments vs
overlapping coverage).

**Связь с PX4/SIH:** waypoints по периметру — прямо в формат `sitl_supervisor`.
Это единственный кандидат, который можно прогнать через реальный PX4/SIH без изменений
в transport layer.

**Сложность:** низкая–средняя. Нет breaking changes в `swarm-runtime`. Новый scenario
builder + DSL extension + новый `MissionAdapter`. Геометрия полигона — стандартные операции.

**Рекомендация:** B3 как первый шаг — минимальная сложность, максимальная связь с
реальными задачами и с существующим SITL pipeline. B1 или B2 после — в зависимости от
того, что важнее: динамические задачи или task dependencies.

---

## Вектор C — Research Benchmark / Publication

### Суть

Закрыть открытые интерпретационные вопросы из M62 и довести доказательную базу до уровня,
когда результаты можно предъявлять внешним людям.

### Пункт C1 — Открытые вопросы из BENCHMARK_RESULTS.md

M62 честно зафиксировал три проблемных области. Прежде чем делать publication-level
benchmark, их нужно интерпретировать.

**SAR success ≈ 0:**
`success = 0.001` у всех стратегий кроме CBBA (у которого `completion = 0.036`). Причина:
`gs.all_targets_found()` требует `detection_probability^n` чудес. При `prior = 0.5`
и 5 агентах это очень строгий критерий. Возможные решения:
- Relaxed success: `targets_found_rate >= threshold` вместо `all_targets_found()`.
- Документировать SAR success как "probability-of-detection based" metric, не mission success.
- Добавить `pod_success_threshold` в `SarRunConfig`.

**Wildfire success = 0.247:**
`mapped_ratio >= 0.8` срабатывает только если все зоны с приоритетом < 8 тоже замаплены.
При 4 зонах и limited тиках агенты не всегда успевают. Возможные решения:
- Сделать threshold профиле-специфичным (small-static: 0.7, medium-dynamic: 0.5).
- Добавить `zones_mapped_rate` как отдельную метрику рядом с `success`.

**CBBA coverage при heavy-loss/high-latency:**
6 профилей с `success = 0.000`, `completion = 1.000`. Задачи выполняются, но success
predicate fails. Это может быть проблема реконвергенции при потере агентов: coverage
success требует `all_expected_failures_detected`, а CBBA не переконвергировал до конца.
Нужен replay анализ этих профилей.

### Пункт C2 — 1000-seed run

После закрытия интерпретационных вопросов — полный 1000-seed release run.

Артефакты:
- JSON/CSV/Markdown как в M62, но с confidence intervals (`mean ± stderr`).
- Degradation curves: `success` vs `agents_count` (2–8), vs `packet_loss_rate` (0–0.5),
  vs `grid_size` (для coverage).
- Strategy comparison report: для каждой пары (mission, profile) — winner strategy
  по каждой метрике с p-value если статистика будет.

**Сложность:** низкая по коду (runner готов, нужна обвязка для CI ranges),
средняя по аналитике (интерпретация результатов).

### Пункт C3 — Wildfire success semantics тест

Отдельный маленький fix из B.19 M61-C: зафиксировать success rule тестом.

```rust
// crates/swarm-sim/tests/support_matrix.rs (или отдельный файл)
fn wildfire_small_static_success_semantics() {
    // success == (mapped_ratio >= threshold) && all_expected_failures_detected
    // при 100% completion: agents visit all zones -> mapped_ratio should be high
    // threshold = 0.8 для small-static
}
```

**Сложность:** низкая. Один тест + doc comment в runner.rs.

---

## Вектор D — Глубже в SITL / реальные сценарии

### Суть

Развить то, что уже работает с PX4/SIH: больше failure modes, повторяемость, анализ.

### Пункт D1 — Больше failure modes для M59

Текущий M59 артефакт: kill PX4 process → `disconnected` → reallocation. Это один
контролируемый path. Следующие пути:

- **No-progress timeout:** агент завис на waypoint (PX4 не движется из-за configuration
  issue, GPS glitch в SIH). Supervisor детектирует через `no_progress_timeout`.
- **Mission rejection:** PX4 отклоняет mission upload (`MISSION_ACK` с ошибкой).
  Supervisor должен пометить агента как failed и реаллоцировать.
- **Partial completion:** агент завершил N из M задач, потом упал. Survivor получает
  только оставшиеся M-N задач.

Каждый из этих path требует отдельный артефакт и тест.

**Сложность:** средняя. Код для большинства paths уже есть в `Px4AgentController`,
нужны сценарии для воспроизводства каждого failure.

### Пункт D2 — Replay seq fix (технический долг M59)

Зафиксированный баг: completion events для recovered tasks пишутся с seq из оригинального
manifest, а не из replacement mission. Например, wp-0 был seq=0 у agent-0, после
replacement у agent-1 он стал seq=2, но completion event пишет seq=0.

**Минимальный fix:** хранить в `LiveAgentRun` не только `completed_task_ids: Vec<String>`,
а `completed_task_items: Vec<(u16, String)>` — пары `(seq, task_id)` из активной mission
controller-а. `SitlTaskProgress` уже знает seq из телеметрии — нужно только прокинуть
эту информацию в `LiveAgentRun`.

**Сложность:** низкая–средняя. Изменение типа `LiveAgentRun`, обновление
`Px4AgentController`, `FakeLiveAgentController`, `record_live_agent_run`, тестов.

### Пункт D3 — Local integration harness

Сейчас воспроизвести M58/M59 прогон требует ручных шагов: запустить два PX4, ждать
инициализации, запустить supervisor с нужными параметрами. Нужен reproducible script.

```bash
# scripts/run_m58_local.sh
# - запускает два PX4 SIH в фоне с known PIDs
# - ждёт порты
# - запускает supervisor
# - убивает PX4 по завершению
# - кладёт артефакт в results/local_YYYY-MM-DD/
```

**Сложность:** низкая. Shell script + документация.

### Пункт D4 — Replay timeline / ASCII visualization

Текущий `replay --summary` даёт статистику. Для анализа live прогонов нужен
хоть какой-то способ смотреть что произошло в каком порядке.

Минимальный вариант (без визуализации):
- `replay --timeline`: печатает события в хронологическом порядке по `elapsed_ms`
  с agent_id префиксом. Легко читается как лог.
- `replay --agent <id>`: только события одного агента.

Это не "мультик" — это инструмент отладки для разработчика.

**Сложность:** низкая. Новые флаги в `crates/swarm-examples/src/bin/replay.rs` +
форматирование событий.

---

## Сравнение векторов

| Вектор | Главная ценность | Входит в текущий вектор | Трудоёмкость |
|---|---|---|---|
| **A1** Comms-aware scoring | Дифференцирует стратегии измеримо | Да | Средняя |
| **A2** Mission-specific planners | Улучшает алгоритмы для конкретных миссий | Да | Средняя–высокая |
| **A3** CBBA convergence | Закрывает known unsupported gaps | Да | Средняя |
| **A4** Hierarchical (8+ агентов) | Масштабируемость | Нет (новая область) | Высокая |
| **B1** Pursuit | Динамические задачи, stress-test | Да (через M61 extension) | Высокая |
| **B2** Logistics | Task dependencies | Да (через M61 extension) | Высокая |
| **B3** Perimeter Patrol | Реальная задача + PX4 ready | Да (через M61 extension) | Низкая–средняя |
| **C1** Интерпретация SAR/wildfire | Честность benchmark claims | Да | Аналитическая |
| **C2** 1000-seed run | Publication-level evidence | Да | Низкая по коду |
| **C3** Wildfire success тест | Маленький долг | Да | Низкая |
| **D1** Больше failure modes | Глубина M59 | Да | Средняя |
| **D2** Replay seq fix | Корректность replay | Да | Низкая–средняя |
| **D3** Local harness script | Повторяемость | Да | Низкая |
| **D4** Replay timeline | Инструмент отладки | Да | Низкая |

---

## Рекомендуемый порядок

### Если приоритет — алгоритмы

C3 (wildfire тест, маленький) → C1 (SAR/wildfire интерпретация) → A1 (comms scoring)
→ A2 (mission-specific planners) → C2 (1000-seed) → A3 (CBBA).

### Если приоритет — новая миссия

B3 (perimeter patrol, проще всего) → убедиться что extension path работает → B1 или B2.

### Если приоритет — SITL глубина

D2 (replay seq fix, маленький долг) → D3 (local harness) → D4 (timeline) → D1 (failure modes).

### Если приоритет — что-то показать

B3 + D3 + D4: реальная задача (квартальный патруль) через SITL с воспроизводимым
скриптом и читаемым replay — это готовый демо-пакет.

---

## Что явно не в этом плане

- Hardware / HIL — не планируется, граница задокументирована в `docs/HARDWARE_READINESS.md`.
- Motion planning / obstacle avoidance / sensor fusion на уровне ниже PX4 — это другой
  домен; PX4 отвечает за физику движения, проект отвечает за координацию.
- Визуальный 2D/3D viewer — полезен, но не является ни алгоритмической, ни
  инженерной ценностью для текущей стадии.
- Distributed onboard autonomy — требует hardware и другой архитектуры.
