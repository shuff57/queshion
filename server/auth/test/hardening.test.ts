// Regression tests for the round-2 review findings. Each one fails on the code before the fix.
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AuthEvent } from "../src/index.js";
import { env, GOOD_PW, Harness, sha256b64url, snapshot, uniqueEmail } from "./helpers.js";

const STUDENT_PW = "students-own-password-1";
const TEACHER_PW = "teachers-real-password-2";

async function sessionsOf(email: string): Promise<number> {
  const r = await env.DB.prepare("SELECT COUNT(*) AS n FROM sessions s JOIN users u ON u.id = s.user_id WHERE u.email = ? AND s.revoked_at IS NULL")
    .bind(email)
    .first<{ n: number }>();
  return r?.n ?? 0;
}

describe("1. pre-hijack: confirming an address never activates someone else's password", () => {
  it("student signs up with the teacher's address, spams resend; teacher confirms; the student's password and session are dead", async () => {
    const t = new Harness();
    const email = uniqueEmail("teacher");
    await t.signup(email, STUDENT_PW);
    // The student cannot log in before confirmation ...
    expect((await t.call("POST", "/auth/login", { body: { email, password: STUDENT_PW } })).status).toBe(401);
    // ... and a session that somehow exists (say, created before this rule) must not survive confirmation.
    const uid = (await env.DB.prepare("SELECT id FROM users WHERE email = ?").bind(email).first<{ id: string }>())!.id;
    const planted = "P".repeat(43);
    await env.DB.prepare(
      "INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
      .bind(crypto.randomUUID(), crypto.randomUUID(), uid, await sha256b64url(planted), t.clock, t.clock, t.clock + 86400_000)
      .run();
    for (let i = 0; i < 3; i++) await t.call("POST", "/auth/resend", { body: { email } });
    // The teacher clicks the newest link without choosing a password.
    const link = t.mailer.last("verify", email)!.token!;
    const v = await t.call("POST", "/auth/verify", { body: { token: link } });
    expect(v.status).toBe(200);
    expect(await v.json()).toEqual({ ok: true, password_set: false });
    expect((await t.call("POST", "/auth/login", { body: { email, password: STUDENT_PW } })).status).toBe(401);
    expect((await t.refreshNative(planted)).status).toBe(401);
    expect(await sessionsOf(email)).toBe(0);
    const row = await env.DB.prepare("SELECT pw_hash, trust, email_verified_at FROM users WHERE id = ?").bind(uid).first<Record<string, unknown>>();
    expect(row?.pw_hash).toBeNull();
    expect(row?.trust).toBe("unverified"); // confirmed but not active until the owner sets a password
    // The teacher completes the account through the reset flow, entering the profile there.
    await t.call("POST", "/auth/forgot", { body: { email } });
    const reset = t.mailer.last("reset", email)!.token!;
    expect((await t.call("POST", "/auth/reset", { body: { token: reset, password: TEACHER_PW, display_name: "Ms. Real" } })).status).toBe(200);
    expect((await t.call("POST", "/auth/login", { body: { email, password: TEACHER_PW } })).status).toBe(200);
    const after = await env.DB.prepare("SELECT trust, display_name FROM users WHERE id = ?").bind(uid).first<Record<string, unknown>>();
    expect(after).toEqual({ trust: "pending", display_name: "Ms. Real" });
  });

  it("the person confirming can choose the password in the same step; the sign-up password is discarded", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const token = await t.signup(email, STUDENT_PW);
    const v = await t.call("POST", "/auth/verify", { body: { token, password: TEACHER_PW } });
    expect(await v.json()).toEqual({ ok: true, password_set: true });
    expect((await t.call("POST", "/auth/login", { body: { email, password: STUDENT_PW } })).status).toBe(401);
    expect((await t.call("POST", "/auth/login", { body: { email, password: TEACHER_PW } })).status).toBe(200);
  });

  it("a weak password at verify is refused without consuming the link; a bad token costs no hash", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const token = await t.signup(email);
    t.hasher.calls = 0;
    expect(await (await t.call("POST", "/auth/verify", { body: { token, password: "password123" } })).json()).toEqual({ error: "weak_password", reason: "common" });
    expect((await t.call("POST", "/auth/verify", { body: { token: "Q".repeat(43), password: TEACHER_PW } })).status).toBe(400);
    expect(t.hasher.calls).toBe(0);
    expect((await t.call("POST", "/auth/verify", { body: { token, password: TEACHER_PW } })).status).toBe(200);
  });

  it("bounds mail per account with a gap and a daily ceiling, without ever answering differently", async () => {
    const t = new Harness();
    t.build({ mailCeiling: 4 });
    const email = uniqueEmail();
    await t.signup(email); // mail 1
    const statuses = [];
    for (let i = 0; i < 10; i++) {
      statuses.push((await t.call("POST", "/auth/resend", { ip: `10.7.0.${i}`, body: { email } })).status);
      t.advance(60); // every other request falls inside the 2-minute gap
    }
    expect(statuses).toEqual(new Array(10).fill(202));
    // Mails at 0 s (sign-up), 120 s, 240 s, 360 s; the gap skipped the ones between, and the
    // ceiling of 4 skipped 480 s.
    expect(t.mailer.sent.filter((m) => m.kind === "verify")).toHaveLength(4);
  });

  it("purges never-confirmed accounts after 7 days by default", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signup(email);
    t.advance(7 * 86400);
    await t.auth.purgeExpired();
    expect(await env.DB.prepare("SELECT 1 FROM users WHERE email = ?").bind(email).first()).toBeNull();
  });
});

describe("2. lockout griefing: only failures count, per (account, IP)", () => {
  it("10 failures from IP A do not block a correct login from IP B", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 10; i++) await t.call("POST", "/auth/login", { ip: "10.66.0.1", body: { email, password: "attacker-guess-xx" } });
    expect((await t.call("POST", "/auth/login", { ip: "10.66.0.2", body: { email, password: GOOD_PW } })).status).toBe(200);
  });

  it("the failing IP itself is blocked for that account after 5 failures, before any hash", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const st = [];
    for (let i = 0; i < 6; i++) st.push((await t.call("POST", "/auth/login", { ip: "10.66.1.1", body: { email, password: "attacker-guess-yy" } })).status);
    expect(st).toEqual([401, 401, 401, 401, 401, 429]);
    t.hasher.calls = 0;
    expect((await t.call("POST", "/auth/login", { ip: "10.66.1.1", body: { email, password: GOOD_PW } })).status).toBe(429);
    expect(t.hasher.calls).toBe(0);
    // Back-off, not a hard lock: the window ages out.
    t.advance(30 * 60);
    expect((await t.call("POST", "/auth/login", { ip: "10.66.1.1", body: { email, password: GOOD_PW } })).status).toBe(200);
  });

  it("successful logins never count against the account", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 15; i++) expect((await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } })).status).toBe(200);
  });

  it("under a distributed attack the account is locked for every network, except the owner's known device", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const device = (await t.loginNative(email)).res;
    const deviceToken = ((await device.json()) as { device_token: string }).device_token;
    for (let i = 0; i < 31; i++) await t.call("POST", "/auth/login", { ip: `10.88.${i}.1`, body: { email, password: "spray-guess-zzzz" } });
    t.hasher.calls = 0;
    // A fresh network with a wrong password is refused, and so is one with the right password.
    expect((await t.call("POST", "/auth/login", { ip: "192.0.2.10", body: { email, password: "fresh-wrong-guess" } })).status).toBe(429);
    expect((await t.call("POST", "/auth/login", { ip: "192.0.2.11", body: { email, password: GOOD_PW } })).status).toBe(429);
    expect(t.hasher.calls).toBe(0);
    // The owner's known device still gets in, from a new network.
    const ok = await t.call("POST", "/auth/login", { native: true, ip: "192.0.2.12", headers: { "X-Device-Token": deviceToken }, body: { email, password: GOOD_PW } });
    expect(ok.status).toBe(200);
  });

  it("unknown addresses get exactly the same failure accounting", async () => {
    for (const exists of [true, false]) {
      const t = new Harness();
      const email = uniqueEmail();
      if (exists) await t.signupVerified(email);
      const st = [];
      for (let i = 0; i < 6; i++) st.push((await t.call("POST", "/auth/login", { body: { email, password: "guess-guess-guess" } })).status);
      expect(st).toEqual([401, 401, 401, 401, 401, 429]);
    }
  });
});

describe("3. login racing a reset or a deletion", () => {
  it("a reset committing between the password check and the session insert makes the login fail", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    t.hasher.onHash = async () => {
      await env.DB.prepare("UPDATE users SET pw_hash = '$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA' WHERE email = ?")
        .bind(email)
        .run();
    };
    const raced = await snapshot(await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } }));
    const wrong = await snapshot(await t.call("POST", "/auth/login", { body: { email: uniqueEmail(), password: GOOD_PW } }));
    expect(raced).toEqual(wrong);
    expect(await sessionsOf(email)).toBe(0);
  });

  it("a deletion committing in between leaves no session for the tombstone", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const uid = (await env.DB.prepare("SELECT id FROM users WHERE email = ?").bind(email).first<{ id: string }>())!.id;
    t.hasher.onHash = async () => {
      await env.DB.prepare(
        "UPDATE users SET email = NULL, email_verified_at = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL, profile = NULL, deleted_at = 1 WHERE id = ?",
      )
        .bind(uid)
        .run();
    };
    expect((await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } })).status).toBe(401);
    const n = await env.DB.prepare("SELECT COUNT(*) AS n FROM sessions WHERE user_id = ?").bind(uid).first<{ n: number }>();
    expect(n?.n).toBe(0);
  });

  it("a deletion whose password check is overtaken by a reset does not delete", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { access } = await t.loginWeb(email);
    t.hasher.onHash = async () => {
      await env.DB.prepare("UPDATE users SET pw_hash = 'changed-by-reset' WHERE email = ?").bind(email).run();
    };
    expect((await t.call("DELETE", "/account", { bearer: access, body: { password: GOOD_PW } })).status).toBe(401);
    const row = await env.DB.prepare("SELECT deleted_at FROM users WHERE email = ?").bind(email).first<{ deleted_at: number | null }>();
    expect(row?.deleted_at).toBeNull();
  });
});

describe("4/8. authenticate() always checks revocation; there is no token-only mode", () => {
  it("a logged-out session's access token is refused, whatever options are passed", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { access, refresh } = await t.loginWeb(email);
    await t.call("POST", "/auth/logout", { cookie: `__Host-refresh=${refresh}` });
    const req = new Request("https://api.example.org/x", { headers: { Authorization: `Bearer ${access}` } });
    expect(await t.auth.authenticate(req)).toBeNull();
    // The removed opt-out must not come back through a loosely typed caller.
    const loose = t.auth.authenticate as (r: Request, o?: unknown) => Promise<unknown>;
    expect(await loose(req, { live: false })).toBeNull();
  });
});

describe("5. refresh grace window and reuse alerts", () => {
  it("defaults to 10 seconds", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginNative(email);
    await t.refreshNative(refresh);
    t.advance(11);
    expect((await t.refreshNative(refresh)).status).toBe(401);
  });

  it("calls onRefreshReuse only for the revoking replay; a grace repeat goes to the audit stream only", async () => {
    const seen: AuthEvent[] = [];
    const t = new Harness();
    t.build({ onRefreshReuse: (e) => seen.push(e) });
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginNative(email);
    await t.refreshNative(refresh);
    await t.refreshNative(refresh);
    t.advance(60);
    await t.refreshNative(refresh);
    expect(seen.map((e) => e.type)).toEqual(["refresh_reuse_revoked"]);
    expect(seen[0]!.sessionFamily).toBeTruthy();
    expect(t.events.map((e) => e.type)).toContain("refresh_grace_reuse");
  });
});

describe("6. deletion without an onDelete hook is loud", () => {
  afterEach(() => vi.restoreAllMocks());
  it("warns that app data was not cleaned up", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { access } = await t.loginWeb(email);
    expect((await t.call("DELETE", "/account", { bearer: access, body: { password: GOOD_PW } })).status).toBe(204);
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("no onDelete hook"));
  });
});

describe("8. a missing client IP fails closed", () => {
  it("refuses rate-limited routes without CF-Connecting-IP, before any hash", async () => {
    const t = new Harness();
    for (const [path, body] of [
      ["/auth/login", { email: uniqueEmail(), password: GOOD_PW }],
      ["/auth/signup", { email: uniqueEmail(), password: GOOD_PW }],
      ["/auth/forgot", { email: uniqueEmail() }],
    ] as const) {
      const res = await t.call("POST", path, { ip: null, body });
      expect(res.status).toBe(403);
      expect(await res.json()).toEqual({ error: "client_ip_required" });
    }
    expect(t.hasher.calls).toBe(0);
    expect(t.mailer.sent).toHaveLength(0);
  });

  it("works with an explicit resolver", async () => {
    const t = new Harness();
    t.build({ clientIp: (req) => req.headers.get("X-Real-IP") });
    const res = await t.call("POST", "/auth/forgot", { ip: null, headers: { "X-Real-IP": "198.51.100.9" }, body: { email: uniqueEmail() } });
    expect(res.status).toBe(202);
  });
});
