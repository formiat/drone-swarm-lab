# DRONE_A.7 — Сравнение DRONE_A.6 / DRONE_B.6 и итоговый набор направлений

Дата фиксации: 2026-05-18

## Короткий вывод

`docs/DRONE_A.6.md` и `docs/DRONE_B.6.md` описывают одно и то же состояние проекта, но режут дальнейший roadmap на разных уровнях.

`DRONE_B.6.md` укрупняет развилку до трёх больших стратегий:

1. исследовательская платформа;
2. путь к реальному железу;
3. алгоритмическое исследование.

`DRONE_A.6.md` раскладывает те же направления на практические workstreams:

- M11 hardening;
- Mission DSL;
- SAR v2 / Uncertainty Map;
- Infrastructure Inspection;
- Safety Layer;
- CBBA Robustness / Scaling;
- PX4 / MAVLink / zenoh / SITL;
- Visualization / Replay UI.

Противоречия между документами нет. `DRONE_B.6.md` даёт крупные типы проекта, `DRONE_A.6.md` даёт инженерную декомпозицию.

## Статус Milestone 11

`DRONE_B.6.md` формулирует статус жёстче:

> Milestone 11 полностью завершён.

Функционально это верно:

- True Distributed CBBA реализован;
- `strategy_comparison --mission all` работает;
- Coverage, EmergencyMesh и SAR подключены в один runner;
- 5 стратегий участвуют в benchmark matrix;
- JSON/CSV export есть;
- README содержит реальные benchmark-числа.

Но `DRONE_A.6.md` точнее фиксирует остаточные инженерные долги.

Milestone 11 сейчас скорее:

> feature-complete, но не fully hardened.

Оставшиеся проблемы:

- `mission` / `scenario` в JSON/CSV пока пустые;
- markdown merged report показывает `coverage` как mission/scenario для всех строк;
- `benchmark_run_id` всё ещё coverage-oriented;
- нет сильного distributed CBBA stress/proptest набора;
- README-числа взяты из quick run, а не из полного 1000-seed publishable matrix;
- `--mission all` запускается, но quick-прогон уже достаточно тяжёлый для частой проверки.

Значит перед новым крупным направлением нужен стабилизационный слой.

## Насколько отличаются предложенные варианты

По сути варианты отличаются не сильно.

Отличается уровень абстракции.

Соответствие такое:

| DRONE_B.6 | DRONE_A.6 |
|-----------|-----------|
| Исследовательская платформа | Mission DSL + SAR v2 + Infrastructure Inspection + частично Safety Layer |
| Путь к реальному железу | Safety Layer + PX4/MAVLink/zenoh/SITL |
| Алгоритмическое исследование | CBBA Robustness / Scaling + TSP-ordering + retransmission |
| Не выделено явно | Visualization / Replay UI как вспомогательная ветка |
| Не выделено явно | M11 hardening как обязательный ближайший слой |

Я бы не соглашался с формулировкой `DRONE_B.6.md`, что направления прямо "несовместимы".

Они несовместимы как главный фокус на 2-3 milestones вперёд, но не архитектурно.

Например:

- Mission DSL полезен и для research platform, и для algorithmic benchmark, и для SITL validation.
- Safety Layer нужен real-world path, но также может быть частью исследовательской платформы как constrained planning benchmark.
- CBBA robustness нужен алгоритмической ветке, но его результаты полезны и для общей benchmark-платформы.
- Visualization не определяет цель проекта, но помогает почти всем веткам как debugging/demo слой.

Поэтому лучше говорить не о взаимоисключающих дорогах, а о наборе направлений с разными приоритетами.

## Итоговый набор направлений

Я бы зафиксировал не 3 и не 7 направлений, а 5.

### 1. M11 Hardening

Ближайший обязательный слой.

Цель: довести текущий CBBA + multi-mission benchmark до аккуратного, воспроизводимого и методологически чистого состояния.

Что входит:

- заполнить `mission` и `scenario` в JSON/CSV;
- исправить markdown merged report, чтобы строки EmergencyMesh/SAR не отображались как `coverage`;
- сделать `benchmark_run_id` и `run_id` не coverage-specific для `--mission all`;
- добавить CLI/export tests для `--mission all`;
- добавить distributed CBBA stress/proptest;
- разделить быстрый smoke benchmark и полный publishable benchmark;
- обновить README после исправленного прогона.

Зачем нужно:

- текущий результат уже работает, но отчётность ещё не достаточно чистая;
- следующий roadmap должен опираться на корректные данные;
- это закрывает остаточный долг Milestone 11 без добавления новой предметной сложности.

### 2. Platformization / Mission DSL

Цель: сделать сценарии декларативными и воспроизводимыми.

Что входит:

- YAML/RON/JSON schema для миссий;
- загрузка сценариев из файлов;
- validation layer;
- сохранение config snapshot рядом с benchmark output;
- набор reference scenario configs в репозитории.

Зачем нужно:

- перестать добавлять каждый benchmark через hardcoded Rust builder;
- сделать проект удобным для внешнего пользователя;
- упростить regression suites;
- подготовить базу для новых миссий.

Где пригодится:

- research benchmark suite;
- CI;
- demo packs;
- future SITL validation scenarios.

### 3. Mission Research Depth

Цель: сделать reference missions более содержательными.

Основные ветки:

- SAR v2 / Uncertainty Map;
- Infrastructure Inspection.

SAR v2 даёт:

- confidence map;
- repeated scans;
- false positives;
- target belief state;
- richer sensor model;
- метрики качества поиска.

Infrastructure Inspection даёт:

- route coverage;
- missed segments;
- battery/kinematics stress;
- прикладной industrial use case.

Зачем нужно:

- чтобы платформа сравнивала стратегии на содержательных задачах, а не только на allocation toy problems;
- чтобы появился прикладной и исследовательский вес.

Где пригодится:

- search-and-rescue;
- environmental monitoring;
- power line / pipeline / solar farm inspection;
- route planning research.

### 4. Algorithmic Depth

Цель: глубже исследовать и улучшить алгоритмы, прежде всего CBBA.

Что входит:

- CBBA robustness / scaling;
- random topology stress;
- packet loss / partitions / healing;
- convergence time distributions;
- TSP-ordering в bundles;
- retransmission policy при высоком message loss;
- сравнение communication cost между CBBA, auction, greedy, connectivity-aware, centralized;
- 1000-seed analysis с методологическими выводами.

Зачем нужно:

- получить не просто "работает", а понимание, где и почему алгоритмы выигрывают или проигрывают;
- подготовить материал для algorithmic paper/report;
- укрепить доверие к CBBA и benchmark matrix.

Где пригодится:

- distributed systems research;
- swarm robotics research;
- академические публикации;
- tuning real-world coordination algorithms before SITL/hardware.

### 5. Real-World Bridge

Цель: построить путь от headless simulation к SITL/real robotics stack.

Правильный порядок:

1. Safety Layer.
2. Только потом PX4 / MAVLink / zenoh / SITL.

Safety Layer включает:

- geofence;
- no-fly cells;
- separation constraints;
- collision avoidance-lite;
- movement constraints;
- safety-aware allocation.

PX4/MAVLink/SITL включает:

- новую реализацию `Transport`;
- bridge между mission runtime и autopilot/control layer;
- hardware-in-the-loop или software-in-the-loop сценарий сначала на одном агенте;
- validation через уже существующие benchmark scenarios.

Зачем нужно:

- без safety layer переход к реальным дронам преждевременен;
- transport abstraction уже даёт архитектурную возможность для такого шага;
- реальные системы требуют ограничения, которых сейчас нет в достаточном виде.

Где пригодится:

- PX4 SITL;
- robotics demos;
- future hardware validation;
- middleware experiments for coordinated fleets.

## Роль Visualization / Replay UI

Visualization / Replay UI я бы не считал отдельным главным направлением уровня остальных пяти.

Это cross-cutting support workstream.

Он полезен для:

- debugging;
- презентаций;
- анализа странных benchmark outcomes;
- сравнения стратегий;
- демонстрации SAR/Infrastructure missions.

Но сам по себе он не определяет цель проекта. Его стоит делать после M11 hardening и лучше после появления стабильной report/replay schema.

## Рекомендуемый порядок

Базовый порядок:

1. M11 Hardening.
2. Platformization / Mission DSL.
3. Mission Research Depth.
4. Algorithmic Depth.
5. Real-World Bridge.

Более конкретно:

> M11 hardening → Mission DSL → SAR v2 / Uncertainty Map → CBBA robustness/scaling → Infrastructure Inspection → Safety Layer → SITL/PX4.

Если цель быстрее получить прикладную демонстрацию, можно поменять местами CBBA robustness и Infrastructure Inspection:

> M11 hardening → Mission DSL → Infrastructure Inspection → Visualization → SAR v2 → Safety Layer.

Если цель алгоритмическая публикация:

> M11 hardening → CBBA robustness/scaling → TSP-ordering → 1000-seed analysis → SAR v2.

Если цель реальные дроны:

> M11 hardening → Safety Layer → Transport abstraction hardening → PX4/MAVLink/SITL single-agent → multi-agent SITL.

## Итоговая рекомендация

Главная рекомендация:

> не выбирать сейчас "одну дорогу навсегда", а зафиксировать общий фундамент и явные ветки.

Общий фундамент:

1. M11 hardening.
2. Mission DSL.

После этого выбирать фокус:

- research depth: SAR v2 + uncertainty;
- applied mission value: Infrastructure Inspection;
- algorithmic result: CBBA robustness;
- real-world path: Safety Layer + SITL.

Мой предпочтительный маршрут:

> M11 hardening → Mission DSL → SAR v2 / Uncertainty Map → CBBA robustness/scaling → Infrastructure Inspection → Safety Layer.

Почему:

- M11 hardening делает текущий результат честным.
- Mission DSL превращает benchmark из hardcoded harness в платформу.
- SAR v2 даёт исследовательскую глубину.
- CBBA robustness превращает CBBA из "есть реализация" в "понятны пределы".
- Infrastructure Inspection даёт прикладной use case.
- Safety Layer нужен перед любым серьёзным разговором про реальные дроны.
