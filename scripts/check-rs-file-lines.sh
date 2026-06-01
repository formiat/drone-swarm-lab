#!/usr/bin/env bash
set -euo pipefail

hard_limit=2000
warn_limit=1000
strict_warn=0

for arg in "$@"; do
  case "$arg" in
    --strict-1000)
      strict_warn=1
      ;;
    --help|-h)
      cat <<'USAGE'
Usage: scripts/check-rs-file-lines.sh [--strict-1000]

Fails when any tracked Rust source file is above 2000 lines.
Prints warnings for files above 1000 lines. With --strict-1000 those
warnings also fail the command.
USAGE
      exit 0
      ;;
    *)
      echo "unknown argument: $arg" >&2
      exit 2
      ;;
  esac
done

hard_failed=0
warn_failed=0

while IFS= read -r file; do
  lines=$(wc -l < "$file")
  if (( lines > hard_limit )); then
    printf 'error: %s lines > %s: %s\n' "$lines" "$hard_limit" "$file" >&2
    hard_failed=1
  elif (( lines > warn_limit )); then
    printf 'warning: %s lines > %s: %s\n' "$lines" "$warn_limit" "$file" >&2
    warn_failed=1
  fi
done < <(rg --files -g '*.rs')

if (( hard_failed != 0 )); then
  exit 1
fi

if (( strict_warn != 0 && warn_failed != 0 )); then
  exit 1
fi
