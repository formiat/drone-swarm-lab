# Итоговое направление после DRONE_A.2 / DRONE_B.2

> Сессия: май 2026. Синтез DRONE_A.2 и DRONE_B.2.

---

## Сравнение двух репортов

DRONE_A.2 и DRONE_B.2 совпадают в главном, расходятся в трёх точках.

**Следующий шаг — совпадение:** оба называют Replay первым следующим milestone.

**Порядок после Replay — расхождение:**
- DRONE_A.2: Replay → Mission DSL → SAR → CBBA. Логика: сначала фундамент
  (воспроизводимость + декларативные сценарии), потом содержание.
- DRONE_B.2: Replay+proptest → Kinematic+SAR → Sensor model. Логика: быстрее
  закрыть gap в "критерии не-песочницы", DSL пропускается.

**Что пропущено в каждом:**
- DRONE_A.2 не упоминает proptest — прямой пункт из "критерия не-песочницы" в
  DRONE_A.1.
- DRONE_B.2 не упоминает CBBA — один из ключевых алгоритмов из DRONE_B.1, который
  отличает "настоящий распределённый алгоритм" от простого greedy/auction.
- DRONE_B.2 не упоминает Mission DSL — некритично, но без него сценарии навсегда
  остаются hard-coded Rust-кодом.

---

## Итоговое направление

Берём DRONE_A.2 как более методичный, вставляем пропущенные детали.

### Milestone 7 — Replay + proptest + export

- Event log + deterministic replay (`swarm-replay` из placeholder → рабочий).
- `proptest`: 1000+ случайных комбинаций отказов / packet_loss / latency.
- JSON/CSV export для `ComparisonReport`.

Закрывает единственный открытый пункт критерия из DRONE_A.1 ("property-based tests")
и даёт инфраструктуру для серьёзного анализа. После этого любой новый алгоритм
автоматически тестируется на случайных сценариях, не только на hand-crafted.

### Milestone 8 — SAR + kinematic model

- Движение `position += velocity * dt`, расход батареи по расстоянию.
- Миссия SAR: grid, скрытые цели, probabilistic detection, роли (scout/thermal/relay).
- Метрики: `probability_of_detection`, `time_to_find`, `coverage_over_time`.

SAR — первый настоящий benchmark из roadmap, который нельзя "сломать" trivially и
который является содержательным исследовательским вопросом.

### Milestone 9 — CBBA

- Consensus-Based Bundle Algorithm как отдельная стратегия в `swarm-alloc`.
- Сравнение на SAR + EmergencyMesh: Greedy vs Auction vs CBBA vs Centralized.
- Единственный из 4 алгоритмов, который реально "распределённый" — остальные три
  принимают решение локально на одном агенте.

После Milestone 9 у проекта появляется публикуемый результат: сравнение 4 стратегий
на 2 reference missions с 1000 seed × нескольких network profiles × property-based
тестами. Это то, что DRONE_A.1 называет "это не песочница".

---

## Что откладывается

**Mission DSL (YAML/RON)** — полезно, но не меняет исследовательскую ценность.
Текущий подход (сценарии в Rust-коде) достаточно удобен для разработчика.

**Sensor model / uncertainty map** — входит в Milestone 8 через probabilistic
detection в SAR, но полноценный "уровень D" симуляции откладывается на после CBBA.

**PX4 / Visualization** — за горизонтом текущего плана.
