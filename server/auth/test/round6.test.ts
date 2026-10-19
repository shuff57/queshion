// Regression tests for the round-6 review. P-A..P-E are the reviewer's probes, turned into assertions.
// Threat model: students share the school NAT with the teacher, rotate IPv6, run many IPs; the
// teacher must never be kept out of recovery or login for more than minutes.
import { describe, expect, it } from "vitest";
import { D1RateLimitStore, MemoryRateLimitStore } from "../src/index.js";
import { cookieValue, env, GOOD_PW, Harness, snapshot, uniqueEmail } from "./helpers.js";

const NAT = "203.0.113.50";
const mem = () => new Harness({ rateLimitStore: new MemoryRateLimitStore() });

async function lockDown(t: Harness, email: string) {
  for (let i = 0; i < 31; i++) await t.call("POST", "/auth/login", { ip: `2001:db8:${i}::1`, body: { email, password: `guess-${i}-xxxxx` } });
}

describe("1. forgot and resend have no per-IP cap", () => {
  it("P-A: 400 junk forgots from the NAT (more than the old 300 cap), twice, never stop the teacher's own forgot or its mail", async () => {
    const t = mem();
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 400; i++) await t.call("POST", "/auth/forgot", { ip: NAT, body: { email: `x${i}@nowhere.example` } });
    const before = t.mailer.sent.length;
    expect((await t.call("POST", "/auth/forgot", { ip: NAT, body: { email } })).status).toBe(202);
    expect(t.mailer.sent.length - before).toBe(1);
    t.advance(9 * 60);
    for (let i = 0; i < 400; i++) expect((await t.call("POST", "/auth/forgot", { ip: NAT, body: { email: `y${i}@nowhere.example` } })).status).toBe(202);
    const before2 = t.mailer.sent.length;
    expect((await t.call("POST", "/auth/forgot", { ip: NAT, body: { email } })).status).toBe(202);
    expect(t.mailer.sent.length - before2).toBe(1);
  });

  it("an optional global mail budget (not keyed by IP) suppresses mail beyond it and answers the same", async () => {
    const t = new Harness({ mailBudget: { limit: 2, windowSeconds: 3600 }, rateLimitStore: new MemoryRateLimitStore() });
    const emails = [uniqueEmail("a"), uniqueEmail("b"), uniqueEmail("c")];
    for (const e of emails) await t.signupVerified(e);
    t.mailer.sent = [];
    const answers = [];
    for (const e of emails) answers.push(await snapshot(await t.call("POST", "/auth/forgot", { ip: `10.5.${answers.length}.1`, body: { email: e } })));
    expect(answers[0]).toEqual(answers[2]);
    expect(t.mailer.sent.map((m) => m.to)).toEqual([emails[0], emails[1]]);
    // The suppressed request left no row behind, so the third account's next request is not blocked by the gap.
    t.advance(3600 + 300); // the hour, plus the 5-minute sub-bucket holding the first two mails
    await t.call("POST", "/auth/forgot", { body: { email: emails[2] } });
    expect(t.mailer.last("reset", emails[2])).toBeTruthy();
  });
});

describe("2. refresh and logout charge the IP only for unknown tokens", () => {
  it("200 junk refresh tokens from the NAT do not stop a real refresh there", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginWeb(email);
    for (let i = 0; i < 200; i++) await t.call("POST", "/auth/refresh", { ip: NAT, cookie: `__Host-refresh=${String(i).padStart(43, "J")}` });
    expect((await t.call("POST", "/auth/refresh", { ip: NAT, cookie: `__Host-refresh=${"K".repeat(43)}` })).status).toBe(429);
    expect((await t.call("POST", "/auth/refresh", { ip: NAT, cookie: `__Host-refresh=${refresh}` })).status).toBe(200);
  });

  it("junk logouts from the NAT do not stop a real logout there", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginNative(email);
    for (let i = 0; i < 130; i++) await t.call("POST", "/auth/logout", { native: true, ip: NAT, body: { refresh_token: String(i).padStart(43, "L") } });
    expect((await t.call("POST", "/auth/logout", { native: true, ip: NAT, body: { refresh_token: refresh } })).status).toBe(204);
    expect((await t.refreshNative(refresh)).status).toBe(401);
  });
});

describe("3a. device enrolment: a native app gets in during a sustained lockdown", () => {
  it("P-C: after a reset, the mailed enrolment code plus the password gives the app a device token that logs in", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    await t.call("POST", "/auth/forgot", { ip: "198.51.100.7", body: { email } });
    const rr = await t.call("POST", "/auth/reset", { ip: "198.51.100.7", body: { token: t.mailer.last("reset", email)!.token, password: "teacher-new-pw-77" } });
    expect(rr.status).toBe(200);
    const enrolMail = t.mailer.last("device_enrol", email)!;
    expect(enrolMail.token).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(enrolMail.expiresAt).toBe(t.clock + 15 * 60 * 1000);
    await lockDown(t, email); // the student keeps the account locked
    // A native app without a device token is refused ...
    expect((await t.call("POST", "/auth/login", { ip: NAT, native: true, body: { email, password: "teacher-new-pw-77" } })).status).toBe(429);
    // ... until it enrols with the mailed code and the password.
    const en = await t.call("POST", "/auth/device/enrol", { ip: NAT, native: true, body: { code: enrolMail.token, password: "teacher-new-pw-77" } });
    expect(en.status).toBe(200);
    const deviceToken = ((await en.json()) as { device_token: string }).device_token;
    const ok = await t.call("POST", "/auth/login", { ip: NAT, native: true, headers: { "X-Device-Token": deviceToken }, body: { email, password: "teacher-new-pw-77" } });
    expect(ok.status).toBe(200);
    // The browser that did the reset is a known device too.
    expect((await t.call("POST", "/auth/login", { ip: NAT, cookie: `__Host-device=${cookieValue(rr, "__Host-device")}`, body: { email, password: "teacher-new-pw-77" } })).status).toBe(200);
  });

  it("verify with a password mails an enrolment code too", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const token = await t.signup(email);
    await t.call("POST", "/auth/verify", { body: { token, password: GOOD_PW, display_name: "Ms. T" } });
    const code = t.mailer.last("device_enrol", email)!.token;
    expect((await t.call("POST", "/auth/device/enrol", { native: true, body: { code, password: GOOD_PW } })).status).toBe(200);
  });

  it("codes are single use, expire after 15 minutes, die with a later reset, and burn after 5 wrong passwords", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const resetAndCode = async (pw: string) => {
      await t.call("POST", "/auth/forgot", { body: { email } });
      await t.call("POST", "/auth/reset", { body: { token: t.mailer.last("reset", email)!.token, password: pw } });
      return t.mailer.last("device_enrol", email)!.token!;
    };
    const enrol = (code: string, password: string) => t.call("POST", "/auth/device/enrol", { native: true, body: { code, password } });
    const c1 = await resetAndCode("password-round-one");
    expect((await enrol(c1, "password-round-one")).status).toBe(200);
    expect((await enrol(c1, "password-round-one")).status).toBe(400); // single use
    t.advance(121);
    const c2 = await resetAndCode("password-round-two");
    t.advance(15 * 60);
    expect((await enrol(c2, "password-round-two")).status).toBe(400); // expired
    t.advance(121);
    const c3 = await resetAndCode("password-round-three");
    t.advance(121);
    await resetAndCode("password-round-four");
    expect((await enrol(c3, "password-round-four")).status).toBe(400); // a later reset voided it
    t.advance(121);
    const c5 = await resetAndCode("password-round-five");
    for (let i = 0; i < 5; i++) expect((await enrol(c5, `wrong-password-${i}`)).status).toBe(401);
    expect((await enrol(c5, "password-round-five")).status).toBe(400); // burned
  });

  it("only an unknown code is charged to the IP", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 20; i++) await t.call("POST", "/auth/device/enrol", { ip: NAT, native: true, body: { code: "E".repeat(43), password: GOOD_PW } });
    expect((await t.call("POST", "/auth/device/enrol", { ip: NAT, native: true, body: { code: "F".repeat(43), password: GOOD_PW } })).status).toBe(429);
    await t.call("POST", "/auth/forgot", { body: { email } });
    await t.call("POST", "/auth/reset", { body: { token: t.mailer.last("reset", email)!.token, password: "fresh-password-xx" } });
    const code = t.mailer.last("device_enrol", email)!.token;
    expect((await t.call("POST", "/auth/device/enrol", { ip: NAT, native: true, body: { code, password: "fresh-password-xx" } })).status).toBe(200);
  });
});

describe("3b. random-address login floods cannot starve real accounts on the NAT", () => {
  it("P-B: 60 wrong-password logins for addresses with no account leave the teacher's new device able to log in", async () => {
    const t = mem();
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 60; i++) await t.call("POST", "/auth/login", { ip: NAT, body: { email: `z${i}@nowhere.example`, password: "wrong-pw-guess-1" } });
    expect((await t.call("POST", "/auth/login", { ip: NAT, body: { email, password: GOOD_PW } })).status).toBe(200);
  });

  it("without the unknown-address allowance the same flood would have starved it (the refund is what helps)", async () => {
    const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    t.build({
      rateLimitStore: new MemoryRateLimitStore(),
      rateLimits: {
        login: {
          ip: [{ limit: 60, windowSeconds: 300 }],
          ipFailed: [],
          failures: { perAccountIp: { limit: 5, windowSeconds: 900 }, perAccount: { limit: 30, windowSeconds: 3600 } },
        },
      },
    });
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 60; i++) await t.call("POST", "/auth/login", { ip: NAT, body: { email: `z${i}@nowhere.example`, password: "wrong-pw-guess-1" } });
    expect((await t.call("POST", "/auth/login", { ip: NAT, body: { email, password: GOOD_PW } })).status).toBe(429);
  });

  it("the refund changes nothing a client can see: the same sequence ends the same for a real and a made-up target", async () => {
    // A small per-IP login budget makes the budget effect visible within a few requests.
    const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() });
    t.build({
      rateLimitStore: new MemoryRateLimitStore(),
      rateLimits: {
        login: {
          ip: [{ limit: 4, windowSeconds: 300 }],
          ipFailed: [{ limit: 120, windowSeconds: 300 }],
          failures: { perAccountIp: { limit: 5, windowSeconds: 900 }, perAccount: { limit: 30, windowSeconds: 3600 } },
        },
      },
    });
    const mine = uniqueEmail("mine");
    const teacher = uniqueEmail("teacher");
    await t.signupVerified(mine);
    await t.signupVerified(teacher);
    const run = async (target: string, ip: string) => {
      const seq = [];
      for (let i = 0; i < 3; i++) seq.push((await t.call("POST", "/auth/login", { ip, body: { email: mine, password: GOOD_PW } })).status);
      const miss = await snapshot(await t.call("POST", "/auth/login", { ip, body: { email: target, password: "not-my-password-x" } }));
      for (let i = 0; i < 2; i++) seq.push((await t.call("POST", "/auth/login", { ip, body: { email: mine, password: GOOD_PW } })).status);
      return { seq, miss };
    };
    const known = await run(teacher, "198.51.100.61");
    const unknown = await run(uniqueEmail("ghost"), "198.51.100.62");
    expect(known).toEqual(unknown);
    expect(known.seq).toEqual([200, 200, 200, 200, 429]); // the miss was refunded for both
  });
});

describe("4. one valid token, many concurrent requests: one hash", () => {
  it("P-D: 50 concurrent verifies and 50 concurrent resets each cost a single hash", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const token = await t.signup(email);
    const c0 = t.hasher.calls;
    const rs = await Promise.all(Array.from({ length: 50 }, () => t.call("POST", "/auth/verify", { ip: NAT, body: { token, password: "teacher-pw-12345", display_name: "x" } })));
    expect(rs.filter((r) => r.status === 200)).toHaveLength(1);
    // The others never hashed: 409 while the winner held the lease, 400 once it had consumed the link.
    expect(rs.filter((r) => r.status === 400 || r.status === 409)).toHaveLength(49);
    expect(t.hasher.calls - c0).toBe(1);
    await t.call("POST", "/auth/forgot", { ip: NAT, body: { email } });
    const tk = t.mailer.last("reset", email)!.token;
    const c1 = t.hasher.calls;
    const rs2 = await Promise.all(Array.from({ length: 50 }, () => t.call("POST", "/auth/reset", { ip: NAT, body: { token: tk, password: "teacher-pw-99999" } })));
    expect(rs2.filter((r) => r.status === 200)).toHaveLength(1);
    expect(t.hasher.calls - c1).toBe(1);
  });

  it("a failed attempt releases its lease, and a stale lease expires after 30 s", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    await t.call("POST", "/auth/forgot", { body: { email } });
    const tk = t.mailer.last("reset", email)!.token!;
    // The hash blows up mid-request: the lease is given back and the link still works.
    const orig = t.hasher.argon2id.bind(t.hasher);
    let boom = true;
    t.hasher.argon2id = (pw, salt, p) => {
      if (boom) {
        boom = false;
        throw new Error("hasher down");
      }
      return orig(pw, salt, p);
    };
    expect((await t.call("POST", "/auth/reset", { body: { token: tk, password: "after-failure-pw-1" } })).status).toBe(500);
    expect((await t.call("POST", "/auth/reset", { body: { token: tk, password: "after-failure-pw-1" } })).status).toBe(200);
    // A lease left behind (a crashed isolate) only holds the link for 30 s.
    await t.call("POST", "/auth/forgot", { body: { email } });
    t.advance(121);
    await t.call("POST", "/auth/forgot", { body: { email } });
    const tk2 = t.mailer.last("reset", email)!.token!;
    await env.DB.prepare("UPDATE email_tokens SET claimed_until = ? WHERE used_at IS NULL").bind(t.clock + 30_000).run();
    // While the lease is held the holder of the link is told it is in progress (D6), not that it is dead.
    expect((await t.call("POST", "/auth/reset", { body: { token: tk2, password: "after-lease-pw-12" } })).status).toBe(409);
    t.advance(30);
    expect((await t.call("POST", "/auth/reset", { body: { token: tk2, password: "after-lease-pw-12" } })).status).toBe(200);
  });
});

describe("5. logout everywhere needs the refresh token", () => {
  it("P-E: a stolen access token alone cannot log out everywhere or wipe the known devices", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { res, access } = await t.loginWeb(email);
    const dv = cookieValue(res, "__Host-device");
    const r = await t.call("POST", "/auth/logout", { bearer: access, body: { scope: "global" } });
    expect(r.status).toBe(401);
    expect(await r.json()).toEqual({ error: "refresh_token_required" });
    await lockDown(t, email);
    expect((await t.call("POST", "/auth/login", { ip: NAT, cookie: `__Host-device=${dv}`, body: { email, password: GOOD_PW } })).status).toBe(200);
  });
});

describe("7. Retry-After agrees with the estimate", () => {
  it("a retry exactly Retry-After seconds later is allowed, and one second earlier is not", async () => {
    for (const store of [new D1RateLimitStore(env.DB), new MemoryRateLimitStore()]) {
      const key = `k-${crypto.randomUUID()}`;
      const rule = { limit: 6, windowSeconds: 600 }; // 50 s sub-buckets
      const t0 = 3_000_000_000_000;
      for (let i = 0; i < 6; i++) await store.hit(key, rule, t0 + i * 20_000); // spread across buckets
      const now = t0 + 200_000;
      const refused = await store.hit(key, rule, now);
      expect(refused.allowed).toBe(false);
      await store.undo!(key, rule, now);
      const early = now + (refused.retryAfter - 1) * 1000;
      const early2 = await store.hit(key, rule, early);
      expect(early2.allowed).toBe(false);
      await store.undo!(key, rule, early);
      expect((await store.hit(key, rule, now + refused.retryAfter * 1000)).allowed).toBe(true);
    }
  });
});
