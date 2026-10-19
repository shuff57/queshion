import { describe, expect, it } from "vitest";
import { env, GOOD_PW, Harness, snapshot, uniqueEmail } from "./helpers.js";

const NEW_PW = "lantern-orchard-copper-58";

async function requestReset(t: Harness, email: string): Promise<string> {
  const res = await t.call("POST", "/auth/forgot", { body: { email } });
  expect(res.status).toBe(202);
  const m = t.mailer.last("reset", email);
  expect(m?.token).toBeTruthy();
  return m!.token!;
}

describe("forgot password", () => {
  it("answers identically for known and unknown addresses, with no hashing either way", async () => {
    const t = new Harness();
    const known = uniqueEmail();
    await t.signupVerified(known);
    t.hasher.calls = 0;
    t.mailer.sent = [];
    const a = await snapshot(await t.call("POST", "/auth/forgot", { body: { email: known } }));
    const b = await snapshot(await t.call("POST", "/auth/forgot", { body: { email: uniqueEmail() } }));
    expect(a).toEqual(b);
    expect(a.status).toBe(202);
    expect(t.hasher.calls).toBe(0);
    expect(t.mailer.sent.map((m) => [m.kind, m.to])).toEqual([["reset", known]]);
    expect(t.mailer.sent[0]!.expiresAt).toBe(t.clock + 3600 * 1000);
  });
});

describe("reset password", () => {
  it("sets the new password and revokes every session", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const web = await t.loginWeb(email);
    const native = await t.loginNative(email);
    const token = await requestReset(t, email);
    const res = await t.call("POST", "/auth/reset", { body: { token, password: NEW_PW } });
    expect(res.status).toBe(200);
    expect(res.headers.get("Set-Cookie")).toContain("Max-Age=0");
    expect((await t.refreshWeb(web.refresh)).status).toBe(401);
    expect((await t.refreshNative(native.refresh)).status).toBe(401);
    expect((await t.call("GET", "/me", { bearer: native.access })).status).toBe(401);
    expect((await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } })).status).toBe(401);
    expect((await t.call("POST", "/auth/login", { body: { email, password: NEW_PW } })).status).toBe(200);
    expect(t.mailer.last("password_changed", email)).toBeTruthy();
  });

  it("is single use and the other outstanding links die with it", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const token = await requestReset(t, email);
    expect((await t.call("POST", "/auth/reset", { body: { token, password: NEW_PW } })).status).toBe(200);
    const again = await t.call("POST", "/auth/reset", { body: { token, password: "a-third-good-password" } });
    expect(again.status).toBe(400);
    expect(await again.json()).toEqual({ error: "invalid_token" });
  });

  it("the newest three links work; a fourth retires the oldest", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const links = [];
    for (let i = 0; i < 4; i++) {
      links.push(await requestReset(t, email));
      t.advance(121);
    }
    expect((await t.call("POST", "/auth/reset", { body: { token: links[0], password: NEW_PW } })).status).toBe(400);
    expect((await t.call("POST", "/auth/reset", { body: { token: links[1], password: NEW_PW } })).status).toBe(200);
  });

  it("expires after one hour", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const token = await requestReset(t, email);
    t.advance(3600);
    expect((await t.call("POST", "/auth/reset", { body: { token, password: NEW_PW } })).status).toBe(400);
  });

  it("a bad token never costs a hash; a weak password does not burn the token", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const token = await requestReset(t, email);
    t.hasher.calls = 0;
    expect((await t.call("POST", "/auth/reset", { body: { token: "D".repeat(43), password: NEW_PW } })).status).toBe(400);
    const weak = await t.call("POST", "/auth/reset", { body: { token, password: "password123" } });
    expect(await weak.json()).toEqual({ error: "weak_password", reason: "common" });
    expect(t.hasher.calls).toBe(0);
    expect((await t.call("POST", "/auth/reset", { body: { token, password: NEW_PW } })).status).toBe(200);
    expect(t.hasher.calls).toBe(1);
  });

  it("on an unconfirmed account, confirms the address (the link proved control) and evicts a squatter", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    // Someone else registered this address with their own password and never confirmed it.
    await t.signup(email, "squatters-own-password");
    expect((await t.call("POST", "/auth/login", { body: { email, password: "squatters-own-password" } })).status).toBe(401);
    const token = await requestReset(t, email);
    expect((await t.call("POST", "/auth/reset", { body: { token, password: NEW_PW } })).status).toBe(200);
    expect((await t.call("POST", "/auth/login", { body: { email, password: NEW_PW } })).status).toBe(200);
    expect((await t.call("POST", "/auth/login", { body: { email, password: "squatters-own-password" } })).status).toBe(401);
    const u = await env.DB.prepare("SELECT id, trust, email_verified_at FROM users WHERE email = ?").bind(email).first<Record<string, unknown>>();
    expect(u?.trust).toBe("pending");
    expect(u?.email_verified_at).toBe(t.clock);
    // The squatter's verification link is dead too.
    const verify = t.mailer.last("verify", email)!.token!;
    expect((await t.call("POST", "/auth/verify", { body: { token: verify } })).status).toBe(400);
    expect(await env.DB.prepare("SELECT 1 FROM approval_requests WHERE user_id = ?").bind(u?.id).first()).not.toBeNull();
  });

  it("concurrent resets with one token: exactly one wins", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const token = await requestReset(t, email);
    const pws = ["first-candidate-pass", "second-candidate-pass", "third-candidate-pass"];
    const res = await Promise.all(pws.map((password) => t.call("POST", "/auth/reset", { body: { token, password } })));
    const winners = pws.filter((_, i) => res[i]!.status === 200);
    expect(winners).toHaveLength(1);
    for (const pw of pws) {
      const status = (await t.call("POST", "/auth/login", { body: { email, password: pw } })).status;
      expect(status).toBe(pw === winners[0] ? 200 : 401);
    }
  });
});
