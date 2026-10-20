#!/usr/bin/env sh
# The only way the Worker gets its share-core package: a release build WITHOUT test features,
# then the export guard. Wrangler runs this before every deploy and `wrangler dev`
# (server/worker/wrangler.toml, wrangler.staging.toml: [build] command).
set -eu
cd "$(dirname "$0")/.."
wasm-pack build --release --target web
sh scripts/check-wasm-exports.sh pkg
