import { describe, expect, it } from "vitest";
import { InputError } from "../src/index.js";
import { env, GOOD_PW, Harness, sha256b64url, snapshot, uniqueEmail } from "./helpers.js";

describe("sign-up", () => {
  it("creates an unverified account, hashes once, and mails a verification token", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const res = await t.call("POST", "/auth/signup", { body: { email: `  ${email.toUpperCase()} `, password: GOOD_PW, display_name: "Ms. A" } });
    expect(res.status).toBe(202);
    expect(await res.json()).toEqual({ ok: true });
    expect(t.hasher.calls).toBe(1);
    const mail = t.mailer.last("verify");
    expect(mail?.to).toBe(email); // normalized
    expect(mail?.token).toMatch(/^[A-Za-z0-9_-]{43}$/);
    const row = await env.DB.prepare("SELECT * FROM users WHERE email = ?").bind(email).first<Record<string, unknown>>();
    expect(row?.trust).toBe("unverified");
    expect(row?.email_verified_at).toBeNull();
    expect(row?.display_name).toBe("Ms. A");
    expect(String(row?.pw_hash)).toMatch(/^\$argon2id\$v=19\$m=19456,t=2,p=1\$/);
    expect(JSON.parse(String(row?.pw_params))).toEqual({ alg: "argon2id", v: 19, m: 19456, t: 2, p: 1 });
    // Only the hash of the token is stored.
    const tok = await env.DB.prepare("SELECT token_hash, kind, expires_at FROM email_tokens WHERE user_id = ?").bind(row?.id).first<Record<string, unknown>>();
    expect(tok?.kind).toBe("verify");
    expect(tok?.token_hash).toBe(await sha256b64url(mail!.token!));
    expect(tok?.expires_at).toBe(t.clock + 24 * 3600 * 1000);
  });

  it("answers an existing address identically, still hashes, and mails 'someone tried to sign up' instead", async () => {
    const t = new Harness();
    const known = uniqueEmail("known");
    await t.signupVerified(known);
    const unknown = uniqueEmail("fresh");
    t.hasher.calls = 0;
    t.mailer.sent = [];
    const a = await snapshot(await t.call("POST", "/auth/signup", { body: { email: known, password: "another-long-passphrase", display_name: "X" } }));
    const callsKnown = t.hasher.calls;
    const b = await snapshot(await t.call("POST", "/auth/signup", { body: { email: unknown, password: "another-long-passphrase", display_name: "X" } }));
    expect(a).toEqual(b);
    expect(callsKnown).toBe(1);
    expect(t.hasher.calls).toBe(2);
    expect(t.mailer.sent.map((m) => [m.kind, m.to, m.accountVerified])).toEqual([
      ["signup_existing", known, true],
      ["verify", unknown, undefined],
    ]);
    // The existing account was not touched.
    const n = await env.DB.prepare("SELECT COUNT(*) AS n FROM users WHERE email = ?").bind(known).first<{ n: number }>();
    expect(n?.n).toBe(1);
  });

  it("tells an unverified existing account apart only inside the mail", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signup(email);
    t.mailer.sent = [];
    const res = await t.call("POST", "/auth/signup", { body: { email, password: GOOD_PW, display_name: "X" } });
    expect(res.status).toBe(202);
    expect(t.mailer.sent).toEqual([{ kind: "signup_existing", to: email, accountVerified: false }]);
  });

  it.each([
    ["short", "too_short"],
    ["1234567890", "common"],
    ["Password2026!", "common"],
    ["p@ssw0rd2024", "common"],
    ["qwertyuiop", "common"],
    ["aaaaaaaaaaaa", "common"],
    ["abababababab", "common"],
  ])("rejects weak password %s (%s) without hashing", async (pw, reason) => {
    const t = new Harness();
    const res = await t.call("POST", "/auth/signup", { body: { email: uniqueEmail(), password: pw, display_name: "X" } });
    expect(res.status).toBe(400);
    expect(await res.json()).toEqual({ error: "weak_password", reason });
    expect(t.hasher.calls).toBe(0);
    expect(t.mailer.sent).toHaveLength(0);
  });

  it("rejects a password that contains the email's local part", async () => {
    const t = new Harness();
    const email = `jonathan.q.${Date.now()}@school.example.org`;
    const res = await t.call("POST", "/auth/signup", { body: { email, password: `${email.split("@")[0]}!xyz`, display_name: "X" } });
    expect(await res.json()).toEqual({ error: "weak_password", reason: "contains_email" });
  });

  it("runs the pluggable password check after the rate limit and before the hash", async () => {
    const seen: string[] = [];
    const t = new Harness();
    t.build({ passwordCheck: async (pw) => (seen.push(pw), pw.includes("breached") ? "breached" : null) });
    const res = await t.call("POST", "/auth/signup", { body: { email: uniqueEmail(), password: "this-was-breached-before", display_name: "X" } });
    expect(await res.json()).toEqual({ error: "weak_password", reason: "breached" });
    expect(seen).toHaveLength(1);
    expect(t.hasher.calls).toBe(0);
  });

  it("validates app-defined profile fields with the profile hook", async () => {
    const t = new Harness();
    t.build({
      profile: (b) => {
        if (b.adult !== true) throw new InputError("must_be_adult", "adult");
        if (typeof b.school !== "string") throw new InputError("required", "school");
        return { displayName: String(b.display_name), profile: { school: b.school } };
      },
    });
    const bad = await t.call("POST", "/auth/signup", { body: { email: uniqueEmail(), password: GOOD_PW, display_name: "X", school: "PVHS" } });
    expect(bad.status).toBe(400);
    expect(await bad.json()).toEqual({ error: "invalid_request", field: "adult", reason: "must_be_adult" });
    expect(t.hasher.calls).toBe(0);
    const email = uniqueEmail();
    const ok = await t.call("POST", "/auth/signup", { body: { email, password: GOOD_PW, display_name: "Y", school: "PVHS", adult: true } });
    expect(ok.status).toBe(202);
    const row = await env.DB.prepare("SELECT profile FROM users WHERE email = ?").bind(email).first<{ profile: string }>();
    expect(JSON.parse(row!.profile)).toEqual({ school: "PVHS" });
  });

  it("rejects malformed input before doing any work", async () => {
    const t = new Harness();
    const cases: [Record<string, unknown> | string, number, string][] = [
      [{ email: "not-an-email", password: GOOD_PW }, 400, "invalid_request"],
      [{ email: uniqueEmail() }, 400, "invalid_request"],
      [{ email: 42, password: GOOD_PW }, 400, "invalid_request"],
      ["{not json", 400, "invalid_json"],
      ["[1,2]", 400, "invalid_json"],
    ];
    for (const [body, status, code] of cases) {
      const res = await t.call("POST", "/auth/signup", typeof body === "string" ? { rawBody: body } : { body });
      expect(res.status).toBe(status);
      expect(((await res.json()) as { error: string }).error).toBe(code);
    }
    const form = await t.call("POST", "/auth/signup", { rawBody: "email=a@b.co&password=x", contentType: "application/x-www-form-urlencoded" });
    expect(form.status).toBe(415);
    const big = await t.call("POST", "/auth/signup", { body: { email: uniqueEmail(), password: GOOD_PW, pad: "x".repeat(9000) } });
    expect(big.status).toBe(413);
    expect(t.hasher.calls).toBe(0);
  });

  it("refuses a GET on a POST route with 405 and an Allow header", async () => {
    const t = new Harness();
    const res = await t.call("GET", "/auth/signup");
    expect(res.status).toBe(405);
    expect(res.headers.get("Allow")).toBe("POST");
  });
});

describe("email verification", () => {
  it("with a password and profile, confirms the address, activates the account and opens an approval request", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const token = await t.signup(email);
    const res = await t.call("POST", "/auth/verify", { body: { token, password: GOOD_PW, display_name: "Ms. Real" } });
    expect(res.status).toBe(200);
    const u = await env.DB.prepare("SELECT id, trust, email_verified_at FROM users WHERE email = ?").bind(email).first<Record<string, unknown>>();
    expect(u?.trust).toBe("pending");
    expect(u?.email_verified_at).toBe(t.clock);
    const ap = await env.DB.prepare("SELECT * FROM approval_requests WHERE user_id = ?").bind(u?.id).first<Record<string, unknown>>();
    expect(ap?.requested_at).toBe(t.clock);
    expect(ap?.decided_at).toBeNull();
    expect(t.events.some((e) => e.type === "verified" && e.userId === u?.id)).toBe(true);
    expect(t.events.some((e) => e.type === "activated" && e.userId === u?.id)).toBe(true);
  });

  it("without a password, confirms the address but leaves the account inactive and outside the approval queue", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const token = await t.signup(email);
    expect((await t.call("POST", "/auth/verify", { body: { token } })).status).toBe(200);
    const u = await env.DB.prepare("SELECT id, trust, email_verified_at, pw_hash FROM users WHERE email = ?").bind(email).first<Record<string, unknown>>();
    expect(u?.email_verified_at).toBe(t.clock);
    expect(u?.trust).toBe("unverified");
    expect(u?.pw_hash).toBeNull();
    expect(await env.DB.prepare("SELECT 1 FROM approval_requests WHERE user_id = ?").bind(u?.id).first()).toBeNull();
  });

  it("is single use", async () => {
    const t = new Harness();
    const token = await t.signup(uniqueEmail());
    expect((await t.call("POST", "/auth/verify", { body: { token } })).status).toBe(200);
    const again = await t.call("POST", "/auth/verify", { body: { token } });
    expect(again.status).toBe(400);
    expect(await again.json()).toEqual({ error: "invalid_token" });
  });

  it("expires after 24 hours", async () => {
    const t = new Harness();
    const token = await t.signup(uniqueEmail());
    t.advance(24 * 3600 + 1);
    expect((await t.call("POST", "/auth/verify", { body: { token } })).status).toBe(400);
  });

  it("never demotes an account that is already past the initial level", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    const token = await t.signup(email);
    await env.DB.prepare("UPDATE users SET trust = 'member' WHERE email = ?").bind(email).run();
    expect((await t.call("POST", "/auth/verify", { body: { token } })).status).toBe(200);
    const u = await env.DB.prepare("SELECT trust FROM users WHERE email = ?").bind(email).first<{ trust: string }>();
    expect(u?.trust).toBe("member");
  });

  it("rejects garbage tokens without touching the database", async () => {
    const t = new Harness();
    for (const token of ["", "short", "x".repeat(43) + "!", 12, null]) {
      const res = await t.call("POST", "/auth/verify", { body: { token } });
      expect(res.status).toBe(400);
    }
  });

  it("does not consume the token on GET (mail scanners prefetch links)", async () => {
    const t = new Harness();
    const token = await t.signup(uniqueEmail());
    expect((await t.call("GET", `/auth/verify?token=${token}`)).status).toBe(405);
    expect((await t.call("POST", "/auth/verify", { body: { token } })).status).toBe(200);
  });

  it("runs onVerified in the background", async () => {
    const seen: string[] = [];
    const t = new Harness();
    t.build({ onVerified: async (id) => void seen.push(id) });
    const token = await t.signup(uniqueEmail());
    await t.call("POST", "/auth/verify", { body: { token, password: GOOD_PW } });
    expect(seen).toHaveLength(1);
  });
});

describe("resend verification", () => {
  it("is identical for unknown, verified and unverified addresses; only the unverified one gets mail", async () => {
    const t = new Harness();
    const verified = uniqueEmail("v");
    await t.signupVerified(verified);
    const unverified = uniqueEmail("u");
    const first = await t.signup(unverified);
    const unknown = uniqueEmail("n");
    t.advance(121); // past the gap after the sign-up mail
    t.mailer.sent = [];
    t.hasher.calls = 0;
    const snaps = [];
    for (const email of [unknown, verified, unverified]) snaps.push(await snapshot(await t.call("POST", "/auth/resend", { body: { email } })));
    expect(snaps[0]).toEqual(snaps[1]);
    expect(snaps[1]).toEqual(snaps[2]);
    expect(snaps[0]!.status).toBe(202);
    expect(t.hasher.calls).toBe(0);
    expect(t.mailer.sent.map((m) => [m.kind, m.to])).toEqual([["verify", unverified]]);
    // Earlier links stay valid alongside the new one (the newest three do).
    expect((await t.call("POST", "/auth/verify", { body: { token: first } })).status).toBe(200);
    expect((await t.call("POST", "/auth/verify", { body: { token: t.mailer.sent[0]!.token } })).status).toBe(400); // spent by confirmation
  });
});
