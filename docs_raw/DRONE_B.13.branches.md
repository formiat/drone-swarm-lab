# DRONE_B.13 — Ветки дальнейшего развития

Дата фиксации: 2026-05-23

Синтез DRONE_A.12 и DRONE_B.12.

---

## Ветка 1 — Algorithm / Mission Correctness

### Суть

Закрыть самые заметные слабые места текущих стратегий на уже существующих миссиях.

### Проблемы

- CBBA и centralized дают 0% success на SAR grid tasks. Причина неизвестна — либо
  алгоритм не умеет работать с `grid_cell`-задачами, либо scoring/completion
  условия не срабатывают корректно.
- CBBA на inspection perimeter: 0% success при `edge_coverage=0.795`. Алгоритм
  что-то делает, но считается что не справился.
- `success=0.0` при `completion=1.0` и `edge_coverage=1.0` в inspection — метрика
  success определена некорректно или слишком жёстко.

### Что сделать

1. Разобраться с CBBA / centralized на SAR: пройти по пути task → scorer → allocator →
   runner → completion check, найти где теряется `grid_cell`.
2. Исправить обработку SAR grid tasks в allocator scoring.
3. Разобраться с `success` метрикой в inspection — почему 0 при полном покрытии.
4. Исправить или задокументировать CBBA на inspection perimeter.
5. Добавить strategy support matrix:
   - что expected stable;
   - что experimental;
   - что intentionally unsupported с причиной.
6. Добавить тесты на проблемные mission-strategy пары.

### Зачем

Сейчас benchmark сравнивает стратегии, но часть строк методологически нечистая:
алгоритм может давать 0% не потому что плохой, а потому что неправильно интегрирован
с миссией. Без этого любой следующий шаг строится на ненадёжной базе.

### Где пригодится

- SAR и inspection benchmark.
- CBBA research.
- Real-world/SITL — слабые стратегии нельзя переносить на реальный контур.

### Зависимости

Не зависит ни от чего. Желательна перед любым следующим шагом.

---

## Ветка 2 — Mission Semantics Layer

### Суть

Перестать трактовать все задачи как одинаковые allocation tasks. Сейчас проект видит
разные смыслы задач одинаково:

- coverage cell;
- SAR scan cell;
- confirmation scan;
- inspection edge;
- relay placement;
- waypoint.

Часть алгоритмических проблем — не плохой алгоритм, а алгоритм не понимает тип задачи.

### Что сделать

1. Ввести `TaskKind` enum:
   - `CoverageCell`;
   - `SarScan`;
   - `SarConfirmationScan`;
   - `InspectionEdge`;
   - `RelayPlacement`;
   - `Waypoint`.

2. Сделать adapters: mission task → allocation cost, mission task → completion condition,
   mission task → route cost.

3. Добавить scoring hooks для разных миссий.

4. Уточнить validation:
   - SAR task → `grid_cell` обязателен;
   - inspection task → `edge_id` обязателен;
   - waypoint task → `pose` обязателен;
   - relay task → role/capability requirements.

5. Обновить стратегии чтобы они не игнорировали mission-specific fields.

6. Обновить DSL validation: несоответствие `TaskKind` и полей — ошибка при загрузке.

### Зачем

Придаёт mission-specific смысл тому, что сейчас является generic allocation. Делает
алгоритмы более корректными без переписывания их ядра.

### Где пригодится

- Все существующие миссии.
- Новые миссии (Ветка 5) будет проще добавлять.
- SITL: mission semantics важны для корректного перевода задач в waypoints.

### Зависимости

Лучше после Ветки 1 (сначала понять где ломается, потом вводить слой абстракции).

---

## Ветка 3 — Planner Quality Upgrade

### Суть

Улучшить планирование маршрутов и bundles. Сейчас используется жадный nearest-neighbour
TSP — простой, но далёкий от оптимального.

### Что сделать

1. Реализовать 2-opt для inspection и SAR bundles.
2. Учитывать battery при route planning:
   - return-to-base reserve;
   - max range;
   - route feasibility check (агент физически может выполнить bundle?).
3. Сделать `RouteCost` общей функцией с поддержкой mission-specific весов.
4. Сравнить результаты: greedy TSP vs 2-opt vs mission-specific cost.
5. Добавить метрики:
   - route length;
   - wasted travel (лишний путь без полезной работы);
   - return reserve при финише;
   - infeasible route count.

### Зачем

Алгоритм coordination может быть хорошим, но если planning маршрутов плохой —
результат всё равно слабый. 2-opt даёт заметное улучшение на inspection и SAR
за небольшую сложность.

### Где пригодится

- Inspection: прямое влияние на route efficiency.
- SAR: порядок сканирования влияет на time-to-find.
- Battery-constrained сценарии.

### Зависимости

Лучше после Ветки 2 (Mission Semantics) — иначе оптимизируем неправильную модель.

---

## Ветка 4 — Simulation Realism Foundation

### Суть

Сделать симуляцию ближе к реальным миссиям — не полный физический движок, а
mission-level realism: высота, реалистичная батарея, sensor FoV, noise.

### Что сделать

1. **3D pose / altitude:**
   - `Pose3` или расширение `Pose` полем `z: f64`;
   - compatibility layer для старых 2D сценариев (z = 0.0).

2. **Battery model v2:**
   - hover cost (агент висит на месте);
   - climb/descend cost;
   - cruise cost (уже есть, уточнить);
   - payload coefficient;
   - return-to-base reserve: агент не берёт задачу, если не хватит батареи вернуться.

3. **Sensor model v3:**
   - range (радиус сканирования);
   - field-of-view (угол);
   - altitude-dependent detection probability (выше → хуже);
   - обновить SAR BeliefMap под новую модель.

4. **Environment noise:**
   - wind (drift в Pose);
   - GPS/pose noise (небольшая случайная ошибка позиции);
   - communication jitter (задержка варьируется).

5. **Dynamic obstacles:**
   - простые blocked regions;
   - time-varying no-fly zones (зона появляется / исчезает по тику).

### Зачем

Повышает ценность симулятора как digital twin. Позволяет сравнивать алгоритмы
в более реалистичных условиях. Результаты становятся менее игрушечными.

### Риск

Резко увеличивает сложность сценариев и вероятность регрессий. Без Ветки 1/2
(correctness / semantics) можно получить более реалистичную, но всё ещё неверную систему.

### Зависимости

Лучше после Веток 1 и 2. Ветка 3 (planner) выигрывает от battery model v2 —
хорошо делать параллельно или сразу после.

---

## Ветка 5 — New Mission

### Суть

Добавить принципиально новый класс миссий — с другой механикой, не просто вариацией
Coverage/SAR/Inspection.

### Кандидаты

**Wildfire / Flood Mapping:**
- карта угрозы динамически меняется по тикам (огонь распространяется);
- агенты перераспределяются по мере изменения угрозы;
- задачи появляются и исчезают;
- метрики: threat coverage, latency of detection, area-under-threat missed.

**Multi-target Pursuit:**
- цели движутся по заданным траекториям или убегают;
- агенты перехватывают;
- задачи: догнать и сопровождать, а не просто посетить точку;
- метрики: capture rate, time-to-intercept, total pursuit distance.

**Logistics / Delivery:**
- задачи с pickup + dropoff;
- зависимости между задачами (нельзя доставить не забрав);
- depot как база;
- метрики: delivery rate, late deliveries, total route cost.

### Зачем

Существующие миссии — Coverage, SAR, Inspection — все про "посети точки/ячейки/рёбра".
Новый класс миссий (динамические цели, зависимые задачи) раскрывает другие грани
coordination алгоритмов.

### Зависимости

Лучше после Ветки 2 (Mission Semantics) — новая миссия будет строиться на `TaskKind`
и mission adapters, что сильно упрощает добавление.

---

## Ветка 6 — Real-World / SITL Bridge

### Суть

Двигать PX4 / MAVLink дальше scaffold/mock. Сейчас `--connection` парсится но
не используется; реальный PX4 loop не проверен.

### Что сделать

1. Подключить настоящий `MavlinkTransport` в `sitl_agent` когда `--connection` задан.
2. Реализовать mission item upload в PX4 SITL (MISSION_COUNT, MISSION_ITEM_INT,
   MISSION_REQUEST, MISSION_ACK).
3. Telemetry → `TaskStatus`: HEARTBEAT → InProgress, MISSION_ITEM_REACHED → Completed.
4. Arm / takeoff / execute sequence.
5. Single-agent SITL end-to-end: один агент пролетает по waypoints из coverage/inspection.
6. Multi-agent SITL: N агентов с отдельными UDP портами, coordinator в-memory.
7. Safety enforcement перед upload: задачи проходят `filter_safe_tasks`.
8. Failure handling: PX4 не ответил → `TaskStatus::Failed`, реаллокация.

### Зачем

Закрывает единственный пункт M17 который пока реализован только на mock. Доказывает,
что transport abstraction работает на реальном autopilot стеке.

### Риск

Самая дорогая и хрупкая ветка. Требует внешнего окружения (PX4 + Gazebo). Без
сильного mission correctness и safety интеграции — демонстрация без надёжности.

### Зависимости

Лучше после Веток 1 и 2. Safety Layer должен быть интегрирован в аллокаторы.

---

## Ветка 7 — Visualization / Operator Tooling

### Суть

Видеть миссию глазами. Сейчас есть только ASCII replay CLI.

### Что сделать

**Минимальный вариант (egui):**
- интерактивный replay с паузой/перемоткой;
- grid/map view с позициями агентов;
- BeliefMap overlay (SAR: posterior по ячейкам);
- InspectionGraph overlay (покрытые/непокрытые рёбра);
- timeline событий (assignment, failure, conflict, convergence);
- side-by-side сравнение двух прогонов.

**Расширенный вариант (Bevy):**
- 3D view (после Ветки 4);
- real-time monitoring во время прогона;
- flight path visualization;
- CBBA convergence state per agent.

### Зачем

- Debugging: странный benchmark outcome виден сразу.
- Демонстрация: поведение алгоритмов наглядно, не только в числах.
- Разработка: новые сценарии и стратегии легче проектировать.

### Зависимости

Независима от остальных. Полезнее после стабилизации replay schema (Ветка 1/2).
3D визуализация имеет смысл после Ветки 4.

---

## Ветка 8 — Research Benchmark Depth

### Суть

Не новые фичи, а серьёзное исследование на текущей базе.

### Что сделать

1. Прогнать `--full` (1000 seeds) по всем миссиям.
2. Добавить confidence intervals к метрикам.
3. Degradation curves: как метрики меняются при увеличении packet loss / числа агентов /
   размера сетки.
4. Benchmark regression thresholds (Ветка M28 из линейного плана).
5. Подробный анализ: где CBBA выигрывает, где проигрывает, почему.
6. Reproducible result packs с manifest + command line.
7. `docs/BENCHMARK_RESULTS.md` с реальными числами и методологическими выводами.

### Зачем

Превращает платформу в доказательный research artifact. Показывает не "вот что умеет",
а "вот что измерено и понято".

### Риск

Смыкается с публикационной стадией. Если пока не хотим думать о публикации —
можно отложить. Но 1000-seed runs полезны и как инженерный контроль.

### Зависимости

Лучше после Ветки 1 (correctness) — иначе 1000-seed анализ будет показывать
методологически нечистые результаты.

---

## Ветка 9 — Platform / API Extensibility

### Суть

Сделать систему расширяемой для новых стратегий и миссий без правки ядра.

### Что сделать

1. Plugin-like strategy registration: новая стратегия добавляется без изменения
   `StrategyRegistry` и `strategy_comparison`.
2. Stable internal APIs для allocator/scorer/runner.
3. Scenario generators: API для создания новых сценариев без копипаста существующих
   builders.
4. Documented extension points: как добавить стратегию, как добавить миссию, как
   добавить метрику.
5. External strategy harness: возможность подключить стратегию из отдельного крейта.

### Зачем

Снижает стоимость добавления новых миссий и алгоритмов. Полезно если проектом
будут пользоваться другие разработчики или если хочется экспериментировать быстрее.

### Риск

Может превратиться в абстрактное платформостроение. Лучше делать после того как
mission semantics и correctness ясны — иначе стабилизируем нестабильный API.

### Зависимости

После Веток 1 и 2.

---

## Совместимость и зависимости между ветками

```
Ветка 1 (Correctness)     — независима, желательна первой
    ↓
Ветка 2 (Semantics)       — после 1, база для 3/5/9
    ↓              ↓
Ветка 3 (Planner)   Ветка 5 (New Mission)
    ↓
Ветка 4 (Realism)         — независима, лучше после 1/2
Ветка 6 (SITL)            — после 1/2, нужен внешний PX4
Ветка 7 (Visualization)   — независима в любой момент
Ветка 8 (Benchmark Depth) — после 1 (correctness обязательна)
Ветка 9 (Extensibility)   — после 1/2
```
