# PLAN: M11 Hardening

## Context

Phase 2 реализовал Unified Experiment Runner с `--mission` флагом и multi-mission export. Но остались баги и недоделки, мешающие публикуемому качеству.

**Источники контекста:** `docs/DRONE_A.7.md`, `docs/DRONE_B.7.md`. INVESTIGATION.md отсутствует.

**Текущее состояние (Phase 2 complete):**
- `mission` и `scenario` колонки в JSON/CSV заполняются пустыми строками (`String::new()`)
- `benchmark_run_id` генерируется с `"coverage"` для всех mission run (даже для SAR и EmergencyMesh)
- proptest для CBBA не существует — нет property-based тестов на случайных топологиях
- README содержит quick-mode числа (10 seeds), нужен full-mode (1000 seeds)

**Критерий готовности:**
1. JSON/CSV export содержит фактические mission и scenario имена, а не пустые строки.
2. `benchmark_run_id` содержит mission name вместо хардкоженного `"coverage"`.
3. `crates/swarm-sim/tests/proptest_cbba.rs` содержит proptest для distributed CBBA на случайных agents/tasks/packet_loss/partitions.
4. README обновлён с full-mode числами (1000 seeds).

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.7.md` и `docs/DRONE_B.7.md`:
- Оба документа требуют hardening: fix report bugs, add proptest, full benchmark.
- "Исправить конкретные баги в текущем report" — priority.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-sim/src/report_export.rs` | Заполнять `mission`/`scenario` из `ComparisonReport.mission_names`/`scenario_names` |
| `crates/swarm-sim/src/benchmark.rs` | `generate_benchmark_run_id` принимать `mission` параметр вместо хардкоженного "coverage" |
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | Передавать `mission_name` в `BenchmarkOptions` при запуске harness |
| `crates/swarm-sim/tests/proptest_cbba.rs` | **NEW** — proptest для distributed CBBA |
| `README.md` | Обновить таблицу с full-mode числами (1000 seeds) |

---

## Implementation Steps

### Шаг 1 — Заполнить mission/scenario в export

Файл: `crates/swarm-sim/src/report_export.rs`

В `export_json()` и `export_csv()`, заменить `String::new()` на значения из `ComparisonReport`:

```rust
mission: report.mission_names.first().cloned().unwrap_or_default(),
scenario: report.scenario_names.first().cloned().unwrap_or_default(),
```

### Шаг 2 — mission-aware benchmark_run_id

Файл: `crates/swarm-sim/src/benchmark.rs`

Изменить `generate_benchmark_run_id` сигнатуру: добавить `mission_name: &str` параметр. Заменить `"coverage"` на `mission_name`.

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

Добавить `mission_name: &str` в `BenchmarkOptions` (или передавать отдельно). При вызове `harness.run_with_options()` передавать `mission_name(mission)`.

### Шаг 3 — Proptest для CBBA

Файл: `crates/swarm-sim/tests/proptest_cbba.rs` (новый)

```rust
proptest! {
    #[test]
    fn cbba_no_panic_with_random_topology(
        agent_count in 2usize..=6,
        task_count in 1usize..=8,
        packet_loss in 0.0f64..0.3,
    ) {
        let agents = generate_agents(agent_count);
        let tasks = generate_tasks(task_count);
        let config = RunConfig {
            enable_cbba: true,
            packet_loss_rate: packet_loss,
            max_ticks: 50,
            ..RunConfig::default()
        };
        ScenarioRunner::run_with(&scenario, config, CbbaAllocator::default());
    }
}
```

### Шаг 4 — Full benchmark + README update

Запустить `cargo run --bin strategy_comparison -- --mission all --full --json /tmp/full.json` (1000 seeds). Обновить таблицу в README с новыми числами.

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run --bin strategy_comparison -- --mission all --full --json /tmp/full.json
```

---

## Risks and Tradeoffs

**1. mission/scenario колонки на один report**

`export_json` вызывается для одного `ComparisonReport` (с одной миссией). `mission_names.first()` возвращает корректное имя миссии. Для merged report — неоднозначно. Митигация: merged report тоже заполняет эти поля (уже сделано в `merge_reports`).

**2. benchmark_run_id uniqueness**

При `--mission all` каждый harness вызов генерирует свой run_id. Merge всех в один — приемлемо, run_id первого репорта используется.

**3. proptest runtime**

CBBA с 6 агентами может требовать много тиков. `max_ticks=50` + `enable_cbba=true` — небольшая нагрузка. Приемлемо для CI.

---

## Что могло сломаться

| Риск | Что сломается | Как проверить |
|---|---|---|
| mission/scenario берутся из merged report | Все строки имеют одинаковую mission — нормально для merged report | Проверить JSON output |
| benchmark_run_id с mission именем | Существующие тесты парсят run_id формат | `cargo test -p swarm-sim` |
| proptest CBBA добавляет latency | CI timeout если слишком много комбинаций | Ограничить agent_count до 6 |

---

## Open Questions

1. **Нужен ли separate proptest crate?** — Нет, `swarm-sim/tests/` подходит как location.
2. **Full-mode benchmark — сколько времени?** — 1000 seeds × 5 strategies × 11 profiles × 3 missions ≈ 165,000 runs. ~30-60 минут. Запускать отдельно не в CI.
