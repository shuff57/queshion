import { describe, expect, it } from "vitest";
import { createAuth } from "../src/index.js";
import { env, GOOD_PW, Harness, ORIGIN, OTHER_ORIGIN, RecordingMailer, StubHasher, uniqueEmail } from "./helpers.js";

describe("CORS", () => {
  it("echoes the exact allowed origin with credentials, never a wildcard", async () => {
    const t = new Harness();
    const res = await t.call("POST", "/auth/forgot", { body: { email: uniqueEmail() } });
    expect(res.status).toBe(202);
    expect(res.headers.get("Access-Control-Allow-Origin")).toBe(ORIGIN);
    expect(res.headers.get("Access-Control-Allow-Credentials")).toBe("true");
    expect(res.headers.get("Vary")).toContain("Origin");
  });

  it("refuses a request from an origin not on the list, with no CORS headers", async () => {
    const t = new Harness();
    for (const origin of [OTHER_ORIGIN, "null", `${ORIGIN}.evil.net`, "http://app.example.org", `${ORIGIN}:443`]) {
      const res = await t.call("POST", "/auth/login", { origin, body: { email: uniqueEmail(), password: GOOD_PW } });
      expect(res.status).toBe(403);
      expect(await res.json()).toEqual({ error: "origin_not_allowed" });
      expect(res.headers.get("Access-Control-Allow-Origin")).toBeNull();
      expect(res.headers.get("Access-Control-Allow-Credentials")).toBeNull();
    }
    expect(t.hasher.calls).toBe(0);
  });

  it("answers preflights only for allowed origins and never allows X-Client", async () => {
    const t = new Harness();
    const ok = await t.call("OPTIONS", "/auth/refresh", { headers: { "Access-Control-Request-Method": "POST", "Access-Control-Request-Headers": "content-type, x-client" } });
    expect(ok.status).toBe(204);
    expect(ok.headers.get("Access-Control-Allow-Origin")).toBe(ORIGIN);
    expect(ok.headers.get("Access-Control-Allow-Methods")).toBe("POST");
    expect(ok.headers.get("Access-Control-Allow-Headers")!.toLowerCase()).not.toContain("x-client");
    const bad = await t.call("OPTIONS", "/auth/refresh", { origin: OTHER_ORIGIN });
    expect(bad.status).toBe(403);
    expect(bad.headers.get("Access-Control-Allow-Origin")).toBeNull();
  });

  it("refuses a wildcard or non-origin in the configuration", () => {
    const base = { db: env.DB, hasher: new StubHasher(), mailer: new RecordingMailer(), secret: "x".repeat(40) };
    expect(() => createAuth({ ...base, allowedOrigins: ["*"] })).toThrow();
    expect(() => createAuth({ ...base, allowedOrigins: ["https://*.example.org"] })).toThrow();
    expect(() => createAuth({ ...base, allowedOrigins: ["https://app.example.org/"] })).toThrow();
    expect(() => createAuth({ ...base, allowedOrigins: [ORIGIN], secret: "short" })).toThrow();
  });

  it("helps the app's own routes with the same rules", async () => {
    const t = new Harness();
    const req = new Request("https://api.example.org/items", { headers: { Origin: ORIGIN } });
    const res = t.auth.withCors(req, new Response("x"));
    expect(res.headers.get("Access-Control-Allow-Origin")).toBe(ORIGIN);
    const evil = t.auth.withCors(new Request("https://api.example.org/items", { headers: { Origin: OTHER_ORIGIN } }), new Response("x"));
    expect(evil.headers.get("Access-Control-Allow-Origin")).toBeNull();
    expect(t.auth.preflight(new Request("https://api.example.org/items"), "GET")).toBeNull();
    expect(await t.auth.handle(new Request("https://api.example.org/items"), { waitUntil() {} })).toBeNull();
  });
});

describe("native vs web", () => {
  it("rejects X-Client: native when an Origin header is present, on every route", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (const [method, path, body] of [
      ["POST", "/auth/login", { email, password: GOOD_PW }],
      ["POST", "/auth/refresh", { refresh_token: "x".repeat(43) }],
      ["POST", "/auth/signup", { email: uniqueEmail(), password: GOOD_PW }],
      ["POST", "/auth/logout", {}],
      ["GET", "/me", undefined],
    ] as const) {
      for (const origin of [ORIGIN, OTHER_ORIGIN, "null"]) {
        const res = await t.call(method, path, { native: true, origin, body });
        expect(res.status).toBe(400);
        expect(await res.json()).toEqual({ error: "native_client_with_origin" });
        expect(res.headers.get("Set-Cookie")).toBeNull();
      }
    }
    expect(t.hasher.calls).toBe(2); // the setup sign-up and its verify; no refused request hashed
  });

  it("rejects an unknown X-Client value", async () => {
    const t = new Harness();
    const res = await t.call("POST", "/auth/login", { headers: { "X-Client": "nativeish" }, origin: null, body: { email: uniqueEmail(), password: GOOD_PW } });
    expect(res.status).toBe(400);
    expect(await res.json()).toEqual({ error: "invalid_client" });
  });

  it("web refresh ignores a token in the body; native refresh ignores the cookie", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const web = await t.loginWeb(email);
    const native = await t.loginNative(email);
    const r1 = await t.call("POST", "/auth/refresh", { body: { refresh_token: native.refresh } });
    expect(r1.status).toBe(401);
    const r2 = await t.call("POST", "/auth/refresh", { native: true, cookie: `__Host-refresh=${web.refresh}`, body: {} });
    expect(r2.status).toBe(401);
  });

  it("a web refresh must come from an allowed browser origin (CSRF defence in depth)", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginWeb(email);
    const res = await t.refreshWeb(refresh, { origin: null });
    expect(res.status).toBe(403);
    expect(await res.json()).toEqual({ error: "origin_required" });
    expect((await t.refreshWeb(refresh)).status).toBe(200);
  });

  it("refuses an ambiguous cookie header carrying two refresh cookies", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginWeb(email);
    const res = await t.call("POST", "/auth/refresh", { cookie: `__Host-refresh=${refresh}; __Host-refresh=${"a".repeat(43)}` });
    expect(res.status).toBe(401);
  });

  it("never puts a refresh token in any web response body", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const login = await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } });
    const texts = [await login.clone().text()];
    let rt = /__Host-refresh=([^;]+)/.exec(login.headers.get("Set-Cookie")!)![1]!;
    const seen = [rt];
    for (let i = 0; i < 3; i++) {
      const r = await t.refreshWeb(rt);
      texts.push(await r.clone().text());
      rt = /__Host-refresh=([^;]+)/.exec(r.headers.get("Set-Cookie")!)![1]!;
      seen.push(rt);
    }
    // Grace reuse also re-sends via cookie only.
    const again = await t.refreshWeb(seen[2]!);
    texts.push(await again.clone().text());
    expect(again.headers.get("Set-Cookie")).toContain(`__Host-refresh=${seen[3]}`);
    for (const text of texts) {
      expect(text).not.toMatch(/refresh_token/);
      for (const s of seen) expect(text).not.toContain(s);
    }
  });

  it("logout clears the cookie with the same flags", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginWeb(email);
    const res = await t.call("POST", "/auth/logout", { cookie: `__Host-refresh=${refresh}` });
    expect(res.status).toBe(204);
    expect(res.headers.get("Set-Cookie")).toBe("__Host-refresh=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict");
  });
});
