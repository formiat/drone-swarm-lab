# DRONE_B.9 — Статус после DRONE_B.8 и новая развилка

Дата фиксации: 2026-05-23

## Что реализовано

Весь DRONE_B.8 roadmap пройден полностью.

| Stage | Версия | Содержание |
|-------|--------|------------|
| M11 Hardening | v0.11 | mission/scenario в JSON/CSV, mission-aware benchmark_run_id, proptest CBBA, README с реальными числами |
| Mission DSL | v0.12 | ScenarioSuite, load_scenario_suite, --scenario-suite, примеры в scenarios/, 10 unit-тестов |
| Safety Layer | v0.13 | крейт swarm-safety: Geofence, NoFlyZone, SeparationConstraint, check_agent, filter_safe_tasks |
| SAR v2 / Uncertainty Map | M14 | BeliefMap, Bayes-обновление posterior, repeated scans, SAR v2 метрики в экспорте |
| CBBA Robustness | v0.15 | TSP-ordering в bundle, retransmission policy, convergence distribution (p50/p95/max), proptest |
| Infrastructure Inspection | M16 | inspection.rs, edge.rs, InspectionGraph, EdgeTask, метрики покрытия рёбер |
| SITL / MAVLink | M17 | mavlink.rs, sitl_agent binary, MockMavlinkTransport, MavlinkTransport за feature flag |

## Текущая развилка

Roadmap завершён. Следующее направление не определено — открытая развилка.

### Ветка 1 — Интеграция и доводка

Всё реализовано по отдельности, но части слабо связаны между собой.

Что недоделано:

- Safety Layer существует как standalone крейт, но `filter_safe_tasks` не вызывается
  внутри аллокаторов — интеграция в Greedy/CBBA/Auction/Centralized отсутствует.
- MAVLink транспорт есть за feature flag, но реального end-to-end SITL прогона
  (Task → Waypoint → MAVLink → PX4) не было.
- Infrastructure Inspection benchmark сценарии в `scenarios/` не добавлены.

Зачем: довести существующую работу до состояния, в котором каждая часть
действительно работает в связке с остальными, а не только в изоляции.

### Ветка 2 — Visualization / Replay UI

Единственный пункт из исходного roadmap, который не реализован.

Сейчас вся аналитика — CSV/JSON/Markdown таблицы. Поведение алгоритмов не видно глазами.

Минимальный вариант: CLI-утилита `replay` с ASCII-визуализацией grid по тикам из JSONL лога.

Полноценный вариант: egui или Bevy — интерактивный просмотр с BeliefMap,
InspectionGraph, позициями агентов, историей назначений.

Зачем: debugging, демонстрация, сравнение стратегий визуально, а не только числами.

### Ветка 3 — Новый алгоритмический слой

Варианты:

- Multi-robot path planning с учётом Safety Layer (геометрические ограничения
  влияют на маршруты, а не только на аллокацию).
- Динамическое перераспределение при отказах агентов поверх CBBA (агент упал —
  его bundle переаллоцируется без полного перезапуска консенсуса).
- Оптимизация маршрутов для Infrastructure Inspection (сейчас жадный TSP,
  можно 2-opt или OR-Tools для сравнения).
- Многоуровневая координация: группы агентов с локальным лидером и глобальным
  координатором.

Зачем: исследовательская глубина, новые публикуемые результаты.

### Ветка 4 — Реальный SITL прогон

Поднять PX4 SITL (Gazebo), запустить `sitl_agent`, убедиться что цепочка
Task → Waypoint → MAVLink → PX4 работает end-to-end на одном агенте.
Затем — multi-agent SITL.

Зачем: закрыть единственный пункт M17, который пока реализован только на уровне
MockMavlink без реального автопилота.

Ограничение: требует установки PX4 и Gazebo.

## Рекомендация

Наиболее логичный следующий шаг — **Ветка 1 (Интеграция)**, потому что:

- Safety Layer без интеграции в аллокаторы не даёт реального эффекта на аллокацию.
- SITL без реального прогона — незакрытый M17.
- Ветки 2, 3, 4 опираются на стабильную связку существующих частей.

После интеграции — выбор между Visualization (быстрый видимый результат)
и алгоритмическим слоем (исследовательская глубина).
