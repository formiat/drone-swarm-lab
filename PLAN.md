# Context

Запланировать M68 `Algorithm Depth On Urban + Existing Missions` по
`docs_raw/DRONE_C.21.md`.

Текущая база уже содержит M64-M67 Urban substrate: `UrbanMap`, road graph,
детерминированный Dijkstra, AABB static obstacle judge, Urban Patrol/Search,
replay timeline, route trace, judge report, двухагентную analysis fixture и
диагностические separation/conflict metrics. M68 должен добавить не просто
новый параметр, а одно измеримое algorithmic improvement с понятным
benchmark delta, обновленной support matrix и честным статусом слабых /
unsupported пар.

Основной выбранный improvement для M68: **urban corridor-aware planner**.

Причины выбора:

- это самый прямой следующий шаг после Urban Patrol/Search/Replay;
- `UrbanEdge.corridor_width_m` уже есть в DSL/types, но текущий Dijkstra
  фактически планирует только по `edge.cost`;
- improvement можно проверить на маленьком deterministic Urban scenario без
  500/1000-seed long run;
- результат можно измерять не только временем/длиной, но и новым route-risk /
  clearance-oriented metric;
- это не конфликтует с текущей архитектурной границей: проект остается
  mission-level planning / judge / replay layer, а не low-level flight control.

Не выбранные primary improvements остаются ограниченным анализом в M68:

- `communication-aware scoring` слишком широко затрагивает allocator API и
  все миссии; его лучше делать после доказанного Urban delta;
- `wildfire priority-triggered reallocation` полезен, но привязан к старому
  wildfire interpretation debt;
- `SAR uncertainty-aware planner` потенциально крупный отдельный этап, потому
  что SAR success predicate и unsupported matrix уже имеют известные сложные
  места.

`PLAN.md` до этого запуска отсутствовал. `INVESTIGATION.md` отсутствует.
Данные Notion/GitLab не читались: prompt не содержит Notion task id, MR или
GitLab target, а политика Notion в inbox указана как `optional`.

# Investigation context

`INVESTIGATION.md` в корне workspace отсутствует, поэтому отдельных findings
из investigation нет.

Локально проверены:

- `docs_raw/DRONE_C.21.md` — roadmap M63-M70 и требования M68;
- `README.md` — текущий статус M64-M67, strategy support matrix, benchmark
  status;
- `docs/STATUS.md` — M67 отмечен как diagnostic tooling, M68 указан как
  следующий Urban/algorithm work;
- `docs/BENCHMARK_RESULTS.md` — M62 pack исторический, Urban M64-M67 не
  являются benchmark refresh;
- `docs/SCENARIO_DSL.md` — Urban DSL сейчас допускает только planner
  `"dijkstra"`;
- `docs/EXTENSION_GUIDE.md` — Urban extension path и metric/report checklist;
- `docs/REPLAY.md` — Urban replay/timeline/analysis artifact semantics;
- `crates/swarm-types/src/urban.rs` — `UrbanEdge.corridor_width_m` уже есть;
- `crates/swarm-sim/src/urban.rs` — текущий `plan_route` игнорирует planner
  string и строит shortest path только по `edge.cost`;
- `crates/swarm-sim/src/runner.rs` — `UrbanState.planner: String`, Urban
  Patrol/Search вызывают `expand_route_loop`, а не planner-aware variant;
- `crates/swarm-sim/src/dsl.rs` — validation сейчас требует planner
  `"dijkstra"` для `urban-patrol` и `urban-search`;
- `crates/swarm-scenarios/src/urban.rs` — встроенные Urban profiles пока:
  `patrol-small-block`, `multi-agent-small-block`, `search-*`;
- `crates/swarm-sim/src/support_matrix.rs` и
  `crates/swarm-examples/tests/support_matrix.rs` — support matrix уже
  фиксирует SAR/CBBA и SAR/centralized unsupported reasons;
- `crates/swarm-sim/tests/scenario_catalog.rs` — каталог уже проверяет Urban
  fixtures и Urban replay analysis metrics.

# Affected components

- `crates/swarm-types/src/urban.rs`
  - возможно добавить lightweight route-risk/clearance input fields только
    если текущих `corridor_width_m` и AABB obstacles недостаточно;
  - предпочтительно не менять DSL schema несовместимо.
- `crates/swarm-sim/src/urban.rs`
  - parser/enum для Urban planner mode;
  - planner-aware route expansion;
  - corridor-aware edge scoring;
  - route risk / clearance helper;
  - unit tests for baseline vs corridor-aware routes.
- `crates/swarm-sim/src/runner.rs`
  - использовать `urban_state.planner` при Urban Patrol/Search;
  - populate new route-risk metrics in `RunMetrics`;
  - keep `"dijkstra"` as default/backward-compatible behavior.
- `crates/swarm-sim/src/dsl.rs`
  - разрешить `"dijkstra"` и `"corridor-aware"`;
  - добавить validation for unknown planner;
  - сохранить старые fixtures валидными.
- `crates/swarm-scenarios/src/urban.rs`
  - добавить deterministic risky-corridor profile/fixture builder if useful;
  - не включать 8/16-agent scaling profile, если оно не нужно для выбранного
    planner delta.
- `scenarios/urban.corridor-delta.json`
  - новый scenario-suite с before/after profiles:
    `corridor-delta-dijkstra` и `corridor-delta-corridor-aware`.
- `crates/swarm-metrics/src/metrics.rs`
  - добавить additive/defaulted per-run и aggregate route-risk metric, если
    corridor-aware benefit нельзя честно показать существующими метриками.
- `crates/swarm-sim/src/benchmark.rs` и
  `crates/swarm-sim/src/report_export.rs`
  - вывести новые metrics в JSON/CSV/Markdown только если они нужны для
    M68 delta interpretation.
- `crates/swarm-sim/src/support_matrix.rs`
  - добавить Urban corridor-aware support classification.
- `crates/swarm-examples/src/bin/strategy_comparison.rs`
  - убедиться, что scenario-suite mode и benchmark pack могут сохранить M68
    delta artifact с Urban analysis, без специального long-run harness.
- `crates/swarm-sim/tests/scenario_catalog.rs`
  - добавить fixture/catalog checks для M68 corridor delta.
- `crates/swarm-examples/tests/support_matrix.rs`
  - добавить support matrix tests for Urban stable/experimental/unsupported
    statuses.
- `crates/swarm-examples/tests/benchmark_pack.rs`
  - smoke-test benchmark delta pack / manifest if cheap.
- Documentation:
  - `README.md`;
  - `docs/STATUS.md`;
  - `docs/BENCHMARK_RESULTS.md`;
  - `docs/SCENARIO_DSL.md`;
  - `docs/EXTENSION_GUIDE.md`;
  - `docs/REPLAY.md` only if replay wording/artifacts change;
  - optional new `docs/CBBA_ANALYSIS.md` or a concise section in
    `docs/BENCHMARK_RESULTS.md`.
- Result artifacts:
  - `results/m68_urban_corridor_delta/README.md`;
  - generated `manifest.json`, `results.json`, `results.csv`, `table.md`;
  - replay/urban_analysis artifacts if `--replay-log` is used.

# Implementation steps

1. Add planner mode parsing and route scoring.
   - In `crates/swarm-sim/src/urban.rs`, add a small
     `UrbanPlannerMode` enum or equivalent parser accepting:
     - `"dijkstra"`;
     - `"corridor-aware"`.
   - Keep `plan_route(...)` as the backward-compatible Dijkstra wrapper.
   - Add `plan_route_with_mode(...)` and
     `expand_route_loop_with_planner(...)`.
   - Implement corridor-aware score as a deterministic edge score:
     `base edge.cost + corridor/clearance risk penalty`.
   - Prefer fixed policy constants for M68 instead of exposing many knobs.
     The goal is one interpretable planner, not a parameter farm.
   - Use existing `UrbanEdge.corridor_width_m` and AABB obstacles first.
     Add shared helper only after searching for existing geometry/clearance
     helpers in the repo.

2. Add route-risk metric only if needed for honest delta.
   - In `crates/swarm-sim/src/urban.rs`, add a helper such as
     `route_risk_score(map, route)` or `route_clearance_score(map, route)`.
   - In `crates/swarm-metrics/src/metrics.rs`, add defaulted fields such as:
     - `urban_route_risk_score`;
     - `avg_urban_route_risk_score`.
   - In `crates/swarm-sim/src/runner.rs`, populate the metric for Urban
     Patrol/Search.
   - In `crates/swarm-sim/src/benchmark.rs` and report export paths, expose
     it only if it is used in the committed M68 delta interpretation.
   - Expected improvement: corridor-aware route has lower risk score while
     staying valid and completed; expected tradeoff: route length/time may be
     higher.

3. Wire planner mode into Urban Patrol/Search.
   - In `crates/swarm-sim/src/runner.rs`, replace direct
     `expand_route_loop(&map, &route_loop)` calls with planner-aware expansion
     based on `urban_state.planner`.
   - Preserve Dijkstra behavior for existing scenarios and old JSON.
   - Ensure invalid planner values return explicit `unsupported_reason` or DSL
     validation errors, not panic.
   - Update `compute_urban_foundation_metrics(...)` to use the same planner
     semantics as the runtime path.

4. Update DSL validation.
   - In `crates/swarm-sim/src/dsl.rs`, change Urban planner validation from
     "must be dijkstra" to "must be one of dijkstra/corridor-aware".
   - Validation must still:
     - reject unknown planner strings;
     - validate route loop;
     - run judge on the planner-selected route;
     - check start pose contract.
   - Keep schema version `0.1` unless an incompatible DSL structure is added.

5. Add a deterministic M68 Urban corridor delta fixture.
   - Add `scenarios/urban.corridor-delta.json`.
   - Include two scenario entries over the same map:
     - `urban-patrol / corridor-delta-dijkstra`;
     - `urban-patrol / corridor-delta-corridor-aware`.
   - Construct a road graph with:
     - a short/narrow risky shortcut;
     - a longer/wider safer path;
     - AABB building/no-fly obstacles close enough to affect risk but not
       necessarily produce a judge violation for both routes.
   - The before/after must be deterministic and visible in metrics:
     - Dijkstra: shorter route, higher route-risk;
     - corridor-aware: lower route-risk, still completes, with explicit
       distance/time tradeoff.
   - Do not add 8/16-agent profile in M68 unless the selected planner cannot
     be evaluated without it. For this planner, scaling is not required.

6. Add benchmark delta artifact.
   - Build/run a small current-HEAD delta, not a publication benchmark:

     ```bash
     /home/formi/.local/bin/runlim cargo run -p swarm-examples --bin strategy_comparison -- \
       --scenario-suite scenarios/urban.corridor-delta.json \
       --output-dir results/m68_urban_corridor_delta \
       --replay-log results/m68_urban_corridor_delta/replay \
       --jobs 4
     ```

   - If this debug run is too slow, run the already-built binary or release
     build, but still keep the run bounded by `/home/formi/.local/bin/runlim`.
   - Commit generated result files only after reviewing that they are small,
     deterministic enough, and include a human-readable README.
   - Add `results/m68_urban_corridor_delta/README.md` explaining:
     - exact command;
     - current HEAD commit;
     - compared profiles;
     - primary metric improvement;
     - tradeoff in distance/time;
     - why this is M68 algorithm evidence, not M69 full benchmark refresh.

7. CBBA weak/unsupported pair analysis.
   - Re-read `docs/BENCHMARK_RESULTS.md` M62 weak rows:
     - coverage CBBA high-loss/high-latency rows;
     - SAR CBBA delayed reconvergence;
     - SAR centralized static pre-plan.
   - Add a small replay-enabled diagnostic only if it stays short:

     ```bash
     /home/formi/.local/bin/runlim cargo run -p swarm-examples --bin strategy_comparison -- \
       --smoke \
       --mission coverage \
       --planner nn \
       --replay-log results/m68_cbba_diagnostics/replay \
       --output-dir results/m68_cbba_diagnostics \
       --jobs 4
     ```

   - If the existing CLI cannot isolate the weak CBBA row cheaply, do not add
     a broad run. Instead document that M68 analysis uses existing M62 evidence
     plus targeted support-matrix tests, and leave richer convergence replay
     helper to the light-refactor bucket.
   - Do not implement failure-triggered gossip burst unless the replay
     evidence shows a concrete reconvergence gap that is parameter-fixable
     rather than an inherent mission/strategy mismatch.
   - Update docs so unsupported pairs are described as unsupported/known
     limitation with reason, not as generic failures.

8. Update support matrix.
   - In `crates/swarm-sim/src/support_matrix.rs`, add explicit Urban entries:
     - existing `urban-patrol / patrol-small-block / greedy` stable;
     - `urban-search / search-static-bus / greedy` stable already exists;
     - M68 `urban-patrol / corridor-delta-dijkstra / greedy` stable baseline;
     - M68 `urban-patrol / corridor-delta-corridor-aware / greedy`
       experimental until M69 full benchmark refresh.
   - If all allocators are semantically ignored by single-agent Urban Patrol,
     document that support status is planner/profile-specific and strategy is
     currently a benchmark harness dimension rather than a control difference.
   - Keep SAR CBBA and SAR centralized as unsupported with explicit reasons.

9. Update README and сопутствующие docs.
   - `README.md`:
     - add M68 row to milestone table;
     - update current status table;
     - update Strategy Support Matrix with Urban corridor-aware status;
     - add short Urban corridor-aware scenario section near Urban docs.
   - `docs/STATUS.md`:
     - mark M68 as algorithmic Urban planner improvement after implementation;
     - keep limitation that this is mission-level route planning, not physical
       avoidance or PX4 proof.
   - `docs/BENCHMARK_RESULTS.md`:
     - add M68 delta section;
     - clearly distinguish M68 small delta from historical M62 500-seed pack
       and from future M69 benchmark refresh;
     - summarize CBBA analysis status and unsupported reasons.
   - `docs/SCENARIO_DSL.md`:
     - document allowed Urban planner values and M68 corridor-delta fixture;
     - explain that `"corridor-aware"` uses road graph metadata, not lidar.
   - `docs/EXTENSION_GUIDE.md`:
     - update Urban mission path with planner extension and route-risk metric.
   - `docs/REPLAY.md`:
     - update only if the benchmark delta uses new replay fields/artifacts.

10. Verification and formatting.
    - For Rust changes, run:

      ```bash
      cargo fmt --all
      cargo clippy --all-targets -- -D warnings
      ```

    - For tests that include `cargo test`, use:

      ```bash
      PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test ...
      ```

    - Do not run 500/1000-seed benchmark in M68. That belongs to M69 unless
      explicitly requested later.

# Testing strategy

## 1. Tests that need no refactoring

Эти тесты планируются к реализации вместе с основными M68 code changes.

- `crates/swarm-sim/src/urban.rs`
  - unit test: Dijkstra keeps choosing shortest path on existing `block_map`;
  - unit test: corridor-aware planner chooses wider/safer route on a map with
    narrow shortcut and safe detour;
  - unit test: unknown planner string is rejected with typed/explicit error;
  - unit test: route risk score is deterministic and lower for the
    corridor-aware route;
  - edge case: missing `corridor_width_m` falls back to neutral/default risk
    without panic;
  - edge case: blocked edge is still avoided by both planners.
- `crates/swarm-sim/src/dsl.rs`
  - validation accepts `"dijkstra"` and `"corridor-aware"`;
  - validation rejects an unknown planner value;
  - validation still catches invalid start pose and judge violations.
- `crates/swarm-sim/src/runner.rs`
  - Urban Patrol with corridor-aware planner completes deterministic fixture;
  - Urban Search still works with default `"dijkstra"`;
  - existing scenarios without explicit new fields deserialize and run.
- `crates/swarm-scenarios/src/urban.rs`
  - risky-corridor builder/fixture has valid map and route loop;
  - corridor-aware fixture produces expected lower risk than baseline.
- `crates/swarm-sim/tests/scenario_catalog.rs`
  - `scenarios/urban.corridor-delta.json` loads and validates;
  - both profiles run successfully;
  - metric delta assertion: lower risk with corridor-aware, no new violation.
- `crates/swarm-examples/tests/support_matrix.rs`
  - Urban corridor-delta baseline is explicit stable/experimental as planned;
  - SAR CBBA and SAR centralized remain explicit unsupported, not accidental
    failed rows.
- `crates/swarm-metrics/src/metrics.rs`
  - new route-risk fields default from legacy JSON;
  - aggregate route-risk average is correct.
- `crates/swarm-examples/tests/benchmark_pack.rs`
  - if M68 delta pack adds a new metric column, assert JSON/CSV/Markdown
    output includes it;
  - smoke assertion that benchmark pack manifest can represent the
    corridor-delta scenario-suite.
- Docs smoke tests where existing `sitl_docs`/docs tests already cover required
  phrases:
  - `README.md` mentions M68 and its experimental scope;
  - `docs/SCENARIO_DSL.md` documents allowed planner values;
  - `docs/BENCHMARK_RESULTS.md` distinguishes M68 delta from M62/M69.

Suggested targeted commands:

```bash
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim urban
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-sim --test scenario_catalog urban
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-scenarios urban
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-metrics urban
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test support_matrix
PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 /home/formi/.local/bin/runlim cargo test -p swarm-examples --test benchmark_pack
```

## 2. Tests that need light refactoring

- Shared Urban planner/scoring fixture builder to avoid duplicating custom
  risky-corridor maps across `swarm-sim`, `swarm-scenarios`, and
  `scenario_catalog`.
- Shared route-risk assertion helper:
  - route completed;
  - violations unchanged;
  - risk improved;
  - length/time tradeoff documented.
- Strategy comparison fixture helper that runs a tiny scenario-suite in-memory
  without shelling out to the CLI.
- Replay diagnostic helper for convergence events:
  - summarize CBBA message drops;
  - summarize convergence ticks;
  - summarize task release/reassignment timing;
  - useful before attempting failure-triggered gossip burst.
- Shared docs/status assertion helper for support-matrix and benchmark wording.

## 3. Tests that need heavy refactoring

- CBBA convergence property tests under arbitrary packet loss, latency,
  partitions, and failure timing.
- Multi-agent scaling benchmark harness with 8/16 agents and deterministic
  profile generation.
- Hierarchical coordination integration tests.
- Statistical delta validation across multiple benchmark packs.
- Full replay diff tooling for before/after comparison.
- Geometry property tests for arbitrary road graphs, AABB obstacle layouts, and
  route-loop generation.

# Risks and tradeoffs

- Corridor-aware scoring can overfit one synthetic map. Mitigation: keep the
  delta fixture explicit, document that it is M68 evidence only, and defer
  broader validation to M69.
- A new route-risk metric can become misleading if it is not tied to a clear
  formula. Mitigation: document exact formula and treat it as a planning-risk
  proxy, not physical collision probability.
- Adding user-visible report columns can affect CSV/Markdown consumers.
  Mitigation: additive fields only, serde defaults for JSON, export tests.
- DSL planner values can break old scenarios if validation is too strict.
  Mitigation: keep `"dijkstra"` default and schema `0.1`; unknown values fail
  with explicit validation errors.
- Urban single-agent planner delta does not prove multi-agent deconfliction.
  Mitigation: do not add 8/16-agent claims unless a separate profile is added
  and measured.
- CBBA analysis can balloon into a separate milestone. Mitigation: analyze and
  document weak/unsupported pairs, but implement gossip burst only if targeted
  replay evidence supports a small fix.
- Route scoring adds some per-edge computation over obstacles. Mitigation:
  keep helper O(edges * obstacles) for small Urban maps and add performance
  note if future maps grow.
- Benchmark delta through `strategy_comparison` may duplicate Urban results
  across strategies because current Urban Patrol ignores allocator behavior.
  Mitigation: interpret profiles/planners, not strategies, and state this in
  the result README.

# Open questions

- Нужно ли сделать `corridor-aware` user-facing planner value сразу
  `experimental`, а `dijkstra` оставить единственным `stable` до M69?
  Рекомендуемый ответ: да.
- Достаточно ли route-risk metric как primary measurable improvement?
  Рекомендуемый ответ: да, если формула фиксирована, тестируется и
  документируется вместе с distance/time tradeoff.
- Добавлять ли 8-agent Urban profile в M68?
  Рекомендуемый ответ: нет. Для corridor-aware single-route planner это не
  нужно; scaling belongs to M69+ unless evidence shows route conflicts matter
  for the chosen improvement.
- Реализовывать ли failure-triggered gossip burst в M68?
  Рекомендуемый ответ: только если targeted replay ясно показывает
  parameter-level reconvergence issue. По текущему статусу SAR CBBA уже
  классифицирован как unsupported delayed reconvergence, поэтому безопаснее
  ограничиться анализом и support-matrix honesty.
- Нужен ли release build для M68 delta?
  Рекомендуемый ответ: нет, если scenario-suite delta короткий и не
  публикуется как performance benchmark. Release имеет смысл только если debug
  run выходит за разумное время.

# Что могло сломаться

- Поведение Urban route planning:
  - старые `"dijkstra"` scenarios могут начать выбирать другой путь, если
    wrapper меняет tie-breaking;
  - проверка: `cargo test -p swarm-sim urban` и `cargo test -p swarm-sim --test
    scenario_catalog urban`.
- DSL contracts:
  - старые JSON fixtures могут не пройти validation после расширения planner
    values;
  - проверка: `cargo test -p swarm-sim --test scenario_catalog`.
- Report/export schema:
  - новые metric columns могут нарушить snapshot/header expectations;
  - проверка: `cargo test -p swarm-examples --test benchmark_pack` и export
    tests.
- Support matrix wording:
  - Urban corridor-aware можно случайно представить как stable broad algorithm
    instead of experimental profile-specific result;
  - проверка: support-matrix tests and docs smoke assertions.
- Benchmark interpretation:
  - small M68 delta artifact можно спутать с M69 full benchmark refresh;
  - проверка: `docs/BENCHMARK_RESULTS.md`,
    `results/m68_urban_corridor_delta/README.md`, `docs/STATUS.md`.
- Performance/resources:
  - clearance/risk scoring по obstacles может стать дорогим на больших maps;
  - проверка: targeted scenario-suite run and, later, M69 benchmark refresh.
- Replay/analysis integration:
  - если route planning changes alter edge_ids/order, route-trace and judge
    reports may change;
  - проверка: replay CLI tests and scenario-suite run with `--replay-log`.
