# PLAN — M21: Reproducible Benchmark Pack

## Context

M20 (SITL Path Consolidation) завершён. Платформа имеет:
- `strategy_comparison` CLI с `--full` (1000 seeds) и quick (10 seeds) режимами.
- `--json`, `--csv`, `--replay-log-dir`, `--run-id-prefix` флаги для экспорта.
- `BenchmarkHarness` — `run_quick_with_options()` и `run_full_with_options()`.
- `ComparisonReport` с markdown-таблицей, JSON/CSV export.
- 4 миссии: coverage, emergency-mesh, sar, inspection.
- 5 стратегий: greedy, auction, connectivity-aware, centralized, cbba.

**Проблемы текущего состояния:**
1. Нет unified output directory — результаты разбросаны по `--json`, `--csv`, `--replay-log-dir`.
2. Нет manifest — невозможно связать результат с git commit, командой, seed range.
3. Нет scenario snapshot — нельзя повторить benchmark без догадок.
4. Нет smoke mode — CI использует тот же quick mode.
5. Markdown table fragment не сохраняется в файл.

**Критерий готовности:**
1. `--smoke`, `--quick`, `--full` режимы явно разделены.
2. `--output-dir <dir>` создаёт self-contained directory.
3. `manifest.json` содержит timestamp, git commit, command line, suite name, schema version, seed range, strategy list.
4. `scenario_snapshot.json` — полная копия входного suite.
5. `results.json` + `results.csv` + `table.md` внутри output directory.
6. README содержит команды для всех режимов.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.10.linear.md` и `docs/DRONE_B.10.linear.md`:
- M21 должен сделать benchmark output воспроизводимым.
- smoke/quick/full разделение нужно для CI vs локальной проверки vs publishable run.
- manifest + snapshot позволяют повторить benchmark без догадок.

**Ключевое наблюдение:** текущий `strategy_comparison`:
- `run_quick` — 10 seeds, подходит для smoke (CI).
- `run_full` — 1000 seeds, подходит для publishable.
- Нет отдельного smoke (1 seed) для быстрой проверки.
- Export разбросан по `--json`, `--csv`, `--replay-log-dir`.
- Нет manifest файла.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-examples/src/bin/strategy_comparison.rs` | CLI: `--smoke`, `--quick`, `--full`, `--output-dir`; unified export logic |
| `crates/swarm-sim/src/benchmark.rs` | `BenchmarkHarness::run_smoke_with_options()` — 1 seed |
| `crates/swarm-sim/src/report_export.rs` | `export_markdown()` — сохранить markdown fragment |
| `crates/swarm-sim/src/lib.rs` | Новые экспорты |
| `README.md` | Раздел M21 — Reproducible Benchmark Pack |

---

## Implementation Steps

### Шаг 1 — Режимы: smoke / quick / full

Файл: `crates/swarm-sim/src/benchmark.rs`

Добавить:
```rust
impl BenchmarkHarness {
    /// Run a minimal smoke benchmark (1 seed).
    pub fn run_smoke(
        strategies: &[StrategyFactory],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
    ) -> BenchmarkResult {
        Self::run_with_seeds(strategies, profile_names, scenario_builder, 0..1, None)
    }

    /// Run a smoke benchmark with options.
    pub fn run_smoke_with_options(
        strategies: &[StrategyFactory],
        profile_names: &[String],
        scenario_builder: &ScenarioBuilder,
        options: BenchmarkOptions,
    ) -> BenchmarkResult {
        Self::run_with_seeds(strategies, profile_names, scenario_builder, 0..1, Some(options))
    }
}
```

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

Заменить `full_mode: bool` на enum:
```rust
#[derive(Clone, Copy)]
enum RunMode {
    Smoke,  // 1 seed
    Quick,  // 10 seeds (default)
    Full,   // 1000 seeds
}

struct CliArgs {
    mode: RunMode,
    // ...
}
```

CLI флаги:
- `--smoke` → `RunMode::Smoke`
- `--quick` → `RunMode::Quick` (default)
- `--full` → `RunMode::Full`
- `--output-dir <path>` → unified output directory

### Шаг 2 — Unified output directory

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

Если `--output-dir` задан — создать directory и записать в него:
```rust
fn write_benchmark_pack(
    output_dir: &str,
    report: &ComparisonReport,
    suite: Option<&ScenarioSuite>,
    cli: &CliArgs,
    replay_logs: &[EventLog],
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(output_dir)?;

    // results.json
    let json = export_json(report)?;
    std::fs::write(format!("{}/results.json", output_dir), json)?;

    // results.csv
    let csv = export_csv(report)?;
    std::fs::write(format!("{}/results.csv", output_dir), csv)?;

    // table.md
    std::fs::write(format!("{}/table.md", output_dir), format!("{}", report))?;

    // manifest.json
    let manifest = BenchmarkManifest {
        timestamp: chrono::Utc::now().to_rfc3339(),
        git_commit: get_git_commit(),
        command_line: std::env::args().collect::<Vec<_>>().join(" "),
        suite_name: report.mission_names.join(","),
        schema_version: "0.1".to_owned(),
        seed_range_start: report.seed_range_start,
        seed_range_end: report.seed_range_end,
        strategy_names: report.strategy_names.clone(),
        profile_names: report.profile_names.clone(),
        metric_schema_version: "0.1".to_owned(),
    };
    std::fs::write(
        format!("{}/manifest.json", output_dir),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    // scenario_snapshot.json
    if let Some(suite) = suite {
        let snapshot = export_suite(suite)?;
        std::fs::write(format!("{}/scenario_snapshot.json", output_dir), snapshot)?;
    }

    // replay logs (optional)
    if !replay_logs.is_empty() {
        let replay_dir = format!("{}/replay_logs", output_dir);
        std::fs::create_dir_all(&replay_dir)?;
        for (i, log) in replay_logs.iter().enumerate() {
            let path = format!("{}/replay_{}.json", replay_dir, i);
            let json = serde_json::to_string_pretty(log)?;
            std::fs::write(path, json)?;
        }
    }

    Ok(())
}
```

### Шаг 3 — BenchmarkManifest

Файл: `crates/swarm-sim/src/report_export.rs` (или новый `crates/swarm-sim/src/manifest.rs`)

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkManifest {
    pub timestamp: String,
    pub git_commit: String,
    pub command_line: String,
    pub suite_name: String,
    pub schema_version: String,
    pub seed_range_start: u64,
    pub seed_range_end: u64,
    pub strategy_names: Vec<String>,
    pub profile_names: Vec<String>,
    pub metric_schema_version: String,
}

fn get_git_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_owned())
}
```

### Шаг 4 — markdown table fragment export

Файл: `crates/swarm-sim/src/report_export.rs`

Добавить:
```rust
pub fn export_markdown(report: &ComparisonReport) -> String {
    format!("{}", report)
}
```

Уже существует в `Display for ComparisonReport`. Добавить функцию-обёртку для явного API.

### Шаг 5 — Stable commands for missions

Файл: `README.md`

Добавить раздел **M21 — Reproducible Benchmark Pack** с командами:

```markdown
### Smoke (1 seed, CI)
```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --smoke --mission sar --output-dir results/sar_smoke/
```

### Quick (10 seeds, local check)
```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --quick --mission all --output-dir results/all_quick/
```

### Full (1000 seeds, publishable)
```bash
cargo run -p swarm-examples --bin strategy_comparison -- \
  --full --mission all --output-dir results/all_full/
```

### Output directory structure
```
results/all_quick/
  manifest.json           # timestamp, git commit, command line, seed range
  scenario_snapshot.json  # full scenario suite for reproducibility
  results.json            # JSON export of ComparisonReport
  results.csv             # CSV export
  table.md                # Markdown table fragment
  replay_logs/            # optional replay logs
```
```

### Шаг 6 — Обновить backward-compatible CLI

Файл: `crates/swarm-examples/src/bin/strategy_comparison.rs`

Сохранить backward compatibility:
- `--full` → `RunMode::Full` (без `--smoke`/`--quick`)
- Без флага режима → `RunMode::Quick` (default)
- `--json <path>` и `--csv <path>` — работают как раньше (в дополнение к `--output-dir`)
- Если `--output-dir` не задан — работает как раньше (stdout + `--json`/`--csv`)

---

## Testing Strategy

### Категория 1 — unit (swarm-sim)

- `benchmark_manifest_serde_roundtrip` — BenchmarkManifest JSON roundtrip
- `get_git_commit_returns_nonempty` — git commit не пустой
- `export_markdown_contains_header` — markdown содержит таблицу

### Категория 2 — integration (swarm-examples)

- `strategy_comparison_smoke_creates_output_dir` — `--smoke --output-dir` создаёт directory
- `strategy_comparison_output_contains_manifest` — manifest.json присутствует
- `strategy_comparison_output_contains_snapshot` — scenario_snapshot.json присутствует
- `strategy_comparison_output_contains_results` — results.json + results.csv + table.md
- `strategy_comparison_backward_compat_no_output_dir` — без `--output-dir` работает как раньше

### Категория 3 — e2e

- `cargo run ... --full --output-dir /tmp/bench_full/` — 1000 seeds, directory содержит 5+ файлов

---

## Risks and Tradeoffs

**1. Output directory может перезаписать существующие файлы**

Митигация: `std::fs::create_dir_all` не удаляет существующие файлы; документировать, что пользователь должен использовать уникальные имена директорий.

**2. `get_git_commit()` требует git CLI**

Митигация: `unwrap_or("unknown")` — fallback если git недоступен.

**3. Scenario snapshot может быть большим**

Митигация: snapshot — JSON suite, обычно < 100 KB. Опциональный через флаг `--no-snapshot`.

**4. Backward compatibility `--json`/`--csv` flags**

Митигация: флаги работают как раньше; `--output-dir` добавляет unified export, не заменяет.

---

## Что могло сломаться

| Риск | Проверка |
|---|---|
| `--full` перестал работать | `cargo run ... --full` |
| `--json` / `--csv` перестали работать | `cargo run ... --json /tmp/x.json` |
| BenchmarkHarness новый метод не компилируется | `cargo build --workspace` |
| Output directory создан без файлов | `cargo test` integration tests |
| Manifest JSON не парсится | `cargo test` manifest tests |

---

## Open Questions

1. **Compression для replay logs?** — v0.1: нет, raw JSON. Future: gzip.
2. **Metric schema version?** — v0.1: hardcoded "0.1". Future: derive from AggregateMetrics struct.
3. **Should `--output-dir` be the default?** — v0.1: opt-in. Future: make default with `--no-output-dir` to disable.

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test --workspace
cargo run -p swarm-examples --bin strategy_comparison -- --smoke --mission coverage --output-dir /tmp/bench_smoke
cargo run -p swarm-examples --bin strategy_comparison -- --quick --mission sar --output-dir /tmp/bench_quick
cargo run -p swarm-examples --bin strategy_comparison -- --full --mission all --output-dir /tmp/bench_full
```
