// Regression tests for the round-4 review. C1-C4 are the reviewer's probes, turned into assertions.
import { describe, expect, it } from "vitest";
import { MemoryRateLimitStore } from "../src/index.js";
import { cookieValue, env, GOOD_PW, Harness, uniqueEmail, worst24h } from "./helpers.js";

async function lockDown(t: Harness, email: string, base = 100) {
  for (let i = 0; i < 30; i++) await t.call("POST", "/auth/login", { ip: `10.${base + i}.0.1`, body: { email, password: `spray-${i}-guess` } });
}

const deviceCookie = (res: Response) => `__Host-device=${cookieValue(res, "__Host-device")}`;


describe("1. a completed reset or activation restores login", () => {
  it("C1: during a lockdown, the teacher resets and then logs in (fresh device token, counters cleared)", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const web = await t.call("POST", "/auth/login", { ip: "198.51.100.1", body: { email, password: GOOD_PW } });
    const oldCookie = deviceCookie(web);
    await lockDown(t, email);
    await t.call("POST", "/auth/forgot", { ip: "198.51.100.1", body: { email } });
    const r = await t.call("POST", "/auth/reset", { ip: "198.51.100.1", cookie: oldCookie, body: { token: t.mailer.last("reset", email)!.token, password: "new-teacher-pass-42" } });
    expect(r.status).toBe(200);
    const fresh = deviceCookie(r);
    expect(fresh).not.toBe(oldCookie);
    expect((await t.call("POST", "/auth/login", { ip: "198.51.100.1", cookie: fresh, body: { email, password: "new-teacher-pass-42" } })).status).toBe(200);
    // The old device token died with the reset ...
    await lockDown(t, email, 140);
    expect((await t.call("POST", "/auth/login", { ip: "198.51.100.2", cookie: oldCookie, body: { email, password: "new-teacher-pass-42" } })).status).toBe(429);
    // ... and the new one keeps working through a renewed lockdown.
    expect((await t.call("POST", "/auth/login", { ip: "198.51.100.2", cookie: fresh, body: { email, password: "new-teacher-pass-42" } })).status).toBe(200);
  });

  it("C1 native: the reset response carries the new device token in the body", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    await lockDown(t, email);
    await t.call("POST", "/auth/forgot", { body: { email } });
    const r = await t.call("POST", "/auth/reset", { native: true, body: { token: t.mailer.last("reset", email)!.token, password: "native-new-pass-1" } });
    const body = (await r.json()) as { ok: boolean; device_token: string };
    expect(body.ok).toBe(true);
    expect(body.device_token).toMatch(/^v1\./);
    expect(r.headers.get("Set-Cookie")).toBeNull();
    // The reset re-keyed the counters, so a network that took part in the lockdown (its pair
    // spent too) gets in right after, with no device token.
    expect((await t.call("POST", "/auth/login", { ip: "10.100.0.1", body: { email, password: "native-new-pass-1" } })).status).toBe(200);
  });

  it("C1b: a patient attacker is held to the daily rule's budget, and a reset gets the teacher back in for good", async () => {
    const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    const email = uniqueEmail();
    await t.signupVerified(email);
    const hits: number[] = [];
    let cookie: string | null = null;
    let teacherOkAfterReset = 0;
    let teacherTriesAfterReset = 0;
    for (let m = 0; m < 24 * 60; m += 2) {
      const before = t.hasher.calls;
      await t.call("POST", "/auth/login", { ip: `2001:db8:${(m / 2) % 4}::1`, body: { email, password: `g-${m}-guessxx` } });
      if (t.hasher.calls > before) hits.push(m);
      if (m === 6 * 60) {
        await t.call("POST", "/auth/forgot", { ip: "198.51.100.9", body: { email } });
        const r = await t.call("POST", "/auth/reset", { ip: "198.51.100.9", body: { token: t.mailer.last("reset", email)!.token, password: "teacher-after-reset" } });
        cookie = deviceCookie(r);
      }
      if (cookie && m % 30 === 0) {
        teacherTriesAfterReset++;
        const r = await t.call("POST", "/auth/login", { ip: "198.51.100.9", cookie, body: { email, password: "teacher-after-reset" } });
        if (r.status === 200) teacherOkAfterReset++;
      }
      t.advance(120);
    }
    // The reset re-keys the counters and so starts a fresh daily budget: the worst 24 h span holds
    // one budget before the reset and one after (measured exactly 200).
    expect(worst24h(hits)).toBeLessThanOrEqual(200);
    expect(teacherOkAfterReset).toBe(teacherTriesAfterReset);
    expect(teacherTriesAfterReset).toBeGreaterThan(30);
  });

  it("verify with a password ends a lockdown planted before activation and returns a device token", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const token = await t.signup(email, "students-own-pass-1");
    await lockDown(t, email); // the student sprays before the teacher ever confirms
    const v = await t.call("POST", "/auth/verify", { body: { token, password: "teacher-chosen-pw-1", display_name: "Ms. T" } });
    expect(v.status).toBe(200);
    expect(await v.json()).toEqual({ ok: true, password_set: true });
    expect(cookieValue(v, "__Host-device")).toMatch(/^v1\./);
    expect((await t.call("POST", "/auth/login", { ip: "192.0.2.90", body: { email, password: "teacher-chosen-pw-1" } })).status).toBe(200);
  });
});

describe("2. a known device is never refused for other people's failures", () => {
  it("C2: on a shared school NAT, students burn the pair; the teacher with a device token still gets in", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const nat = "203.0.113.200";
    const web = await t.call("POST", "/auth/login", { ip: nat, body: { email, password: GOOD_PW } });
    const cookie = deviceCookie(web);
    for (let i = 0; i < 5; i++) await t.call("POST", "/auth/login", { ip: nat, body: { email, password: `student-${i}-guess` } });
    expect((await t.call("POST", "/auth/login", { ip: nat, body: { email, password: GOOD_PW } })).status).toBe(429);
    expect((await t.call("POST", "/auth/login", { ip: nat, cookie, body: { email, password: GOOD_PW } })).status).toBe(200);
  });

  it("a device token also skips the per-IP login limit saturated by others", async () => {
    const t = new Harness();
    t.build({ rateLimits: { login: { ip: [{ limit: 3, windowSeconds: 300 }], failures: { perAccountIp: { limit: 5, windowSeconds: 900 }, perAccount: { limit: 30, windowSeconds: 3600 } } } } });
    const email = uniqueEmail();
    await t.signupVerified(email);
    const nat = "203.0.113.201";
    const cookie = deviceCookie(await t.call("POST", "/auth/login", { ip: nat, body: { email, password: GOOD_PW } }));
    for (let i = 0; i < 3; i++) await t.call("POST", "/auth/login", { ip: nat, body: { email: uniqueEmail(), password: GOOD_PW } });
    expect((await t.call("POST", "/auth/login", { ip: nat, body: { email, password: GOOD_PW } })).status).toBe(429);
    expect((await t.call("POST", "/auth/login", { ip: nat, cookie, body: { email, password: GOOD_PW } })).status).toBe(200);
  });

  it("a stolen device token still cannot guess freely: 10 failures per device per 15 minutes", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const cookie = deviceCookie(await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } }));
    const st = [];
    for (let i = 0; i < 11; i++) st.push((await t.call("POST", "/auth/login", { ip: `10.200.${i}.1`, cookie, body: { email, password: `thief-${i}-guess` } })).status);
    expect(st).toEqual([...new Array(10).fill(401), 429]);
  });
});

describe("3/4. the mail ceiling never outlives the link", () => {
  it("C3: a forgot burst past the ceiling always leaves a valid link, and a fresh one after the lifetime", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 31; i++) {
      expect((await t.call("POST", "/auth/forgot", { ip: `10.60.${i}.1`, body: { email } })).status).toBe(202);
      // At every moment the newest mailed link is unexpired and within the newest three.
      const newest = t.mailer.last("reset", email)!;
      expect(newest.expiresAt!).toBeGreaterThan(t.clock);
      t.advance(121);
    }
    const sent = t.mailer.sent.filter((m) => m.kind === "reset" && m.to === email);
    expect(sent.length).toBeLessThan(31); // the ceiling did bind
    expect(sent.length).toBeGreaterThan(10); // and released once the window moved on
    t.advance(2 * 3600); // past every link's lifetime
    const before = t.mailer.sent.length;
    expect((await t.call("POST", "/auth/forgot", { ip: "198.51.100.3", body: { email } })).status).toBe(202);
    expect(t.mailer.sent.length - before).toBe(1);
    const use = await t.call("POST", "/auth/reset", { ip: "198.51.100.3", body: { token: t.mailer.last("reset", email)!.token, password: "owner-wants-in-1" } });
    expect(use.status).toBe(200);
  });

});

describe("5. a daily cap on failures", () => {
  it.each([0, 21])("a persistent attacker gets at most ~115 hashed guesses in ANY 24 h span (start %ih)", async (startH) => {
    {
      const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
      t.clock = Date.UTC(2026, 9, 19, startH, 0, 0);
      const email = uniqueEmail();
      await t.signupVerified(email);
      const hits: number[] = [];
      // One guess every 2 minutes (30 an hour, right at the hourly rule) for 36 hours.
      for (let m = 0; m < 36 * 60; m += 2) {
        const before = t.hasher.calls;
        await t.call("POST", "/auth/login", { ip: `2001:db8:${(m / 2) % 16}::1`, body: { email, password: `g-${m}-guessxx` } });
        if (t.hasher.calls > before) hits.push(m);
        t.advance(120);
      }
      // Limit 100 plus at most one 2-hour sub-bucket's weighting error (measured 100 to 114).
      expect(worst24h(hits), `start ${startH}h`).toBeLessThanOrEqual(115);
    }
  });

  it("staying under the hourly rule still runs into 100 failures a day; a device holder is exempt", async () => {
    const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    const email = uniqueEmail();
    await t.signupVerified(email);
    const cookie = deviceCookie(await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } }));
    let hashed = 0;
    for (let i = 0; i < 140; i++) {
      const before = t.hasher.calls;
      await t.call("POST", "/auth/login", { ip: `10.${i % 200}.${i}.9`, body: { email, password: `slow-${i}-guess` } });
      hashed += t.hasher.calls - before;
      t.advance(180); // 20 an hour, under the hourly 30
    }
    expect(hashed).toBe(100);
    expect((await t.call("POST", "/auth/login", { ip: "192.0.2.150", body: { email, password: GOOD_PW } })).status).toBe(429);
    expect((await t.call("POST", "/auth/login", { ip: "192.0.2.150", cookie, body: { email, password: GOOD_PW } })).status).toBe(200);
  });
});

describe("6. minor fixes", () => {
  it("C4: confirming retires reset links requested before confirmation", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const vt = await t.signup(email, "student-pass-123");
    await t.call("POST", "/auth/forgot", { ip: "10.70.0.1", body: { email } });
    const rt = t.mailer.last("reset", email)!.token!;
    expect((await t.call("POST", "/auth/verify", { body: { token: vt, password: "teacher-pw-12345", display_name: "Ms T" } })).status).toBe(200);
    const row = await env.DB.prepare("SELECT used_at FROM email_tokens WHERE kind = 'reset' AND user_id = (SELECT id FROM users WHERE email = ?)").bind(email).first<{ used_at: number | null }>();
    expect(row?.used_at).not.toBeNull();
    expect((await t.call("POST", "/auth/reset", { body: { token: rt, password: "squatter-wins-pw-1" } })).status).toBe(400);
  });

  it("deletion has its own per-account limiter and is never blocked by the login lockdown", async () => {
    const t = new Harness();
    // An owner on no known device can delete while a student keeps the login locked down.
    const locked = uniqueEmail("locked");
    await t.signupVerified(locked);
    const a = await t.loginNative(locked);
    for (let i = 0; i < 31; i++) await t.call("POST", "/auth/login", { ip: `2001:db8:${i}::1`, body: { email: locked, password: `spray-${i}-guess` } });
    expect((await t.call("POST", "/auth/login", { ip: "192.0.2.5", body: { email: locked, password: GOOD_PW } })).status).toBe(429);
    expect((await t.call("DELETE", "/account", { native: true, ip: "192.0.2.5", bearer: a.access, body: { password: GOOD_PW } })).status).toBe(204);
    // Wrong passwords at deletion are held by the delete limiter (5 an hour), not shared with login.
    const other = uniqueEmail("other");
    await t.signupVerified(other);
    const b = await t.loginNative(other);
    const st = [];
    for (let i = 0; i < 6; i++) st.push((await t.call("DELETE", "/account", { native: true, ip: `10.233.${i}.1`, bearer: b.access, body: { password: `wrong-${i}-password` } })).status);
    expect(st).toEqual([401, 401, 401, 401, 401, 429]);
    expect((await t.call("POST", "/auth/login", { ip: "10.233.0.1", body: { email: other, password: GOOD_PW } })).status).toBe(200);
  });
});
