/// <reference types="vite/client" />
// Runs the package against a real Argon2id build: the wasm-bindgen output of a Rust crate
// exposing `argon2id(password, salt, m_kib, t, p) -> Uint8Array`. Skipped when no build is present.
import { describe, expect, it } from "vitest";
import { createAuth, verifyPassword, type Argon2idHasher } from "../src/index.js";
import { env, GOOD_PW, Harness, uniqueEmail } from "./helpers.js";

const wasmMods = import.meta.glob("../../share-core/pkg/share_core_bg.wasm", { eager: true, import: "default", query: "?module" });
const glueMods = import.meta.glob("../../share-core/pkg/share_core.js", { eager: true });
const wasm = Object.values(wasmMods)[0] as WebAssembly.Module | undefined;
const glue = Object.values(glueMods)[0] as
  | { initSync(m: { module: WebAssembly.Module }): unknown; argon2id(pw: Uint8Array, salt: Uint8Array, m: number, t: number, p: number): Uint8Array }
  | undefined;

describe.skipIf(!wasm || !glue)("real Argon2id (wasm)", () => {
  const hasher: Argon2idHasher = {
    argon2id(pw, salt, p) {
      return glue!.argon2id(pw, salt, p.m, p.t, p.p);
    },
  };

  // Known answers, cross-checked against an independent Argon2id (Bun.password.verify accepted
  // the PHC strings built from these tags and rejected a wrong password).
  it.each([
    ["password", "somesaltsomesalt", 64, "1a98fba8e6394425c9bdc333a9c750aaf811df070f5fb4a5788e1cf4db5793a1"],
    ["correct horse battery staple", "0123456789abcdef", 19456, "832e52b959b967b570ee4781f6c7bda7ced019ca266ac781fd2d94d4e853b0cd"],
  ])("argon2id(%s, %s, m=%i, t=2, p=1) matches the known answer", async (pw, salt, m, hex) => {
    glue!.initSync({ module: wasm! });
    const enc = new TextEncoder();
    const tag = (await hasher.argon2id(enc.encode(pw), enc.encode(salt), { m, t: 2, p: 1 })) as Uint8Array;
    expect([...tag].map((x) => x.toString(16).padStart(2, "0")).join("")).toBe(hex);
    // And the package's own PHC verify accepts it.
    const b64 = (u: Uint8Array) => btoa(String.fromCharCode(...u)).replace(/=+$/, "");
    expect(await verifyPassword(hasher, `$argon2id$v=19$m=${m},t=2,p=1$${b64(enc.encode(salt))}$${b64(tag)}`, pw)).toBe(true);
  });

  it("drives the full sign-up, login and refresh flow with the production parameters", async () => {
    glue!.initSync({ module: wasm! });
    const t = new Harness();
    t.auth = createAuth({ db: env.DB, hasher, mailer: t.mailer, secret: t.secret, allowedOrigins: ["https://app.example.org"], now: () => t.clock });
    const email = uniqueEmail();
    const token = await t.signup(email);
    expect((await t.call("POST", "/auth/verify", { body: { token, password: GOOD_PW } })).status).toBe(200);
    const row = await env.DB.prepare("SELECT pw_hash FROM users WHERE email = ?").bind(email).first<{ pw_hash: string }>();
    expect(row!.pw_hash).toMatch(/^\$argon2id\$v=19\$m=19456,t=2,p=1\$/);
    expect(await verifyPassword(hasher, row!.pw_hash, GOOD_PW)).toBe(true);
    expect(await verifyPassword(hasher, row!.pw_hash, GOOD_PW + "x")).toBe(false);
    const { res, refresh } = await t.loginNative(email);
    expect(res.status).toBe(200);
    expect((await t.refreshNative(refresh)).status).toBe(200);
    expect((await t.call("POST", "/auth/login", { body: { email, password: "wrong-wrong-wrong" } })).status).toBe(401);
  });
});
