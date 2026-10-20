// share-core is built by ../share-core/scripts/build-wasm.sh, which refuses a package with
// test-only exports. This guard is the second line: the Worker will not even load one.

/** Exports that only a `--features test-seed` build has (a caller-chosen shuffle seed). */
export const TEST_ONLY_EXPORTS = ["strip_with_seed"] as const;

/** Throws if `mod` (the share-core package's module namespace) has a test-only export. */
export function assertProductionShareCore(mod: object): void {
  for (const name of TEST_ONLY_EXPORTS) {
    if (name in mod) {
      throw new Error(`share-core package exports test-only "${name}": rebuild it with scripts/build-wasm.sh (no --features test-seed)`);
    }
  }
}
