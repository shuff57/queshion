import { describe, expect, it } from "vitest";
import { cookieValue, env, GOOD_PW, Harness, sha256b64url, snapshot, uniqueEmail } from "./helpers.js";

function decodeJwt(t: string): Record<string, unknown> {
  const p = t.split(".")[1]!;
  return JSON.parse(atob(p.replace(/-/g, "+").replace(/_/g, "/")));
}

describe("login", () => {
  it("web: access token in the body, refresh token only in a locked-down cookie", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const res = await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } });
    expect(res.status).toBe(200);
    const text = await res.clone().text();
    const body = JSON.parse(text);
    expect(Object.keys(body).sort()).toEqual(["access_token", "expires_in", "token_type", "user"]);
    expect(body.expires_in).toBe(900);
    const rt = cookieValue(res, "__Host-refresh")!;
    expect(rt).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(text).not.toContain(rt);
    const cookies = res.headers.getSetCookie();
    expect(cookies).toHaveLength(2);
    expect(cookies[0]).toBe(`__Host-refresh=${rt}; Path=/; Max-Age=${30 * 86400}; HttpOnly; Secure; SameSite=Strict`);
    expect(cookies[1]).toMatch(/^__Host-device=v1\.[^;]+; Path=\/; Max-Age=7776000; HttpOnly; Secure; SameSite=Strict$/);
    for (const c of cookies) expect(c).not.toMatch(/Domain=/i);
    expect(text).not.toContain(cookieValue(res, "__Host-device")!);
    expect(res.headers.get("Cache-Control")).toBe("no-store");
    // Only the hash is stored.
    const row = await env.DB.prepare("SELECT refresh_hash FROM sessions WHERE refresh_hash = ?").bind(await sha256b64url(rt)).first();
    expect(row).not.toBeNull();
    const claims = decodeJwt(body.access_token);
    expect(claims.trust).toBe("pending");
    expect((claims.exp as number) - (claims.iat as number)).toBe(900);
  });

  it("native: refresh token in the body and no cookie", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { res, refresh } = await t.loginNative(email);
    expect(res.status).toBe(200);
    expect(refresh).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(res.headers.get("Set-Cookie")).toBeNull();
    expect(res.headers.get("Access-Control-Allow-Origin")).toBeNull();
  });

  it("an unconfirmed account cannot log in, even with the right password, and the refusal looks like a wrong password", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signup(email);
    t.hasher.calls = 0;
    const right = await snapshot(await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } }));
    const wrong = await snapshot(await t.call("POST", "/auth/login", { body: { email: uniqueEmail(), password: GOOD_PW } }));
    expect(right).toEqual(wrong);
    expect(right.status).toBe(401);
    expect(t.hasher.calls).toBe(2); // full cost either way
    const n = await env.DB.prepare("SELECT COUNT(*) AS n FROM sessions s JOIN users u ON u.id = s.user_id WHERE u.email = ?").bind(email).first<{ n: number }>();
    expect(n?.n).toBe(0);
  });

  it("unknown email and wrong password are indistinguishable and cost one full hash each", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    t.hasher.calls = 0;
    const wrong = await snapshot(await t.call("POST", "/auth/login", { body: { email, password: "not-the-right-password" } }));
    expect(t.hasher.calls).toBe(1);
    const unknown = await snapshot(await t.call("POST", "/auth/login", { body: { email: uniqueEmail(), password: "not-the-right-password" } }));
    expect(t.hasher.calls).toBe(2);
    expect(wrong).toEqual(unknown);
    expect(wrong.status).toBe(401);
    expect(JSON.parse(wrong.body)).toEqual({ error: "invalid_credentials" });
  });

  it("a deleted account's address behaves exactly like an unknown one", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { access } = await t.loginWeb(email);
    expect((await t.call("DELETE", "/account", { bearer: access, body: { password: GOOD_PW } })).status).toBe(204);
    const a = await snapshot(await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } }));
    const b = await snapshot(await t.call("POST", "/auth/login", { body: { email: uniqueEmail(), password: GOOD_PW } }));
    expect(a).toEqual(b);
  });

  it("matches the email case-insensitively and the password after Unicode normalization", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const composed = "café-terrace-seventy";
    await t.signupVerified(email, composed);
    const decomposed = "café-terrace-seventy";
    const res = await t.call("POST", "/auth/login", { body: { email: email.toUpperCase(), password: decomposed } });
    expect(res.status).toBe(200);
  });

  it("re-hashes on login when the configured parameters were raised", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    t.build({ argon2: { m: 46080, t: 2, p: 1 } });
    t.hasher.calls = 0;
    expect((await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } })).status).toBe(200);
    expect(t.hasher.calls).toBe(2); // verify with the old params, hash with the new
    const row = await env.DB.prepare("SELECT pw_hash, pw_params FROM users WHERE email = ?").bind(email).first<{ pw_hash: string; pw_params: string }>();
    expect(row!.pw_hash).toMatch(/^\$argon2id\$v=19\$m=46080,t=2,p=1\$/);
    expect(JSON.parse(row!.pw_params).m).toBe(46080);
    t.hasher.calls = 0;
    expect((await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } })).status).toBe(200);
    expect(t.hasher.calls).toBe(1);
  });

  it("a fresh web login ends the session the browser's old cookie belonged to", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const first = await t.loginWeb(email);
    const second = await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW }, cookie: `__Host-refresh=${first.refresh}` });
    expect(second.status).toBe(200);
    expect((await t.refreshWeb(first.refresh)).status).toBe(401);
    expect((await t.refreshWeb(cookieValue(second, "__Host-refresh")!)).status).toBe(200);
  });
});

describe("access tokens", () => {
  it("/me needs a valid, live token", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { access, refresh } = await t.loginWeb(email);
    const me = await t.call("GET", "/me", { bearer: access });
    expect(me.status).toBe(200);
    expect(await me.json()).toMatchObject({ email, email_verified: true, trust: "pending", display_name: "Ms. Test" });

    const [h, p, s] = access.split(".");
    const forged = `${h}.${btoa(JSON.stringify({ ...decodeJwt(access), trust: "admin" })).replace(/=+$/, "").replace(/\+/g, "-").replace(/\//g, "_")}.${s}`;
    const none = `${btoa(JSON.stringify({ alg: "none", typ: "JWT" })).replace(/=+$/, "")}.${p}.`;
    const hs512 = `${btoa(JSON.stringify({ alg: "HS512", typ: "at+jwt" })).replace(/=+$/, "")}.${p}.${s}`;
    for (const bad of [forged, none, hs512, "garbage", `${h}.${p}.${s}x`]) {
      const r = await t.call("GET", "/me", { bearer: bad });
      expect(r.status).toBe(401);
      expect(r.headers.get("WWW-Authenticate")).toContain("Bearer");
    }
    // Revoked by logout: the stateless token is still in date, but the live check refuses it.
    expect((await t.call("POST", "/auth/logout", { cookie: `__Host-refresh=${refresh}` })).status).toBe(204);
    expect((await t.call("GET", "/me", { bearer: access })).status).toBe(401);
  });

  it("expire after 15 minutes", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { access } = await t.loginWeb(email);
    t.advance(15 * 60);
    expect((await t.call("GET", "/me", { bearer: access })).status).toBe(401);
  });

  it("from another deployment (different secret) are refused", async () => {
    const a = new Harness();
    const email = uniqueEmail();
    await a.signupVerified(email);
    const { access } = await a.loginWeb(email);
    const b = new Harness();
    expect((await b.call("GET", "/me", { bearer: access })).status).toBe(401);
  });

  it("authenticate() returns the trust level as it is now in D1, not as it was in the token", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { access } = await t.loginWeb(email);
    const req = new Request("https://api.example.org/x", { headers: { Authorization: `Bearer ${access}` } });
    expect((await t.auth.authenticate(req))?.trust).toBe("pending");
    await env.DB.prepare("UPDATE users SET trust = 'member' WHERE email = ?").bind(email).run();
    expect((await t.auth.authenticate(req))?.trust).toBe("member");
  });
});
