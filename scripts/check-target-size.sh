#!/usr/bin/env bash
# Keep target/ under a size limit. Cargo never removes outputs it no longer
# uses (old dependency hashes, other feature sets, other targets), so a warm
# checkout grows without bound; this once reached 100 GB. Run before builds.
#
# Over the limit, prune the cheapest to rebuild first, stopping once under:
#   1. cross-compiled targets (lint-windows and the like)
#   2. incremental compilation caches
#   3. each profile's compiled dependencies and fingerprints
# The Herdr.app bundles and top-level binaries are kept, so the running app
# and its bundle survive.
#
# HERDR_TARGET_LIMIT_GB overrides the limit (default 40).

set -euo pipefail

LIMIT_GB=${HERDR_TARGET_LIMIT_GB:-40}

cd "$(git rev-parse --show-toplevel)"
[[ -d target ]] || exit 0

size_gb() {
  du -sk target | awk '{ printf "%d", $1 / 1048576 }'
}

size=$(size_gb)
if ((size <= LIMIT_GB)); then
  echo "target/ is ${size} GB (limit ${LIMIT_GB} GB)."
  exit 0
fi
echo "target/ is ${size} GB, over its ${LIMIT_GB} GB limit; pruning."

prune() {
  local step=$1
  shift
  local paths=()
  local path
  for path in "$@"; do
    [[ -e "$path" ]] && paths+=("$path")
  done
  ((${#paths[@]} > 0)) || return 1
  echo "  removing ${step}: ${paths[*]}"
  rm -rf "${paths[@]}"
  size=$(size_gb)
  echo "  target/ is now ${size} GB."
  ((size <= LIMIT_GB))
}

shopt -s nullglob
# A cross target's directory holds a profile directory; host profiles do not
# sit under a triple.
cross=()
for dir in target/*-*-*/; do
  cross+=("${dir%/}")
done
prune "cross-compiled targets" "${cross[@]+"${cross[@]}"}" && exit 0
prune "incremental caches" target/*/incremental target/*/*/incremental && exit 0
prune "compiled dependencies" \
  target/{debug,release}/{deps,build,.fingerprint,examples} && exit 0
echo "target/ is still ${size} GB; look at what else lives there." >&2
