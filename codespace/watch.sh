#!/usr/bin/env bash
# Report on the runner inside the codespace until it exits or the time budget runs out.
#
# The guard job keeps this script running over SSH.  The output doubles as the keep-alive signal that
# prevents the codespace from being stopped for inactivity, so it must never go quiet for long.
#
# Usage: watch.sh <budget-minutes> [<epoch-seconds-when-github-stops-the-codespace>]
set -uo pipefail

budget=${1:?usage: watch.sh <budget-minutes> [<limit-epoch>]}
limit=${2:-}
state=/tmp/nrgha/state
deadline=$((SECONDS + budget * 60))
warned=

# Stream new runner log lines as they appear
tail -n 0 -F "$state/runner.log" 2>/dev/null &
trap 'kill $! 2>/dev/null' EXIT

while [ "$SECONDS" -lt "$deadline" ]; do
  if [ -e "$state/runner.exit" ]; then
    echo "runner exited with status $(cat "$state/runner.exit")"
    exit 0
  fi
  if [ -n "$limit" ] && [ -z "$warned" ] && [ "$(date +%s)" -gt $((limit - 1800)) ]; then
    echo "::warning::The codespace reaches GitHub's 12 hour limit in less than 30 minutes"
    warned=1
  fi
  printf '%s load=%s mem_available=%sM nix_free=%s\n' \
    "$(date -u +%T)" \
    "$(cut -d' ' -f1-3 /proc/loadavg)" \
    "$(awk '/MemAvailable/ { print int($2 / 1024) }' /proc/meminfo)" \
    "$(df -h --output=avail /nix | tail -n 1 | tr -d ' ')"
  sleep 30
done
echo "time budget of $budget minutes is spent"
exit 0
