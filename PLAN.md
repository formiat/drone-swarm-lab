# PLAN — M26: Mission / Strategy Correctness

## Контекст

M26 из `docs_raw/DRONE_B.13.linear.md`. Цель — закрыть самые заметные слабые места
текущих стратегий: устранить или явно задокументировать 0% success на SAR+CBBA/Centralized,
разобраться с потенциальным расхождением success/edge_coverage в inspection,
исправить allocation gap CBBA на perimeter, добавить support matrix.

## Ключевые находки (исследование кодовой базы)

### Проблема 1: SAR + CBBA / Centralized → 0% success

**Корень проблемы CBBA:**

Файл: `crates/swarm-sim/src/runner.rs:664-667`

После того как агент сканирует SAR-клетку, вызывается `release_task()` — задача
переходит в статус `Unassigned`. На следующем тике CBBA получает её как новую
неназначенную задачу и сбрасывает `converged = false` (cbba.rs:288). Это запускает
новый цикл сборки бандлов (3–5 тиков). За эти тики задача считается неназначенной.
При 4×4 сетке (16 клеток) и многократных сбросах `max_task_unassigned_ticks`
превышает `config.max_unassigned_ticks = 10` → `success = false`.

**Корень проблемы CentralizedPlanner:**

Файл: `crates/swarm-alloc/src/centralized.rs:20-52`

`CentralizedPlanner::new()` вычисляет оптимальное распределение ОДИН РАЗ на основе
начального состояния. После `release_task()` отпущенная задача переназначается
тому же агенту (по старому плану), но агент теперь далеко. Агент тратит тики
на возврат к уже просканированной клетке (сканирование идемпотентно — результата нет),
клетка отпускается снова — бесконечный цикл. Итог: `max_unassigned_ticks` или `max_ticks`
исчерпывается до завершения всех задач.

**Решение:** Явная документация как unsupported. Глубокий fix (адаптивное переplanирование
CentralizedPlanner, ограничение bundle_size=1 для CBBA на SAR) — scope M27.

---

### Проблема 2: success=0.0 при edge_coverage=1.0 (inspection)

**Текущее поведение:** `complete_assigned_task()` вызывается в runner.rs:603 когда
агент, КОТОРОМУ назначена задача, физически пересекает ребро. Значит `success=1.0`
и `edge_coverage=1.0` движутся синхронно.

**Граничный случай (реальная ошибка):** Агент B физически пересекает ребро, но задача
назначена агенту A. Фильтр в runner.rs:566 (`assigned_to == Some(agent_id)`) пропустит
эту ситуацию — ребро остаётся физически непосещённым с точки зрения A, задача остаётся
`Assigned` у A. Это невозможно при нормальном движении (агенты движутся только к своим
задачам), но возможно при edge case с перераспределением задач.

**Решение:** Написать тест, подтверждающий что `success=1.0` при `edge_coverage=1.0`.
Если тест показывает реальную проблему — добавить completion-check на основе
`edge_coverage` (если все рёбра покрыты, проставить задачи как Completed).

---

### Проблема 3: CBBA на inspection perimeter (0% success, coverage=0.795)

**Корень проблемы — allocation gap:**

Файл: `crates/swarm-alloc/src/cbba.rs:10,23`

`CbbaConfig::default()` → `max_bundle_size = 5`. На perimeter 10×10: 40 рёбер, 4 агента
→ max 4×5 = 20 задач могут быть назначены за раз. Остальные 20 остаются Unassigned.

**Усугубляющий баг — bundle slot leak:**

Файл: `crates/swarm-alloc/src/cbba.rs:292-301`

В `allocate()` `winning_bids` очищается от завершённых задач (not in `tasks`). Но
`bundles` НЕ очищается от completed task_ids (строка 293: retain only by agent_id).
Это значит что после завершения задачи её task_id остаётся в бандле, занимая слот
вплоть до `max_bundle_size`. Новые задачи не добавляются, несмотря на то что часть
слотов "логически" освободилась.

**Решение:** После очистки `winning_bids` также очищать бандлы от task_ids которых
нет в `winning_bids` — освобождать слоты для новых задач. Ожидаемый эффект:
CBBA perimeter coverage существенно возрастёт.

---

## Затронутые компоненты

| Файл | Что меняется |
|------|-------------|
| `crates/swarm-alloc/src/cbba.rs` | Fix bundle slot leak |
| `crates/swarm-sim/src/runner.rs` | Нет изменений кода; тесты |
| `README.md` | Strategy support matrix |
| `docs/BENCHMARK_RESULTS.md` | Обновление данных после fix |

---

## Шаги реализации

### Шаг 1 — Fix: очистка CBBA бандлов от завершённых задач

**Файл:** `crates/swarm-alloc/src/cbba.rs`

В методе `Allocator::allocate()`, после блока `winning_bids.retain(...)` (строка ~297),
добавить очистку бандлов:

```rust
// Free bundle slots occupied by completed tasks (not in winning_bids anymore).
for bundle in self.bundles.values_mut() {
    bundle.retain(|task_id| self.winning_bids.contains_key(task_id));
}
```

Это освобождает bundle-слоты для завершённых задач, позволяя CBBA назначать новые задачи
без ограничения старыми.

**Примечание:** Нужно убедиться, что очистка происходит ПОСЛЕ cleanup winning_bids,
чтобы сохранить актуальные назначения.

---

### Шаг 2 — Документирование: SAR + CBBA/Centralized как unsupported

**Файл:** `README.md` → раздел Known Limitations и новый раздел Strategy Support Matrix.

Добавить явную таблицу support matrix с причинами:

| Mission | Strategy | Status | Notes |
|---------|----------|--------|-------|
| coverage | все | stable | — |
| sar | greedy, auction, connectivity-aware | stable | — |
| sar | cbba | unsupported | CBBA re-convergence delay after SAR task release exceeds max_unassigned_ticks |
| sar | centralized | unsupported | Static pre-planning incompatible with SAR dynamic task release; agent revisits stale cell assignments |
| inspection (linear/random) | все | stable | — |
| inspection (perimeter) | greedy, auction, connectivity-aware | experimental | battery/time constraint |
| inspection (perimeter) | centralized | experimental | static plan; moderate coverage |
| inspection (perimeter) | cbba | experimental | allocation gap (max_bundle_size); improves after bundle fix |

---

### Шаг 3 — Верификация: inspection success ↔ edge_coverage

**Файл:** `crates/swarm-sim/src/benchmark.rs` или отдельный integration test в `crates/swarm-sim/tests/`.

Написать тест: запустить inspection linear со стратегией greedy. Если `edge_coverage_rate == 1.0`,
убедиться что `success_rate == 1.0`. Если нет — найти и исправить расхождение.

Если тест выявит реальный баг (edge covered by wrong agent → task not completed):
- **Файл:** `crates/swarm-sim/src/runner.rs` — изменить логику completion inspection:
  добавить проверку на edge_coverage и помечать задачи Completed если `inspection_state.covered`
  содержит соответствующий `edge_id`, независимо от агента.

---

### Шаг 4 — Обновление README: Strategy Support Matrix

**Файл:** `README.md`

- Удалить пункты 6 и 7 из "Known Limitations".
- Добавить раздел "## Strategy Support Matrix" с полной таблицей из Шага 2.
- Таблица должна включать: Mission, Strategy, Status (stable/experimental/unsupported), Notes.

---

### Шаг 5 — Обновление BENCHMARK_RESULTS.md

**Файл:** `docs/BENCHMARK_RESULTS.md`

После реализации fix из Шага 1 и верификации из Шага 3:
- Перезапустить `--quick --mission inspection` и обновить таблицу Inspection.
- Добавить примечание об изменении CBBA bundle management.
- Обновить раздел "Answers to Key Questions".

Команда:
```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --quick --mission inspection --output-dir results/inspection_quick_m26/
```

---

### Шаг 6 — Автотесты

Файл: `crates/swarm-alloc/src/cbba.rs` (unit тесты)  
Файл: `crates/swarm-sim/tests/` (integration тесты)

Детальный перечень — в разделе Testing Strategy ниже.

---

## Testing Strategy

### Категория 1 — без рефакторинга

**cbba.rs unit:**

1. `cbba_bundle_slots_freed_after_task_completion` — проверить что после того как
   задача переходит в completed-статус (удаляется из winning_bids), слот в бандле
   освобождается и новая задача может быть добавлена:
   - Создать CBBA с `max_bundle_size=2`, 3 задачи, 1 агент
   - Вызвать `allocate()` дважды (бандл полный: 2 задачи)
   - Убрать одну задачу из `tasks` (симулируем completion)
   - Вызвать `allocate()` ещё раз
   - Проверить что третья задача теперь включена в бандл

2. `cbba_no_regression_bundle_stability` — убедиться что existing convergence тест
   всё ещё работает (no regression).

**Integration тесты (crates/swarm-sim/tests/ или inline в benchmark.rs):**

3. `inspection_linear_success_equals_edge_coverage` — запустить inspection linear + greedy
   (smoke, 1 seed). Если `avg_edge_coverage_rate == 1.0`, то `success_rate == 1.0`:
   ```
   assert_eq!(m.avg_edge_coverage_rate, 1.0);
   assert_eq!(m.success_rate, 1.0);
   ```

4. `sar_cbba_has_documented_status` — запустить SAR + CBBA (smoke). Результат должен
   быть либо `success_rate > 0` (если исправлено), либо иметь явную документацию
   причины неудачи. Тест проверяет что результат стабильно воспроизводится
   (детерминизм по seed). *Это документирующий тест, а не «должен упасть».*

5. `sar_centralized_has_documented_status` — аналогично для centralized.

6. `inspection_perimeter_cbba_coverage_improves` — после fix из Шага 1, проверить что
   `avg_edge_coverage_rate > 0.5` для CBBA на perimeter (текущий baseline: 0.795).
   Тест закрепляет что fix не регрессирует покрытие. Порог: `> 0.5` (консервативно).

### Категория 2 — лёгкий рефакторинг

7. `support_matrix_all_pairs_no_panic` — параметрический тест: для каждой пары
   (mission, strategy) из support matrix запустить smoke (1 seed). Ни один вариант
   не должен паниковать. Требует `make_scenario_builder` helper для каждого mission.

8. `cbba_perimeter_assignment_count_improves` — тест сравнивает число assigned задач
   до и после bundle fix (через snapshot). Для этого нужен helper чтобы считать
   assigned/unassigned после N тиков.

### Категория 3 — тяжёлый рефакторинг (backlog, не в M26)

9. Property-based тесты: для произвольных assignment сценариев CBBA не теряет задачи
   (не уменьшает coverage после fix).

10. Long-run тесты (1000 seeds) для подтверждения статистической значимости улучшений.

---

## Что могло сломаться

### CBBA bundle management fix

**Риск:** Очистка бандлов от completed tasks может нарушить convergence detection.
`check_convergence` сравнивает `winning_bids` с `prev_winning_bids`. Если бандлы
очищаются, то winning_bids тоже уже очищены — расхождение минимально.

**Проверить:** Все существующие CBBA тесты в `cbba.rs` (unit) и
`tests/proptest_cbba.rs` (property) должны пройти без изменений.

**Риск:** Задача может быть в бандле одного агента и одновременно выиграна другим.
После очистки бандла первый агент теряет задачу — это **корректно** (konsensus).

### API/контракты

`StrategyFactory` и `ScenarioBuilder` уже помечены `+ Send + Sync` (M25). Новых
изменений типов не планируется.

### Данные/отчёты

`BENCHMARK_RESULTS.md` обновляется вручную после нового прогона. Старые числа должны
быть помечены как "pre-M26" или заменены.

### Производительность

Очистка бандлов: O(tasks_per_agent × num_agents) per tick — незначительно.

---

## Открытые вопросы

1. **SAR + CBBA fix scope:** Если `max_unassigned_ticks` увеличить в SAR профиле
   (например до 50), CBBA может получить успех на SAR без изменения алгоритма.
   Стоит ли это делать в M26 или отложить на M27? → Решить по итогам тестов.

2. **Inspection perimeter + CBBA после fix:** После очистки бандлов ожидается рост
   coverage. Если coverage не достигает 1.0 (что вероятно из-за battery/time constraint),
   остаётся ли статус "experimental" или переходит в "stable with limitations"?

3. **support matrix format:** Должна ли support matrix быть только в README.md или
   также в отдельном `docs/SUPPORT_MATRIX.md`? Решение по ситуации при написании.
