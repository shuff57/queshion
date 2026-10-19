// Regression tests for the round-5 review. P1-P8 are the reviewer's probes, turned into assertions.
// Threat model: students share the school's IPv4 with teachers, rotate IPv6, run many IPs, and must
// never keep a real teacher out for more than minutes.
import { describe, expect, it } from "vitest";
import { MemoryRateLimitStore } from "../src/index.js";
import { cookieValue, env, GOOD_PW, Harness, uniqueEmail, worst24h } from "./helpers.js";

const dev = (r: Response) => `__Host-device=${cookieValue(r, "__Host-device")}`;
const NAT = "203.0.113.50";

describe("1. recovery from the shared school address", () => {
  it("P2: junk resets and 1000+ junk forgots from the school NAT never block the teacher's forgot or valid link there", async () => {
    const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 20; i++) await t.call("POST", "/auth/reset", { ip: NAT, body: { token: "A".repeat(43), password: "whatever-pw-123" } });
    for (let i = 0; i < 1200; i++) expect((await t.call("POST", "/auth/forgot", { ip: NAT, body: { email: `junk${i}@nowhere.example` } })).status).toBe(202);
    // The junk reset tokens did get charged to the NAT ...
    expect((await t.call("POST", "/auth/reset", { ip: NAT, body: { token: "B".repeat(43), password: "whatever-pw-123" } })).status).toBe(429);
    // ... but the teacher's forgot from the NAT goes through and mails a link,
    expect((await t.call("POST", "/auth/forgot", { ip: NAT, body: { email } })).status).toBe(202);
    expect(t.mailer.last("reset", email)).toBeTruthy();
    // and a valid link is never charged, so it works from the NAT.
    const r = await t.call("POST", "/auth/reset", { ip: NAT, body: { token: t.mailer.last("reset", email)!.token, password: "teacher-new-pw-78" } });
    expect(r.status).toBe(200);
  });

  it("verify: only invalid tokens are charged to the IP; a valid link works from a saturated NAT", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const token = await t.signup(email, "students-pw-1234");
    for (let i = 0; i < 30; i++) expect((await t.call("POST", "/auth/verify", { ip: NAT, body: { token: "C".repeat(43) } })).status).toBe(400);
    expect((await t.call("POST", "/auth/verify", { ip: NAT, body: { token: "D".repeat(43) } })).status).toBe(429);
    expect((await t.call("POST", "/auth/verify", { ip: NAT, body: { token, password: "teacher-pw-12345", display_name: "Ms T" } })).status).toBe(200);
  });

  it("the per-(IP, address) forgot limit binds only the address that is hammered", async () => {
    const t = new Harness();
    const victim = uniqueEmail("v");
    const teacher = uniqueEmail("t");
    await t.signupVerified(victim);
    await t.signupVerified(teacher);
    const st = [];
    for (let i = 0; i < 6; i++) st.push((await t.call("POST", "/auth/forgot", { ip: NAT, body: { email: victim } })).status);
    expect(st).toEqual([202, 202, 202, 202, 202, 429]);
    expect((await t.call("POST", "/auth/forgot", { ip: NAT, body: { email: teacher } })).status).toBe(202);
  });
});

describe("2. a completed reset restores login everywhere, in the same batch", () => {
  it("P1: students guess at the school NAT, the teacher resets at home, then a new school desktop logs in (web and native)", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 5; i++) await t.call("POST", "/auth/login", { ip: NAT, body: { email, password: `s-${i}-guessxx` } });
    expect((await t.call("POST", "/auth/login", { ip: NAT, body: { email, password: GOOD_PW } })).status).toBe(429);
    await t.call("POST", "/auth/forgot", { ip: "198.51.100.7", body: { email } });
    expect((await t.call("POST", "/auth/reset", { ip: "198.51.100.7", body: { token: t.mailer.last("reset", email)!.token, password: "teacher-new-pw-77" } })).status).toBe(200);
    expect((await t.call("POST", "/auth/login", { ip: NAT, body: { email, password: "teacher-new-pw-77" } })).status).toBe(200);
    expect((await t.call("POST", "/auth/login", { ip: NAT, native: true, body: { email, password: "teacher-new-pw-77" } })).status).toBe(200);
  });

  it("logout everywhere also re-keys the counters", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginNative(email);
    for (let i = 0; i < 5; i++) await t.call("POST", "/auth/login", { ip: NAT, body: { email, password: `s-${i}-guessxx` } });
    expect((await t.call("POST", "/auth/logout", { native: true, body: { refresh_token: refresh, scope: "global" } })).status).toBe(204);
    expect((await t.call("POST", "/auth/login", { ip: NAT, body: { email, password: GOOD_PW } })).status).toBe(200);
  });
});

describe("3. refused logins do not use up the shared address's budget", () => {
  it("P3: 60 requests at the NAT of which 5 hashed leave room for another teacher's new device", async () => {
    const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    const email = uniqueEmail();
    await t.signupVerified(email);
    const victim = uniqueEmail();
    const before = t.hasher.calls;
    for (let i = 0; i < 60; i++) await t.call("POST", "/auth/login", { ip: NAT, body: { email: victim, password: `x-${i}-guessxx` } });
    expect(t.hasher.calls - before).toBe(5);
    expect((await t.call("POST", "/auth/login", { ip: NAT, body: { email, password: GOOD_PW } })).status).toBe(200);
    // Even 300 refused requests do not touch it.
    for (let i = 0; i < 300; i++) await t.call("POST", "/auth/login", { ip: NAT, body: { email: victim, password: `y-${i}-guessxx` } });
    expect((await t.call("POST", "/auth/login", { ip: NAT, body: { email, password: GOOD_PW } })).status).toBe(200);
  });
});

describe("4. device tokens: daily cap, no lockdown power, logout flag", () => {
  it("P4: a lifted device cookie gets at most 20 hashed guesses a day, and cannot lock the owner out", async () => {
    const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    const email = uniqueEmail();
    await t.signupVerified(email);
    const c = dev(await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } }));
    const lo = await t.call("POST", "/auth/logout", { cookie: `${c}; __Host-refresh=${"x".repeat(43)}` });
    expect(lo.headers.getSetCookie().some((x) => x.startsWith("__Host-device="))).toBe(false); // kept by default
    const hits: number[] = [];
    for (let m = 0; m < 24 * 60; m += 2) {
      const b = t.hasher.calls;
      await t.call("POST", "/auth/login", { ip: `10.9.${(m / 2) % 250}.1`, cookie: c, body: { email, password: `d-${m}-guessxx` } });
      if (t.hasher.calls > b) hits.push(m);
      t.advance(120);
    }
    expect(worst24h(hits)).toBeLessThanOrEqual(20);
    // The device's failures never fed the lockdown: the owner, without a device, from anywhere.
    expect((await t.call("POST", "/auth/login", { ip: "192.0.2.1", body: { email, password: GOOD_PW } })).status).toBe(200);
  });

  it("several stolen device tokens together are capped per account (40 a day); the owner's current device is exempt", async () => {
    const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    const email = uniqueEmail();
    await t.signupVerified(email);
    const stolen = [];
    for (let i = 0; i < 4; i++) stolen.push(dev(await t.call("POST", "/auth/login", { ip: `10.77.${i}.1`, body: { email, password: GOOD_PW } })));
    // The owner logs in last: theirs is the account's current device.
    const owner = dev(await t.call("POST", "/auth/login", { ip: "198.51.100.40", body: { email, password: GOOD_PW } }));
    let hashed = 0;
    for (let round = 0; round < 30; round++) {
      for (const cookie of stolen) {
        const b = t.hasher.calls;
        await t.call("POST", "/auth/login", { ip: `10.78.${round}.1`, cookie, body: { email, password: `steal-${round}-guess` } });
        hashed += t.hasher.calls - b;
      }
      t.advance(20 * 60);
    }
    expect(hashed).toBeLessThanOrEqual(40);
    expect((await t.call("POST", "/auth/login", { ip: "198.51.100.40", cookie: owner, body: { email, password: GOOD_PW } })).status).toBe(200);
  });

  it("web logout clears the device cookie only with forget_device: true", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginWeb(email);
    const res = await t.call("POST", "/auth/logout", { cookie: `__Host-refresh=${refresh}`, body: { forget_device: true } });
    expect(res.headers.getSetCookie()).toContain("__Host-device=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict");
  });

  it("P5: another account's device token gives no exemption", async () => {
    const t = new Harness();
    const v = uniqueEmail("v");
    const s = uniqueEmail("s");
    await t.signupVerified(v);
    await t.signupVerified(s);
    const sc = dev(await t.call("POST", "/auth/login", { body: { email: s, password: GOOD_PW } }));
    const st = [];
    for (let i = 0; i < 7; i++) st.push((await t.call("POST", "/auth/login", { ip: NAT, cookie: sc, body: { email: v, password: `z-${i}-guessxx` } })).status);
    expect(st).toEqual([401, 401, 401, 401, 401, 429, 429]);
  });
});

describe("5. measured bounds (P6, P7)", () => {
  it("P7: with a reset every morning the worst 24 h span is two budgets (200)", async () => {
    const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    const email = uniqueEmail();
    await t.signupVerified(email);
    const hits: number[] = [];
    for (let m = 0; m < 48 * 60; m += 2) {
      if (m % 1440 === 480) {
        await t.call("POST", "/auth/forgot", { ip: "198.51.100.9", body: { email } });
        await t.call("POST", "/auth/reset", { ip: "198.51.100.9", body: { token: t.mailer.last("reset", email)!.token, password: `teacher-day-${m}-pw` } });
      }
      const b = t.hasher.calls;
      await t.call("POST", "/auth/login", { ip: `2001:db8:${(m / 2) % 16}::1`, body: { email, password: `g-${m}-guessxx` } });
      if (t.hasher.calls > b) hits.push(m);
      t.advance(120);
    }
    expect(worst24h(hits)).toBeLessThanOrEqual(200);
  });
});

describe("7. deletion purges the account's rate-limit counters at once", () => {
  it("P8: counter rows created by the account's activity are gone right after deletion", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const keysNow = async () => new Set((await env.DB.prepare("SELECT key FROM rate_limits").all<{ key: string }>()).results.map((r) => r.key));
    const before = await keysNow();
    for (let i = 0; i < 3; i++) await t.call("POST", "/auth/login", { ip: `10.88.${i}.1`, body: { email, password: `oops-${i}-guess` } });
    await t.call("POST", "/auth/forgot", { body: { email } });
    const { access } = await t.loginNative(email);
    const mine = [...(await keysNow())].filter((k) => !before.has(k) && k.startsWith("a:"));
    expect(mine.length).toBeGreaterThan(3);
    expect((await t.call("DELETE", "/account", { native: true, bearer: access, body: { password: GOOD_PW } })).status).toBe(204);
    const after = await keysNow();
    expect(mine.filter((k) => after.has(k))).toEqual([]);
  });
});
