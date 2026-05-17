# План: Milestone 7 — Experiment Infrastructure

## Context

Текущий проект находится на завершающей стадии Milestone 6 (Strategy Comparison Platform). Реализованы:

- 4 стратегии: Greedy, Auction, ConnectivityAware, CentralizedPlanner.
- BenchmarkHarness с quick/full режимами.
- ComparisonReport с markdown-таблицей и 14 метриками.
- StandardProfiles (6 network × 4 failure профиля).
- 127 unit + integration тестов.

Однако проект всё ещё не имеет ключевой инфраструктуры для серьёзного исследования:

- Нет property-based тестирования (proptest).
- Нет воспроизводимых event log / replay (`swarm-replay` — пустой placeholder).
- Нет экспорта результатов в JSON/CSV.
- Нет стабильного `run_id` для идентификации прогонов.
- CLI `strategy_comparison` имеет только `--full`, без `--json`, `--csv`, `--replay-log`.

Критерий "это не песочница" из DRONE_A.1 требует property-based tests и replay. Milestone 7 закрывает этот пробел.

## Investigation context

DRONE_A.3.md и DRONE_B.3.md сходятся в том, что следующий шаг — Experiment Infrastructure:

- `proptest` + replay + structured reports.
- Затем kinematics/battery (Milestone 8).
- Затем SAR (Milestone 9).
- Затем CBBA (Milestone 10).

DRONE_A.3.md подчёркивает платформенность (сначала инфраструктура, потом содержание).
DRONE_B.3.md подчёркивает проверяемость (property-based tests закрывают gap в критерии "не-песочница").

## Affected components

| Компонент | Что меняется |
|-----------|-------------|
| `swarm-replay` | Placeholder → рабочий crate: event log, replay engine, сериализация |
| `swarm-sim` | BenchmarkHarness: экспорт JSON/CSV, run_id |
| `swarm-metrics` | AggregateMetrics: serde для JSON/CSV, возможно flatten |
| `swarm-examples` | `strategy_comparison`: CLI `--json`, `--csv`, `--replay-log` |
| `swarm-scenarios` | Property-based generators для Scenario/RunConfig |
| workspace Cargo.toml | Добавление `proptest`, `csv`, `chrono` (для timestamps) |
| README.md | Новый раздел Milestone 7 |

## Implementation steps

### 1. Property-based generators (`swarm-scenarios`)

**Цель:** Генерация случайных, но валидных `Scenario` и `RunConfig` через `proptest`.

**Файлы:**
- `crates/swarm-scenarios/src/proptest_generators.rs` (новый)

**Что генерируется:**
- `Agent` с валидными poses, capabilities, battery ∈ [10, 100].
- `Task` с валидными priorities, required_capabilities, poses.
- `Scenario` с 3..20 агентами, 5..30 задачами.
- `RunConfig` с packet_loss_rate ∈ [0.0, 0.5], latency_ticks ∈ [0, 5], failures ∈ [0, 3].
- `PartitionEvent` с вероятностью 30%.

**Контракт генераторов:**
- Все generated scenarios должны быть валидны для `ScenarioRunner::run_with`.
- Battery > 0, comms_range > 0, task poses внутри area.

### 2. `swarm-replay`: event log + replay

**Цель:** Сделать `swarm-replay` рабочим crate.

**Файлы:**
- `crates/swarm-replay/src/lib.rs` — основные типы
- `crates/swarm-replay/src/event_log.rs` — структура событий
- `crates/swarm-replay/src/replay.rs` — детерминированный replay
- `crates/swarm-replay/src/serde_support.rs` — сериализация/десериализация

**Event log формат:**
```rust
pub struct EventLog {
    pub run_id: String,
    pub seed: u64,
    pub scenario_name: String,
    pub events: Vec<Event>,
}

pub enum Event {
    TickStart { tick: u64 },
    AgentFailed { agent_id: AgentId, tick: u64 },
    TaskAssigned { task_id: TaskId, agent_id: AgentId, tick: u64 },
    MessageSent { from: AgentId, to: AgentId, tick: u64, payload_len: usize },
    MessageDropped { from: AgentId, to: AgentId, tick: u64, reason: DropReason },
    PartitionAdded { agent_a: AgentId, agent_b: AgentId, tick: u64 },
    PartitionRemoved { agent_a: AgentId, agent_b: AgentId, tick: u64 },
    PoseUpdated { agent_id: AgentId, pose: Pose, tick: u64 },
}
```

**Replay engine:**
- Читает EventLog из JSON.
- Воспроизводит состояние системы tick-by-tick.
- Сравнивает финальные метрики с оригинальным прогоном (assert_eq).

**Интеграция с `swarm-sim`:**
- `ScenarioRunner::run_with` возвращает `(RunMetrics, EventLog)` при включённом логировании.
- Флаг `enable_event_log: bool` в `RunConfig`.

### 3. Structured reports: JSON/CSV export

**Цель:** Экспорт `ComparisonReport` и `AggregateMetrics` в машиночитаемые форматы.

**Файлы:**
- `crates/swarm-sim/src/report_export.rs` (новый)

**JSON формат:**
```json
{
  "run_id": "2026-05-17T12:00:00Z_seed42_ideal_greedy_coverage",
  "strategy_names": ["greedy", "auction"],
  "profile_names": ["ideal-no-failures"],
  "results": {
    "(greedy, ideal-no-failures)": {
      "total_runs": 10,
      "success_rate": 1.0,
      "avg_task_completion_rate": 1.0,
      ...
    }
  }
}
```

**CSV формат:**
- Одна строка на (strategy, profile) пару.
- Колонки: run_id, strategy, profile, total_runs, success_rate, avg_task_completion_rate, ...

### 4. Стабильный `run_id`

**Цель:** Уникальная идентификация каждого прогона.

**Формат:**
```
{timestamp}_{seed}_{profile}_{strategy}_{scenario}
```

Пример: `2026-05-17T120000Z_42_ideal-no-failures_greedy_coverage`

**Где используется:**
- В `ComparisonReport` как поле `run_id`.
- В JSON/CSV экспорте.
- В имени replay log файла.

### 5. CLI-флаги для `strategy_comparison`

**Цель:** Расширить CLI бинарника.

**Новые флаги:**
- `--json <path>`: экспорт ComparisonReport в JSON.
- `--csv <path>`: экспорт ComparisonReport в CSV.
- `--replay-log <dir>`: сохранить EventLog для каждого прогона в директорию.
- `--run-id-prefix <prefix>`: префикс для run_id (для batch-запусков).

**Примеры:**
```bash
# Quick + JSON
cargo run -p swarm-examples --bin strategy_comparison -- --json results.json

# Full + CSV + replay logs
cargo run -p swarm-examples --bin strategy_comparison -- --full --csv results.csv --replay-log ./replays/

# Batch с префиксом
cargo run -p swarm-examples --bin strategy_comparison -- --run-id-prefix batch-2026-05-17
```

### 6. Property-based тесты

**Цель:** Использовать `proptest` для генерации сценариев.

**Файлы:**
- `crates/swarm-scenarios/tests/proptest_scenarios.rs`
- `crates/swarm-sim/tests/proptest_runner.rs`

**Тесты:**
1. `proptest_runner_does_not_panic` — любой валидный scenario + run_config не паникует.
2. `proptest_success_rate_bounded` — success_rate ∈ [0, 1].
3. `proptest_replay_matches_original` — replay финальных метрик совпадает с оригиналом.
4. `proptest_centralized_beats_greedy` — на идеальной сети centralized >= greedy.

**Конфигурация proptest:**
- cases: 100 (quick), 1000 (full).
- Таймаут на case: 5 секунд.

### 7. Актуализация README

**Раздел Milestone 7:**
- Описание proptest, replay, JSON/CSV export.
- Примеры CLI команд.
- Пример JSON output.
- Пример CSV output.

## Testing strategy

### Категория 1: Без рефакторинга

- `swarm-replay` unit tests: сериализация/десериализация EventLog.
- `report_export` unit tests: JSON/CSV round-trip для ComparisonReport.
- `run_id` unit tests: форматирование, уникальность.

### Категория 2: Лёгкий рефакторинг

- `ScenarioRunner` модификация для возврата EventLog.
- `BenchmarkHarness` модификация для run_id и экспорта.
- CLI parsing tests для strategy_comparison.

### Категория 3: Тяжёлый рефакторинг / интеграция

- `proptest` интеграция: генерация 100+ случайных сценариев.
- End-to-end replay: прогон → сохранение log → replay → сравнение метрик.
- Full benchmark (24 profiles × 4 strategies × 1000 seeds) + JSON/CSV экспорт.

## Risks and tradeoffs

| Риск | Вероятность | Влияние | Митигация |
|------|------------|---------|-----------|
| `proptest` замедляет CI | Высокая | Среднее | Separate CI job, configurable case count (100 quick / 1000 full) |
| EventLog раздувает память | Средняя | Высокое | Логировать только ключевые события, опциональное включение |
| Replay engine отличается от оригинала | Средняя | Высокое | Replay — это не перезапуск runner, а state reconstruction из log; assert только на финальные метрики |
| JSON/CSV форматы меняются | Низкая | Среднее | Версионирование в JSON, stable column order в CSV |
| Cargo.lock изменится от новых deps | Высокая | Низкое | `proptest`, `csv`, `chrono` — стабильные crates |

## Open questions

1. **Размер EventLog:** Стоит ли логировать *все* сообщения или только assignment/failure/partition? Полный лог 1000 seeds × 200 ticks × 10 agents = ~2M событий.

2. **Replay fidelity:** Должен ли replay воспроизводить *точное* состояние на каждом tick, или достаточно финальных метрик? Точный tick-by-tick replay сложнее, но ценнее для отладки.

3. **CSV schema:** Фиксированный набор колонок или динамический? AggregateMetrics расширяется каждый milestone.

4. **Mission DSL откладывается:** Сценарии остаются в Rust-коде. Это приемлемо до Milestone 9 (SAR), когда появятся реальные требования к декларативному описанию.
