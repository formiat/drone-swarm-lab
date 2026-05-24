# DRONE_B.13 — Линейный план без выбора стратегического направления

Дата фиксации: 2026-05-23

Синтез DRONE_A.12 и DRONE_B.12.

## Идея

Не добавлять новые крупные фичи, а укрепить техническое ядро до состояния,
когда любой следующий стратегический выбор будет опираться на чистую базу.

План direction-agnostic: полезен для всех веток из DRONE_B.13.branches.md.

## Последовательность

```
M25 Benchmark Parallelization (rayon)
M26 Mission / Strategy Correctness
M27 Mission Semantics Layer
M28 Planner Quality Upgrade
M29 Stress & Regression Harness
M30 New Mission Prototype
M31 Simulation Realism Foundation
M32 Decision Point
```

---

## M25 — Benchmark Parallelization (rayon)

### Цель

Параллелизовать seed loop в benchmark runner через rayon. Без этого full run
(1000 seeds) занимает ~90 минут на одну миссию — M29 (Regression Harness)
практически нежизнеспособен как CI-friendly инструмент.

### Контекст

Текущие замеры на Ryzen 9 5900HX:
- quick run (10 seeds, SAR): ~54 секунды (single-threaded)
- один прогон: ~0.5 секунды
- full run (1000 seeds, SAR, single-threaded): ~90 минут
- full run с rayon (16 ядер): ~6 минут

### Что сделать

1. Добавить `rayon` в `swarm-sim/Cargo.toml`.
2. Заменить seed loop с `for seed in seeds` на `seeds.par_iter()` в benchmark runner.
3. Убедиться что `ScenarioRunner` и все вызываемые структуры `Send + Sync`
   (или клонируются per-thread).
4. Добавить `--jobs N` флаг в `strategy_comparison` для ограничения параллелизма
   (полезно при запуске на слабом железе или в CI).
5. Проверить что результаты детерминированы: одинаковые seeds дают одинаковые числа
   при любом числе потоков.

### Done criteria

- `--full --mission sar` завершается за < 10 минут.
- Результаты совпадают с single-threaded прогоном (детерминизм по seed).
- `--jobs 1` восстанавливает однопоточный режим.

### Тестовая стратегия

Категория 1:
- Детерминизм: запустить quick с `--jobs 1` и `--jobs 4` на одних seeds —
  результаты совпадают.

---

## M26 — Mission / Strategy Correctness


### Цель

Закрыть самые заметные слабые места текущих стратегий на существующих миссиях.

### Что сделать

**1. CBBA / centralized на SAR grid tasks (0% success):**

- Пройти по пути: task builder → scorer → allocator → runner → completion check.
- Найти где теряется `grid_cell`: либо алгоритм не учитывает `grid_cell` при scoring,
  либо completion condition не срабатывает для grid-based задач.
- Исправить scoring/completion или явно задокументировать как unsupported с причиной.

**2. success=0.0 при edge_coverage=1.0 в inspection:**

- Проверить как `success` определяется для inspection runner.
- Если success требует completion всех задач, а задачи с `edge_id` не считаются
  completed — исправить condition.
- Убедиться что `success=1.0` при `edge_coverage=1.0` и `completion=1.0`.

**3. CBBA на inspection perimeter (0% success, coverage=0.795):**

- Выяснить: CBBA не назначает часть рёбер (allocation gap) или назначает но агент
  не успевает (battery/time)?
- Проверить convergence metrics и conflicting_assignments для perimeter профиля.
- Если это battery constraint — документировать.
- Если это allocation gap — исправить scoring или добавить fallback.

**4. Strategy support matrix:**

Добавить в README и docs:

| Mission | Strategy | Status | Notes |
|---------|----------|--------|-------|
| coverage | all | stable | — |
| sar | greedy, auction, connectivity-aware | stable | — |
| sar | cbba, centralized | unsupported | grid_cell handling |
| inspection (linear/random) | all | stable | — |
| inspection (perimeter) | greedy | experimental | battery constraint |
| inspection (perimeter) | cbba | unsupported | allocation gap |

**5. Тесты:**

- SAR + CBBA: тест явно проверяет что либо success > 0 (после исправления), либо
  сценарий корректно отклоняется/помечается.
- SAR + centralized: аналогично.
- Inspection + success metric: тест что success=1.0 при edge_coverage=1.0.
- Inspection perimeter + CBBA: тест с задокументированным ожидаемым поведением.

### Done criteria

- Нет строк в benchmark с "0% success без ясной причины".
- Support matrix задокументирована.
- Все тесты проходят.
- BENCHMARK_RESULTS.md обновлён с корректными данными.

### Тестовая стратегия

Категория 1 (без рефакторинга):
- Тесты на SAR + CBBA/centralized.
- Тест на inspection success metric.
- Support matrix как doc-тест или snapshot-тест.

Категория 2 (лёгкий рефакторинг):
- Параметрический тест: все mission-strategy пары из support matrix.

---

## M27 — Mission Semantics Layer

### Цель

Ввести явные типы задач вместо generic allocation tasks. Убрать корень части
алгоритмических проблем M25 — алгоритмы не понимают тип задачи.

### Что сделать

**1. `TaskKind` enum в `swarm-types`:**

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    CoverageCell,
    SarScan,
    SarConfirmationScan,
    InspectionEdge,
    RelayPlacement,
    Waypoint,
}
```

Добавить `kind: Option<TaskKind>` в `Task` (None = legacy / unknown).

**2. Mission adapters:**

```rust
pub trait MissionAdapter {
    fn task_kind(&self, task: &Task) -> TaskKind;
    fn route_cost(&self, from: Pose, task: &Task) -> f64;
    fn is_completed(&self, task: &Task, state: &RunState) -> bool;
    fn score(&self, agent: &Agent, task: &Task) -> f64;
}
```

Реализации: `CoverageAdapter`, `SarAdapter`, `InspectionAdapter`, `WaypointAdapter`.

**3. Scoring через adapter:**

Все аллокаторы получают `&dyn MissionAdapter` как контекст. Scoring и route cost
делегируются адаптеру — не hardcoded в allocator.

**4. Validation:**

В DSL loader: проверять что `TaskKind` соответствует полям:
- `SarScan` → `grid_cell` обязателен;
- `InspectionEdge` → `edge_id` обязателен;
- `Waypoint` → `pose` обязателен.

**5. Обновить существующих builderов сценариев:**

- `build_sar_scenario` → `TaskKind::SarScan` / `SarConfirmationScan`.
- `build_inspection_scenario` → `TaskKind::InspectionEdge`.
- `build_coverage_scenario` → `TaskKind::CoverageCell`.

### Done criteria

- `TaskKind` существует и используется во всех сценариях.
- Allocator scoring идёт через adapter, не через hardcoded field checks.
- Validation ловит mismatch task kind / fields.
- Regression тесты покрывают SAR/inspection/coverage/waypoint task kinds.
- Старые сценарии (без `kind`) десериализуются без ошибок (backward compat).

### Тестовая стратегия

Категория 1:
- Unit тесты каждого adapter: route_cost, is_completed, score.
- Validation тест: SarScan без grid_cell → ошибка загрузки.
- Roundtrip serde для `TaskKind`.

Категория 2:
- Параметрический тест: все адаптеры на случайных задачах не паникуют.

Категория 3:
- Property-based: adapter score всегда finite, route_cost ≥ 0.

---

## M28 — Planner Quality Upgrade

### Цель

Улучшить route ordering для bundles — перейти от жадного nearest-neighbour TSP к
2-opt с учётом battery.

### Что сделать

**1. Выделить `RoutePlanner` trait:**

```rust
pub trait RoutePlanner {
    fn order(&self, start: Pose, tasks: &[Task], agent: &Agent) -> Vec<TaskId>;
    fn is_feasible(&self, start: Pose, tasks: &[Task], agent: &Agent) -> bool;
}
```

Реализации:
- `NearestNeighbourPlanner` (текущий);
- `TwoOptPlanner`;
- `BatteryAwarePlanner` (отказывается от задач при нехватке батареи).

**2. 2-opt для inspection и SAR:**

- Реализовать 2-opt swap: O(n²) итераций, остановка при отсутствии улучшения.
- Benchmark: среднее улучшение route length vs nearest-neighbour на inspection linear.

**3. Battery-aware feasibility:**

- Перед добавлением задачи в bundle: проверить что агент вернётся на базу с резервом.
- `reserve_fraction: f64` — минимальный % батареи для возврата (default 0.2).
- Если bundle нефeasible — отказаться от последней задачи.

**4. `RouteCost` как общая функция:**

Вынести из allocators в `swarm-alloc` или `swarm-sim`:

```rust
pub fn route_cost(start: Pose, tasks: &[Task], agent: &Agent) -> f64;
```

Используется одинаково во всех аллокаторах.

**5. Новые метрики:**

- `avg_route_length` — суммарный путь агента;
- `avg_wasted_travel` — путь без полезной работы;
- `avg_return_reserve` — остаток батареи при возврате;
- `avg_infeasible_routes` — сколько раз bundle был отклонён как нефeasible.

**6. Benchmark comparison:**

Запустить inspection linear с NN vs 2-opt — показать разницу в route length и efficiency.

### Done criteria

- `RoutePlanner` trait существует с двумя реализациями.
- 2-opt даёт измеримое снижение route length на inspection.
- Battery-aware feasibility предотвращает исчерпание батареи на constrained сценариях.
- Новые метрики в экспорте.

### Тестовая стратегия

Категория 1:
- Unit: 2-opt не ухудшает маршрут (итоговая длина ≤ исходной).
- Unit: battery-aware — агент с малой батареей не берёт нефeasible bundle.
- Integration: inspection linear, 2-opt vs NN — 2-opt route_length ≤ NN.

Категория 2:
- Proptest: случайные задачи, случайный агент — 2-opt не паникует, возвращает permutation.

---

## M29 — Stress & Regression Harness

### Цель

Превратить benchmark в инженерный контроль качества — защиту от деградаций.

### Что сделать

**1. Regression suites:**

Набор фиксированных smoke-runs для каждой миссии:
- SAR ideal/standard;
- inspection linear/perimeter;
- CBBA stress pl-0.0/pl-0.2;
- safety coverage;
- emergency mesh.

**2. Thresholds:**

Для каждого suite — минимальные допустимые значения:

```toml
[sar.ideal.greedy]
min_success_rate = 0.7
max_avg_pod = 0.5        # pod должен быть значимым
max_belief_entropy = 0.5

[inspection.linear.all]
min_edge_coverage = 0.95
min_success_rate = 0.9

[cbba_stress.pl_0_2.cbba]
max_convergence_p95 = 15
max_safety_violations = 0
```

**3. Stress profiles:**

Дополнительные параметрические тесты:
- packet loss 0.0 → 0.5 с шагом 0.1;
- число агентов 2 → 10;
- размер сетки 4×4 → 12×12;
- noisy sensors (false positive rate 0.0 → 0.5).

**4. Baseline artifact:**

- `results/baseline/` — checked-in summary с референсными числами;
- команда `compare-baseline` сравнивает текущий прогон с baseline;
- вывод: что деградировало / что улучшилось / что стабильно.

**5. CI-friendly subset:**

- `--smoke` режим (1 seed) — работает в CI за < 30 секунд;
- `--regression` режим — все suites, thresholds checked, ~2 минуты.

### Done criteria

- `cargo run --bin strategy_comparison -- --regression` завершается с кодом 0 если все
  thresholds выполнены, ненулевым если есть деградация.
- Baseline artifact checked in.
- Документация: как обновить baseline.

### Тестовая стратегия

Категория 1:
- Smoke: каждый regression suite запускается без ошибок.
- Threshold check: известно что greedy на SAR ideal даёт success > 0.7.

Категория 2:
- Parametric stress: packet loss sweep — convergence p95 монотонно растёт.

---

## M30 — New Mission Prototype

### Цель

Проверить, что система реально расширяется на новую миссию, а не только поддерживает
уже реализованные сценарии.

### Рекомендованная миссия

> Wildfire / flood mapping.

Почему не pursuit/logistics первым:

- wildfire/flood ближе к текущим SAR/coverage primitives;
- можно переиспользовать BeliefMap-like model;
- естественно появляются risk zones и changing priorities;
- хорошо проверяются DSL, semantics, allocation, safety и replay;
- не нужно сразу вводить сложную динамику moving targets или pickup/dropoff dependencies.

### Что сделать

**1. Domain model:**

- hazard map (grid или polygon zones);
- changing threat level (per-zone, per-tick);
- priority zones — влияют на scoring в аллокаторе;
- detection/update events — агент обнаруживает изменение и обновляет hazard map.

**2. DSL:**

```rust
pub struct WildfireScenario {
    pub hazard_map: Vec<HazardZone>,
    pub update_interval_ticks: u64,
    pub task_generation: WildfireTaskParams,
}

pub struct HazardZone {
    pub bounds: Aabb,
    pub threat_level: f64,
    pub priority: u8,
}
```

Backward compat: новые поля за пределами `MissionKind` — не ломают старые сценарии.

**3. Allocation:**

- mapping tasks генерируются из hazard map;
- re-prioritization при получении detection/update event;
- совместимость с `greedy` и `auction` стратегиями;
- явный `unsupported` маркер для стратегий, которые не поддерживают динамические приоритеты.

**4. Replay:**

- события обновления hazard map;
- agent observation events;
- updated task priorities в replay stream.

**5. Benchmark:**

- small scenario (4×4, 2 агента, статичный hazard map);
- medium scenario (8×8, 4 агента, 2 update события);
- baseline comparison между `greedy`, `auction`, `cbba` на small scenario.

### Done criteria

- Новая миссия описывается через DSL без изменений ядра.
- Минимум два сценария.
- Benchmark запускается хотя бы для stable стратегий.
- Replay содержит достаточно событий для анализа.
- Docs объясняют semantics и ограничения.

### Тестовая стратегия

Категория 1 (без рефакторинга):
- DSL parse/validation тесты для wildfire/flood сценария.
- Task generation тесты: из hazard_map → TaskList.
- Completion semantics: все mapping tasks completed → mission success.
- Replay event serialization тесты.
- Benchmark smoke test для small scenario.

Категория 2 (лёгкий рефакторинг):
- Hazard map builders для test fixtures.
- Fake observation/update event helpers.
- Reusable mission benchmark fixtures.
- Assertions для priority updates.

Категория 3 (тяжёлый рефакторинг):
- Dynamic re-prioritization property tests.
- Multi-seed mission stability tests.
- Comparative тесты по всем стратегиям.

---

## M31 — Simulation Realism Foundation

### Цель

Добавить mission-level realism без ухода в полный физический движок.

### Что сделать

**1. 3D pose / altitude:**

```rust
pub struct Pose3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
```

Опция: расширить `Pose` полем `z: f64` с `#[serde(default)]` для backward compat.
Все старые 2D сценарии работают с z = 0.0.

**2. Battery model v2:**

```rust
pub struct BatteryModel {
    pub hover_drain_per_tick: f64,     // % за тик на месте
    pub climb_drain_per_meter: f64,    // % за метр вертикального подъёма
    pub cruise_drain_per_meter: f64,   // % за метр горизонтального полёта
    pub reserve_fraction: f64,         // минимум для возврата на базу
}
```

Backward compat: если `BatteryModel` не задан — старое поведение (`battery_drain_rate`).

**3. Sensor model v3:**

- `detection_range_m: f64` — радиус сканирования;
- `field_of_view_deg: f64` — угол обзора;
- `altitude_factor: f64` — коэффициент: detection prob *= (1 - altitude_factor * z);
- BeliefMap обновляется с учётом нового sensor model.

**4. Environment noise:**

- `wind: Option<(f64, f64)>` — drift в Pose за тик;
- `pose_noise_m: f64` — случайная ошибка позиции (std dev);
- `comms_jitter_ticks: u64` — задержка сообщений варьируется ±jitter.

**5. Time-varying no-fly zones:**

Расширить `NoFlyZone`:

```rust
pub struct NoFlyZone {
    pub bounds: Aabb,
    pub active_from_tick: Option<u64>,
    pub active_until_tick: Option<u64>,
}
```

Safety checker проверяет текущий тик.

**6. Backward compat:**

- Все новые поля — `#[serde(default)]`.
- Старые сценарии не требуют изменений.
- Добавить migration guide в SCENARIO_DSL.md.

### Done criteria

- Все старые сценарии проходят без изменений.
- Хотя бы один сценарий показывает отличие: battery v2 vs v1, sensor v3 vs v2.
- Новые поля покрыты тестами.
- Документация обновлена.

### Тестовая стратегия

Категория 1:
- Backward compat: все `scenarios/*.json` грузятся без ошибок после изменений.
- Battery v2: агент с hover_drain не может выполнить infinite hover без разряда.
- Sensor v3: detection prob снижается с высотой.
- Time-varying no-fly: агент не входит в зону в active период.

Категория 2:
- Proptest: случайные battery model параметры — drain ∈ [0, 100] за любой маршрут.

---

## M32 — Decision Point

### Суть

Аналитический checkpoint, не обязательно кодовый milestone.

### Что оценить

После M25-M31 ответить на вопросы:

1. **Качество алгоритмов:** стали ли стратегии методологически корректными на всех
   основных mission-strategy парах?
2. **Качество симулятора:** достаточно ли реалистичны сценарии для содержательных
   выводов?
3. **Интерес:** что из нового хочется строить больше всего?

### Возможные решения

| Если... | → |
|---------|---|
| Результаты стали сильными и хочется опубликовать | Ветка 8: Research Benchmark Depth |
| Хочется новых алгоритмических задач | Ветка 5: New Mission (wildfire/pursuit/logistics) |
| Хочется внешних пользователей | Ветка 9: Platform / API Extensibility |
| Хочется видеть миссии | Ветка 7: Visualization |
| Хочется реального железа | Ветка 6: Real-World / SITL Bridge |
| Хочется более глубокой симуляции | Ветка 4: Simulation Realism (продолжение) |

M32 — это момент выбора, который сейчас можно не делать.

---

## Почему этот план не требует выбора направления

Каждый milestone из M25-M31 полезен для всех будущих веток:

- **Research (Ветка 8):** нужны correctness (M26), semantics (M27), regression (M29).
- **New Mission (Ветка 5):** нужны semantics (M27) как инфраструктура; M30 — прямая реализация.
- **SITL (Ветка 6):** нужны correctness (M26), semantics (M27), realism (M31).
- **Visualization (Ветка 7):** нужны стабильные schemas из M27.
- **Platform (Ветка 9):** нужны стабильные APIs из M27.

Можно спокойно двигаться линейно до M32, не выбирая стратегическое направление.

---

## Тестовая стратегия (сводная)

### Категория 1 — без рефакторинга

- SAR + CBBA/centralized: ожидаемое поведение (исправленное или явно unsupported).
- Inspection success metric: success=1.0 при edge_coverage=1.0.
- TaskKind roundtrip serde.
- MissionAdapter unit тесты.
- 2-opt не ухудшает маршрут.
- Battery-aware feasibility.
- Regression smoke: все suites без ошибок.
- Backward compat: все scenarios/*.json грузятся.

### Категория 2 — лёгкий рефакторинг

- Параметрический тест mission-strategy support matrix.
- Proptest: 2-opt возвращает permutation, не паникует.
- Parametric stress: packet loss sweep.
- Sensor v3 altitude dependence.

### Категория 3 — тяжёлый рефакторинг (backlog)

- Property-based: adapter score всегда finite.
- Long-running full benchmark CI (1000 seeds).
- Real PX4 integration tests.
- Hardware-in-the-loop tests.
