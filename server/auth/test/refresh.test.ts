import { describe, expect, it } from "vitest";
import { cookieValue, env, Harness, sha256b64url, uniqueEmail } from "./helpers.js";

async function familyRows(refresh: string) {
  const h = await sha256b64url(refresh);
  const fam = await env.DB.prepare("SELECT family_id FROM sessions WHERE refresh_hash = ?").bind(h).first<{ family_id: string }>();
  const rows = await env.DB.prepare("SELECT * FROM sessions WHERE family_id = ? ORDER BY created_at, rotated_at IS NULL")
    .bind(fam!.family_id)
    .all<Record<string, number | string | null>>();
  return rows.results;
}

async function nativeUser(t: Harness) {
  const email = uniqueEmail();
  await t.signupVerified(email);
  return t.loginNative(email);
}

describe("refresh rotation", () => {
  it("rotates: new refresh token, new access token, old row marked rotated", async () => {
    const t = new Harness();
    const { refresh } = await nativeUser(t);
    t.advance(60);
    const res = await t.refreshNative(refresh);
    expect(res.status).toBe(200);
    const body = (await res.json()) as { access_token: string; refresh_token: string; refresh_expires_in: number };
    expect(body.refresh_token).not.toBe(refresh);
    expect(body.refresh_token).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(body.refresh_expires_in).toBe(30 * 86400);
    const rows = await familyRows(refresh);
    expect(rows).toHaveLength(2);
    expect(rows[0]!.rotated_at).toBe(t.clock);
    expect(rows[1]!.rotated_at).toBeNull();
    expect(rows[1]!.refresh_hash).toBe(await sha256b64url(body.refresh_token));
    // The successor is the next link: it rotates too.
    t.advance(60);
    const next = await t.refreshNative(body.refresh_token);
    expect(next.status).toBe(200);
  });

  it("web: reads the cookie and re-sets it", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginWeb(email);
    const res = await t.refreshWeb(refresh);
    expect(res.status).toBe(200);
    const next = cookieValue(res, "__Host-refresh");
    expect(next).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(next).not.toBe(refresh);
  });

  it("inside the 10 s default grace window a repeat returns the same derived successor, web and native", async () => {
    const t = new Harness();
    const { refresh } = await nativeUser(t);
    const a = (await (await t.refreshNative(refresh)).json()) as { refresh_token: string };
    t.advance(9);
    const b = await t.refreshNative(refresh);
    expect(b.status).toBe(200);
    expect(((await b.json()) as { refresh_token: string }).refresh_token).toBe(a.refresh_token);
    expect(await familyRows(refresh)).toHaveLength(2); // no extra row
    expect(t.events.map((e) => e.type)).toContain("refresh_grace_reuse");

    const email = uniqueEmail();
    await t.signupVerified(email);
    const web = await t.loginWeb(email);
    const w1 = cookieValue(await t.refreshWeb(web.refresh), "__Host-refresh");
    t.advance(10);
    const w2 = await t.refreshWeb(web.refresh);
    expect(w2.status).toBe(200);
    expect(cookieValue(w2, "__Host-refresh")).toBe(w1);
  });

  it("after the grace window a repeat revokes the whole family, including the live successor", async () => {
    const t = new Harness();
    const { refresh } = await nativeUser(t);
    const succ = ((await (await t.refreshNative(refresh)).json()) as { refresh_token: string }).refresh_token;
    t.advance(11);
    const replay = await t.refreshNative(refresh);
    expect(replay.status).toBe(401);
    expect(await replay.json()).toEqual({ error: "invalid_refresh_token" });
    for (const row of await familyRows(refresh)) expect(row.revoked_at).toBe(t.clock);
    expect((await t.refreshNative(succ)).status).toBe(401);
    expect(t.events.map((e) => e.type)).toContain("refresh_reuse_revoked");
  });

  it("a grace repeat after the successor itself rotated gets the live head, not a dead link", async () => {
    const t = new Harness();
    const { refresh: r0 } = await nativeUser(t);
    const r1 = ((await (await t.refreshNative(r0)).json()) as { refresh_token: string }).refresh_token;
    t.advance(4);
    const r2 = ((await (await t.refreshNative(r1)).json()) as { refresh_token: string }).refresh_token;
    t.advance(4);
    const late = await t.refreshNative(r0);
    expect(late.status).toBe(200);
    expect(((await late.json()) as { refresh_token: string }).refresh_token).toBe(r2);
    // And that head keeps working long after, with no family revocation.
    t.advance(15 * 60);
    expect((await t.refreshNative(r2)).status).toBe(200);
  });

  it("a replay of an older ancestor also revokes the family", async () => {
    const t = new Harness();
    const { refresh: r0 } = await nativeUser(t);
    const r1 = ((await (await t.refreshNative(r0)).json()) as { refresh_token: string }).refresh_token;
    t.advance(100);
    const r2 = ((await (await t.refreshNative(r1)).json()) as { refresh_token: string }).refresh_token;
    t.advance(100);
    expect((await t.refreshNative(r0)).status).toBe(401);
    expect((await t.refreshNative(r2)).status).toBe(401);
  });

  it("revoking one family leaves the user's other sessions alone", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const a = await t.loginNative(email);
    const b = await t.loginNative(email);
    await t.refreshNative(a.refresh);
    t.advance(60);
    expect((await t.refreshNative(a.refresh)).status).toBe(401);
    expect((await t.refreshNative(b.refresh)).status).toBe(200);
  });

  it("concurrent refreshes of one token all succeed and converge on one successor row", async () => {
    const t = new Harness();
    const { refresh } = await nativeUser(t);
    const results = await Promise.all(Array.from({ length: 12 }, () => t.refreshNative(refresh)));
    const tokens = await Promise.all(
      results.map(async (r) => {
        expect(r.status).toBe(200);
        return ((await r.json()) as { refresh_token: string }).refresh_token;
      }),
    );
    expect(new Set(tokens).size).toBe(1);
    const rows = await familyRows(refresh);
    expect(rows).toHaveLength(2);
    expect(rows.filter((r) => r.rotated_at === null)).toHaveLength(1);
    expect(t.events.filter((e) => e.type === "refresh_rotated")).toHaveLength(1);
    // The other eleven really raced (lost the atomic UPDATE) rather than running one after another.
    expect(t.events.filter((e) => e.type === "refresh_grace_reuse")).toHaveLength(11);
  });

  it("concurrent web refreshes converge too", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginWeb(email);
    const results = await Promise.all(Array.from({ length: 8 }, () => t.refreshWeb(refresh)));
    const cookies = results.map((r) => cookieValue(r, "__Host-refresh"));
    expect(new Set(cookies).size).toBe(1);
    expect(cookies[0]).toMatch(/^[A-Za-z0-9_-]{43}$/);
  });

  it("refuses unknown, malformed, expired and over-age tokens", async () => {
    const t = new Harness();
    expect((await t.refreshNative("A".repeat(43))).status).toBe(401);
    expect((await t.refreshNative("short")).status).toBe(401);
    expect((await t.call("POST", "/auth/refresh", { native: true, body: { refresh_token: 5 } })).status).toBe(401);
    expect((await t.call("POST", "/auth/refresh", {})).status).toBe(401);

    const { refresh } = await nativeUser(t);
    t.advance(30 * 86400);
    expect((await t.refreshNative(refresh)).status).toBe(401);

    // Sliding refreshes cannot outlive the family's maximum age.
    const t2 = new Harness();
    t2.build({ sessionMaxAgeSeconds: 3 * 86400, refreshTokenTtlSeconds: 2 * 86400 });
    let rt = (await nativeUser(t2)).refresh;
    t2.advance(1.5 * 86400);
    let r = await t2.refreshNative(rt);
    const body = (await r.json()) as { refresh_token: string; refresh_expires_in: number };
    expect(body.refresh_expires_in).toBe(1.5 * 86400); // capped at the family's end, not a fresh 2 days
    rt = body.refresh_token;
    t2.advance(1.5 * 86400);
    r = await t2.refreshNative(rt);
    expect(r.status).toBe(401);
  });

  it("web failures clear the cookie", async () => {
    const t = new Harness();
    const res = await t.refreshWeb("B".repeat(43));
    expect(res.status).toBe(401);
    expect(res.headers.get("Set-Cookie")).toContain("Max-Age=0");
  });

  it("carries the current trust level into the new access token", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const { refresh } = await t.loginNative(email);
    await env.DB.prepare("UPDATE users SET trust = 'member' WHERE email = ?").bind(email).run();
    const body = (await (await t.refreshNative(refresh)).json()) as { access_token: string; user: { trust: string } };
    expect(body.user.trust).toBe("member");
  });

  it("logout revokes the family; scope=global revokes every session of the user", async () => {
    const t = new Harness();
    const email = uniqueEmail();
    await t.signupVerified(email);
    const a = await t.loginNative(email);
    const b = await t.loginNative(email);
    const c = await t.loginNative(email);
    expect((await t.call("POST", "/auth/logout", { native: true, body: { refresh_token: a.refresh } })).status).toBe(204);
    expect((await t.refreshNative(a.refresh)).status).toBe(401);
    expect((await t.refreshNative(b.refresh)).status).toBe(200);
    expect((await t.call("POST", "/auth/logout", { native: true, body: { refresh_token: c.refresh, scope: "global" } })).status).toBe(204);
    expect((await t.refreshNative(c.refresh)).status).toBe(401);
    const live = await env.DB.prepare(
      "SELECT COUNT(*) AS n FROM sessions s JOIN users u ON u.id = s.user_id WHERE u.email = ? AND s.revoked_at IS NULL",
    )
      .bind(email)
      .first<{ n: number }>();
    expect(live?.n).toBe(0);
  });

  it("logout answers 204 the same way for an unknown token", async () => {
    const t = new Harness();
    const res = await t.call("POST", "/auth/logout", { native: true, body: { refresh_token: "C".repeat(43) } });
    expect(res.status).toBe(204);
  });
});
