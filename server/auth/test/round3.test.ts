// Regression tests for the round-3 review. P1-P4 are the reviewer's probes, turned into assertions.
import { describe, expect, it } from "vitest";
import { InputError, MemoryRateLimitStore } from "../src/index.js";
import { cookieValue, env, GOOD_PW, Harness, uniqueEmail } from "./helpers.js";

const v6 = (i: number) => `2001:db8:7:${i.toString(16)}::1`; // a different /64 each, all in one /48

async function deviceTokenFor(t: Harness, email: string): Promise<string> {
  const { res } = await t.loginNative(email);
  return ((await res.json()) as { device_token: string }).device_token;
}

async function lockDown(t: Harness, email: string) {
  for (let i = 0; i < 30; i++) await t.call("POST", "/auth/login", { ip: `10.${100 + i}.0.1`, body: { email, password: `spray-${i}-guess` } });
}

describe("1. online guessing: lockdown with known devices", () => {
  it("P1: rotating 300 IPv6 /64s gets at most the threshold of hashed guesses, then only refusals", async () => {
    const h = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    const victim = uniqueEmail("t");
    await h.signupVerified(victim);
    const before = h.hasher.calls;
    const st: Record<number, number> = {};
    for (let i = 0; i < 300; i++) {
      const r = await h.call("POST", "/auth/login", { body: { email: victim, password: `wrong-guess-${i}xx` }, ip: v6(i) });
      st[r.status] = (st[r.status] ?? 0) + 1;
    }
    // All 300 /64s share one /48, so the pair limit (5) binds long before the account limit.
    expect(st[401]).toBe(5);
    expect(st[429]).toBe(295);
    expect(h.hasher.calls - before).toBe(5);
  });

  it("P1 across /48s: after 30 failures every network is refused without a hash", async () => {
    const h = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    const victim = uniqueEmail("t");
    await h.signupVerified(victim);
    const before = h.hasher.calls;
    let refused = 0;
    for (let i = 0; i < 300; i++) {
      const r = await h.call("POST", "/auth/login", { body: { email: victim, password: `wrong-guess-${i}xx` }, ip: `2001:db8:${i.toString(16)}::1` });
      if (r.status === 429) refused++;
    }
    expect(h.hasher.calls - before).toBe(30);
    expect(refused).toBe(270);
    // The real teacher, from home, without a known device: refused too (recovery is via reset) ...
    const home = await h.call("POST", "/auth/login", { ip: "198.51.100.7", body: { email: victim, password: GOOD_PW } });
    expect(home.status).toBe(429);
    expect(Number(home.headers.get("Retry-After"))).toBeGreaterThan(0);
  });

  it("a known device logs in during lockdown; forgot/reset keeps working for everyone else", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const dev = await deviceTokenFor(t, email);
    await lockDown(t, email);
    const ok = await t.call("POST", "/auth/login", { native: true, ip: "203.0.113.50", headers: { "X-Device-Token": dev }, body: { email, password: GOOD_PW } });
    expect(ok.status).toBe(200);
    // A web browser that logged in before carries the device cookie.
    const t2 = new Harness();
    const e2 = uniqueEmail();
    await t2.signupVerified(e2);
    const web = await t2.call("POST", "/auth/login", { body: { email: e2, password: GOOD_PW } });
    const cookie = `__Host-device=${cookieValue(web, "__Host-device")}`;
    await lockDown(t2, e2);
    expect((await t2.call("POST", "/auth/login", { ip: "203.0.113.51", body: { email: e2, password: GOOD_PW } })).status).toBe(429);
    expect((await t2.call("POST", "/auth/login", { ip: "203.0.113.51", cookie, body: { email: e2, password: GOOD_PW } })).status).toBe(200);
    // Recovery is never blocked by the lockdown.
    expect((await t.call("POST", "/auth/forgot", { ip: "203.0.113.52", body: { email } })).status).toBe(202);
    expect(t.mailer.last("reset", email)).toBeTruthy();
  });

  it("a known device token unlocks only its own account, and a forged or expired one nothing", async () => {
    const t = new Harness();
    const a = uniqueEmail("a");
    const b = uniqueEmail("b");
    await t.signupVerified(a);
    await t.signupVerified(b);
    const devA = await deviceTokenFor(t, a);
    await lockDown(t, b);
    const tryB = (token: string) => t.call("POST", "/auth/login", { native: true, ip: "203.0.113.60", headers: { "X-Device-Token": token }, body: { email: b, password: GOOD_PW } });
    expect((await tryB(devA)).status).toBe(429);
    const [v, , epoch, exp, nonce, mac] = devA.split(".");
    const bId = (await env.DB.prepare("SELECT id FROM users WHERE email = ?").bind(b).first<{ id: string }>())!.id;
    expect((await tryB(`${v}.${bId}.${epoch}.${exp}.${nonce}.${mac}`)).status).toBe(429); // re-addressed, bad MAC
    // Expiry: 90 days.
    await lockDown(t, a);
    t.advance(90 * 86400);
    await lockDown(t, a);
    const late = await t.call("POST", "/auth/login", { native: true, ip: "203.0.113.61", headers: { "X-Device-Token": devA }, body: { email: a, password: GOOD_PW } });
    expect(late.status).toBe(429);
  });

  it("device tokens die on reset, on logout everywhere, and on deletion", async () => {
    for (const how of ["reset", "logout-all"] as const) {
      const t = new Harness();
      const email = uniqueEmail();
      await t.signupVerified(email);
      const dev = await deviceTokenFor(t, email);
      let pw = GOOD_PW;
      if (how === "reset") {
        await t.call("POST", "/auth/forgot", { body: { email } });
        pw = "after-reset-password-1";
        expect((await t.call("POST", "/auth/reset", { body: { token: t.mailer.last("reset", email)!.token, password: pw } })).status).toBe(200);
      } else {
        const { refresh } = await t.loginNative(email);
        expect((await t.call("POST", "/auth/logout", { native: true, body: { refresh_token: refresh, scope: "global" } })).status).toBe(204);
      }
      await lockDown(t, email);
      const r = await t.call("POST", "/auth/login", { native: true, ip: "203.0.113.70", headers: { "X-Device-Token": dev }, body: { email, password: pw } });
      expect(r.status, how).toBe(429);
    }
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { access } = await t.loginNative(email);
    await t.call("DELETE", "/account", { bearer: access, native: true, body: { password: GOOD_PW } });
    const row = await env.DB.prepare("SELECT device_epoch FROM users WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC LIMIT 1").first<{ device_epoch: number }>();
    expect(row!.device_epoch).toBeGreaterThan(0);
  });

  it("concurrent guesses cannot overshoot the pair limit (slots are reserved atomically)", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    t.hasher.calls = 0;
    const res = await Promise.all(Array.from({ length: 20 }, (_, i) => t.call("POST", "/auth/login", { ip: "10.250.0.1", body: { email, password: `parallel-${i}-xx` } })));
    expect(res.filter((r) => r.status === 401)).toHaveLength(5);
    expect(t.hasher.calls).toBe(5);
  });
});

describe("2. recovery cannot be blocked", () => {
  it("P2: a squatter's forgot requests never kill the teacher's link, and the replies stay uniform", async () => {
    const h = new Harness();
    const victim = uniqueEmail("t");
    await h.signupVerified(victim);
    const t1 = await h.call("POST", "/auth/forgot", { body: { email: victim }, ip: "203.0.113.5" });
    const teacherTok = h.mailer.last("reset", victim)!.token!;
    const statuses = [];
    for (let i = 0; i < 6; i++) {
      statuses.push((await h.call("POST", "/auth/forgot", { body: { email: victim }, ip: "10.9.9." + i })).status);
      h.advance(10);
    }
    const t2 = await h.call("POST", "/auth/forgot", { body: { email: victim }, ip: "203.0.113.5" });
    const use = await h.call("POST", "/auth/reset", { body: { token: teacherTok, password: "brand-new-pass-77" }, ip: "203.0.113.5" });
    expect([t1.status, t2.status, use.status]).toEqual([202, 202, 200]);
    expect(statuses).toEqual(new Array(6).fill(202));
    // Unknown address: the same answers.
    const ghost = uniqueEmail("g");
    for (let i = 0; i < 4; i++) expect((await h.call("POST", "/auth/forgot", { body: { email: ghost }, ip: "10.8.8." + i })).status).toBe(202);
  });

  it("a burst well past the ceiling, then past the link lifetime: the owner can still get a working link", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 60; i++) {
      await t.call("POST", "/auth/forgot", { ip: `10.41.${i}.1`, body: { email } });
      t.advance(30); // four requests per gap: 60 requests in 30 minutes, far past the ceiling of 10
    }
    expect(t.mailer.sent.filter((m) => m.kind === "reset").length).toBeLessThanOrEqual(10);
    // A link mailed during the burst still works now ...
    expect((await t.call("POST", "/auth/reset", { body: { token: t.mailer.last("reset", email)!.token, password: "owner-gets-back-in" } })).status).toBe(200);
    // ... and after every burst link has expired, a new request mails a new working one.
    t.advance(61 * 60);
    await t.call("POST", "/auth/forgot", { body: { email } });
    expect((await t.call("POST", "/auth/reset", { body: { token: t.mailer.last("reset", email)!.token, password: "owner-gets-back-in-2" } })).status).toBe(200);
  });
});

describe("3. a squatter's profile never survives confirmation", () => {
  const withProfileHook = () =>
    new Harness({
      profile: (b) => {
        if (typeof b.display_name !== "string" || !b.display_name) throw new InputError("required", "display_name");
        if (typeof b.school !== "string" || !b.school) throw new InputError("required", "school");
        return { displayName: b.display_name, profile: { school: b.school, adult: true } };
      },
    });

  it("P3: confirming clears the student's name and school; the confirmer's own profile replaces them", async () => {
    const h = withProfileHook();
    const victim = uniqueEmail("t");
    await h.call("POST", "/auth/signup", { body: { email: victim, password: "student-pass-123", display_name: "Mr Haxx0r", school: "Fake High" }, ip: "10.1.1.1" });
    for (let i = 0; i < 5; i++) await h.call("POST", "/auth/resend", { body: { email: victim }, ip: "10.2.2." + i });
    // The teacher's own resend is answered normally, and the links already mailed still work.
    expect((await h.call("POST", "/auth/resend", { body: { email: victim }, ip: "203.0.113.9" })).status).toBe(202);
    const tok = h.mailer.last("verify", victim)!.token!;
    // Choosing a password at confirmation requires the profile too.
    const missing = await h.call("POST", "/auth/verify", { body: { token: tok, password: "teacher-chosen-99" } });
    expect(missing.status).toBe(400);
    const v = await h.call("POST", "/auth/verify", { body: { token: tok, password: "teacher-chosen-99", display_name: "Ms. Teacher", school: "Real High" } });
    expect(v.status).toBe(200);
    const row = await env.DB.prepare("SELECT display_name, profile, trust FROM users WHERE email = ?1").bind(victim).first<Record<string, string>>();
    expect(row).toMatchObject({ display_name: "Ms. Teacher", trust: "pending" });
    expect(JSON.parse(row!.profile!)).toEqual({ school: "Real High", adult: true });
    expect((await h.call("POST", "/auth/login", { body: { email: victim, password: "student-pass-123" } })).status).toBe(401);
  });

  it("confirming without a password clears the profile; the reset that completes it requires a new one", async () => {
    const h = withProfileHook();
    const email = uniqueEmail();
    await h.call("POST", "/auth/signup", { body: { email, password: "student-pass-123", display_name: "Mr Haxx0r", school: "Fake High" } });
    await h.call("POST", "/auth/verify", { body: { token: h.mailer.last("verify", email)!.token } });
    const cleared = await env.DB.prepare("SELECT display_name, profile FROM users WHERE email = ?").bind(email).first();
    expect(cleared).toEqual({ display_name: null, profile: null });
    await h.call("POST", "/auth/forgot", { body: { email } });
    const tok = h.mailer.last("reset", email)!.token!;
    expect((await h.call("POST", "/auth/reset", { body: { token: tok, password: "teacher-chosen-99" } })).status).toBe(400);
    expect((await h.call("POST", "/auth/reset", { body: { token: tok, password: "teacher-chosen-99", display_name: "Ms. T", school: "Real High" } })).status).toBe(200);
    const row = await env.DB.prepare("SELECT display_name, trust FROM users WHERE email = ?").bind(email).first();
    expect(row).toEqual({ display_name: "Ms. T", trust: "pending" });
  });

  it("an active account's reset neither needs nor changes the profile", async () => {
    const h = withProfileHook();
    const email = uniqueEmail();
    await h.call("POST", "/auth/signup", { body: { email, password: GOOD_PW, display_name: "x", school: "y" } });
    await h.call("POST", "/auth/verify", { body: { token: h.mailer.last("verify", email)!.token, password: GOOD_PW, display_name: "Ms. Kept", school: "Kept High" } });
    await h.call("POST", "/auth/forgot", { body: { email } });
    expect((await h.call("POST", "/auth/reset", { body: { token: h.mailer.last("reset", email)!.token, password: "changed-password-1", display_name: "Evil" } })).status).toBe(200);
    expect((await env.DB.prepare("SELECT display_name FROM users WHERE email = ?").bind(email).first())).toEqual({ display_name: "Ms. Kept" });
  });
});

describe("4. activation through reset notifies once", () => {
  it("P4: reset on a never-confirmed account opens the approval request and calls onVerified exactly once", async () => {
    let notified = 0;
    const h = new Harness({ onVerified: async () => void notified++ });
    const victim = uniqueEmail("t");
    await h.call("POST", "/auth/signup", { body: { email: victim, password: "student-pass-123", display_name: "x" } });
    await h.call("POST", "/auth/forgot", { body: { email: victim } });
    const r = await h.call("POST", "/auth/reset", { body: { token: h.mailer.last("reset", victim)!.token, password: "teacher-chosen-99", display_name: "Ms. T" } });
    expect(r.status).toBe(200);
    const row = await env.DB.prepare("SELECT u.trust, a.requested_at FROM users u LEFT JOIN approval_requests a ON a.user_id = u.id WHERE email = ?1").bind(victim).first<Record<string, unknown>>();
    expect(row?.trust).toBe("pending");
    expect(row?.requested_at).toBe(h.clock);
    expect(notified).toBe(1);
    // A second reset later does not notify again.
    h.advance(200);
    await h.call("POST", "/auth/forgot", { body: { email: victim } });
    await h.call("POST", "/auth/reset", { body: { token: h.mailer.last("reset", victim)!.token, password: "another-password-9" } });
    expect(notified).toBe(1);
  });
});

describe("6. login hashing cost after a cost raise", () => {
  it("unknown addresses hash at the current cost; an old account at its stored cost (documented residual)", async () => {
    const seen: number[] = [];
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email); // stored at m=19456
    const orig = t.hasher.argon2id.bind(t.hasher);
    t.hasher.argon2id = (pw, salt, p) => (seen.push(p.m), orig(pw, salt, p));
    t.build({ argon2: { m: 46080, t: 2, p: 1 } });
    await t.call("POST", "/auth/login", { body: { email: uniqueEmail(), password: "wrong-wrong-wrong" } });
    await t.call("POST", "/auth/login", { body: { email, password: "wrong-wrong-wrong" } });
    // The residual: until the old hash is upgraded (on its next successful login) this account's
    // failures are cheaper than an unknown address's. See README, "Raising the cost".
    expect(seen).toEqual([46080, 19456]);
  });
});

describe("9. deletion keeps an anonymised approval tombstone", () => {
  it("a decided approval keeps decision, decided_by and decided_at; the note goes", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const id = (await env.DB.prepare("SELECT id FROM users WHERE email = ?").bind(email).first<{ id: string }>())!.id;
    await t.signupVerified(uniqueEmail("admin"));
    const admin = (await env.DB.prepare("SELECT id FROM users WHERE id != ? ORDER BY created_at DESC LIMIT 1").bind(id).first<{ id: string }>())!.id;
    await env.DB.prepare(
      "UPDATE approval_requests SET decided_at = 5, decided_by = ?, decision = 'approved', evidence = 'school_confirmed', evidence_note = 'office of Jane Doe', note = 'called the office, spoke to Jane Doe', reason = NULL WHERE user_id = ?",
    )
      .bind(admin, id)
      .run();
    const { access } = await t.loginNative(email);
    await t.call("DELETE", "/account", { native: true, bearer: access, body: { password: GOOD_PW } });
    const ap = await env.DB.prepare("SELECT * FROM approval_requests WHERE user_id = ?").bind(id).first();
    expect(ap).toMatchObject({ user_id: id, requested_at: t.clock, decided_at: 5, decided_by: admin, decision: "approved", evidence: "school_confirmed" });
    expect(ap).toMatchObject({ note: null, evidence_note: null, reason: null });
  });
});
