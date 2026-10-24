#!/usr/bin/env bash
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readme="$root/README.md"
failures=0

documented="$(grep -E '^\| `(GET|POST|PUT|DELETE)` \| `/' "$readme" | awk -F'`' '{print $4}')"
if [ -z "$documented" ]; then
  echo "route drift: no route table in README.md, nothing to check"
  exit 0
fi

source_routes="$(grep -rhoE --include='*.rs' --exclude-dir=target --exclude-dir=.verify '\.route\("[^"]+"' "$root" | sed -E 's/^\.route\("//; s/"$//' | sort -u)"

while IFS= read -r route; do
  if ! printf '%s\n' "$source_routes" | grep -Fxq -- "$route"; then
    echo "DRIFT    README route $route has no .route() in src"
    failures=$((failures + 1))
  fi
done <<< "$documented"

while IFS= read -r route; do
  if ! printf '%s\n' "$documented" | grep -Fxq -- "$route"; then
    echo "DRIFT    src route $route is not in README.md"
    failures=$((failures + 1))
  fi
done <<< "$source_routes"

if [ "$failures" -gt 0 ]; then
  echo "route drift: $failures problem(s)"
  exit 1
fi
echo "route drift: README routes and src routes agree"
