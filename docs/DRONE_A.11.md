# DRONE_A.11 — Что отделяет research prototype от готового продукта

Дата фиксации: 2026-05-23

## Короткий вывод

Проект уже близок к аккуратной публикации как **research prototype / benchmark platform**.

Но он ещё не является **готовым продуктом** и не является системой, готовой к реальным роевым полётам.

Разница не в том, что "надо чуть-чуть дописать UI". Разница в типе обязательств:

> research prototype показывает работающие идеи и воспроизводимые эксперименты; готовый продукт обещает пользователю стабильное поведение, понятные ошибки, поддерживаемый API, operational safety и end-to-end workflows.

Ниже — конкретные gaps.

## 1. Real PX4/SITL не является полноценным end-to-end workflow

Сейчас есть:

- mock SITL;
- experimental MAVLink scaffold;
- `sitl_agent`;
- conversion task -> waypoint;
- feature-gated `MavlinkTransport`.

Этого достаточно для research/demo scaffold.

Но продуктовый workflow должен быть примерно таким:

```text
scenario / mission
-> allocation
-> waypoint mission upload в PX4
-> arm / takeoff / execute
-> telemetry / status feedback
-> task status update
-> failure handling / reallocation
-> logs / report
```

Что ещё не закрыто end-to-end:

- реальный PX4 status feedback;
- подтверждение выполнения waypoint-ов;
- обработка ошибок MAVLink-соединения;
- обработка отказов во время выполнения;
- reallocation после отказа;
- multi-agent SITL;
- устойчивое восстановление после потери связи;
- связка telemetry -> `TaskStatus`;
- проверенный full loop "mission -> execution -> report".

Практический вывод:

> mock SITL можно показывать; real PX4 path нужно честно обозначать как experimental.

## 2. Нет production-grade error handling в CLI

Для research prototype допустимо, что CLI иногда делает:

- `panic!`;
- `expect`;
- резкое завершение на неизвестной mission;
- вывод Rust panic на пользовательскую ошибку.

Для продукта это неприемлемо.

Продуктовый CLI должен:

- возвращать понятные ошибки;
- не показывать panic на обычную пользовательскую ошибку;
- различать invalid config / IO error / unsupported feature / runtime failure;
- иметь стабильные exit codes;
- писать диагностический контекст;
- поддерживать `--validate`;
- поддерживать `--dry-run`;
- иметь `--verbose` / structured diagnostics.

Пример нужного уровня ошибки:

```text
Invalid scenario suite:
  file: scenarios/foo.json
  entry: 2
  mission: sar
  field: run_config.grid_state
  error: SAR mission requires grid_state
```

А не:

```text
thread 'main' panicked at ...
```

Практический вывод:

> CLI уже пригоден для разработчика/research user, но ещё не является продуктовым интерфейсом.

## 3. Нет safety-гарантий уровня реальных дронов

Текущий safety layer полезен:

- geofence;
- no-fly zones;
- separation constraints;
- safety checks in simulation;
- `safety_violations` metrics.

Но это не operational safety system для реальных полётов.

Для реального уровня нужны:

- формальная модель ограничений;
- проверка маршрута до отправки в autopilot;
- runtime enforcement при отклонении;
- fail-safe behavior:
  - return-to-home;
  - hold;
  - abort mission;
  - emergency landing;
- учёт высоты;
- учёт скорости и ускорений;
- учёт инерции;
- учёт GPS noise;
- учёт задержек связи;
- conflict resolution между несколькими агентами;
- логирование safety decisions;
- тесты на edge cases;
- интеграция с PX4 failsafe/geofence mechanisms.

Сейчас safety скорее:

> constraint checker in simulation.

А для продукта нужен:

> operational safety layer.

Практический вывод:

> текущий safety layer нельзя трактовать как готовность к реальному полёту.

## 4. Алгоритмы имеют известные слабые места

Для research это нормально: слабые места — часть результата.

Для продукта это надо либо исправлять, либо явно оформлять как support matrix.

Известные проблемы:

- CBBA / centralized плохо работают на SAR grid tasks;
- CBBA слаб на inspection perimeter;
- некоторые стратегии генерируют много конфликтов;
- некоторые стратегии генерируют высокий communication overhead;
- не все стратегии одинаково учитывают mission-specific semantics;
- CBBA convergence / communication overhead ещё не доказан на больших full runs;
- часть результатов выглядит как "работает, но не всегда понятно почему".

Для продукта нужно одно из:

1. Улучшить алгоритмы.
2. Ограничить support matrix:
   - "для SAR используйте greedy/auction";
   - "CBBA для SAR grid tasks не поддерживается";
   - "inspection perimeter experimental".
3. Добавить strategy selection / recommendation.
4. Добавить fallback behavior.
5. Добавить runtime warnings, если выбрана неподходящая strategy/mission combo.

Практический вывод:

> сейчас проект умеет сравнивать стратегии, но ещё не обещает пользователю "эта стратегия надёжно решает эту миссию".

## 5. Benchmark не доведён до полноценного long-run / publishable анализа

Сейчас есть:

- benchmark infrastructure;
- smoke / quick / full modes;
- output packs;
- manifest;
- JSON/CSV/Markdown export;
- quick benchmark results.

Это хороший уровень для research prototype.

Но для зрелого продукта или полноценной публикации нужны:

- full 1000-seed runs;
- несколько профилей сложности;
- confidence intervals / variance;
- стабильные artifacts;
- повторяемость на другой машине;
- сравнение по ключевым метрикам;
- интерпретация, где какая стратегия применима;
- regression thresholds:
  - если success просел;
  - если convergence ухудшилась;
  - если messages выросли;
  - если safety violations появились.

Сейчас проект показывает:

> мы можем сравнивать стратегии.

Для зрелого результата нужно:

> мы знаем, какие режимы работают, насколько стабильно, почему, и как это воспроизвести.

Практический вывод:

> benchmark готов как инфраструктура; как доказательный publishable результат он требует long-run анализа.

## 6. Нет стабильной публичной API / семантической версии

Сейчас это рабочий Rust workspace с большим количеством crates.

Для продукта нужен явный контракт:

- какие crates публичные;
- какие crates internal;
- какие structs/functions stable;
- какие CLI flags stable;
- какая версия DSL schema;
- какая версия report schema;
- какая версия replay schema;
- SemVer policy;
- deprecation policy;
- changelog;
- release tags;
- migration path между версиями.

Без этого внешний пользователь не понимает:

- можно ли строить поверх этих crates;
- можно ли автоматизировать CLI;
- можно ли хранить scenario JSON как долгоживущий artifact;
- сломается ли report parser после следующего commit.

Практический вывод:

> проект уже хорошо организован как repo, но ещё не оформлен как стабильный публичный API/product.

## Что это значит для публикации

### Можно публиковать как research prototype

Да, после небольших организационных правок:

- обновить benchmark commit/reference;
- добавить `CHANGELOG.md` / release notes;
- проверить golden path в чистом clone;
- добавить CI;
- поставить явный tag вроде `v0.1.0-research`;
- ещё раз подчеркнуть non-goals.

Оценка:

> примерно полдня-день спокойной работы.

### Нельзя публиковать как готовый продукт

Если под "готовым продуктом" понимать систему, которую пользователь может применять как надёжный инструмент или путь к реальным дронам, то работы существенно больше.

Там нужны:

- real PX4 end-to-end flow;
- production-grade CLI/API;
- operational safety;
- strategy support matrix;
- long-run benchmark validation;
- public API stability;
- release engineering.

Оценка:

> недели или месяцы, в зависимости от целевого уровня продукта.

## Итог

Формула текущего состояния:

> Остались организационные мелочи, если цель — аккуратно опубликовать проект как исследовательский прототип.

Но:

> Осталась большая инженерная работа, если цель — сделать завершённый продукт или систему для реальных дронов.

Главное различие:

- research prototype демонстрирует возможности;
- продукт даёт пользователю обещание стабильности, диагностики, поддержки и безопасного поведения.
