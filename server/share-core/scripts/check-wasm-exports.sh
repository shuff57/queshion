#!/usr/bin/env sh
# Fails if a production wasm package exports the test-only explicit-seed strip. Run after every
# `wasm-pack build --release --target web` (and in CI) before the package is bundled.
#   scripts/check-wasm-exports.sh [pkg-dir]      (default: pkg)
set -eu
dir="${1:-pkg}"
for f in "$dir"/share_core.d.ts "$dir"/share_core.js "$dir"/share_core_bg.wasm.d.ts; do
  [ -f "$f" ] || { echo "missing $f" >&2; exit 2; }
  if grep -q "strip_with_seed" "$f"; then
    echo "FAIL: $f exports strip_with_seed: this package was built with --features test-seed" >&2
    exit 1
  fi
done
# The wasm binary's export section too (names are stored as plain bytes).
if grep -aq "strip_with_seed" "$dir"/share_core_bg.wasm; then
  echo "FAIL: $dir/share_core_bg.wasm contains strip_with_seed" >&2
  exit 1
fi
echo "ok: $dir has no test-only exports"
