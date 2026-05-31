# Regression Harness

Этот документ описывает текущий `regression_runner` после M56.

## Default regression

Default regression запускается командой:

```bash
cargo run -p swarm-examples --bin regression_runner -- --jobs 4
```

Без `--suite` runner выбирает только gating suites:

- `smoke` - быстрые structural checks;
- `quick` - более устойчивые behavioural checks на расширенном наборе seed.

`experimental` и `validation` suites не входят в default gate. Они запускаются только явно:

```bash
cargo run -p swarm-examples --bin regression_runner -- --suite experimental --jobs 4
cargo run -p swarm-examples --bin regression_runner -- --suite validation --jobs 4
```

`experimental` и `validation` failures показываются в отчете, но не меняют `overall_pass`.
Baseline обновлять из отчета с threshold violations нельзя.

Текущий default gate прошел release sweep для `regression_runner` и
`strategy_comparison --regression` на `jobs=1/4/14`; артефакты лежат в
`results/m56_regression_determinism_2026-05-30/`.

M64 Urban Foundations добавляет `scenarios/urban.patrol.json`,
road-graph planning и judge/metrics skeleton. M65 Urban Patrol v0 делает этот
fixture executable simulation smoke: one scout follows the ordered road-graph
loop, completes before timeout, emits Urban replay events, and reports patrol
metrics. Это не publication benchmark и не обновляет M62 evidence.

M66 Urban Search v1 добавляет `urban_search_static_bus_greedy` в smoke gate:
один scout идёт по road graph, mocked bus detector находит static bus, а
thresholds проверяют `success_rate`, `bus_detection_rate`,
`search_success_without_violation` и отсутствие false positives. Это
portable simulation gate, не publication benchmark и не PX4/SITL evidence.

Urban regression также может использовать explicit `--mission urban-patrol`
или `--mission urban-search` smoke. Длинные 500/1000-seed sweeps остаются
будущей M69-style evidence work.

## CLI

Полезные команды:

```bash
cargo run -p swarm-examples --bin regression_runner -- --list-suites
cargo run -p swarm-examples --bin regression_runner -- --suite smoke
cargo run -p swarm-examples --bin regression_runner -- --suite quick --format json
cargo run -p swarm-examples --bin regression_runner -- --suite experimental --suite-name wildfire_medium_dynamic_greedy
```

Exit codes:

- `0` - команда выполнилась, gating suites прошли;
- `1` - gating regression failed или baseline update отказался писать из отчета с violations;
- `2` - ошибка CLI/configuration: неизвестный suite, плохой baseline path, неверный аргумент.

## Failure reproduction

Human-readable отчет для каждого threshold violation печатает:

- suite name;
- mission/profile/strategy;
- group/mode/seed range;
- actual metric, threshold and delta;
- reproduction command.

Пример команды воспроизведения из отчета:

```bash
cargo run -p swarm-examples --bin regression_runner -- --suite smoke --suite-name sar_ideal_greedy --jobs 1
```

## Baseline workflow

Baseline сравнение:

```bash
cargo run -p swarm-examples --bin regression_runner -- --compare-baseline baselines/default.json
```

Baseline update:

```bash
cargo run -p swarm-examples --bin regression_runner -- --update-baseline baselines/default.json
```

Runner пишет baseline только в путь, который передал caller. Update разрешен только из отчета без threshold violations. Baseline содержит:

- git commit;
- creation timestamp в RFC 3339;
- seed range/count;
- suite group;
- aggregate metrics per suite key.

Если baseline не содержит entry для текущего suite, отчет явно печатает `Missing Baseline Entries`.

## Promotion path

Чтобы перевести suite из `experimental` в default gate:

1. Запустить suite явно несколько раз локально и/или в CI.
2. Проверить, что metric semantics не спорные и threshold не является `min=0.0`.
3. Для volatile behavior использовать `quick`, а не single-seed `smoke`.
4. Зафиксировать threshold как behavioural или structural.
5. Перевести group в `smoke` или `quick`.
6. Обновить baseline только после green run.

`validation` suites предназначены для long/manual/CI-optional artifacts. Они могут подкреплять milestone evidence, но сами не являются milestone gate.
