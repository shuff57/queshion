import { describe, expect, it } from "vitest";
import { env, GOOD_PW, Harness, uniqueEmail } from "./helpers.js";

async function userByEmail(email: string) {
  return env.DB.prepare("SELECT * FROM users WHERE email = ?").bind(email).first<Record<string, unknown>>();
}

describe("account deletion", () => {
  it("needs a live session and the password", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { access } = await t.loginWeb(email);
    expect((await t.call("DELETE", "/account", { body: { password: GOOD_PW } })).status).toBe(401);
    expect((await t.call("DELETE", "/account", { bearer: access, body: {} })).status).toBe(400);
    const wrong = await t.call("DELETE", "/account", { bearer: access, body: { password: "not-my-password-1" } });
    expect(wrong.status).toBe(401);
    expect(await wrong.json()).toEqual({ error: "invalid_credentials" });
    expect(await userByEmail(email)).not.toBeNull();
  });

  it("leaves a tombstone: ids stable, PII nulled, salted email hash kept, sessions and tokens gone", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const before = (await userByEmail(email))!;
    const id = before.id as string;
    const web = await t.loginWeb(email);
    const native = await t.loginNative(email);
    await t.call("POST", "/auth/forgot", { body: { email } });
    const res = await t.call("DELETE", "/account", { bearer: web.access, body: { password: GOOD_PW } });
    expect(res.status).toBe(204);
    expect(res.headers.get("Set-Cookie")).toContain("Max-Age=0");

    const row = (await env.DB.prepare("SELECT * FROM users WHERE id = ?").bind(id).first<Record<string, unknown>>())!;
    expect(row.id).toBe(id);
    for (const col of ["email", "email_verified_at", "pw_hash", "pw_params", "display_name", "profile"]) expect(row[col]).toBeNull();
    expect(row.deleted_at).toBe(t.clock);
    expect(row.deleted_email_hmac).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(row.deleted_email_hmac).not.toContain(email);
    expect(row.deleted_email_hmac_expires_at).toBe(t.clock + 14 * 86400 * 1000);
    // Nothing else in the row mentions the address.
    expect(JSON.stringify(row)).not.toContain(email.split("@")[0]);

    for (const table of ["sessions", "email_tokens"]) {
      const n = await env.DB.prepare(`SELECT COUNT(*) AS n FROM ${table} WHERE user_id = ?`).bind(id).first<{ n: number }>();
      expect(n?.n, table).toBe(0);
    }
    // The approval record survives as an anonymised tombstone, closed out of the open queue.
    const ap = await env.DB.prepare("SELECT * FROM approval_requests WHERE user_id = ?").bind(id).first<Record<string, unknown>>();
    expect(ap).toMatchObject({ user_id: id, decision: "account_deleted", decided_at: t.clock, note: null });
    expect((await t.refreshNative(native.refresh)).status).toBe(401);
    expect((await t.call("GET", "/me", { bearer: native.access })).status).toBe(401);
    expect(await t.auth.findTombstoneByEmail(email.toUpperCase())).toBe(id);
  });

  it("the same address can sign up again as a new account", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const oldId = (await userByEmail(email))!.id;
    const { access } = await t.loginWeb(email);
    await t.call("DELETE", "/account", { bearer: access, body: { password: GOOD_PW } });
    t.mailer.sent = [];
    await t.signup(email);
    expect(t.mailer.sent[0]!.kind).toBe("verify");
    const fresh = (await userByEmail(email))!;
    expect(fresh.id).not.toBe(oldId);
    expect(fresh.trust).toBe("unverified");
  });

  it("runs the app's cleanup statements in the same batch", async () => {
    const t = new Harness();
    await env.DB.prepare("CREATE TABLE IF NOT EXISTS app_ratings (user_id TEXT, stars INTEGER)").run();
    t.build({ onDelete: (uid) => [env.DB.prepare("DELETE FROM app_ratings WHERE user_id = ?").bind(uid)] });
    const email = uniqueEmail();
    await t.signupVerified(email);
    const id = (await userByEmail(email))!.id as string;
    await env.DB.prepare("INSERT INTO app_ratings VALUES (?, 5)").bind(id).run();
    const { access } = await t.loginWeb(email);
    expect((await t.call("DELETE", "/account", { bearer: access, body: { password: GOOD_PW } })).status).toBe(204);
    expect(await env.DB.prepare("SELECT 1 FROM app_ratings WHERE user_id = ?").bind(id).first()).toBeNull();
  });

  it("the schema itself refuses a tombstone that still holds PII", async () => {
    const id = crypto.randomUUID();
    await expect(
      env.DB.prepare("INSERT INTO users (id, email, trust, created_at, deleted_at) VALUES (?, 'x@y.zz', 'unverified', 1, 2)").bind(id).run(),
    ).rejects.toThrow(/CHECK/);
  });
});

describe("export", () => {
  it("returns the account, its live sessions and the approval state", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const a = await t.loginWeb(email);
    await t.loginNative(email);
    const res = await t.call("GET", "/account/export", { bearer: a.access, headers: { "User-Agent": "TestAgent/1" } });
    expect(res.status).toBe(200);
    const body = (await res.json()) as { account: Record<string, unknown>; sessions: { current: boolean }[]; approval: Record<string, unknown> };
    expect(body.account.email).toBe(email);
    expect(body.account).not.toHaveProperty("pw_hash");
    expect(body.sessions).toHaveLength(2);
    expect(body.sessions.filter((s) => s.current)).toHaveLength(1);
    expect(body.approval.decided_at).toBeNull();
  });
});

describe("purgeExpired", () => {
  it("drops expired rows and tombstone hashes past 14 days, and keeps rotated rows until they expire", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const id = (await userByEmail(email))!.id as string;
    const { access, refresh } = await t.loginNative(email);
    await t.refreshNative(refresh);
    await t.auth.purgeExpired();
    // The rotated predecessor survives so a later replay is still detected.
    const n = await env.DB.prepare("SELECT COUNT(*) AS n FROM sessions WHERE user_id = ?").bind(id).first<{ n: number }>();
    expect(n?.n).toBe(2);
    await t.call("DELETE", "/account", { bearer: access, body: { password: GOOD_PW } });
    t.advance(14 * 86400);
    const report = await t.auth.purgeExpired();
    expect(report.tombstoneHashes).toBeGreaterThanOrEqual(1);
    const row = await env.DB.prepare("SELECT deleted_email_hmac, deleted_email_hmac_expires_at FROM users WHERE id = ?").bind(id).first();
    expect(row).toEqual({ deleted_email_hmac: null, deleted_email_hmac_expires_at: null });
    expect(await t.auth.findTombstoneByEmail(email)).toBeNull();
  });

  it("optionally removes accounts never confirmed, freeing a squatted address", async () => {
    const t = new Harness();
    t.build({ purgeUnverifiedAfterSeconds: 7 * 86400 });
    const squatted = uniqueEmail();
    const confirmed = uniqueEmail();
    await t.signup(squatted);
    await t.signupVerified(confirmed);
    t.advance(7 * 86400);
    const report = await t.auth.purgeExpired();
    expect(report.unverifiedAccounts).toBeGreaterThanOrEqual(1);
    expect(await userByEmail(squatted)).toBeNull();
    expect(await userByEmail(confirmed)).not.toBeNull();
  });
});
