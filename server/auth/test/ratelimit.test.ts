import { describe, expect, it } from "vitest";
import { D1RateLimitStore, ipBucket, MemoryRateLimitStore } from "../src/index.js";
import { env, GOOD_PW, Harness, uniqueEmail } from "./helpers.js";

const tight = (limit: number, windowSeconds = 60) => [{ limit, windowSeconds }];

describe("rate limiting", () => {
  it("login: the per-IP limit trips before any hash is computed", async () => {
    const t = new Harness();
    t.build({ rateLimits: { login: { ip: tight(3), account: tight(100) } } });
    for (let i = 0; i < 3; i++) {
      const r = await t.call("POST", "/auth/login", { body: { email: uniqueEmail(), password: GOOD_PW } });
      expect(r.status).toBe(401);
    }
    expect(t.hasher.calls).toBe(3);
    const r = await t.call("POST", "/auth/login", { body: { email: uniqueEmail(), password: GOOD_PW } });
    expect(r.status).toBe(429);
    expect(await r.json()).toEqual({ error: "rate_limited" });
    expect(Number(r.headers.get("Retry-After"))).toBeGreaterThan(0);
    expect(r.headers.get("Access-Control-Expose-Headers")).toContain("Retry-After");
    expect(t.hasher.calls).toBe(3); // the refused request never reached the hasher
    // A different IP is unaffected.
    expect((await t.call("POST", "/auth/login", { ip: "10.9.9.9", body: { email: uniqueEmail(), password: GOOD_PW } })).status).toBe(401);
  });

  it("login, shipped limits: 30 failures from 30 networks lock the account for every network, unknown addresses alike", async () => {
    for (const exists of [true, false]) {
      const t = new Harness({ rateLimitStore: new MemoryRateLimitStore() }); // DEFAULT_LIMITS, as shipped
      const email = uniqueEmail();
      if (exists) await t.signupVerified(email);
      t.hasher.calls = 0;
      const statuses = [];
      for (let i = 0; i < 31; i++) statuses.push((await t.call("POST", "/auth/login", { ip: `10.${i}.1.1`, body: { email, password: "wrong-password-here" } })).status);
      expect(statuses.slice(0, 30)).toEqual(new Array(30).fill(401));
      expect(statuses[30]).toBe(429);
      expect(t.hasher.calls).toBe(30);
      // A network never seen before is refused too, without a hash.
      const fresh = await t.call("POST", "/auth/login", { ip: "192.0.2.200", body: { email, password: "wrong-password-here" } });
      expect(fresh.status).toBe(429);
      expect(t.hasher.calls).toBe(30);
    }
  });

  it("sign-up, forgot and resend trip before hashing or mailing", async () => {
    const t = new Harness();
    t.build({ rateLimits: { signup: { ip: tight(1) }, forgot: { ip: tight(1) }, resend: { ip: tight(1) } } });
    const email = uniqueEmail();
    expect((await t.call("POST", "/auth/signup", { body: { email, password: GOOD_PW } })).status).toBe(202);
    expect((await t.call("POST", "/auth/signup", { body: { email: uniqueEmail(), password: GOOD_PW } })).status).toBe(429);
    expect(t.hasher.calls).toBe(1);
    expect((await t.call("POST", "/auth/forgot", { body: { email } })).status).toBe(202);
    expect((await t.call("POST", "/auth/forgot", { body: { email } })).status).toBe(429);
    t.advance(121); // past the per-account mail gap after the sign-up mail
    expect((await t.call("POST", "/auth/resend", { body: { email } })).status).toBe(202);
    expect((await t.call("POST", "/auth/resend", { body: { email } })).status).toBe(429);
    expect(t.mailer.sent.map((m) => m.kind)).toEqual(["verify", "reset", "verify"]);
  });

  it("reset, verify and delete are limited before their hash or token lookup", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { access } = await t.loginWeb(email);
    t.build({ rateLimits: { reset: { ip: tight(1) }, verify: { ip: tight(1) }, delete: { account: tight(1) } } });
    t.hasher.calls = 0;
    expect((await t.call("POST", "/auth/reset", { body: { token: "E".repeat(43), password: "fine-long-password" } })).status).toBe(400);
    expect((await t.call("POST", "/auth/reset", { body: { token: "E".repeat(43), password: "fine-long-password" } })).status).toBe(429);
    expect((await t.call("POST", "/auth/verify", { body: { token: "F".repeat(43) } })).status).toBe(400);
    expect((await t.call("POST", "/auth/verify", { body: { token: "F".repeat(43) } })).status).toBe(429);
    expect((await t.call("DELETE", "/account", { bearer: access, body: { password: "wrong-password-x" } })).status).toBe(401);
    expect((await t.call("DELETE", "/account", { bearer: access, body: { password: GOOD_PW } })).status).toBe(429);
    expect(t.hasher.calls).toBe(1);
  });

  it("backs off instead of hard-locking: refused attempts do not extend the lockout", async () => {
    const t = new Harness();
    t.build({ rateLimits: { login: { account: tight(2, 60) } } });
    const email = uniqueEmail();
    await t.signupVerified(email);
    t.clock = Math.floor(t.clock / 60000) * 60000; // start of a window
    await t.call("POST", "/auth/login", { body: { email, password: "bad-guess-number-1" } });
    await t.call("POST", "/auth/login", { body: { email, password: "bad-guess-number-2" } });
    for (let i = 0; i < 20; i++) expect((await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } })).status).toBe(429);
    // Two windows later the old hits have aged out entirely, despite the 20 refused tries.
    t.advance(120);
    expect((await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } })).status).toBe(200);
  });

  it("stores no raw IP or email in the counters", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.call("POST", "/auth/forgot", { ip: "203.0.113.77", body: { email } });
    const rows = await env.DB.prepare("SELECT key FROM rate_limits").all<{ key: string }>();
    for (const r of rows.results) {
      expect(r.key).not.toContain("203.0.113.77");
      expect(r.key).not.toContain(email);
    }
  });
});

describe("D1RateLimitStore", () => {
  it("counts hits over any span of one window, to within one sub-bucket", async () => {
    const s = new D1RateLimitStore(env.DB);
    const key = `k-${crypto.randomUUID()}`;
    const rule = { limit: 4, windowSeconds: 120 }; // 12 sub-buckets of 10 s
    const t0 = 1_000_000_000_000;
    for (let i = 0; i < 4; i++) expect((await s.hit(key, rule, t0 + i)).allowed).toBe(true);
    const refused = await s.hit(key, rule, t0 + 5);
    expect(refused.allowed).toBe(false);
    // The weighted estimate of the five (four plus the refused one) falls to 4 once 1/5 of their
    // bucket has left the window: t0 + 120 s + 2 s.
    expect(refused.retryAfter).toBe(122);
    await s.undo(key, rule, t0 + 5);
    // Still inside the window a minute later: refused. Once that bucket has left it: allowed.
    expect((await s.hit(key, rule, t0 + 60_000)).allowed).toBe(false);
    await s.undo(key, rule, t0 + 60_000);
    expect((await s.hit(key, rule, t0 + 130_000)).allowed).toBe(true);
  });

  it("hits bunched just before a boundary do not buy a second allowance (the two-bucket estimate's flaw)", async () => {
    const s = new D1RateLimitStore(env.DB);
    const key = `k-${crypto.randomUUID()}`;
    const rule = { limit: 10, windowSeconds: 1200 }; // 12 sub-buckets of 100 s
    const t0 = 1_999_999_200_000 - 1000; // one second before a 1200 s boundary (1_999_999_200_000 = 1_666_666 windows)
    for (let i = 0; i < 10; i++) await s.hit(key, rule, t0);
    // Just after the boundary the classic estimate would weigh the ten old hits at about 0 and
    // allow nearly ten more; here they are still inside the 20-minute window.
    let allowed = 0;
    for (let i = 0; i < 10; i++) if ((await s.hit(key, rule, t0 + 2000 + i)).allowed) allowed++;
    expect(allowed).toBe(0);
  });

  it("never lets concurrent hits overshoot", async () => {
    const s = new D1RateLimitStore(env.DB);
    const key = `k-${crypto.randomUUID()}`;
    const rule = { limit: 5, windowSeconds: 3600 };
    const now = 2_000_000_000_000;
    const res = await Promise.all(Array.from({ length: 20 }, () => s.hit(key, rule, now)));
    expect(res.filter((r) => r.allowed)).toHaveLength(5);
  });
});

describe("ipBucket", () => {
  it("groups IPv6 by /64 and leaves IPv4 alone", () => {
    expect(ipBucket("198.51.100.4")).toBe("198.51.100.4");
    expect(ipBucket("2001:db8:abcd:12:1:2:3:4")).toBe("2001:db8:abcd:12::/64");
    expect(ipBucket("2001:0db8:abcd:0012::99")).toBe("2001:db8:abcd:12::/64");
    expect(ipBucket("2001:db8::1")).toBe("2001:db8:0:0::/64");
    expect(ipBucket("::1")).toBe("0:0:0:0::/64");
  });
});
