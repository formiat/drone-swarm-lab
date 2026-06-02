#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_ID="${RUN_ID:-m59-px4-sih-failure-reallocation}"
OUTPUT_ROOT="${OUTPUT_ROOT:-$ROOT_DIR/results/m59_px4_sih_failure_reallocation_local}"
OUTPUT_DIR="$OUTPUT_ROOT/$RUN_ID"
SCENARIO="${SCENARIO:-scenarios/sitl.multi-agent.failure.json}"
CONFIG="${CONFIG:-scenarios/sitl.multi-agent.failure.config.json}"
RUNLIM="${RUNLIM:-/home/formi/.local/bin/runlim}"
DRY_RUN="${DRY_RUN:-0}"
FAIL_AFTER_SECONDS="${FAIL_AFTER_SECONDS:-20}"

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
  --reupload-on-failure
  --force
)
validator_cmd=(
  "$RUNLIM" cargo run -p swarm-examples --bin artifact_validator --
  --output-dir "$OUTPUT_DIR"
  --mode supervisor-run
  --strict
)

if [[ "$DRY_RUN" == "1" ]]; then
  printf 'M59 local harness dry-run\n'
  printf 'supervisor:'
  printf ' %q' "${cmd[@]}"
  printf '\nvalidator:'
  printf ' %q' "${validator_cmd[@]}"
  printf '\nfailure injection: kill agent-0 process after %s seconds\n' "$FAIL_AFTER_SECONDS"
  exit 0
fi

if [[ -z "${PX4_AGENT0_CMD:-}" || -z "${PX4_AGENT1_CMD:-}" ]]; then
  cat >&2 <<'EOF'
PX4_AGENT0_CMD and PX4_AGENT1_CMD are required for live M59 harness mode.
Use DRY_RUN=1 scripts/run_m59_local.sh to inspect commands without launching PX4/SIH.
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
agent0_pid="$!"
pids+=("$agent0_pid")
bash -lc "$PX4_AGENT1_CMD" >"$OUTPUT_ROOT/px4-agent-1.log" 2>&1 &
pids+=("$!")

sleep "${PX4_STARTUP_WAIT_SECONDS:-10}"
(
  sleep "$FAIL_AFTER_SECONDS"
  if kill -0 "$agent0_pid" 2>/dev/null; then
    kill "$agent0_pid"
  fi
) &
pids+=("$!")

"${cmd[@]}"
"${validator_cmd[@]}"
