# PLAN — M23: Replay / Debuggability

## Context

M22 (Benchmark Report / Analysis) завершён. Платформа имеет:
- `strategy_comparison` CLI с `--smoke`/`--quick`/`--full` и `--output-dir`/`--report`.
- `docs/BENCHMARK_RESULTS.md` с реальными числами и выводами.
- `swarm-replay` crate с `EventLog`, `Event` enum, `EventLogBuilder`, `replay()`, serde.
- Текущие event types: `TickStart`, `AgentFailed`, `TaskAssigned`, `MessageSent`, `MessageDropped`, `PartitionAdded`, `PartitionRemoved`, `PoseUpdated`.
- Replay logs сохраняются в `results/<dir>/replay_logs/` при `--replay-log <dir>`.

**Проблемы текущего состояния:**
1. Нет schema version в event log — невозможно определить формат файла.
2. Нет mission-specific событий: SAR scan/detections, inspection edge visits, safety violations, CBBA convergence markers.
3. Нет replay summary CLI — странный benchmark result приходится разбирать ручным чтением JSON.
4. Нет ASCII/grid replay-визуализации — нельзя "увидеть" поведение агентов.
5. `TaskAssigned` — единственное событие про задачи; нет `TaskStarted`, `TaskCompleted`, `TaskExpired`.

**Критерий готовности:**
1. `EventLog` имеет `schema_version`.
2. Добавлены mission-specific event types.
3. `cargo run --bin replay -- --log <file>` выдаёт summary.
4. `cargo run --bin replay -- --log <file> --tick N` выдаёт ASCII snapshot.
5. README описывает replay workflow.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.10.linear.md` и `docs/DRONE_B.10.linear.md`:
- M23 должен сделать странные benchmark outcomes объяснимыми без ручного чтения JSON.
- Нужен replay summary CLI (ticks, assignments, conflicts, failures, safety violations, SAR detections, inspection edge coverage, CBBA convergence).
- Нужна минимальная ASCII/grid replay-визуализация (`--tick N`, `--follow`).
- egui/Bevy UI — не в этом milestone.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-replay/src/event_log.rs` | `schema_version`, новые event types |
| `crates/swarm-replay/src/replay.rs` | `ReplayState` + `ReplaySummary` + per-tick snapshot |
| `crates/swarm-replay/src/lib.rs` | Новые экспорты |
| `crates/swarm-sim/src/runner.rs` | Логирование новых event types |
| `crates/swarm-examples/src/bin/replay.rs` | Новый CLI binary |
| `crates/swarm-examples/Cargo.toml` | Добавить `[[bin]] replay` |
| `README.md` | Раздел M23 — Replay / Debuggability |

---

## Implementation Steps

### Шаг 1 — Event log schema version и новые event types

Файл: `crates/swarm-replay/src/event_log.rs`

Добавить `schema_version` в `EventLog`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventLog {
    pub schema_version: String,  // "0.2"
    pub run_id: String,
    pub seed: u64,
    pub scenario_name: String,
    pub events: Vec<Event>,
}
```

Добавить новые event variants:
```rust
pub enum Event {
    // ... existing events ...

    // Task lifecycle
    TaskStarted { task_id: TaskId, agent_id: AgentId, tick: u64 },
    TaskCompleted { task_id: TaskId, agent_id: AgentId, tick: u64 },
    TaskExpired { task_id: TaskId, tick: u64 },

    // SAR v2
    SarScan { agent_id: AgentId, cell: (u32, u32), tick: u64, detected: bool },
    SarDetection { agent_id: AgentId, target_pose: Pose, tick: u64 },

    // Inspection
    EdgeVisited { edge_id: String, agent_id: AgentId, tick: u64 },

    // Safety
    SafetyViolation { agent_id: AgentId, violation_type: ViolationType, tick: u64 },

    // CBBA
    CbbaConverged { tick: u64 },
    CbbaBundleUpdated { agent_id: AgentId, bundle_size: usize, tick: u64 },
}

pub enum ViolationType {
    NoFly,
    Geofence,
    Separation,
}
```

### Шаг 2 — Replay summary API

Файл: `crates/swarm-replay/src/replay.rs`

Добавить `ReplaySummary`:
```rust
pub struct ReplaySummary {
    pub total_ticks: u64,
    pub assignments: usize,
    pub completions: usize,
    pub conflicts: usize,
    pub failures: usize,
    pub safety_violations: usize,
    pub sar_scans: usize,
    pub sar_detections: usize,
    pub edges_visited: usize,
    pub cbba_convergence_ticks: Vec<u64>,
    pub messages_sent: u64,
    pub messages_dropped: u64,
}

pub fn summarize(log: &EventLog) -> ReplaySummary { ... }
```

Добавить `ReplaySnapshot` для per-tick ASCII:
```rust
pub struct ReplaySnapshot {
    pub tick: u64,
    pub agent_poses: Vec<(AgentId, Pose)>,
    pub assigned_tasks: Vec<(TaskId, AgentId)>,
    pub active_agents: Vec<AgentId>,
    pub failed_agents: Vec<AgentId>,
}

pub fn snapshot_at_tick(log: &EventLog, tick: u64) -> ReplaySnapshot { ... }
```

### Шаг 3 — ASCII/grid visualization

Файл: `crates/swarm-replay/src/replay.rs`

```rust
pub fn render_ascii_grid(
    snapshot: &ReplaySnapshot,
    grid_bounds: (f64, f64, f64, f64), // min_x, max_x, min_y, max_y
    grid_size: usize,
) -> String {
    // Render agents as 'A', tasks as 'T', empty as '.'
}
```

### Шаг 4 — Логирование новых events в runner

Файл: `crates/swarm-sim/src/runner.rs`

Добавить `builder.push(...)` для:
- `TaskStarted` — когда задача переходит в InProgress
- `TaskCompleted` — когда задача завершена
- `TaskExpired` — когда задача истекла
- `SarScan` / `SarDetection` — в SAR-сценарии
- `EdgeVisited` — в inspection-сценарии
- `SafetyViolation` — при обнаружении violation
- `CbbaConverged` / `CbbaBundleUpdated` — при CBBA событиях

### Шаг 5 — Replay CLI binary

Файл: `crates/swarm-examples/src/bin/replay.rs`

```rust
fn main() {
    // Args: --log <path> [--summary] [--tick N] [--follow]
    let log = swarm_replay::read_from_file(path).unwrap();
    
    if summary {
        let s = swarm_replay::summarize(&log);
        println!("Ticks: {}", s.total_ticks);
        println!("Assignments: {}", s.assignments);
        // ... etc
    }
    
    if let Some(t) = tick {
        let snap = swarm_replay::snapshot_at_tick(&log, t);
        println!("{}", swarm_replay::render_ascii_grid(&snap, ...));
    }
}
```

Добавить в `Cargo.toml`:
```toml
[[bin]]
name = "replay"
path = "src/bin/replay.rs"
```

### Шаг 6 — README update

Файл: `README.md`

Добавить раздел **M23 — Replay / Debuggability**:
```markdown
### M23 — Replay / Debuggability

Inspect simulation runs without reading raw JSON.

**Replay summary:**
```bash
cargo run --bin replay -- --log results/replay_logs/run_0.json --summary
```

**ASCII snapshot at tick N:**
```bash
cargo run --bin replay -- --log results/replay_logs/run_0.json --tick 50
```

**Event log schema version:** `0.2`
```

---

## Testing Strategy

### Категория 1 — unit (swarm-replay)

- `event_log_schema_version_roundtrip` — schema_version сериализуется/десериализуется
- `new_event_types_roundtrip` — SarScan, SafetyViolation, CbbaConverged serde
- `summarize_counts_events_correctly` — summarize правильно считает события
- `snapshot_at_tick_reconstructs_poses` — snapshot корректно восстанавливает состояние

### Категория 2 — integration (swarm-examples)

- `replay_cli_summary_outputs_ticks` — `--summary` печатает число ticks
- `replay_cli_tick_outputs_ascii` — `--tick` печатает ASCII grid
- `replay_cli_invalid_log_exits_error` — невалидный JSON → exit(1)

### Категория 3 — e2e (swarm-sim + swarm-examples)

- Прогон coverage с `--replay-log`, затем `replay --summary` → non-zero counts
- Прогон SAR с `--replay-log`, затем `replay --summary` → sar_scans > 0

---

## Risks and Tradeoffs

**1. Schema version bump ломает старые replay logs**

Митигация: `schema_version` default "0.2" через `#[serde(default)]`. Старые logs без `schema_version` получают "0.1" и обрабатываются без новых event types.

**2. Новые event types увеличивают размер replay logs**

Митигация: mission-specific events записываются только при наличии соответствующих данных (SAR scan — только для SAR миссий). Размер увеличивается незначительно.

**3. ASCII visualization ограничена разрешением**

Митигация: ASCII grid — debugging tool, не UI. Для детального анализа используется `--summary` + `--tick` + JSON export.

**4. Runner event logging может замедлить simulation**

Митигация: logging происходит только при `enable_replay_log = true` (opt-in). В production runs логирование отключено.

---

## Что могло сломаться

| Риск | Проверка |
|---|---|
| `EventLog` без `schema_version` не десериализуется | `cargo test -p swarm-replay --lib` |
| Новые event types ломают старые replay logs | roundtrip test с legacy JSON |
| Runner event logging паникует | `cargo test --workspace` |
| Replay CLI не компилируется | `cargo build --bin replay` |
| ASCII render паникует на пустом log | `cargo test -p swarm-replay --lib` |

---

## Open Questions

1. **Should we compress replay logs?** — v0.1: raw JSON. Future: gzip option.
2. **Should snapshot include task poses?** — v0.1: agent poses only. Future: task positions if available.
3. **Should replay CLI support multiple log files?** — v0.1: single file. Future: directory mode.

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo build --bin replay
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test --workspace
cargo run --bin replay -- --log /tmp/test_replay.json --summary
```
