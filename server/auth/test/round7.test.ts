// Regression tests for the round-7 review. E1 and M1 are the reviewer's probes, turned into assertions.
import { describe, expect, it } from "vitest";
import { MemoryRateLimitStore } from "../src/index.js";
import { cookieValue, env, GOOD_PW, Harness, snapshot, uniqueEmail } from "./helpers.js";

const NAT = "203.0.113.50";

async function lockDown(t: Harness, email: string) {
  for (let i = 0; i < 31; i++) await t.call("POST", "/auth/login", { ip: `2001:db8:${i}::1`, body: { email, password: `guess-${i}-xxxxx` } });
}

/** A D1 binding that runs `before` once, just before the first statement whose SQL contains `marker`. */
function interleaved(marker: string, before: () => Promise<void>): D1Database {
  let armed = true;
  const wrap = <T extends object>(obj: T, intercept: (k: PropertyKey, v: unknown) => unknown): T =>
    new Proxy(obj, {
      get(target, k) {
        const v = (target as Record<PropertyKey, unknown>)[k];
        const out = intercept(k, v);
        if (out !== undefined) return out;
        return typeof v === "function" ? (v as (...a: unknown[]) => unknown).bind(target) : v;
      },
    });
  return wrap(env.DB, (k) =>
    k !== "prepare"
      ? undefined
      : (sql: string) => {
          const st = env.DB.prepare(sql);
          if (!sql.includes(marker)) return st;
          return wrap(st, (k2) =>
            k2 !== "bind"
              ? undefined
              : (...args: unknown[]) =>
                  wrap(st.bind(...args), (k3, v3) =>
                    k3 !== "first"
                      ? undefined
                      : async (...a: unknown[]) => {
                          if (armed) {
                            armed = false;
                            await before();
                          }
                          return (v3 as (...x: unknown[]) => unknown).apply(st.bind(...args), a);
                        },
                  ),
          );
        },
  );
}

describe("D1. failed logins are refunded the same way whether or not the address exists", () => {
  it("E1: 59 own logins, one wrong password, then one more own login: same answer for a real and a made-up target", async () => {
    const t = new Harness(); // real D1 store, shipped limits
    const mine = uniqueEmail("atk");
    await t.signupVerified(mine);
    const teacher = uniqueEmail("teacher");
    await t.signupVerified(teacher);
    const ghost = uniqueEmail("ghost");
    const probe = async (target: string, ip: string) => {
      for (let i = 0; i < 59; i++) expect((await t.call("POST", "/auth/login", { ip, body: { email: mine, password: GOOD_PW } })).status).toBe(200);
      const miss = await snapshot(await t.call("POST", "/auth/login", { ip, body: { email: target, password: "wrong-guess-123" } }));
      const after = await t.call("POST", "/auth/login", { ip, body: { email: mine, password: GOOD_PW } });
      return { miss, after: after.status };
    };
    const known = await probe(teacher, "198.51.100.1");
    const unknown = await probe(ghost, "198.51.100.2");
    expect(known).toEqual(unknown);
    expect(known.after).toBe(200);
  });
});

describe("D2. a signed-in owner on a known device can enrol more devices during a lockdown", () => {
  it("a third device enrols while the lockdown holds, and the first two keep working", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    await t.call("POST", "/auth/forgot", { body: { email } });
    const rr = await t.call("POST", "/auth/reset", { body: { token: t.mailer.last("reset", email)!.token, password: "teacher-pw-after-reset" } });
    const web = `__Host-device=${cookieValue(rr, "__Host-device")}`; // device 1
    const en2 = await t.call("POST", "/auth/device/enrol", { native: true, body: { code: t.mailer.last("device_enrol", email)!.token, password: "teacher-pw-after-reset" } });
    const dev2 = ((await en2.json()) as { device_token: string }).device_token; // device 2
    await lockDown(t, email);
    t.advance(121); // past the mail gap after the code the reset itself sent
    // Device 1 is signed in and asks for a fresh code.
    const s1 = await t.call("POST", "/auth/login", { ip: NAT, cookie: web, body: { email, password: "teacher-pw-after-reset" } });
    const access = ((await s1.json()) as { access_token: string }).access_token;
    const before = t.mailer.sent.length;
    const req = await t.call("POST", "/auth/device/enrolment-code", { ip: NAT, cookie: web, bearer: access });
    expect(req.status).toBe(202);
    expect(t.mailer.sent.length - before).toBe(1);
    const code = t.mailer.last("device_enrol", email)!.token!;
    const en3 = await t.call("POST", "/auth/device/enrol", { native: true, ip: NAT, body: { code, password: "teacher-pw-after-reset" } });
    expect(en3.status).toBe(200);
    const dev3 = ((await en3.json()) as { device_token: string }).device_token;
    for (const login of [
      { cookie: web },
      { native: true, headers: { "X-Device-Token": dev2 } },
      { native: true, headers: { "X-Device-Token": dev3 } },
    ])
      expect((await t.call("POST", "/auth/login", { ip: NAT, ...login, body: { email, password: "teacher-pw-after-reset" } })).status).toBe(200);
  });

  it("needs a live session AND a known device; held to the mail gap and a daily cap", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const login = await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } });
    const cookie = `__Host-device=${cookieValue(login, "__Host-device")}`;
    const access = ((await login.json()) as { access_token: string }).access_token;
    const ask = (o: { cookie?: string; bearer?: string }) => t.call("POST", "/auth/device/enrolment-code", o);
    expect((await ask({ cookie })).status).toBe(401);
    expect((await ask({ bearer: access })).status).toBe(403);
    // signupVerified already issued one code (activation); the gap holds the next for 2 minutes.
    expect((await ask({ cookie, bearer: access })).status).toBe(429);
    const st = [];
    for (let i = 0; i < 5; i++) {
      t.advance(121);
      // Re-login now and then so the 15-minute access token stays live.
      const l = await t.call("POST", "/auth/login", { cookie, body: { email, password: GOOD_PW } });
      const a = ((await l.json()) as { access_token: string }).access_token;
      st.push((await ask({ cookie: `__Host-device=${cookieValue(l, "__Host-device")}`, bearer: a })).status);
    }
    // 1 activation code + 4 requested = the daily cap of 5; the fifth request is refused.
    expect(st).toEqual([202, 202, 202, 202, 429]);
  });
});

describe("D3. the enrolment claim re-checks every condition itself", () => {
  const setup = async (marker: string, change: (code: string) => Promise<void>) => {
    let code = "";
    const t = new Harness();
    t.build({ db: interleaved(marker, () => change(code)) });
    const email = uniqueEmail();
    await t.signupVerified(email);
    code = t.mailer.last("device_enrol", email)!.token!;
    return { t, code };
  };
  const claim = "UPDATE device_enrolments SET claimed_until = ?3";
  const byCode = async (code: string, sql: string) => {
    const h = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(code)));
    let bin = "";
    for (const b of h) bin += String.fromCharCode(b);
    await env.DB.prepare(sql).bind(btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "")).run();
  };

  it.each([
    ["the fifth wrong password lands", "UPDATE device_enrolments SET failures = 5 WHERE code_hash = ?1"],
    ["the code expires", "UPDATE device_enrolments SET expires_at = 0 WHERE code_hash = ?1"],
    ["a reset bumps the epoch", "UPDATE users SET device_epoch = device_epoch + 1 WHERE id = (SELECT user_id FROM device_enrolments WHERE code_hash = ?1)"],
  ])("between the read and the claim, %s: no hash, no device token", async (_label, sql) => {
    const { t, code } = await setup(claim, (c) => byCode(c, sql));
    t.hasher.calls = 0;
    const r = await t.call("POST", "/auth/device/enrol", { native: true, body: { code, password: GOOD_PW } });
    expect(r.status).toBe(400);
    expect(t.hasher.calls).toBe(0);
  });
});

describe("D4. an over-budget request changes nothing", () => {
  it("M1: links already mailed survive a request the global mail budget suppresses", async () => {
    // Each budget test gets its own store: the budget is one global counter by design.
    const t = new Harness({ mailBudget: { limit: 4, windowSeconds: 3600 }, rateLimitStore: new MemoryRateLimitStore() });
    const teacher = uniqueEmail("t");
    await t.signupVerified(teacher);
    t.mailer.sent = [];
    const links: string[] = [];
    for (let i = 0; i < 3; i++) {
      await t.call("POST", "/auth/forgot", { body: { email: teacher } });
      links.push(t.mailer.last("reset", teacher)!.token!);
      t.advance(121);
    }
    const other = uniqueEmail("o");
    await t.signupVerified(other);
    await t.call("POST", "/auth/forgot", { body: { email: other } }); // budget now full (4)
    expect((await t.call("POST", "/auth/forgot", { ip: "203.0.113.9", body: { email: teacher } })).status).toBe(202);
    const st = await env.DB.prepare(
      "SELECT COUNT(*) AS n FROM email_tokens e JOIN users u ON u.id = e.user_id WHERE u.email = ? AND kind = 'reset' AND used_at IS NULL",
    )
      .bind(teacher)
      .first<{ n: number }>();
    expect(st!.n).toBe(3);
    expect((await t.call("POST", "/auth/reset", { body: { token: links[0], password: "teacher-new-pw-1" } })).status).toBe(200);
  });

  it("a request for an address with no account gives its budget unit back", async () => {
    const t = new Harness({ mailBudget: { limit: 1, windowSeconds: 3600 }, rateLimitStore: new MemoryRateLimitStore() });
    const email = uniqueEmail();
    await t.signupVerified(email);
    for (let i = 0; i < 5; i++) await t.call("POST", "/auth/forgot", { ip: `10.4.${i}.1`, body: { email: uniqueEmail("ghost") } });
    const before = t.mailer.sent.length;
    await t.call("POST", "/auth/forgot", { body: { email } });
    expect(t.mailer.sent.length - before).toBe(1);
  });
});

describe("D6. a duplicate while the lease is held is told it is in progress", () => {
  it.each(["verify", "reset", "enrol"] as const)("%s: the double click gets 409 in_progress, the first click succeeds", async (kind) => {
    const t = new Harness();
    const email = uniqueEmail();
    let path: string;
    let body: Record<string, unknown>;
    let extra: { native?: boolean } = {};
    if (kind === "verify") {
      const token = await t.signup(email);
      path = "/auth/verify";
      body = { token, password: "teacher-pw-12345", display_name: "Ms. T" };
    } else {
      await t.signupVerified(email);
      if (kind === "reset") {
        await t.call("POST", "/auth/forgot", { body: { email } });
        path = "/auth/reset";
        body = { token: t.mailer.last("reset", email)!.token, password: "teacher-pw-99999" };
      } else {
        path = "/auth/device/enrol";
        body = { code: t.mailer.last("device_enrol", email)!.token, password: GOOD_PW };
        extra = { native: true };
      }
    }
    let second: Response | null = null;
    t.hasher.onHash = async () => {
      second = await t.call("POST", path, { body, ...extra });
    };
    const first = await t.call("POST", path, { body, ...extra });
    expect(first.status).toBe(200);
    expect(second!.status).toBe(409);
    expect(await second!.json()).toEqual({ error: "in_progress" });
  });
});
