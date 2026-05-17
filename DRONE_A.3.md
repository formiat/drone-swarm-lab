# Сравнение DRONE_A.2.md и DRONE_B.2.md

Дата фиксации: 2026-05-17

## Короткий вывод

`DRONE_A.2.md` и `DRONE_B.2.md` отличаются не радикально, а приоритетом следующего шага.

Оба документа сходятся в главном:

> проект должен развиваться как Swarm Coordination Runtime + headless Mission/Scenario Harness.

Текущий код уже является рабочим runtime prototype / scenario harness, но ещё не полноценным mission digital twin и не готовым продуктом.

## Направление DRONE_A.2.md

`DRONE_A.2.md` предлагает двигаться так:

1. Replay + structured reports.
2. Mission DSL.
3. SAR mission.
4. CBBA.

Акцент:

- сначала сделать платформу удобной для исследований и анализа результатов;
- добавить воспроизводимость;
- добавить JSON/CSV-отчёты;
- затем вынести сценарии в данные;
- после этого строить новые миссии и новые алгоритмы.

Это более платформенное направление.

## Направление DRONE_B.2.md

`DRONE_B.2.md` предлагает:

1. Property-based tests + Replay.
2. Kinematic sim + Battery model + SAR.
3. Sensor model + Uncertainty map.

Акцент:

- сначала закрыть инженерную надёжность через `proptest` и replay;
- затем быстро идти в более реалистичный SAR;
- добавить движение, батарею, сенсоры и uncertainty.

Это более исследовательско-сценарное направление.

## Главное различие

Направления совместимы. Конфликт только в порядке работ:

- делать ли сначала structured reports / Mission DSL;
- или сначала proptest + SAR / kinematics.

`DRONE_A.2.md` сильнее заботится о платформенности.

`DRONE_B.2.md` быстрее ведёт к содержательному benchmark-сценарию.

## Фактическое замечание

`DRONE_B.2.md` немного устарел по числам: он пишет, что тест-сьют содержит 78 тестов.

Текущий `cargo test --workspace` показывает 127 тестов.

В остальном оценка статуса близкая.

## Итоговое направление

Не стоит выбирать чистый вариант A или чистый вариант B.

Лучше объединить их:

> сначала experiment infrastructure: `proptest` + replay + structured reports; потом kinematics/battery; потом SAR; потом CBBA.

Это самый прагматичный путь от текущего runnable prototype к серьёзной исследовательской платформе.

## Предлагаемый roadmap

### Milestone 7: Experiment Infrastructure

Состав:

- `proptest` для генерации отказов, packet loss, latency, partitions;
- `swarm-replay`: event log + deterministic replay одного прогона;
- structured reports: JSON/CSV export для benchmark results;
- стабильный `run_id`: seed + profile + strategy + scenario;
- CLI-флаги для `strategy_comparison`: `--json`, `--csv`, возможно `--replay-log`.

Почему это первый шаг:

- закрывает последний важный пункт из критерия "это не песочница";
- даёт воспроизводимость;
- даёт нормальный анализ результатов;
- делает будущие SAR/CBBA/sensor model проверяемыми, а не просто запускаемыми.

### Milestone 8: Kinematic + Battery Foundation

Состав:

- `position += velocity * dt`;
- скорость/дальность/расход батареи;
- mission time;
- реальный battery margin вместо статичного `100.0`;
- влияние движения на связность и достижимость задач.

### Milestone 9: SAR v1

Состав:

- grid/area;
- hidden target;
- scout/thermal/relay roles;
- probability of detection;
- time_to_find;
- coverage over time;
- network availability.

Это переводит проект от абстрактной coordination-проверки к настоящей reference mission.

### Milestone 10: CBBA

Состав:

- отдельная стратегия в `swarm-alloc`;
- message/round model;
- сравнение с `centralized`, `greedy`, `auction`, `connectivity-aware`;
- запуск на SAR + EmergencyMesh.

## Почему Mission DSL не первым

Mission DSL полезен, но если сделать его слишком рано, есть риск зацементировать плохую схему сценариев до появления SAR, kinematics и sensor model.

Лучше сначала накопить реальные требования к сценариям через:

- replay;
- structured reports;
- kinematic/battery model;
- SAR v1.

После этого Mission DSL будет описывать уже понятную модель, а не предположения.

## Финальная рекомендация

Ближайший шаг:

> Milestone 7 — Experiment Infrastructure: `proptest` + replay + structured reports.

Следующий крупный содержательный шаг:

> Milestone 8/9 — kinematics + battery + SAR.

Алгоритмический исследовательский шаг после этого:

> Milestone 10 — CBBA на реальных reference missions.
