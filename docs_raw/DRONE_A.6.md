# DRONE_A.6 — Текущий статус после DRONE_A.5 / DRONE_B.5

Дата фиксации: 2026-05-18

## Короткий вывод

`docs/DRONE_A.5.md` и `docs/DRONE_B.5.md` предлагали один и тот же ближайший порядок:

1. сначала True Distributed CBBA;
2. затем Unified Experiment Runner + Mission Benchmark Matrix.

По текущему коду и локальным коммитам обе фазы уже в основном реализованы.

Проект сейчас находится не в состоянии "сырой демки", а в состоянии рабочей исследовательской benchmark-платформы / сильного runtime prototype. При этом это ещё не готовый продукт и не финальный publishable artifact: перед следующим большим направлением нужен короткий этап стабилизации Milestone 11.

## Что готово из запланированного

### True Distributed CBBA

CBBA-gap, описанный в `DRONE_A.5.md`, в основном закрыт.

Готово:

- `RuntimeMessage::Cbba` реально обрабатывается в `crates/swarm-runtime/src/node.rs`.
- `AgentNode::send_cbba_bids()` рассылает winning bids через transport.
- `CbbaAllocator::apply_remote_bids()` вызывается из runtime path с remote bids.
- `Allocator::is_distributed()` добавлен; `CbbaAllocator` возвращает `true`.
- `RunConfig.enable_cbba` включает distributed CBBA path на уровне сценария.
- Есть integration test `cbba_distributed_path_succeeds`, который проверяет успешный distributed CBBA run, обмен сообщениями и ненулевые rounds to convergence.

Это уже не прежний local CBBA / Greedy+ approximation. В runtime есть настоящий exchange/apply loop через CBBA-сообщения.

### Unified Experiment Runner

Вторая фаза из `DRONE_A.5.md` / `DRONE_B.5.md` тоже реализована в рабочем виде.

Готово:

- `strategy_comparison` поддерживает `--mission coverage|emergency-mesh|sar|all`.
- В один runner подключены Coverage, EmergencyMesh и SAR.
- В matrix участвуют все 5 стратегий:
  - greedy;
  - auction;
  - connectivity-aware;
  - centralized;
  - cbba.
- Есть JSON/CSV export.
- Есть replay-log режим через `--replay-log`.
- `AggregateMetrics` включает SAR-поля:
  - `avg_time_to_find`;
  - `avg_probability_of_detection`;
  - `avg_targets_found`.
- README обновлён реальными числами из benchmark-прогона.

Проверено:

- `cargo test --workspace` проходит.
- `cargo run -q -p swarm-examples --bin strategy_comparison -- --mission all --json ... --csv ...` проходит.
- Quick matrix дала CSV на 56 строк: header + 55 комбинаций.

## Что ещё не идеально

Текущий статус — "рабочая платформа", но не "отполированный финальный benchmark".

Основные оставшиеся шероховатости:

- В JSON/CSV поля `mission` и `scenario` сейчас пустые, хотя информация частично закодирована в `profile` через префиксы `coverage/...`, `emergency-mesh/...`, `sar/...`.
- В markdown-выводе merged report миссия/сценарий остаются `coverage` даже для строк `emergency-mesh/...` и `sar/...`.
- `benchmark_run_id` для `--mission all` всё ещё выглядит как coverage-based id.
- Нет отдельного сильного proptest/stress набора именно для distributed CBBA на 500+ случайных топологий.
- README-таблица основана на quick run по 10 seeds per cell, а не на полном 1000-seed publishable matrix.
- `--mission all` запускается и завершается успешно, но quick-прогон уже занимает несколько минут; для повседневной проверки нужен более лёгкий smoke mode или отдельная CI-конфигурация.

## Нужен ли дальше один линейный roadmap

Сейчас уже нет одного однозначного линейного направления развития.

Есть общий ближайший ствол:

> Milestone 11 hardening: сделать текущий benchmark runner аккуратным, воспроизводимым и методологически чистым.

После этого начинается развилка. Это нормальное состояние проекта: базовая платформа уже есть, дальше направление зависит от цели.

## Ближайший общий шаг — M11 hardening

Перед выбором большого нового направления стоит закрыть стабилизационный слой.

Что сделать:

1. Исправить metadata в report/export:
   - заполнять `mission`;
   - заполнять `scenario`;
   - корректно формировать `seed_range`;
   - сделать `run_id` и `benchmark_run_id` не coverage-specific для `--mission all`.
2. Добавить CLI/export tests для `--mission all`.
3. Добавить distributed CBBA proptest/stress tests.
4. Обновить README числа после исправленного отчёта.
5. Разделить быстрый smoke benchmark и полный publishable benchmark.

Это не новая исследовательская ветка, а доведение текущего Milestone 11 до состояния, на которое можно опираться дальше.

## Развилка направлений

### Вариант 1 — Mission DSL

Суть: описывать сценарии декларативно через YAML/RON/JSON, а не hardcoded Rust builder-ами.

Зачем нужно:

- сценарии станут воспроизводимыми артефактами, а не только кодом;
- новые benchmark cases можно будет добавлять без перекомпиляции и без правки Rust;
- внешнему пользователю проще описать свою миссию;
- CI сможет гонять набор конфигов как test suite.

Что даст:

- платформенность;
- удобное расширение benchmark matrix;
- основу для будущих reference missions;
- более понятный интерфейс для людей, которые не хотят писать Rust.

Где пригодится:

- research benchmark suites;
- regression testing;
- демонстрационные наборы сценариев;
- внешние пользователи, которым нужен configurable simulator.

Риск: если сделать DSL слишком рано и слишком широко, он может зацементировать плохую модель. Сейчас риск ниже, потому что Coverage, EmergencyMesh и SAR уже показали минимальный набор нужных полей.

### Вариант 2 — SAR v2 / Uncertainty Map

Суть: углубить SAR из grid scanning в задачу с неопределённостью.

Возможные элементы:

- confidence map;
- repeated scans;
- confidence decay;
- false positives;
- target belief state;
- разные sensor models для Scout/Thermal/Relay;
- метрики качества поиска, а не только факт назначения задач.

Зачем нужно:

- SAR станет настоящей содержательной reference mission;
- стратегии начнут отличаться не только allocation quality, но и качеством поиска под неопределённостью;
- проект сильнее приблизится к mission-level autonomy research.

Что даст:

- более убедительный research result;
- richer benchmark для алгоритмов;
- основу для sensor fusion / belief-driven planning.

Где пригодится:

- search-and-rescue;
- environmental monitoring;
- поиск объектов на местности;
- любые сценарии "найти цель при неполной информации".

Риск: это усложняет модель и может потребовать пересмотра метрик, чтобы они не стали произвольными.

### Вариант 3 — Infrastructure Inspection

Суть: добавить новую прикладную reference mission: инспекция линий, трубопроводов, дорог, солнечных панелей или другой протяжённой инфраструктуры.

Зачем нужно:

- это более прикладной и понятный сценарий, чем абстрактный coverage;
- хорошо проверяет kinematics, battery, missed segments и route coverage;
- проще количественно оценивать, чем wildfire или полностью открытый SAR.

Что даст:

- третью сильную reference mission;
- прикладную демонстрацию проекта;
- benchmark для route planning и coverage completeness;
- bridge между исследовательской симуляцией и потенциальными industrial use cases.

Где пригодится:

- power line inspection;
- pipeline inspection;
- solar farm inspection;
- railway/road monitoring;
- perimeter patrol.

Риск: без Mission DSL новая миссия снова будет hardcoded в Rust, поэтому её лучше делать после или вместе с DSL.

### Вариант 4 — Safety Layer

Суть: добавить слой ограничений безопасности.

Возможные элементы:

- geofence;
- no-fly cells;
- separation constraints;
- collision avoidance-lite;
- movement constraints;
- safety-aware allocation.

Зачем нужно:

- без safety layer проект нельзя честно приближать к реальным дронам;
- текущий runtime работает на mission coordination уровне, но не контролирует эксплуатационные ограничения;
- PX4/MAVLink/SITL без safety layer будет преждевременным направлением.

Что даст:

- возможность двигаться к более реалистичной симуляции;
- основу для SITL / hardware-in-the-loop;
- снижение разрыва между benchmark и реальной эксплуатацией.

Где пригодится:

- любые реальные или semi-real drone workflows;
- SITL demos;
- regulated airspace simulation;
- mission planning под ограничения.

Риск: это большой пласт, который может потянуть изменения в allocator, movement model и scenario model одновременно.

### Вариант 5 — CBBA Robustness / Scaling

Суть: углубить алгоритмическую часть CBBA.

Возможные элементы:

- random topologies;
- packet loss stress;
- partitions and healing;
- N agents / M tasks scaling;
- convergence time distributions;
- retransmission policy при высоком message loss;
- сравнение CBBA с centralized/auction/connectivity-aware по цене коммуникации.

Зачем нужно:

- текущий CBBA уже distributed, но его пределы ещё не исследованы;
- publishable result требует не только "работает", но и "понятно, где работает хорошо/плохо";
- можно получить сильный distributed systems / swarm robotics benchmark.

Что даст:

- методологически сильное сравнение стратегий;
- понимание degradation curves;
- основание для улучшений consensus/retransmission.

Где пригодится:

- swarm robotics research;
- distributed allocation research;
- статьи/отчёты про устойчивость алгоритмов при плохой связи.

Риск: это меньше развивает продуктовую платформу и больше углубляет один алгоритм.

### Вариант 6 — PX4 / MAVLink / zenoh / SITL

Суть: начать подключать runtime к real-world robotics stack.

Зачем нужно:

- показать путь к реальным дронам;
- получить live/SITL demonstration;
- проверить границу между mission runtime и autopilot/control layer.

Что даст:

- интеграционный showcase;
- мост к hardware/SITL;
- проверку API runtime на практичность.

Где пригодится:

- PX4 SITL;
- robotics demos;
- будущая hardware validation.

Риск: сейчас это преждевременно как основное направление. Без DSL, safety layer и стабильного scenario/report API интеграция может стать дорогой демонстрацией без исследовательской глубины.

### Вариант 7 — Visualization / Replay UI

Суть: сделать визуальное воспроизведение миссий и benchmark runs.

Зачем нужно:

- сейчас проект хорошо работает headless, но его трудно быстро понять глазами;
- replay уже есть, значит есть база для визуализации;
- для демо и debugging визуальный слой очень полезен.

Что даст:

- понятные демонстрации;
- ускорение диагностики;
- лучшее объяснение поведения алгоритмов;
- удобное сравнение стратегий.

Где пригодится:

- презентации;
- debugging;
- teaching/research demos;
- анализ странных benchmark outcomes.

Риск: визуализация сама по себе не повышает научную ценность, если под ней ещё не стабилизированы метрики и сценарии.

## Итоговая рекомендация

Не надо сейчас выбирать "один линейный roadmap навсегда".

Правильнее зафиксировать общий ствол и ветки:

1. **Сначала M11 hardening.**
   Довести текущий CBBA + multi-mission benchmark до аккуратного, воспроизводимого состояния.

2. **Затем Mission DSL как общий инфраструктурный шаг.**
   Он полезен почти для всех дальнейших веток: SAR v2, Infrastructure Inspection, Safety Layer, regression suites.

3. **После DSL выбрать фокус по цели проекта.**

Если цель — исследовательский publishable benchmark:

> SAR v2 / Uncertainty Map + CBBA Robustness / Scaling.

Если цель — прикладная демонстрация и понятный industrial use case:

> Infrastructure Inspection + Visualization.

Если цель — движение к реальным дронам:

> Safety Layer сначала, PX4/MAVLink/SITL потом.

Моя практическая рекомендация:

> M11 hardening → Mission DSL → SAR v2 / Uncertainty Map → Infrastructure Inspection → Safety Layer.

Почему так:

- M11 hardening делает текущий результат честным и стабильным.
- Mission DSL превращает hardcoded benchmark в платформу.
- SAR v2 даёт исследовательскую глубину.
- Infrastructure Inspection даёт прикладную ценность.
- Safety Layer нужен перед любым серьёзным движением к real-world/SITL.

Формула текущего состояния:

> базовый runtime и benchmark platform уже есть; дальше не одна дорога, а осознанный выбор между исследовательской глубиной, платформенностью, прикладными миссиями и приближением к реальным дронам.
