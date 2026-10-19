#!/usr/bin/env sh
# The auth package owns migrations 0001-0099; the worker owns 0100+. wrangler reads one directory,
# so the package's files are copied here. `npm test` fails if a copy drifts from the package.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
cp "$here/../auth/migrations/"*.sql "$here/migrations/"
echo "copied: $(ls "$here/../auth/migrations/")"
