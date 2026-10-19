#!/usr/bin/env sh
# Demote one admin to teacher (sessions and device tokens revoked). Never the last active admin.
# Written to auth_audit with actor 'bootstrap'.
#   scripts/demote-admin.sh admin@school.example.org --remote
set -eu
email=$(printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]')
where="${2:---remote}"
case "$where" in --local|--remote) ;; *) echo "second argument must be --local or --remote" >&2; exit 2 ;; esac
if ! printf '%s' "$email" | grep -Eq '^[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}$'; then
  echo "usage: $0 EMAIL [--local|--remote]" >&2
  exit 2
fi
here=$(cd "$(dirname "$0")/.." && pwd)
sql=$(mktemp)
trap 'rm -f "$sql"' EXIT
node --experimental-strip-types --no-warnings "$here/scripts/admin-sql.ts" demote "$email" > "$sql"
cd "$here"
wrangler d1 execute vershion-pool "$where" --file "$sql"
