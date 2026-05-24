# DRONE_B.11 — Оценка готовности к публикации

Дата фиксации: 2026-05-23

## Статус

Как research prototype — почти да, с оговорками.
Как publication-ready продукт — нет, по нескольким конкретным причинам.

## Что в хорошем состоянии

- Все тесты проходят, 0 failures.
- Golden path в README работает: smoke benchmark, scenario suite, sitl_agent.
- `sitl_agent --mock` реально отправляет 3 waypoints.
- Inspection JSON больше не падает (Infinity исправлен).
- Docs: README, BENCHMARK_RESULTS.md, SCENARIO_DSL.md, REPLAY.md, SITL_SETUP.md — всё есть.
- Known limitations и Non-goals честно прописаны.

## Что мешает публикации

### 1. Benchmark числа из quick mode (10 seeds)

В `BENCHMARK_RESULTS.md` явно написано: *"Numbers come from quick mode. For publishable
results run `--full` (1000 seeds)."* Публиковать research prototype с 10-seed числами —
слабая позиция.

### 2. CBBA и centralized дают 0% success на SAR

Задокументировано в Known Limitations, но не объяснено почему. Читатель видит что
главный алгоритм (CBBA) полностью ломается на одной из ключевых миссий — без объяснения
причины это выглядит как баг, а не как ограничение.

### 3. Inspection: success=0.0 при completion=1.0 и edge_coverage=1.0

В тесте `inspection.linear.json` все стратегии показывают `success=0.0` при
`completion=1.0` и `edge_coverage=1.0`. Выглядит как баг в определении метрики
success для inspection — что-то считается неправильно.

### 4. `<repo-url>` в README

Placeholder, не ссылка. Мелочь, но видно сразу.

### 5. `docs/` завален внутренними рабочими документами

30+ файлов `DRONE_A.*.md` / `DRONE_B.*.md` — рабочие журналы разработки,
не документация для пользователя. Внешний человек заходит в `docs/` и видит хаос.

## Итоговая таблица

| Критерий | Статус |
|---|---|
| Код компилируется и тесты проходят | ✅ |
| Golden path работает | ✅ |
| Документация пользователя | ✅ |
| Честные non-goals | ✅ |
| Publishable benchmark (1000 seeds) | ❌ |
| CBBA на SAR объяснено | ❌ |
| Inspection success метрика корректна | ❌ |
| Чистый `docs/` для внешнего читателя | ❌ |
| README без placeholder | ❌ |

## Приоритет исправлений

Содержательные (блокируют публикацию):

1. Разобраться с `success=0.0` при `edge_coverage=1.0` в inspection — баг в метрике.
2. Объяснить или исправить CBBA 0% на SAR (grid_cell handling).
3. Прогнать `--full` benchmark (1000 seeds) и обновить `BENCHMARK_RESULTS.md`.

Косметические (быстро):

4. Убрать `<repo-url>` placeholder из README.
5. Переместить рабочие `DRONE_*.md` из `docs/` в отдельную папку (например `docs/dev/`).
