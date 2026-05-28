# M39b — Decision / Audit Report

## Context

M39a (Regression Repair) закрыт. Реализовано:
- Shared `regression_lib.rs` модуль;
- Унификация regression entrypoints (`regression_runner` и `strategy_comparison --regression`);
- Исправлена поддержка wildfire и realism в `strategy_comparison --regression`;
- Устранена flakiness в regression integration tests;
- `cargo test --workspace` проходит.

Следующий шаг по линейному плану DRONE_A.15.linear.md — M39b Decision / Audit Report.

## Investigation context

`INVESTIGATION.md` отсутствует. Анализ DRONE_A.15.linear.md показал:

### 1. README Current Status содержит overstatement

**Файл:** `README.md`

- M36 (Regression Harness v2) помечен как "✅ Stable", но до M39a `strategy_comparison --regression` был broken;
- M38 назван "Wildfire / Flood v2", но flood не реализован как отдельная сущность;
- `docs/BENCHMARK_RESULTS.md` описывает 500-seed run на старом commit `8fb5ab1` (до M33-M38), но не помечен как historical.

### 2. Несоответствие названия M38

**Файл:** `README.md:89`

M38 назван "Wildfire / Flood v2", но:
- Нет отдельного flood scenario/model/adapter/profile;
- Кодовая база wildfire-first;
- Flood остаётся future branch.

### 3. Benchmark docs устарели

**Файл:** `docs/BENCHMARK_RESULTS.md`

Описывает 500-seed release run на commit `8fb5ab1`, то есть до M33-M38. Этот результат полезен как историческая валидация M32b, но не является актуальной full validation текущего HEAD.

### 4. Состояние milestones

| Milestone | Фактический статус |
|---|---|
| M32 Reporting & Metrics Hardening | ✅ Закрыт хорошо |
| M33 Mission Semantics Integration | ✅ Закрыт, adapter layer работает |
| M34 Planner Correctness v2 | ✅ Закрыт, но влияние planner ограничено |
| M35 Dynamic Mission Correctness | ⚠️ Частично — классифицированы слабые стратегии, но не все исправлены алгоритмически |
| M36 Regression Harness v2 | ⚠️ Реализован, но стал stable только после M39a |
| M37 Realism Scenario Pack | ✅ Закрыт как scenario pack, но не как research-grade study |
| M38 Wildfire / Flood v2 | ⚠️ Wildfire сделан, flood отсутствует |
| M39a Regression Repair | ✅ Закрыт |

## Affected components

| Компонент | Путь | Что меняется |
|---|---|---|
| Decision report | `docs/STATUS.md` (new) | Status by milestone, known limitations, readiness assessment |
| README | `README.md` | Current Status table, remove overstatements, clarify M38 |
| Benchmark docs | `docs/BENCHMARK_RESULTS.md` | Mark as historical, add commit reference |

## Implementation steps

### 1. Создать `docs/STATUS.md`

**Файл:** `docs/STATUS.md`

Содержание:
- **Milestone Status**: таблица M32-M39a с фактическим статусом;
- **Known Limitations**: что не работает/работает плохо;
- **Benchmark State**: актуальность `docs/BENCHMARK_RESULTS.md`;
- **Readiness Assessment**: готовность к 1000-seed run;
- **Next Steps**: M40-M45 по линейному плану.

### 2. Обновить README Current Status

**Файл:** `README.md`

- M36: уточнить — "Stable after M39a repair";
- M38: переименовать в "Wildfire v2", убрать "Flood";
- Добавить примечание к `docs/BENCHMARK_RESULTS.md` — "Historical, see docs/STATUS.md for current state";
- Убедиться, что все documented commands проходят.

### 3. Обновить `docs/BENCHMARK_RESULTS.md`

Добавить в начало файла:
```markdown
> **Historical Note**: This report reflects a 500-seed run on commit `8fb5ab1`
> (pre-M33). For current project status, see `docs/STATUS.md`.
```

### 4. Обновить `docs/BENCHMARK_RESULTS.md`

**Файл:** `docs/BENCHMARK_RESULTS.md`

Добавить header:
```markdown
---
**Historical Note**: This report reflects a 500-seed benchmark run on commit
`8fb5ab1` (pre-M33 Mission Semantics Integration). It remains useful as a
historical validation of M32b reporting identity, but does not represent the
current HEAD. For up-to-date status, see [`docs/STATUS.md`](STATUS.md).
---
```

## Testing strategy

### Категория 1 — без рефакторинга

- **README command smoke test**: проверить, что все documented commands проходят:
  ```bash
  cargo test --workspace
  cargo run -p swarm-examples --bin regression_runner -- --jobs 4
  cargo run -p swarm-examples --bin strategy_comparison -- --regression --jobs 4
  ```

### Категория 2 — лёгкий рефакторинг

- **Markdown link validation**: проверить, что `docs/STATUS.md` существует и ссылки из README корректны.

### Категория 3 — тяжёлый рефакторинг

- Не требуется для documentation milestone.

## Risks and tradeoffs

| Риск | Вероятность | Влияние | Митигация |
|---|---|---|---|
| STATUS.md устареет быстро | Высокая | Низкое | Дата в заголовке, ссылка на последний commit |
| README становится менее optimistic | Низкая | Низкое | Это цель milestone — честный статус |

## Open questions

1. **Нужен ли отдельный `docs/STATUS.md` или продолжать через README?**
   - Рекомендуется: `docs/STATUS.md` для детального audit, README для high-level summary.

2. **Как часто обновлять STATUS.md?**
   - После каждого major milestone или при значительном изменении статуса.

3. **Нужно ли помечать M35/M36/M38 как partial в README?**
   - Рекомендуется: использовать emojis (✅ / ⚠️ / 📝) для отражения фактического состояния.

## Что могло сломаться

- **Данные**: Новый файл `docs/STATUS.md` добавляется. Старые файлы не затронуты.
- **Документация**: README становится менее optimistic. Это intentional, не regression.
- **Ссылки**: `docs/BENCHMARK_RESULTS.md` получает ссылку на `docs/STATUS.md`. Нужно проверить, что файл создан до ссылки.

## Критерии готовности

- [ ] `docs/STATUS.md` создан и содержит status by milestone.
- [ ] README Current Status отражает фактическое состояние (без overstatement).
- [ ] M38 переименован в "Wildfire v2" (без "Flood").
- [ ] `docs/BENCHMARK_RESULTS.md` помечен как historical.
- [ ] Все documented commands проходят.
- [ ] Локальный commit сделан.
