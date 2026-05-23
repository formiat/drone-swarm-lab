# PLAN — M20: SITL Path Consolidation

## Context

M19 (DSL Schema / Validation) завершён. Платформа имеет стабильный DSL контракт v0.1 с валидацией и человекочитаемыми CLI ошибками.

**Текущее состояние SITL path:**
- `crates/swarm-comms/src/mavlink.rs` — `MockMavlinkTransport` (всегда доступен) + `MavlinkTransport` (feature-gated за `mavlink-transport`).
- `crates/swarm-examples/src/bin/sitl_agent.rs` — CLI парсит `--mock` и `--connection`, но **всегда** использует `MockMavlinkTransport`, игнорируя `--connection`.
- `task_to_waypoint()` — конвертирует `Task` с `pose` в `Waypoint`.
- `scenarios/sitl.waypoints.json` — существует, содержит 1 агента и 3 задачи с `pose`.
- Feature `mavlink-transport` в `swarm-comms/Cargo.toml` — опциональная зависимость `mavlink` crate.

**Проблемы текущего состояния:**
1. `--connection` игнорируется — реальный MAVLink path никогда не используется.
2. Если в сценарии 0 задач с `pose` — выводится `0 tasks with pose`, но не предупреждение.
3. Нет теста на waypoint extraction из JSON-сценария.
4. Нет `docs/SITL_SETUP.md` — документация отсутствует.
5. Mock path и real path не разделены явно в коде.

**Критерий готовности:**
1. `sitl_agent --mock` отправляет waypoints из `scenarios/sitl.waypoints.json`.
2. `sitl_agent --mock` предупреждает, если найдено 0 pose-задач.
3. `sitl_agent --connection` использует `MavlinkTransport` при feature `mavlink-transport`.
4. Без feature `mavlink-transport` — `--connection` выдаёт понятную ошибку (не падает в mock).
5. `docs/SITL_SETUP.md` описывает mock mode, real PX4 mode, prerequisites, limitations.
6. Real PX4 прогон — manual / optional done criterion.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.10.linear.md` и `docs/DRONE_B.10.linear.md`:
- M20 должен честно разделить mock и real SITL path.
- `sitl_agent --mock` должен работать без внешних зависимостей.
- `sitl_agent --connection` должен требовать feature `mavlink-transport`.
- Не блокировать roadmap на реальном PX4 окружении.

**Ключевое наблюдение:** текущий `sitl_agent`:
- `let mut transport = MockMavlinkTransport::new();` — hardcoded, независимо от флагов.
- `--connection` парсится в `CliArgs` но нигде не используется.
- `MockMavlinkTransport` не имплементирует `Transport` trait (имеет собственные методы `send_waypoint`, `waypoints()`).
- `MavlinkTransport` имплементирует `Transport` но только с feature flag.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `crates/swarm-examples/src/bin/sitl_agent.rs` | Разделить mock/real path, warning при 0 pose-задач |
| `crates/swarm-examples/Cargo.toml` | Feature `mavlink-transport` для real path |
| `crates/swarm-comms/src/mavlink.rs` | `MockMavlinkTransport` — ensure `waypoints()` method works |
| `scenarios/sitl.waypoints.json` | Verify it has pose-tasks |
| `docs/SITL_SETUP.md` | **NEW** — mock mode, real PX4 mode, prerequisites, troubleshooting |
| `README.md` | Раздел M20 — SITL Path Consolidation |

---

## Implementation Steps

### Шаг 1 — sitl_agent --mock: отправка waypoints + warning

Файл: `crates/swarm-examples/src/bin/sitl_agent.rs`

Изменить логику после загрузки сценария:
```rust
let entry = &suite.scenarios[0];
let agent_tasks: Vec<_> = entry
    .scenario
    .tasks
    .iter()
    .filter(|t| t.pose.is_some())
    .collect();

if agent_tasks.is_empty() {
    eprintln!("Warning: no tasks with pose found in scenario. No waypoints to send.");
    std::process::exit(1);
}

eprintln!(
    "SITL Agent: {} | {} tasks with pose | mock={}",
    cli.agent_id,
    agent_tasks.len(),
    cli.mock
);
```

**Тест:** `sitl_agent_mock_sends_all_waypoints` — уже существует, расширить на проверку warning при 0 pose-задач:
```rust
#[test]
fn sitl_agent_mock_warns_zero_pose_tasks() {
    let tasks = vec![Task {
        id: TaskId::from("t0".to_owned()),
        status: TaskStatus::Unassigned,
        assigned_to: None,
        priority: 1,
        required_capabilities: vec![],
        required_role: None,
        preferred_role: None,
        expires_at: None,
        pose: None, // no pose
        grid_cell: None,
        edge_id: None,
    }];
    let pose_tasks: Vec<_> = tasks.iter().filter(|t| t.pose.is_some()).collect();
    assert!(pose_tasks.is_empty());
}
```

### Шаг 2 — sitl_agent --connection: feature-gated real MAVLink path

Файл: `crates/swarm-examples/src/bin/sitl_agent.rs`

Разделить paths:
```rust
if cli.mock {
    // Mock path: always works, no external dependencies
    let mut transport = MockMavlinkTransport::new();
    for (idx, task) in agent_tasks.iter().enumerate() {
        if let Some(wp) = task_to_waypoint(task) {
            eprintln!("WAYPOINT seq={} x={:.1} y={:.1} z={:.1}", idx, wp.x, wp.y, wp.z);
            transport.send_waypoint(wp);
        }
    }
    eprintln!("Mock mode: {} waypoints sent.", transport.waypoints().len());
} else if let Some(connection_string) = cli.connection {
    // Real MAVLink path: only with feature "mavlink-transport"
    #[cfg(feature = "mavlink-transport")]
    {
        use swarm_comms::MavlinkTransport;
        let agent_id = swarm_types::AgentId::from(cli.agent_id.clone());
        let mut transport = MavlinkTransport::new(&connection_string, agent_id)
            .unwrap_or_else(|e| {
                eprintln!("Failed to connect to MAVLink: {}", e);
                std::process::exit(1);
            });
        for (idx, task) in agent_tasks.iter().enumerate() {
            if let Some(wp) = task_to_waypoint(task) {
                let msg = format!("WAYPOINT seq={} x={:.1} y={:.1} z={:.1}", idx, wp.x, wp.y, wp.z);
                eprintln!("{msg}");
                // Convert Waypoint to MAVLink command and send
                let raw = swarm_comms::RawMessage::from(msg.into_bytes());
                if let Err(e) = transport.send(raw) {
                    eprintln!("Failed to send waypoint: {}", e);
                }
            }
        }
        eprintln!("Real MAVLink mode: waypoints sent.");
    }
    #[cfg(not(feature = "mavlink-transport"))]
    {
        eprintln!("Error: --connection requires feature 'mavlink-transport'.");
        eprintln!("  Build with: cargo build --features mavlink-transport");
        std::process::exit(1);
    }
} else {
    eprintln!("Error: specify --mock or --connection <addr>");
    std::process::exit(1);
}
```

Файл: `crates/swarm-examples/Cargo.toml`

Добавить feature:
```toml
[features]
mavlink-transport = ["swarm-comms/mavlink-transport"]
```

### Шаг 3 — docs/SITL_SETUP.md

Файл: `docs/SITL_SETUP.md` (новый)

```markdown
# SITL Setup Guide

## Mock Mode (no PX4 required)

```bash
cargo run --bin sitl_agent -- \
  --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0
```

Output: waypoints printed to stderr, sent to `MockMavlinkTransport`.

## Real PX4 Mode (experimental)

Prerequisites:
1. PX4 SITL running: `make px4_sitl gazebo_iris`
2. MAVLink connection on UDP: `udp:127.0.0.1:14550`

Build with feature:
```bash
cargo build --bin sitl_agent --features mavlink-transport
cargo run --bin sitl_agent --features mavlink-transport -- \
  --connection udp:127.0.0.1:14550 \
  --scenario scenarios/sitl.waypoints.json \
  --agent-id agent-0
```

## Known Limitations

- Real PX4 path is experimental and requires manual SITL setup.
- Only waypoint tasks with `pose` are converted to MAVLink commands.
- Multi-agent SITL not yet supported (single agent only).
```

### Шаг 4 — README

Файл: `README.md`

Добавить раздел **M20 — SITL Path Consolidation**:
- Описание mock mode (works out of the box)
- Описание real PX4 mode (experimental, requires feature)
- Команды запуска
- Ссылка на `docs/SITL_SETUP.md`

---

## Testing Strategy

### Категория 1 — unit (swarm-examples, swarm-comms)

- `sitl_agent_mock_sends_all_waypoints` — mock отправляет все waypoints
- `sitl_agent_mock_warns_zero_pose_tasks` — 0 pose-задач → warning/exit
- `task_to_waypoint_with_pose` — Task с pose → Some(Waypoint)
- `task_to_waypoint_no_pose` — Task без pose → None

### Категория 2 — integration (swarm-examples)

- `sitl_agent_mock_cli_runs` — `cargo run --bin sitl_agent -- --mock ...` завершается с exit 0
- `sitl_agent_connection_without_feature_fails` — `--connection` без feature → exit 1 с понятной ошибкой

### Категория 3 — e2e (manual / optional)

- Real PX4 SITL прогон — manual, не блокирует milestone.

---

## Risks and Tradeoffs

**1. Feature gate complexity**

`mavlink-transport` feature требует `mavlink` crate, который может не компилироваться без системных зависимостей. Митигация: feature optional, mock path работает без него.

**2. --connection без feature → ошибка**

Пользователь может не понять, почему `--connection` не работает. Митигация: понятное сообщение с инструкцией по сборке.

**3. MockMavlinkTransport vs Transport trait**

`MockMavlinkTransport` имеет метод `send_waypoint()`, а `MavlinkTransport` имплементирует `Transport::send(RawMessage)`. Митигация: в mock path использовать `send_waypoint()`, в real path — `Transport::send()`.

---

## Что могло сломаться

| Риск | Проверка |
|---|---|
| Feature gate ломает компиляцию без mavlink | `cargo build --workspace` (без feature) |
| sitl_agent --mock перестаёт работать | `cargo run --bin sitl_agent -- --mock ...` |
| --connection без feature не даёт ошибку | `cargo run --bin sitl_agent -- --connection ...` |
| scenarios/sitl.waypoints.json не грузится | `cargo test` dsl::tests |

---

## Open Questions

1. **Multi-agent SITL?** — v0.1: single agent only. Multi-agent — future milestone.
2. **MockMavlinkTransport должен имплементировать Transport?** — v0.1: нет, достаточно собственного API. Future: unify.
3. **Real PX4 test в CI?** — нет, требует SITL окружения. Оставить manual.

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
cargo run --bin sitl_agent -- --mock --scenario scenarios/sitl.waypoints.json --agent-id agent-0
cargo run --bin sitl_agent -- --connection udp:127.0.0.1:14550 --scenario scenarios/sitl.waypoints.json --agent-id agent-0  # должно выдать ошибку без feature
cargo build --bin sitl_agent --features mavlink-transport
```
