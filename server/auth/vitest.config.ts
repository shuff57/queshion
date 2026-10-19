import { fileURLToPath } from "node:url";
import { cloudflareTest, readD1Migrations } from "@cloudflare/vitest-plugin";
import { defineConfig } from "vitest/config";

const here = (p: string) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig(async () => {
  const migrations = await readD1Migrations(here("./migrations"));
  return {
    plugins: [
      cloudflareTest({
        miniflare: {
          compatibilityDate: "2026-09-01",
          compatibilityFlags: ["nodejs_compat"],
          d1Databases: ["DB"],
          // Lets a test import a real Argon2id wasm build as a WebAssembly.Module (`*.wasm?module`).
          modulesRules: [{ type: "CompiledWasm", include: ["**/*.wasm", "**/*.wasm?module"], fallthrough: true }],
          bindings: { TEST_MIGRATIONS: migrations },
        },
      }),
    ],
    test: {
      setupFiles: ["./test/apply-migrations.ts"],
      // Every test runs on an injected clock (no real waiting), and the long simulations use the
      // in-memory rate-limit store, so nothing here needs more than ~2 s on an idle machine. The
      // headroom is for heavily loaded CI machines, not for slow tests; no test overrides it.
      testTimeout: 20_000,
    },
  };
});
