// Run: npm test
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { test } from "node:test";
import { assertProductionShareCore, TEST_ONLY_EXPORTS } from "../src/share-core-guard.ts";

test("a module with a test-only export is refused", () => {
  assert.throws(() => assertProductionShareCore({ strip: () => "", strip_with_seed: () => "" }), /test-only "strip_with_seed"/);
  assert.doesNotThrow(() => assertProductionShareCore({ strip: () => "", validate: () => "" }));
  assert.deepEqual([...TEST_ONLY_EXPORTS], ["strip_with_seed"]);
});

test("the built production package passes; a test-seed build does not", async () => {
  const prod = new URL("../../share-core/pkg/share_core.js", import.meta.url);
  assert.ok(existsSync(prod), "build share-core first: sh ../share-core/scripts/build-wasm.sh");
  const mod = await import(prod.href);
  assert.doesNotThrow(() => assertProductionShareCore(mod));
  const testPkg = new URL("../../share-core/target/pkg-test/share_core.js", import.meta.url);
  if (existsSync(testPkg)) {
    const t = await import(testPkg.href);
    assert.throws(() => assertProductionShareCore(t), /test-only/);
  }
});
