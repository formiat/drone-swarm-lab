#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_ID="${RUN_ID:-m58-multi-agent-px4-sih-execute}"
OUTPUT_ROOT="${OUTPUT_ROOT:-$ROOT_DIR/results/m58_multi_agent_px4_sih_local}"
OUTPUT_DIR="$OUTPUT_ROOT/$RUN_ID"
SCENARIO="${SCENARIO:-scenarios/sitl.multi-agent.execute.json}"
CONFIG="${CONFIG:-scenarios/sitl.multi-agent.execute.config.json}"
RUNLIM="${RUNLIM:-/home/formi/.local/bin/runlim}"
DRY_RUN="${DRY_RUN:-0}"

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 3 ;;
  esac
done

cmd=(
  "$RUNLIM" cargo run -p swarm-examples --features mavlink-transport --bin sitl_supervisor --
  --connection
  --execute
  --scenario "$SCENARIO"
  --config "$CONFIG"
  --output-dir "$OUTPUT_ROOT"
  --run-id "$RUN_ID"
  --force
)
validator_cmd=(
  "$RUNLIM" cargo run -p swarm-examples --bin artifact_validator --
  --output-dir "$OUTPUT_DIR"
  --mode supervisor-run
  --strict
)

if [[ "$DRY_RUN" == "1" ]]; then
  printf 'M58 local harness dry-run\n'
  printf 'supervisor:'
  printf ' %q' "${cmd[@]}"
  printf '\nvalidator:'
  printf ' %q' "${validator_cmd[@]}"
  printf '\n'
  exit 0
fi

if [[ -z "${PX4_AGENT0_CMD:-}" || -z "${PX4_AGENT1_CMD:-}" ]]; then
  cat >&2 <<'EOF'
PX4_AGENT0_CMD and PX4_AGENT1_CMD are required for live M58 harness mode.
Use DRY_RUN=1 scripts/run_m58_local.sh to inspect commands without launching PX4/SIH.
EOF
  exit 4
fi

pids=()
cleanup() {
  for pid in "${pids[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
    fi
  done
}
trap cleanup EXIT

mkdir -p "$OUTPUT_ROOT"
bash -lc "$PX4_AGENT0_CMD" >"$OUTPUT_ROOT/px4-agent-0.log" 2>&1 &
pids+=("$!")
bash -lc "$PX4_AGENT1_CMD" >"$OUTPUT_ROOT/px4-agent-1.log" 2>&1 &
pids+=("$!")

sleep "${PX4_STARTUP_WAIT_SECONDS:-10}"
"${cmd[@]}"
"${validator_cmd[@]}"
