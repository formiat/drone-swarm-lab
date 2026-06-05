# PLAN.md - M88 Swarm Topologies

## Context

Планируем M88 из [docs_raw/DRONE_A.25.md:827](/home/formi/Documents/RustProjects/drone/docs_raw/DRONE_A.25.md:827): добавить топологии роя на coordination layer без заявлений про RF mesh / radio hardware. Целевая цепочка:

```text
topology -> allowed command routing -> supervisor policy -> artifacts/replay
```

Текущий фундамент:

- M87 command plane уже содержит роли `Relay`, `Mothership`, `Carrier`, `Reserve`, `Recovery` в [crates/swarm-command-plane/src/types.rs:12](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/types.rs:12), per-agent command plans в [crates/swarm-command-plane/src/types.rs:143](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/types.rs:143), ownership/handoff в [crates/swarm-command-plane/src/types.rs:84](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/types.rs:84), sync windows/results в [crates/swarm-command-plane/src/types.rs:124](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/types.rs:124).
- `build_swarm_command_plan` сейчас строит artifact без топологии в [crates/swarm-command-plane/src/fanout.rs:34](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/fanout.rs:34).
- `validate_swarm_command_plan` проверяет schema, duplicate agent plans, active ownership, handoff evidence and replacement policy в [crates/swarm-command-plane/src/validation.rs:49](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/validation.rs:49).
- `swarm-comms` уже имеет range/BFS connectivity model в [crates/swarm-comms/src/connectivity.rs:5](/home/formi/Documents/RustProjects/drone/crates/swarm-comms/src/connectivity.rs:5), in-memory network with partitions/drop/delay в [crates/swarm-comms/src/network.rs:12](/home/formi/Documents/RustProjects/drone/crates/swarm-comms/src/network.rs:12), and generic `Transport` trait в [crates/swarm-comms/src/transport.rs:12](/home/formi/Documents/RustProjects/drone/crates/swarm-comms/src/transport.rs:12).
- Generic replay events already include M87 command-plane events in [crates/swarm-replay/src/event_log.rs:311](/home/formi/Documents/RustProjects/drone/crates/swarm-replay/src/event_log.rs:311), summary counters in [crates/swarm-replay/src/replay/summary.rs:38](/home/formi/Documents/RustProjects/drone/crates/swarm-replay/src/replay/summary.rs:38).
- SITL event log has M87 events/counters in [crates/swarm-examples/src/sitl_observability/events.rs:99](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_observability/events.rs:99) and [crates/swarm-examples/src/sitl_observability/events.rs:951](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_observability/events.rs:951).
- SITL manifest currently accepts per-agent role/policy but not topology in [crates/swarm-examples/src/sitl_multi_agent.rs:28](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_multi_agent.rs:28), and embeds `command_plane_artifact` in [crates/swarm-examples/src/sitl_multi_agent.rs:65](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_multi_agent.rs:65).
- Artifact validator already validates M87 full command-plane artifact in [crates/swarm-examples/src/artifact_validator.rs:847](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/artifact_validator.rs:847).

Граница M88: добавляем логические topology/routing contracts, replay/artifacts and tests. Не реализуем production radio routing, RF mesh stack, consensus guarantees, physical mothership, hardware deployment, or real MAVLink router integration.

Notion/GitLab context: в prompt нет Notion task id и нет GitLab/MR target. Согласно `notion_policy=optional` и прочитанным протоколам, Notion/GitLab чтение не требуется и не выполнялось.

## Investigation context

`INVESTIGATION.md` в workspace отсутствует. План опирается на локальный код, `docs_raw/DRONE_A.25.md`, README/docs and current HEAD `298b9f8f925ee6072cc95fdc1b01d3a04ff89353`.

## Affected components

- `crates/swarm-command-plane/src/types.rs` - добавить M88 topology DTOs and route/dependency artifact fields.
- `crates/swarm-command-plane/src/topology.rs` - новый модуль для deterministic routing / reachability policy.
- `crates/swarm-command-plane/src/fanout.rs` - принять topology input, посчитать route decisions, заполнить artifact summary.
- `crates/swarm-command-plane/src/validation.rs` - проверить topology consistency, blocked routes, mothership dependency graph, relay/mesh assumptions.
- `crates/swarm-command-plane/src/summary.rs` - добавить topology counters.
- `crates/swarm-command-plane/src/lib.rs` - export new topology API and tests.
- `crates/swarm-comms/src/transport.rs` - typed command envelope for testable transport boundary.
- `crates/swarm-comms/src/network.rs` / `crates/swarm-comms/src/connectivity.rs` - reuse/extend InMem connectivity for topology path tests; avoid productionizing old UDP.
- `crates/swarm-replay/src/event_log.rs` and `crates/swarm-replay/src/replay/summary.rs` - generic M88 topology replay events and counters.
- `crates/swarm-examples/src/sitl_observability/events.rs` - SITL M88 events/counters.
- `crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs` - emit topology/routing events together with M87 dispatch.
- `crates/swarm-examples/src/sitl_multi_agent.rs` - optional topology config in `multi_sitl.v1`, artifact embedding, default centralized GCS topology for backwards compatibility.
- `crates/swarm-examples/src/artifact_validator.rs` and `crates/swarm-examples/tests/artifact_validator.rs` - strict validation rules for topology section.
- Docs: `README.md`, `docs/STATUS.md`, `docs/SWARM_COMMAND_PLANE.md`, new `docs/SWARM_TOPOLOGIES.md`, `docs/REPLAY.md`, `docs/ARTIFACT_VALIDATION.md`, `docs/HARDWARE_READINESS.md`, `docs/SITL_SETUP.md`, `docs/SCENARIO_DSL.md`, `docs/OPERATIONAL_RUNBOOKS.md`.

## Implementation steps

1. Добавить M88 topology model в `crates/swarm-command-plane/src/types.rs` и новый модуль `crates/swarm-command-plane/src/topology.rs`.

   Ожидаемый результат: `SwarmCommandPlan` получает serializable topology section and route decisions, а API остаётся hardware-neutral.

   Основные типы:

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum SwarmTopologyKind {
       CentralizedGcs,
       P2pLogical,
       Mothership,
       Relay,
       Mesh,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   pub struct SwarmTopologyConfig {
       pub kind: SwarmTopologyKind,
       pub gcs_node_id: String,
       pub nodes: Vec<SwarmTopologyNode>,
       pub links: Vec<SwarmTopologyLink>,
       pub transport: SwarmTransportAssumptions,
       #[serde(default, skip_serializing_if = "Vec::is_empty")]
       pub mothership_dependencies: Vec<SwarmMothershipDependency>,
   }

   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   pub struct SwarmCommandRoute {
       pub from_node_id: String,
       pub to_agent_id: AgentId,
       pub via_node_ids: Vec<String>,
       pub allowed: bool,
       pub degraded: bool,
       pub reason: String,
   }
   ```

   Code anchors:

   - extend `SwarmCommandPlan` near [crates/swarm-command-plane/src/types.rs:167](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/types.rs:167);
   - add summary counters near [crates/swarm-command-plane/src/types.rs:156](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/types.rs:156);
   - export module from [crates/swarm-command-plane/src/lib.rs:1](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/lib.rs:1).

2. Реализовать deterministic topology routing в `crates/swarm-command-plane/src/topology.rs`.

   Ожидаемый результат: command-plane can answer whether a command path is allowed under centralized GCS, P2P, mothership, relay and mesh modes.

   Логика:

   ```text
   centralized_gcs:
     route from gcs_node_id to every agent; no peer route unless explicitly modeled

   p2p_logical:
     allow direct peer links from topology.links; delivery/drop assumptions stay artifact data

   mothership:
     child command route must pass through mothership/carrier dependency root;
     child mission has parent_mission_id / dependency reason

   relay:
     prefer route through available Relay nodes when direct GCS path is blocked;
     mark degraded when relay unavailable

   mesh:
     BFS over logical links; blocked/partitioned links make route unavailable and replay-visible
   ```

   Reuse:

   - reuse BFS idea from `ConnectivityModel::hop_count_between` in [crates/swarm-comms/src/connectivity.rs:110](/home/formi/Documents/RustProjects/drone/crates/swarm-comms/src/connectivity.rs:110), but keep command-plane routing over explicit topology nodes/links so it is not tied to physical RF range.
   - Use stable sorting by ids before BFS neighbor expansion for deterministic artifacts.

3. Extend `SwarmCommandFanoutInput` and builder in `crates/swarm-command-plane/src/fanout.rs`.

   Ожидаемый результат: `build_swarm_command_plan` accepts optional topology config, defaults to centralized GCS for existing callers, computes `command_routes`, updates summary and validates.

   Concrete changes:

   - add `pub topology: Option<SwarmTopologyConfig>` to [crates/swarm-command-plane/src/fanout.rs:23](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/fanout.rs:23);
   - inside [crates/swarm-command-plane/src/fanout.rs:58](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/fanout.rs:58), build default:

     ```rust
     let topology = input
         .topology
         .unwrap_or_else(|| SwarmTopologyConfig::centralized_gcs_for_agents(&agents));
     let command_routes = route_command_plan(&topology, &agents)?;
     ```

   - keep old unit tests passing by defaulting to `centralized_gcs`.

4. Add topology validation in `crates/swarm-command-plane/src/validation.rs`.

   Ожидаемый результат: invalid topology artifact fails before it reaches SITL/supervisor artifacts.

   Checks:

   - every `SwarmAgentCommandPlan.agent_id` has topology node;
   - `gcs_node_id` exists for centralized topology;
   - no duplicate topology node ids;
   - links reference known nodes;
   - centralized topology has GCS route to each active agent or marks route degraded;
   - P2P route events are rejected if no peer link exists;
   - relay topology requires at least one `SwarmCommandRole::Relay` node when any route claims relay recovery;
   - mothership dependencies reference known parent/child agents and are acyclic;
   - mesh blocked route decisions must include a non-empty reason.

   Add new `SwarmCommandPlaneError` variants near [crates/swarm-command-plane/src/validation.rs:8](/home/formi/Documents/RustProjects/drone/crates/swarm-command-plane/src/validation.rs:8), for example:

   ```rust
   MissingTopologyNode { node_id: String },
   DuplicateTopologyNode { node_id: String },
   UnknownTopologyLinkEndpoint { node_id: String },
   MissingCommandRoute { agent_id: AgentId },
   MothershipDependencyCycle { agent_id: AgentId },
   ```

5. Add typed command transport envelope in `crates/swarm-comms/src/transport.rs`.

   Ожидаемый результат: transport abstraction is testable without network/hardware and can serialize command-routing metadata deterministically.

   Proposed type:

   ```rust
   #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   pub struct CommandEnvelope {
       pub envelope_id: String,
       pub route_id: String,
       pub from: AgentId,
       pub to: AgentId,
       pub payload: Vec<u8>,
       pub topology_kind: String,
   }
   ```

   Keep `RawMessage` unchanged for runtime compatibility. Add conversion helper:

   ```rust
   impl CommandEnvelope {
       pub fn into_raw_message(self) -> RawMessage { ... }
   }
   ```

   Do not promote `swarm-comms/src/udp.rs` to production. If UDP remains referenced, docs must call it legacy/test transport only.

6. Integrate topology config into multi-agent SITL config/manifest in `crates/swarm-examples/src/sitl_multi_agent.rs`.

   Ожидаемый результат: `multi_sitl.v1` can optionally carry topology, and generated `command_plane_artifact` records it.

   Concrete changes:

   - add optional `topology: Option<SwarmTopologyConfig>` to `MultiAgentSitlConfig` near [crates/swarm-examples/src/sitl_multi_agent.rs:28](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_multi_agent.rs:28);
   - add optional/top-level topology summary in `MultiAgentSitlManifest` near [crates/swarm-examples/src/sitl_multi_agent.rs:65](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_multi_agent.rs:65) if useful for compact CLI output;
   - pass topology into `SwarmCommandFanoutInput` in `build_manifest_command_plane` near [crates/swarm-examples/src/sitl_multi_agent.rs:200](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_multi_agent.rs:200);
   - add default centralized GCS topology when config omits topology, so existing scenarios stay valid.

7. Emit M88 topology events in generic replay and SITL observability.

   Ожидаемый результат: partition/degraded/routing behavior is replayable and summary-visible.

   Generic `swarm-replay` additions near [crates/swarm-replay/src/event_log.rs:311](/home/formi/Documents/RustProjects/drone/crates/swarm-replay/src/event_log.rs:311):

   ```rust
   SwarmTopologyConfigured { tick, topology_kind, node_count, link_count },
   SwarmCommandRouteSelected { tick, route_id, from_node_id, to_agent_id, via_node_ids, degraded },
   SwarmCommandRouteBlocked { tick, route_id, from_node_id, to_agent_id, reason },
   SwarmTopologyDegraded { tick, topology_kind, affected_agent_ids, reason },
   SwarmMothershipDependencyRecorded { tick, parent_agent_id, child_agent_id, dependency_kind },
   ```

   Add counters to [crates/swarm-replay/src/replay/summary.rs:38](/home/formi/Documents/RustProjects/drone/crates/swarm-replay/src/replay/summary.rs:38).

   SITL analogues:

   - add variants near [crates/swarm-examples/src/sitl_observability/events.rs:99](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_observability/events.rs:99);
   - add summary counters near [crates/swarm-examples/src/sitl_observability/events.rs:1002](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_observability/events.rs:1002);
   - emit from `record_swarm_command_plane_dispatch` near [crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:662](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:662).

8. Extend artifact validation for topology section in `crates/swarm-examples/src/artifact_validator.rs`.

   Ожидаемый результат: current strict supervisor artifacts cannot claim topology support without topology evidence.

   Add rule ids near existing M87 constants in [crates/swarm-examples/src/artifact_validator.rs:69](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/artifact_validator.rs:69):

   ```text
   artifact.swarm_topology_missing
   artifact.swarm_topology_route_missing
   artifact.swarm_topology_blocked_unreported
   artifact.swarm_mothership_dependency_invalid
   artifact.swarm_transport_assumption_missing
   ```

   Validate inside `validate_swarm_command_plane_manifest` near [crates/swarm-examples/src/artifact_validator.rs:847](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/artifact_validator.rs:847):

   - summary/artifact topology counts match;
   - every active agent has at least one route decision;
   - blocked routes have replay/event evidence where an event log is present;
   - mothership dependencies are acyclic and reference manifest agents;
   - `transport` assumptions include delivery model, delay/drop policy, and explicit hardware boundary.

9. Add topology-aware supervisor policy hooks without rewriting the supervisor into actors.

   Ожидаемый результат: M88 influences policy/reporting in current sequential supervisor while leaving actor-style executor as future work.

   Concrete changes:

   - in `record_swarm_command_plane_dispatch` [crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:662](/home/formi/Documents/RustProjects/drone/crates/swarm-examples/src/sitl_supervisor/supervisor_flows.rs:662), emit route selected/blocked/degraded events before per-agent start;
   - if centralized GCS route to an agent is blocked, mark route degraded and let existing failure/degraded machinery handle run status rather than inventing hardware behavior;
   - if relay route is available, record via-node path but do not claim RF-layer relay;
   - if mothership dependencies exist, emit dependency events and include them in report/artifact only; do not create physical deploy/recover commands beyond mission-level dependency records.

10. Add or update scenario/config fixtures.

   Ожидаемый результат: portable examples exercise all M88 topology kinds.

   Candidate files:

   - `scenarios/sitl.multi-agent.topology.centralized.json`;
   - `scenarios/sitl.multi-agent.topology.p2p.json`;
   - `scenarios/sitl.multi-agent.topology.relay.json`;
   - `scenarios/sitl.multi-agent.topology.mesh-partition.json`;
   - `scenarios/sitl.multi-agent.topology.mothership.json`.

   Keep fixtures small and deterministic. They should use existing local/mock/SITL flow and must not require PX4, ArduPilot, network sockets, serial ports, or hardware.

11. Update docs and user-facing status.

   Обязательно обновить:

   - `README.md`: M88 milestone row and feature table; explicitly say no RF mesh/no production radio routing.
   - `docs/STATUS.md`: M88 complete status once implemented, with limitations.
   - `docs/SWARM_COMMAND_PLANE.md`: link topology as extension of command plane.
   - new `docs/SWARM_TOPOLOGIES.md`: topology kinds, schema, routing semantics, non-goals, examples.
   - `docs/REPLAY.md`: new topology events and summary counters.
   - `docs/ARTIFACT_VALIDATION.md`: topology validation rules.
   - `docs/HARDWARE_READINESS.md`: pre-hardware boundary.
   - `docs/SITL_SETUP.md`: how to run mock/local topology fixtures.
   - `docs/SCENARIO_DSL.md`: if scenario/config schema exposes topology fields.
   - `docs/OPERATIONAL_RUNBOOKS.md`: runbook note for interpreting topology artifacts.

12. Do not run long benchmarks for M88.

   M88 is a schema/routing/supervisor-artifact milestone. Required checks are unit/integration/doc tests. No 500/1000 seed benchmark and no PX4/SIH live run are required unless implementation unexpectedly changes benchmark runner behavior.

## Testing strategy

### 1. Tests that need no refactoring - planned with main implementation

- `crates/swarm-command-plane/src/topology.rs` unit tests:
  - `centralized_topology_routes_all_commands_through_gcs`;
  - `p2p_topology_permits_peer_command_when_link_exists`;
  - `p2p_topology_blocks_peer_command_without_link`;
  - `mesh_topology_routes_over_logical_links_deterministically`;
  - `partition_blocks_command_path_and_marks_degraded`;
  - `mothership_deployment_creates_dependent_child_missions`;
  - `relay_node_improves_model_reachability_when_available`;
  - `relay_topology_marks_degraded_when_relay_unavailable`;
  - `topology_config_serializes_snake_case`.
- `crates/swarm-command-plane/src/validation.rs` unit tests:
  - duplicate topology node rejected;
  - unknown link endpoint rejected;
  - mothership dependency cycle rejected;
  - active agent without route rejected;
  - blocked route without reason rejected.
- `crates/swarm-comms/src/transport.rs` unit test:
  - `command_envelope_serializes_deterministically`.
- `crates/swarm-replay/src/replay/summary.rs` unit test:
  - topology events update summary counters.
- `crates/swarm-examples/src/sitl_observability/read_tests.rs` or colocated tests:
  - SITL topology events roundtrip and summarize.
- `crates/swarm-examples/src/sitl_supervisor/tests_cases.rs`:
  - fake/live supervisor fixture emits topology configured + route selected events.

Commands:

```bash
timeout 300 cargo fmt --all
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-command-plane topology -- --nocapture
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-comms command_envelope -- --nocapture
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-replay topology -- --nocapture
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-examples topology -- --nocapture
timeout 300 /home/formi/.local/bin/runlim cargo clippy --workspace --all-targets -- -D warnings
```

### 2. Tests that need light refactoring

- `crates/swarm-examples/tests/artifact_validator.rs`:
  - valid tiny supervisor pack includes topology section;
  - missing topology in strict current artifact fails;
  - blocked route without event evidence fails when event log is present;
  - invalid mothership dependency fails.
- `crates/swarm-examples/tests/sitl_agent.rs` or split support files:
  - `multi_agent_sitl_supervisor_topology_manifest_file_test`;
  - `multi_agent_sitl_supervisor_mesh_partition_mock_test`;
  - `multi_agent_sitl_supervisor_mothership_dependency_artifact_test`.
- `crates/swarm-examples/tests/sitl_docs.rs`:
  - docs mention M88 non-goals: no RF mesh stack, no physical mothership, no production radio routing;
  - docs list required topology event names.
- Fixture support helper for building topology manifests in `crates/swarm-examples/tests/artifact_validator.rs` or a shared test support module.

Commands:

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test artifact_validator topology -- --nocapture
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_agent topology -- --nocapture
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 timeout 300 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test sitl_docs m88 -- --nocapture
```

### 3. Tests that need heavy refactoring

- Actor-style swarm executor where multiple agents actively run command routes concurrently.
- CBBA/gossip over topology-aware abstract transport instead of current simple peer message loop.
- Network partition recovery with partial command logs and delayed/duplicate delivery windows.
- Repeated stochastic topology degradation sweeps over generated graphs.
- Long benchmark/regression evidence for topology-aware mission outcomes.

These are not required for M88 done criteria. They should be planned as follow-up if M89/SITL evidence needs deeper topology runtime behavior.

## Risks and tradeoffs

- Schema growth: `SwarmCommandPlan` and `multi_sitl.v1` artifacts will grow. Mitigation: use defaults/`Option` for backwards compatibility where possible, document historical validator mode.
- Layer confusion: topology could be mistaken for RF mesh implementation. Mitigation: docs and artifact fields must say `logical` / `coordination_layer` / `no_rf_stack`.
- Dependency direction: `swarm-command-plane` already depends on `swarm-comms`; avoid adding reverse dependency from `swarm-comms` to command-plane.
- Runtime semantics: current supervisor is not an actor-style distributed executor. M88 should emit and validate topology-aware decisions, not claim concurrent distributed command execution.
- Performance: BFS route computation is cheap for current small fixtures, but repeated per-agent route recomputation can become noisy. Mitigation: compute routes once per command-plane build and store them in artifact.
- Artifact validator strictness: old artifacts without topology must remain historical-compatible; current strict artifacts should fail only when they claim M88/current schema.
- Mothership wording: `mothership` is a coordination role, not a physical vehicle. Docs and event reasons must avoid physical deploy/recover claims unless backed by command primitives.

## Open questions

- Нужно ли в M88 повышать `swarm_command_plane.v1` до `swarm_command_plane.v2`, или оставить additive optional fields внутри v1? Предпочтение: оставить v1, если serde defaults сохраняют совместимость; поднять schema только если validator strict semantics становятся несовместимыми.
- Должна ли topology config жить только в `SwarmCommandPlan`, или также в Scenario DSL? Предпочтение: для M88 начать с `multi_sitl.v1`/command-plane artifact; Scenario DSL field добавить только если fixture ergonomics требуют этого.
- Должен ли `CommandEnvelope` заменить `RawMessage` в runtime? Предпочтение: нет, для M88 добавить envelope как boundary/test DTO и conversion helper, не переписывать runtime transport.
- Нужно ли сохранять UDP prototype? Предпочтение: не удалять в M88, но явно документировать как legacy/test transport and not production radio layer.

## Что могло сломаться

- Artifact compatibility: strict current supervisor validation может начать требовать topology evidence. Проверка: `cargo test -p swarm-examples --test artifact_validator topology`.
- Replay compatibility: новые event variants/counters могут изменить exact summary output. Проверка: `cargo test -p swarm-replay topology` and `cargo test -p swarm-examples --test replay_cli`.
- SITL manifest compatibility: новые optional fields must not break old `multi_sitl.v1` configs. Проверка: existing `multi_agent_sitl_supervisor_*` tests in `cargo test -p swarm-examples --test sitl_agent`.
- Command-plane validation: default centralized topology must not break existing M87 tests. Проверка: full `cargo test -p swarm-command-plane`.
- Runtime transport: `CommandEnvelope` must not change `RawMessage` behavior. Проверка: `cargo test -p swarm-comms network transport`.
- Documentation claims: README/docs must not imply RF mesh, physical mothership, consensus, or hardware readiness. Проверка: `cargo test -p swarm-examples --test sitl_docs m88`.
