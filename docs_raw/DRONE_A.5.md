# DRONE_A.5 — Итоговое направление после DRONE_A.4 / DRONE_B.4

> Сессия: май 2026. Синтез DRONE_A.4 и DRONE_B.4.

---

## Сравнение двух репортов

DRONE_A.4 и DRONE_B.4 расходятся в двух точках.

**CBBA — сделан или нет?**

DRONE_A.4 нашёл конкретный архитектурный gap через чтение кода:
`apply_remote_bids()` реализован, но никогда не вызывается. В `node.rs:123–125`
CBBA-сообщения молча игнорируются. В `runner.rs` CBBA-оркестрации нет — только
чтение метрик в конце (строки 738–741). Итог: CBBA работает как Greedy+ без сетевого
консенсуса.

DRONE_B.4 оценивал состояние по структуре модулей без чтения кода — считает CBBA
закрытым. Gap пропущен.

**Фокус Milestone 11:**

DRONE_A.4 предлагал починить CBBA и добавить proptest + benchmark.

DRONE_B.4 предлагал собрать компоненты в unified experiment runner с `ScenarioSuite`,
`--mission all`, CLI-флагами и invariant checks. Долгосрочный roadmap у B.4 конкретнее:
DSL → Uncertainty Map → Infrastructure Inspection → Safety Layer.

**Что верно в каждом:**

DRONE_A.4 правильно диагностирует технический долг. Без CBBA fix числа benchmark
вводят в заблуждение — CBBA в таблице будет выглядеть как Greedy с другим scoring, а
не как distributed consensus.

DRONE_B.4 правильно описывает платформенную цель. После M1–M10 следующий шаг — не
новый алгоритм, а связка компонентов в reproducible benchmark platform. Roadmap
M12–M15 обоснован: каждый шаг опирается на предыдущий.

---

## Итоговое направление

Берём архитектурный диагноз из DRONE_A.4, платформенное видение из DRONE_B.4.

### Предпосылка — CBBA fix (до Milestone 11)

Небольшое изменение, блокирующее честный benchmark:

1. Добавить `send_cbba_bids()` и `collect_cbba_messages()` в `AgentNode`
   (`swarm-runtime/src/node.rs`).
2. Добавить `is_distributed() -> bool` в `Allocator` trait
   (`swarm-alloc/src/allocator.rs`).
3. Добавить CBBA tick-path в `runner.rs`: после gossip phase — broadcast
   `RuntimeMessage::Cbba`, collect remote bids, `apply_remote_bids()`, apply
   assignments.

Без этого шага любое сравнение в Milestone 11 некорректно.

### Milestone 11 — Unified Experiment Runner + Mission Benchmark Matrix

**Цель:** собрать уже реализованные компоненты в единую reproducible benchmark
platform. Это превращает набор сильных milestones в исследовательскую платформу с
публикуемым результатом.

**Состав:**

- `MissionBenchmark` / `ScenarioSuite` trait — единый интерфейс для reference миссий.
- Подключить все три текущие миссии:
  - Coverage (уже работает в strategy_comparison);
  - EmergencyMesh;
  - SAR.
- Расширить CLI `strategy_comparison`:
  `--mission coverage`, `--mission emergency-mesh`, `--mission sar`, `--mission all`.
- Добавить SAR profiles: target_count, scout/thermal/relay mix, packet_loss,
  battery constraints, grid size.
- Единый output: JSON, CSV, replay logs, stable `run_id` со scheme
  `{mission}_{strategy}_{profile}_{seed}`.
- Все 5 стратегий на каждой миссии: greedy, auction, connectivity-aware,
  centralized, cbba.
- Invariant checks (property-based):
  - no duplicate task ownership;
  - success_rate ∈ [0, 1];
  - no NaN в метриках;
  - `cbba_converged` всегда заполнен при strategy=cbba;
  - replay восстанавливает финальный assignment state.
- Proptest для CBBA: 500+ случайных топологий (agents × tasks × packet_loss),
  CBBA не паникует, `cbba_converged` в допустимом диапазоне.
- 1000-seed прогон: 5 стратегий × 3 миссии × 3 network profiles = 45 000 прогонов.
- README: числовая таблица из реального прогона — ключевые метрики по стратегиям
  и миссиям.

**Критерий готовности:**

1. `cargo run --bin strategy_comparison -- --mission all --csv /tmp/results.csv`
   выполняется без ошибок, CSV содержит строки для всех 15 комбинаций
   (5 стратегий × 3 миссии).
2. CBBA агенты обмениваются bid-ами через `RuntimeMessage::Cbba` в каждом тике;
   `apply_remote_bids()` получает реальные данные от соседей.
3. Proptest CBBA: 500+ случаев без паники.
4. README содержит числовую таблицу из реального прогона.
5. Все существующие 167+ тестов проходят.

---

## Roadmap после Milestone 11

### Milestone 12 — Mission DSL

Ввести декларативное описание сценариев (YAML / RON / JSON) после того, как станет
понятно, какие поля реально нужны для трёх reference missions. Сейчас преждевременно —
до Milestone 11 поля сценариев продолжают меняться.

### Milestone 13 — Uncertainty Map / Sensor Model v2

Углубить SAR:

- повторные scans с confidence decay;
- confidence map поверх grid;
- false positives;
- target belief state.

Это закрывает gap "уровень D симуляции" из DRONE_A.3.

### Milestone 14 — Infrastructure Inspection

Новая reference mission. Хорошо проверяет:

- kinematics и battery drain по маршруту;
- route coverage и missed segments;
- повторяемость маршрутов при разных стратегиях.

Предпочтительнее Wildfire как следующий шаг: более структурированная задача,
лучше поддаётся количественному сравнению стратегий.

### Milestone 15 — Safety Layer

Добавить:

- geofence и no-fly cells;
- separation constraints;
- collision avoidance-lite.

Это потребует изменений в `Allocator` (учёт пространственных ограничений)
и в movement model (скоростные/угловые ограничения).

---

## Что откладывается

**TSP-ordering в bundles** — нужны данные из Milestone 11, чтобы понять, насколько
упрощённый position penalty отличается от TSP на реальных SAR прогонах.

**CBBA retransmission** — specialised retransmission при message loss > 30%. После
Milestone 11 будет видно, при каком уровне потерь CBBA деградирует.

**PX4 / MAVLink / zenoh / visualization** — за горизонтом текущего плана.
