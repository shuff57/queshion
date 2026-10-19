#!/usr/bin/env sh
# Promote one verified account to admin, or bring back a SUSPENDED admin (the recovery path when
# every admin is suspended). This is the ONLY way an admin is created.
#   scripts/make-admin.sh teacher@school.example.org --remote   # production D1
#   scripts/make-admin.sh teacher@school.example.org --local    # wrangler dev's local D1
# Eligible: confirmed address, password set by the mailbox owner, not deleted, and pending/teacher
# (not suspended) or an admin who is suspended. Written to auth_audit with actor 'bootstrap'.
# Promote only someone whose identity you have verified in person or through the school's
# published phone number.
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
node --experimental-strip-types --no-warnings "$here/scripts/admin-sql.ts" make "$email" > "$sql"
cd "$here"
wrangler d1 execute vershion-pool "$where" --file "$sql"
