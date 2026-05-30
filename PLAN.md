# PLAN.md - M52 Multi-Agent SITL Foundation

## Context

Идем по ветке Real SITL / PX4 из `docs_raw/DRONE_A.17.md`.
M43-M51 уже закрыли single-agent SITL foundation: dry-run/mock path,
MAVLink mission upload, pre-upload safety, execute lifecycle, telemetry mapping,
SITL observability/replay, portable regression checks и dynamic reallocation на
runtime/mock уровне.

M52 должен перейти от single-agent SITL к нескольким агентам без преждевременной
алгоритмической сложности. Это не milestone про distributed coordination,
CBBA-rewrite или real hardware. Нужен foundation:

- явный mapping `agent_id -> MAVLink system/component/connection`;
- явный mapping `agent_id -> assigned task subset`;
- multi-agent dry-run manifest;
- поддержка режима "несколько `sitl_agent` процессов";
- поддержка supervisor process для dry-run/mock orchestration;
- проверка no duplicate task ownership before upload.

Текущее состояние кода:

- `crates/swarm-examples/src/bin/sitl_agent.rs` принимает один `--agent-id` и
  строит single-agent `SitlPlan`.
- `crates/swarm-examples/src/sitl_plan.rs` сейчас извлекает все pose tasks из
  первого SITL scenario entry и не фильтрует их по agent/task ownership.
- `crates/swarm-comms/src/mavlink.rs` уже имеет `MissionUploadOptions` с
  `target_system` и `target_component`, но `sitl_agent` пока всегда использует
  defaults.
- `README.md` и `docs/SITL_SETUP.md` честно фиксируют single-agent SITL
  limitation. Их нужно обновить обязательно.

## Investigation context

`INVESTIGATION.md` отсутствует. План основан на:

- `docs_raw/DRONE_A.17.md`;
- текущих `README.md` и `docs/SITL_SETUP.md`;
- `crates/swarm-examples/src/bin/sitl_agent.rs`;
- `crates/swarm-examples/src/sitl_plan.rs`;
- `crates/swarm-examples/tests/sitl_agent.rs`;
- `crates/swarm-comms/src/mavlink.rs`;
- `crates/swarm-sim/src/dsl.rs`.

Notion/GitLab контекст не использовался: policy optional, task/MR id в prompt
не задан.

## Affected components

- `crates/swarm-examples/src/sitl_multi_agent.rs` - новый модуль с typed config,
  validation, task subset split и dry-run manifest.
- `crates/swarm-examples/src/lib.rs` - экспорт нового модуля.
- `crates/swarm-examples/src/sitl_plan.rs` - построение `SitlPlan` по явному
  subset task ids, без изменения старого single-agent поведения.
- `crates/swarm-examples/src/bin/sitl_agent.rs` - optional shared multi-agent
  config для режима "несколько процессов"; применение per-agent connection,
  lifecycle, start delay, target system/component и task subset.
- `crates/swarm-examples/src/bin/sitl_supervisor.rs` - новый supervisor process
  для multi-agent dry-run/mock foundation.
- `crates/swarm-examples/tests/sitl_agent.rs` - CLI regression tests для
  single-agent compatibility и multi-agent config path.
- `crates/swarm-examples/tests/sitl_docs.rs` - anchors для README/SITL docs.
- `README.md` - обязательная актуализация quick start/status/limitations.
- `docs/SITL_SETUP.md` - новая секция Multi-Agent SITL Foundation.
- `docs/STATUS.md` - заменить устаревшую формулировку "Multi-agent SITL not yet
  supported" на "foundation only: dry-run/mock/manual PX4".

## Implementation steps

1. Добавить typed config и manifest в
   `crates/swarm-examples/src/sitl_multi_agent.rs`.

   Предлагаемый JSON config без новых зависимостей:

   ```json
   {
     "schema_version": "multi_sitl.v1",
     "agents": [
       {
         "agent_id": "agent-0",
         "system_id": 1,
         "component_id": 1,
         "connection_string": "udp:127.0.0.1:14550",
         "start_delay_ms": 0,
         "lifecycle": "upload_only",
         "task_ids": ["wp-0", "wp-1"]
       },
       {
         "agent_id": "agent-1",
         "system_id": 2,
         "component_id": 1,
         "connection_string": "udp:127.0.0.1:14560",
         "start_delay_ms": 250,
         "lifecycle": "execute",
         "task_ids": ["wp-2"]
       }
     ]
   }
   ```

   Ввести типы:

   - `MultiAgentSitlConfig`;
   - `MultiAgentSitlAgentConfig`;
   - `MultiAgentLifecycle` с `#[serde(rename_all = "snake_case")]`:
     `UploadOnly`, `Execute`;
   - `MultiAgentSitlManifest`;
   - `MultiAgentSitlManifestAgent`;
   - `TaskOwnershipSummary`;
   - typed `MultiAgentSitlError` или расширение `SitlError`, если проще
     сохранить единый error surface CLI.

   Validation rules:

   - `schema_version == "multi_sitl.v1"`;
   - `agents` не пустой;
   - `agent_id` не пустой и существует в scenario agents;
   - `system_id` в диапазоне `1..=255`;
   - `component_id` допустим как `0..=255`, но значение должно быть явно
     указано в config;
   - `connection_string` валидируется через существующий
     `validate_connection_string`;
   - `start_delay_ms` хранится как `u64`;
   - `task_ids` не пустой;
   - каждый `task_id` существует в scenario и имеет `pose`;
   - один `task_id` не может быть назначен двум agent configs;
   - неизвестные/дублирующиеся agent ids отклоняются;
   - для M52 не делать auto-allocation: split явный, deterministic and
     inspectable.

2. Добавить subset-aware планирование в
   `crates/swarm-examples/src/sitl_plan.rs`.

   Сохранить текущий public path:

   - `build_sitl_plan(...)` продолжает отдавать все pose tasks и не ломает
     existing single-agent tests.

   Добавить новый helper:

   - `build_sitl_plan_for_task_ids(suite, scenario_path, agent_id, task_ids)`;
   - или общий internal helper, принимающий optional allowed task id set.

   Контракт:

   - порядок waypoint-ов соответствует порядку tasks в scenario, а не порядку
     в config, чтобы output был стабильным;
   - `seq` перенумеровывается внутри subset с `0`;
   - пустой subset возвращает typed error;
   - task without pose возвращает typed error до upload/dry-run.

3. Интегрировать config в `crates/swarm-examples/src/bin/sitl_agent.rs` для
   режима "several `sitl_agent` processes".

   Добавить CLI option:

   - `--multi-agent-config <path>`.

   Поведение:

   - без `--multi-agent-config` старый single-agent path не меняется;
   - с config `sitl_agent` выбирает entry по `--agent-id`;
   - если `--connection` не задан явно, берёт `connection_string` из config;
   - если `--upload-only`/`--execute` не заданы явно, берёт lifecycle из config;
   - CLI flags имеют приоритет над config только для connection/lifecycle, но
     task subset всегда берётся из config;
   - перед `run_connection` вызывается full config validation, включая duplicate
     ownership, чтобы duplicate task ownership был rejected before upload and
     before feature gate/network connection;
   - `start_delay_ms` применяется перед mock/connection execution, но dry-run
     только печатает его в manifest/output;
   - `MissionUploadOptions.target_system` и `target_component` берутся из
     per-agent config.

4. Добавить supervisor process в
   `crates/swarm-examples/src/bin/sitl_supervisor.rs`.

   Минимальный M52 scope:

   - `sitl_supervisor --dry-run --scenario <path> --config <path>`;
   - optional `--manifest <path>`: пишет pretty JSON manifest, иначе печатает в
     stdout;
   - `sitl_supervisor --mock --scenario <path> --config <path>`: запускает
     per-agent mock path внутри одного process, уважает `start_delay_ms`,
     показывает per-agent waypoint subset и ownership summary;
   - real `--connection` orchestration не делать в M52 как автоматический CI
     workflow. Для manual several-process mode supervisor должен печатать
     command lines для каждого `sitl_agent`.

   Manifest должен содержать:

   - scenario path/name, mission/profile;
   - agents count;
   - per agent: agent id, system/component id, connection string, lifecycle,
     start delay, task ids, waypoint count, waypoint seq/x/y/z;
   - ownership summary: total tasks, assigned tasks, unassigned pose tasks,
     duplicate task ids;
   - generated command line for each standalone `sitl_agent` process.

5. Привязать duplicate ownership rejection к upload boundary.

   Важно: duplicate ownership должен падать до:

   - MAVLink feature gate;
   - `MavlinkTransport::new`;
   - safety upload;
   - mission upload.

   Это дает portable negative tests без PX4 и без `mavlink-transport`.

6. Обновить документацию.

   `README.md`:

   - добавить quick start пункт "Inspect multi-agent SITL manifest";
   - добавить M52 status row: `Multi-Agent SITL Foundation | Stable/Experimental`
     с честной формулировкой "dry-run/mock/config foundation, no real
     multi-agent PX4 guarantee";
   - обновить Known Limitations: multi-agent SITL foundation есть, но real
     multi-agent PX4 orchestration/hardware safety не готовы;
   - добавить команды targeted tests для M52.

   `docs/SITL_SETUP.md`:

   - добавить секцию "Multi-Agent SITL Foundation";
   - показать пример config;
   - показать два режима:
     - several `sitl_agent` processes with shared config;
     - `sitl_supervisor --dry-run/--mock`;
   - явно написать, что robust distributed coordination и real multi-agent PX4
     CI остаются out of scope.

   `docs/STATUS.md`:

   - обновить статус multi-agent SITL с "not supported" на "M52 foundation:
     config/dry-run/mock/manual process split".

7. Обновить tests/docs anchors.

   `crates/swarm-examples/tests/sitl_docs.rs` должен проверять новые README и
   SITL_SETUP anchors:

   - `Multi-Agent SITL Foundation`;
   - `sitl_supervisor`;
   - `--multi-agent-config`;
   - `duplicate ownership`.

8. Не добавлять новые external dependencies.

   Использовать `serde`/`serde_json`, `std::time::Duration` на runtime side и
   `u64 start_delay_ms` в config. `Cargo.lock` не должен меняться.

## Testing strategy

### 1. Tests that need no refactoring - planned with main implementation

Unit tests in `crates/swarm-examples/src/sitl_multi_agent.rs`:

- `multi_agent_config_parse_test`: валидный JSON config парсится, lifecycle
  snake_case roundtrips.
- `agent_connection_config_parse_test`: mapping
  `agent_id/system_id/component_id/connection_string/start_delay_ms` читается
  корректно.
- `multi_agent_config_rejects_empty_agents`: negative-path для пустого config.
- `multi_agent_config_rejects_duplicate_agent_id`: duplicate agent ids.
- `multi_agent_config_rejects_bad_connection_string`: использует existing
  `validate_connection_string`.
- `multi_agent_config_rejects_invalid_system_id_zero`: semantic validation,
  потому что MAVLink system id `0` нельзя использовать как конкретный target.
- `task_split_test`: два агента получают разные waypoint subsets, seq
  перенумеровывается внутри каждого subset.
- `duplicate_ownership_rejection_test`: один `task_id` в двух agent entries
  возвращает typed error с task id и agent ids.
- `unknown_task_id_rejection_test`: config с task id, которого нет в scenario,
  падает до dry-run/upload.
- `task_without_pose_rejection_test`: SITL task subset не может содержать task
  без pose.
- `multi_agent_dry_run_manifest_test`: manifest содержит per-agent connection
  strings, system/component ids, lifecycle, start delays, task ids и ownership
  summary.
- `unassigned_pose_tasks_are_reported_test`: pose tasks, не попавшие ни в один
  subset, не блокируют foundation, но отражаются в manifest summary.

Unit tests in `crates/swarm-examples/src/sitl_plan.rs`:

- `build_sitl_plan_for_task_ids_filters_subset`;
- `build_sitl_plan_for_task_ids_preserves_scenario_order`;
- `build_sitl_plan_for_task_ids_rejects_empty_subset`;
- `build_sitl_plan_legacy_path_still_returns_all_pose_tasks`.

Integration/CLI tests in `crates/swarm-examples/tests/sitl_agent.rs`:

- `multi_agent_dry_run_output_test`: `sitl_agent --dry-run --multi-agent-config`
  для `agent-0` печатает только его subset.
- `multi_agent_duplicate_ownership_rejected_before_upload_test`: `sitl_agent
  --connection ... --multi-agent-config duplicate.json` падает на duplicate
  ownership и не доходит до feature missing / network.
- `multi_agent_config_connection_used_when_cli_connection_missing_test`: для
  dry-run/manifest path видно connection из config.
- `multi_agent_config_cli_connection_override_test`: если CLI connection задан,
  output/manifest использует override.

Integration tests for new supervisor binary:

- `sitl_supervisor_dry_run_manifest_stdout_test`;
- `sitl_supervisor_dry_run_manifest_file_test`;
- `sitl_supervisor_mock_runs_two_agents_with_distinct_subsets_test`;
- `sitl_supervisor_duplicate_ownership_rejected_test`.

Docs tests:

- `sitl_docs_mentions_multi_agent_foundation`;
- update existing `sitl_docs` anchors for README/SITL_SETUP.

Recommended verification commands for implementation:

```bash
timeout 300s cargo fmt --all
timeout 300s cargo clippy --workspace --all-targets --all-features -- -D warnings
timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples sitl_multi_agent
timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent multi_agent
timeout 300s env PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs
```

### 2. Tests that need light refactoring

- Вынести общий inline scenario fixture из `crates/swarm-examples/tests/sitl_agent.rs`
  в small helper внутри test file или reusable test helper, чтобы multi-agent
  tests не копировали большой JSON несколько раз.
- Если `parse_args()` в `sitl_agent.rs` окажется трудно тестировать через CLI,
  вынести parsing в helper that accepts `impl IntoIterator<Item = String>`.
- Для supervisor mock path может понадобиться маленький `SupervisorRunner` /
  `SupervisorDriver` seam, чтобы не тестировать через реальные child processes.
- Если manifest formatting будет строковым, лучше тестировать parsed JSON
  manifest, а не brittle stdout substrings.

### 3. Tests that need heavy refactoring

- Real multi-agent PX4 SITL integration test: два PX4 instances / два MAVLink
  endpoints, два system ids, independent mission upload/execute. Это должно
  быть `#[ignore]`/manual или отдельный local script, потому что требует внешние
  simulator processes and endpoint orchestration.
- Concurrent supervisor connection-mode test with spawned `sitl_agent`
  processes and fake MAVLink endpoints. Это полезно позже, но для M52 можно
  ограничиться dry-run/mock supervisor tests.
- End-to-end distributed coordination + SITL failure recovery. Это belongs to
  later milestones, not M52 foundation.

## Risks and tradeoffs

- Explicit task subset вместо auto-allocation снижает "умность", но делает M52
  deterministic, inspectable and testable. Алгоритмическое распределение лучше
  добавить позже отдельным milestone.
- Новая config schema может стать публичным контрактом. Поэтому нужен
  `schema_version` сразу и typed validation errors.
- `component_id = 0` спорный: в MAVLink это часто broadcast-like значение, но в
  текущих tests/командах уже встречается `target_component = 0`. План допускает
  `0..=255` для component id, но требует explicit value.
- Supervisor process в M52 не должен обещать production orchestration. Его
  основная ценность - dry-run manifest, mock execution и generated commands для
  manual multi-process workflow.
- Если добавить `--multi-agent-config` прямо в `sitl_agent`, CLI усложнится.
  Компенсация: старый single-agent path должен остаться unchanged, а новые
  опции должны быть optional.
- Проверка duplicate ownership before upload может изменить порядок ошибок для
  connection mode. Это желаемо для M52, но tests должны закрепить порядок:
  config/ownership errors раньше feature/network/upload.
- Per-agent `start_delay_ms` в tests должен быть малым или обходиться fake clock,
  чтобы не замедлять CI.

## Open questions

- Нужен ли в M52 только JSON config или сразу YAML/RON? Рекомендация: JSON,
  потому что workspace уже использует JSON scenario suites и не нужны новые
  dependencies.
- Должен ли `sitl_supervisor --connection` реально spawn-ить несколько
  `sitl_agent` процессов уже в M52? Рекомендация: нет; в M52 достаточно
  generated commands + dry-run/mock supervisor. Реальный concurrent PX4 path
  оставить manual/ignored.
- Разрешать ли fallback task subset из `Task.assigned_to`, если `task_ids` не
  указан в config? Рекомендация: нет для M52. Явный config проще валидировать и
  объяснять.
- Должен ли duplicate ownership блокировать unassigned pose tasks? Рекомендация:
  duplicate ownership - hard error; unassigned pose tasks - warning/summary в
  manifest, потому что M52 может запускать partial mission subsets.
- Нужно ли сохранять manifest только в JSON или ещё human text? Рекомендация:
  JSON manifest как machine artifact плюс компактный human summary в stdout.

## Что могло сломаться

- CLI compatibility: новые options в `sitl_agent` не должны менять старые
  команды `--mock`, `--dry-run`, `--connection`. Проверять existing
  `sitl_agent` integration tests.
- Error ordering: connection mode может начать падать на multi-agent config
  validation раньше `feature missing` или network errors. Это ожидаемо только
  когда передан `--multi-agent-config`; без config порядок должен остаться
  прежним.
- MAVLink target routing: неправильная прокидка `system_id/component_id` в
  `MissionUploadOptions` приведет к upload на не тот PX4 instance. Проверять
  fake/observed upload options unit tests and manual PX4 logs.
- Task ownership semantics: если subset filtering сделан неверно, два агента
  могут получить один waypoint или потерять waypoint. Проверять duplicate
  rejection, subset tests, manifest summary and dry-run output.
- JSON schema consumers: новый multi-agent manifest станет отдельным артефактом;
  downstream scripts могут зависеть от его полей. Нужен `schema_version` и
  docs.
- Test runtime: supervisor mock with real sleeps может замедлить tests.
  Использовать малые delays или fake delay seam.
- Docs honesty: README/STATUS должны не обещать real multi-agent PX4 readiness.
  Проверять docs anchors and limitations text.
