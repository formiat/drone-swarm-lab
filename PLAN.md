# PLAN — M22: Benchmark Report / Analysis

## Context

M21 (Reproducible Benchmark Pack) завершён. Платформа имеет:
- `strategy_comparison` CLI с `--smoke`/`--quick`/`--full` режимами.
- `--output-dir` создаёт self-contained directory: `results.json`, `results.csv`, `table.md`, `manifest.json`, `scenario_snapshot.json`.
- `BenchmarkManifest` с timestamp, git commit, command line, seed range.
- 12 scenario JSON файлов в `scenarios/` (coverage, sar, inspection, emergency-mesh, cbba_stress, sitl).
- 5 стратегий: greedy, auction, connectivity-aware, centralized, cbba.
- `AggregateMetrics` содержит 30+ полей: success_rate, task_completion_rate, PoD, belief_entropy, false_positive_rate, confirmation_scans, convergence p50/p95, bundle_travel_distance, edge_coverage_rate, missed_edges, route_efficiency, safety_violations, communication cost (messages/bytes).

**Проблемы текущего состояния:**
1. Метрики собираются, но не интерпретируются — нет документа с выводами.
2. README описывает фичи, но не показывает результаты benchmark runs.
3. Нет focused markdown report generation — `table.md` содержит raw таблицу, но без анализа.
4. Нет ответов на ключевые вопросы: где CBBA выигрывает/проигрывает, насколько SAR v2 лучше SAR v1, какой overhead у distributed consensus.

**Критерий готовности:**
1. `docs/BENCHMARK_RESULTS.md` существует с реальными числами и выводами.
2. README содержит summary table с результатами, не только список фич.
3. Есть воспроизводимые команды для всех прогонов.
4. Отчёт отвечает на 6 ключевых вопросов.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.10.linear.md` и `docs/DRONE_B.10.linear.md`:
- M22 должен превратить метрики в понятный технический результат.
- Нужно прогнать SAR v2, CBBA stress, Infrastructure Inspection, Safety coverage.
- Нужны таблицы: success rate, completion rate, PoD, belief entropy, false positive rate, confirmation scans, convergence p50/p95, bundle travel distance, edge coverage, missed edges, route efficiency, safety violations, communication cost.
- README должен показывать результаты, а не только фичи.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | CLI: `--report <path>` для focused markdown report |
| `crates/swarm-sim/src/report_export.rs` | `generate_focused_report()` — markdown с секциями по миссиям |
| `docs/BENCHMARK_RESULTS.md` | Новый документ — таблицы и интерпретация |
| `README.md` | Summary table с результатами |

---

## Implementation Steps

### Шаг 1 — Focused report generation

Файл: `crates/swarm-sim/src/report_export.rs`

Добавить:
```rust
/// Generate a focused markdown report with analysis sections.
pub fn generate_focused_report(
    reports: &[(String, ComparisonReport)],
) -> String {
    // Produces markdown with per-mission tables and summary
}
```

Структура выходного markdown:
- Заголовок с timestamp и git commit
- Per-mission таблицы (только релевантные метрики)
- Summary comparison table (strategy vs strategy)

### Шаг 2 — CLI `--report` flag

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

Добавить `--report <path>` flag. Если задан — после прогона записывать `generate_focused_report()` в файл.

```rust
"--report" => {
    i += 1;
    if i < args.len() {
        cli.report_path = Some(args[i].clone());
    }
}
```

### Шаг 3 — Прогон benchmarks и заполнение `docs/BENCHMARK_RESULTS.md`

Запустить (quick mode, 10 seeds — достаточно для отчёта):
```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --quick --mission sar --output-dir results/sar_quick/

cargo run -p swarm-examples --bin strategy_comparison -- \
  --quick --mission inspection --output-dir results/inspection_quick/

cargo run -p swarm-examples --bin strategy_comparison -- \
  --scenario-suite scenarios/coverage.safety.json --output-dir results/safety_quick/

cargo run -p swarm-examples --bin strategy_comparison -- \
  --scenario-suite scenarios/cbba_stress.json --output-dir results/cbba_quick/
```

Скопировать ключевые числа из `results.json`/`table.md` в `docs/BENCHMARK_RESULTS.md`.

Структура `docs/BENCHMARK_RESULTS.md`:
```markdown
# Benchmark Results

## Methodology
- Seeds: 0-9 (quick mode)
- Strategies: greedy, auction, connectivity-aware, centralized, cbba
- Git commit: <hash>

## SAR v2 (Belief-based Search)

| Strategy | Success | PoD | BeliefEntropy | FalsePosRate | ConfirmationScans |
|---|---|---|---|---|---|
| ... | ... | ... | ... | ... | ... |

**Вывод:** ...

## CBBA Stress Test

| Strategy | Success | ConvP50 | ConvP95 | BundleDist | Messages |
|---|---|---|---|---|---|
| ... | ... | ... | ... | ... | ... |

**Вывод:** ...

## Infrastructure Inspection

| Strategy | Success | EdgeCoverage | MissedEdges | RouteEfficiency |
|---|---|---|---|---|
| ... | ... | ... | ... | ... |

**Вывод:** ...

## Safety Coverage

| Strategy | Success | SafetyViolations | Coverage |
|---|---|---|---|
| ... | ... | ... | ... |

**Вывод:** ...

## Cross-mission Comparison

...summary...

## Answers to Key Questions

### Where does CBBA win?
...

### Where does CBBA lose?
...

### SAR v2 vs SAR v1
...

### Best strategies for inspection
...

### Distributed consensus overhead
...

### Safety constraint impact
...
```

### Шаг 4 — README summary table

Файл: `README.md`

Добавить раздел **Benchmark Results Summary** с сокращённой таблицей (2-3 ключевые метрики на миссию) и ссылкой на `docs/BENCHMARK_RESULTS.md`.

### Шаг 5 — Reproducible commands

В `docs/BENCHMARK_RESULTS.md` и `README.md` добавить:
```bash
# Quick run (10 seeds, ~30 seconds per mission)
cargo run -p swarm-examples --bin strategy_comparison -- --quick --mission <mission> --output-dir results/<mission>_quick/

# Full run (1000 seeds, ~5 minutes per mission)
cargo run -p swarm-examples --bin strategy_comparison -- --full --mission <mission> --output-dir results/<mission>_full/
```

---

## Testing Strategy

### Категория 1 — unit (swarm-sim)

- `focused_report_contains_mission_sections` — generate_focused_report содержит заголовки миссий
- `focused_report_has_summary_table` — summary table присутствует

### Категория 2 — integration (swarm-examples)

- `strategy_comparison_report_flag_creates_file` — `--report` создаёт файл
- `report_contains_key_questions` — файл содержит секции с ответами

### Категория 3 — e2e (manual verification)

- Прогон всех 4 миссий и проверка, что числа не NaN/inf
- Проверка, что `docs/BENCHMARK_RESULTS.md` содержит ≥ 4 таблицы

---

## Risks and Tradeoffs

**1. Quick mode (10 seeds) может давать шумные числа**

Митигация: в отчёте явно указать methodology (quick mode, 10 seeds). Для публикации — full mode (1000 seeds). Таблицы в `BENCHMARK_RESULTS.md` обновляются при перезапуске.

**2. Некоторые метрики не определены для всех миссий**

Митигация: per-mission таблицы показывают только релевантные метрики (SAR — PoD/belief, CBBA — convergence/bundle, inspection — edge coverage, safety — violations).

**3. README summary table может устаревать**

Митигация: summary table содержит ссылку на `docs/BENCHMARK_RESULTS.md` и пометку "run `cargo run ...` to regenerate". Не дублировать все числа в README — только ключевые выводы.

---

## Что могло сломаться

| Риск | Проверка |
|---|---|
| `--report` flag ломает существующий CLI | `cargo test -p swarm-examples --test benchmark_pack` |
| `generate_focused_report` не компилируется | `cargo build --workspace` |
| `docs/BENCHMARK_RESULTS.md` не создаётся | `ls docs/BENCHMARK_RESULTS.md` |
| README форматирование сломано | `cargo fmt --all --check` |

---

## Open Questions

1. **Should we automate benchmark runs in CI?** — v0.1: manual runs. Future: GitHub Actions with `--smoke`.
2. **Should we keep historical benchmark results?** — v0.1: overwrite `BENCHMARK_RESULTS.md`. Future: versioned results per commit.
3. **Should we generate HTML/PDF reports?** — v0.1: markdown only. Future: pandoc or mdBook.

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test --workspace
cargo run -p swarm-examples --bin strategy_comparison -- --quick --mission coverage --report /tmp/report.md
```
