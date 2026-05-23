# PLAN — M24: Release Candidate / Golden Path

## Context

M23 (Replay / Debuggability) завершён. Платформа имеет:
- Полный pipeline: scenario → benchmark → report → replay.
- 10 crates в workspace, 250+ тестов.
- 12 JSON сценариев в `scenarios/`.
- CLI tools: `strategy_comparison`, `sitl_agent`, `replay`.
- Docs: `docs/BENCHMARK_RESULTS.md`, `docs/SITL_SETUP.md`.
- README содержит milestones M1–M23, но структура выросла органически и требует cleanup.

**Проблемы текущего состояния:**
1. README слишком длинный — объединяет описание фич, команды запуска и результаты в одном файле.
2. Нет `docs/SCENARIO_DSL.md` — формат сценариев описан фрагментарно в README.
3. Нет `docs/REPLAY.md` — replay workflow не документирован отдельно.
4. Нет чёткого golden path — новый пользователь не знает, с чего начать.
5. Нет явного разделения simulation-only vs experimental SITL.
6. Нет списка known limitations.
7. Нет честного описания non-goals.

**Критерий готовности:**
1. Новый пользователь проходит golden path без чтения исходников.
2. README — entry point, а не monolith.
3. Все основные сценарии запускаются documented commands.
4. Текущий статус проекта честно описан.

---

## Investigation context

INVESTIGATION.md отсутствует. Контекст из `docs/DRONE_A.10.linear.md` и `docs/DRONE_B.10.linear.md`:
- M24 должен зафиксировать проект как цельный runnable research prototype.
- Нужен golden path: clone → test → smoke benchmark → scenario suite → inspect output → mock SITL.
- Нужны документы: SCENARIO_DSL.md, BENCHMARK_RESULTS.md, REPLAY.md, SITL_SETUP.md.
- Нужны честные non-goals.

---

## Affected Components

| Компонент | Тип изменения |
|---|---|
| `README.md` | Cleanup: restructure, golden path, limitations, non-goals |
| `docs/SCENARIO_DSL.md` | Новый документ — формат сценариев |
| `docs/REPLAY.md` | Новый документ — replay workflow |
| `docs/SITL_SETUP.md` | Обновить — добавить mock path в golden path |
| `docs/BENCHMARK_RESULTS.md` | Добавить ссылку на воспроизведение |

---

## Implementation Steps

### Шаг 1 — README cleanup и restructure

Файл: `README.md`

**Структура после cleanup:**
```markdown
# Swarm Coordination Runtime

Краткое описание (1-2 абзаца).

## Quick Start (Golden Path)

### 1. Clone and test
### 2. Run smoke benchmark
### 3. Run scenario suite
### 4. Create benchmark pack
### 5. Inspect replay
### 6. Run mock SITL

## Current Status

Таблица feature areas со статусом (✅ Stable, 🧪 Experimental, 📝 Documented).

## Known Limitations

## Non-Goals

## Workspace Layout

## Milestones Overview

Краткая таблица M1–M24 (без детального описания каждого).

## Docs

Ссылки на docs/*.md.
```

Конкретные изменения:
- Убрать детальные описания каждого milestone из README (оставить только overview table).
- Вынести детали в `docs/`.
- Добавить Quick Start с командами copy-paste.
- Добавить таблицу статуса feature areas.
- Добавить Known Limitations.
- Добавить Non-Goals.

### Шаг 2 — Создать `docs/SCENARIO_DSL.md`

Новый файл. Содержимое:
- Описание `ScenarioSuite` формата.
- Обязательные поля: `name`, `schema_version`, `scenarios[]`.
- Поля entry: `mission`, `profile`, `scenario` (agents, tasks, run_config).
- Mission-specific constraints (SAR: grid_state, Inspection: edge_id, SITL: pose, Safety: safety_config, CBBA: enable_cbba).
- Примеры минимального и полного сценария.
- Команды валидации.

### Шаг 3 — Создать `docs/REPLAY.md`

Новый файл. Содержимое:
- Описание event log schema (version 0.2).
- Список event types.
- Как сгенерировать replay log (`--replay-log`).
- Как просмотреть summary (`replay --summary`).
- Как сделать ASCII snapshot (`replay --tick N`).
- Backward compatibility note.

### Шаг 4 — Обновить `docs/SITL_SETUP.md`

Добавить:
- Mock path как часть golden path.
- Команда для быстрого старта.
- Ссылку на README Quick Start.

### Шаг 5 — Обновить `docs/BENCHMARK_RESULTS.md`

Добавить:
- Ссылку на воспроизведение (команды).
- Примечание о том, что числа из quick mode (10 seeds).

### Шаг 6 — Feature areas status table

В README добавить:
```markdown
| Feature | Status | Since | Notes |
|---|---|---|---|
| Benchmark (smoke/quick/full) | ✅ Stable | M21 | `--output-dir`, `--report` |
| Mission DSL | ✅ Stable | M19 | `schema_version: "0.1"`, validation |
| Safety Layer | ✅ Stable | M20 | `SafetyAllocator` wrapper |
| SAR v2 | ✅ Stable | M14 | `BeliefMap`, sensor noise |
| CBBA Robustness | ✅ Stable | M15 | Convergence metrics, TSP ordering |
| Infrastructure Inspection | ✅ Stable | M16 | Edge coverage, route efficiency |
| Mock SITL | ✅ Stable | M20 | `sitl_agent --mock` |
| Real PX4 | 🧪 Experimental | M20 | Feature-gated, needs PX4 SITL |
| Replay / Debuggability | ✅ Stable | M23 | `replay` CLI, ASCII viz |
```

---

## Testing Strategy

### Категория 1 — без рефакторинга

- `cargo test --workspace` — sanity check после README changes (нет кода).
- `cargo run --bin replay -- --help` — smoke test.
- `cargo run --bin sitl_agent -- --help` — smoke test.

### Категория 2 — лёгкий рефакторинг

- Проверка, что все ссылки в README работают (файлы существуют).
- Проверка, что golden path команды не содержат опечаток.

### Категория 3 — тяжёлый рефакторинг

- Не применимо (M24 — documentation milestone).

---

## Risks and Tradeoffs

**1. README cleanup может удалить полезную информацию**

Митигация: перед удалением перенести информацию в соответствующий `docs/*.md`. Не удалять, а перемещать.

**2. Новые docs файлы могут устареть быстро**

Митигация: docs содержат ссылки на README и команды, а не дублируют код. Использовать `include`-style ссылки где возможно.

**3. Golden path может не работать на чистой машине**

Митигация: golden path использует только `cargo` и `git`, без внешних зависимостей (кроме optional PX4).

---

## Что могло сломаться

| Риск | Проверка |
|---|---|
| README ссылки ведут в никуда | `ls docs/*.md` на все файлы из README |
| Golden path команды не работают | Ручной прогон команд из README |
| Удалены важные фрагменты README | Сравнение старого и нового README |

---

## Open Questions

1. **Should we add a CHANGELOG.md?** — v0.1: нет, milestones table достаточно. Future: keep per-release notes.
2. **Should we version the docs?** — v0.1: нет, docs относятся к текущему commit. Future: versioned docs for releases.
3. **Should golden path include full benchmark?** — v0.1: нет, quick mode достаточно. Full mode — отдельная секция.

---

## Verification Commands

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo build --workspace
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 cargo test --workspace
# Golden path sanity:
cargo run --bin replay -- --help
cargo run --bin sitl_agent -- --help
```
