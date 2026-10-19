// The approval workflow: queue, decisions, suspension (two-person rule for admins), sudo, audit.
// Threat model: students must never become an approved member; one admin must not be a bottleneck
// or a single point of silent failure, and a compromised admin must not act unnoticed.
import { describe, expect, it } from "vitest";
import { demoteAdminStatements, makeAdminStatements, type AdminAlert, type ApprovalConfig, type AuthConfig, type QueueStats } from "../src/index.js";
import { cookieValue, env, GOOD_PW, Harness, uniqueEmail } from "./helpers.js";

const reminders: (QueueStats & { kind: string; lastDay: unknown })[] = [];
const alerts: AdminAlert[] = [];
const suspendedHook: string[] = [];
const ZERO = "00000000-0000-0000-0000-000000000000";

function approval(extra: Partial<ApprovalConfig> = {}): ApprovalConfig {
  return {
    approvedLevel: "member",
    adminLevels: ["admin"],
    reapplyCooldownSeconds: 3 * 86400,
    domainLabels: async (ds) => Object.fromEntries(ds.filter((d) => d === "school.example.org").map((d) => [d, "Staff domain hint: Example USD"])),
    onQueueReminder: async (e) => void reminders.push(e),
    onAdminAlert: async (e) => void alerts.push(e),
    onSuspend: (uid) => {
      suspendedHook.push(uid);
      return [];
    },
    ...extra,
  };
}

const H = (extra: Partial<AuthConfig> = {}, a: Partial<ApprovalConfig> = {}) => new Harness({ approval: approval(a), ...extra });

const idOf = async (email: string) => (await env.DB.prepare("SELECT id FROM users WHERE email = ?").bind(email).first<{ id: string }>())!.id;
const openRequest = async (uid: string) =>
  (await env.DB.prepare("SELECT id FROM approval_requests WHERE user_id = ? AND decided_at IS NULL").bind(uid).first<{ id: string }>())?.id ?? ZERO;
const run = (stmts: string[]) => env.DB.batch(stmts.map((s) => env.DB.prepare(s)));

interface Admin {
  id: string;
  email: string;
  access: string;
  cookie: string;
  sudo: string;
}

/** Earlier tests in this file left admins behind; the admin-count tests need to be alone. */
const onlyTheseAdmins = () => env.DB.prepare("UPDATE users SET trust = 'member' WHERE trust = 'admin'").run();

/** The real bootstrap path: the same statements the operator script runs. */
async function bootstrapAdmin(t: Harness, email = uniqueEmail("admin")): Promise<Admin> {
  await t.signupVerified(email);
  await run(makeAdminStatements({ email, adminLevel: "admin", fromLevels: ["pending", "member"], now: t.clock }));
  return adminSession(t, email);
}

async function adminSession(t: Harness, email: string): Promise<Admin> {
  const login = await t.call("POST", "/auth/login", { body: { email, password: GOOD_PW } });
  const access = ((await login.json()) as { access_token: string }).access_token;
  const cookie = `__Host-device=${cookieValue(login, "__Host-device")}`;
  const re = await t.call("POST", "/auth/reauth", { bearer: access, cookie, body: { password: GOOD_PW } });
  const sudo = ((await re.json()) as { sudo_token: string }).sudo_token;
  return { id: await idOf(email), email, access, cookie, sudo };
}

/** An admin request: bearer + known-device cookie + sudo header for writes. */
function as(t: Harness, a: Admin, method: string, path: string, body?: unknown, o: { sudo?: boolean; cookie?: boolean } = {}) {
  return t.call(method, path, {
    bearer: a.access,
    cookie: o.cookie === false ? undefined : a.cookie,
    headers: o.sudo === false ? {} : { "X-Sudo-Token": a.sudo },
    body,
  });
}

const approveBody = (user_id: string, request_id: string) => ({ user_id, request_id, note: "checked", evidence: "directory", evidence_note: "listed on the staff page" });
const rejectBody = (user_id: string, request_id: string, reason = "We could not confirm you are staff.") => ({ user_id, request_id, note: "not on directory", reason });
const approveUser = async (t: Harness, a: Admin, uid: string) => as(t, a, "POST", "/admin/approve", approveBody(uid, await openRequest(uid)));

async function applicant(t: Harness, name = "Ms. Real") {
  const email = uniqueEmail("teacher");
  const token = await t.signup(email, "students-own-pass-1", { display_name: "Mr Haxx0r", school: "Fake High" });
  await t.call("POST", "/auth/verify", { body: { token, password: GOOD_PW, display_name: name } });
  return { email, id: await idOf(email) };
}

type QItem = {
  user_id: string;
  request_id: string;
  email: string;
  email_domain: string;
  age_seconds: number;
  status: string;
  server_labels: { domain_hint: string | null; domain_history: { approved: number; rejected: number }; prior_rejections: number; profile_changed: boolean; suspended: boolean };
  applicant: { display_name: string | null; profile: unknown };
};
const queueOf = async (t: Harness, a: Admin, q = "") => (await (await as(t, a, "GET", `/admin/queue${q}`)).json()) as { items: QItem[]; open: number; oldestAgeSeconds: number; next_cursor: string | null };

describe("authorisation", () => {
  const routes: [string, string, unknown?][] = [
    ["GET", "/admin/queue"],
    ["GET", "/admin/audit"],
    ["GET", "/admin/proposals"],
    ["GET", `/admin/users/${ZERO}`],
    ["POST", "/admin/approve", approveBody(ZERO, ZERO)],
    ["POST", "/admin/reject", rejectBody(ZERO, ZERO)],
    ["POST", "/admin/reject-bulk", { items: [{ user_id: ZERO, request_id: ZERO }], note: "n", reason: "r" }],
    ["POST", "/admin/revoke", { user_id: ZERO, note: "n", reason: "r" }],
    ["POST", "/admin/suspend", { user_id: ZERO, note: "n", reason: "r" }],
    ["POST", "/admin/confirm", { proposal_id: ZERO }],
    ["POST", "/admin/cancel", { proposal_id: ZERO }],
    ["POST", "/admin/restore", { user_id: ZERO, note: "n" }],
  ];

  it("every admin route: 401 without a session, 403 for a non-admin, 403 for an admin off a known device", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const plain = await applicant(t);
    const member = await t.loginWeb(plain.email);
    for (const [method, path, body] of routes) {
      expect((await t.call(method, path, { body })).status, `${path} anon`).toBe(401);
      expect((await t.call(method, path, { body, bearer: member.access, cookie: `__Host-device=${cookieValue(member.res, "__Host-device")}` })).status, `${path} non-admin`).toBe(403);
      const noDevice = await as(t, a, method, path, body, { cookie: false });
      expect(noDevice.status, `${path} no device`).toBe(403);
      expect(await noDevice.json()).toEqual({ error: "known_device_required" });
    }
  });

  it("writes need a sudo token: a stolen plain admin access token (even with the device cookie) cannot approve", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const p = await applicant(t);
    for (const [method, path, body] of routes.filter(([m]) => m === "POST")) {
      const r = await as(t, a, method, path, body, { sudo: false });
      expect(r.status, path).toBe(403);
      expect(await r.json()).toEqual({ error: "sudo_required" });
    }
    const body = approveBody(p.id, await openRequest(p.id));
    expect((await as(t, a, "POST", "/admin/approve", body, { sudo: false })).status).toBe(403);
    const other = await adminSession(t, a.email);
    expect((await as(t, { ...a, sudo: other.sudo }, "POST", "/admin/approve", body)).status).toBe(403);
    t.advance(5 * 60 + 1);
    expect((await as(t, a, "POST", "/admin/approve", body)).status).toBe(403);
    expect((await env.DB.prepare("SELECT trust FROM users WHERE id = ?").bind(p.id).first<{ trust: string }>())!.trust).toBe("pending");
  });

  it("reauth: needs a known device and the password; only wrong passwords count", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    expect((await t.call("POST", "/auth/reauth", { bearer: a.access, body: { password: GOOD_PW } })).status).toBe(403);
    for (let i = 0; i < 8; i++) expect((await t.call("POST", "/auth/reauth", { bearer: a.access, cookie: a.cookie, body: { password: GOOD_PW } })).status).toBe(200);
    const st = [];
    for (let i = 0; i < 5; i++) st.push((await t.call("POST", "/auth/reauth", { bearer: a.access, cookie: a.cookie, body: { password: `wrong-${i}-xxxxxx` } })).status);
    expect(st).toEqual([401, 401, 401, 401, 401]);
    expect((await t.call("POST", "/auth/reauth", { bearer: a.access, cookie: a.cookie, body: { password: GOOD_PW } })).status).toBe(429);
  });
});

describe("the queue", () => {
  it("shows what was submitted with the application, oldest first; server labels apart from applicant text", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    t.advance(60);
    const first = await applicant(t, "Ms. First");
    t.advance(60);
    const second = await applicant(t, "Ms. Second");
    const r = await as(t, a, "GET", "/admin/queue");
    const text = await r.clone().text();
    expect(text).not.toContain("Haxx0r"); // activation discarded the sign-up name
    expect(text).not.toContain("Fake High");
    const q = (await r.json()) as { items: QItem[]; oldestAgeSeconds: number };
    const mine = q.items.filter((x) => [first.id, second.id].includes(x.user_id));
    expect(mine.map((x) => x.applicant.display_name)).toEqual(["Ms. First", "Ms. Second"]);
    expect(mine[0]).toMatchObject({ status: "open", email: first.email, email_domain: "school.example.org", age_seconds: 60 });
    expect(mine[0]!.server_labels).toMatchObject({ domain_hint: "Staff domain hint: Example USD", prior_rejections: 0, suspended: false, profile_changed: false });
    expect(q.oldestAgeSeconds).toBeGreaterThanOrEqual(60);
  });

  it("shows the snapshot taken at application time, and flags a later change to the live profile", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const p = await applicant(t, "Ms. Submitted");
    await env.DB.prepare("UPDATE users SET display_name = 'Changed Later' WHERE id = ?").bind(p.id).run();
    const item = (await queueOf(t, a)).items.find((x) => x.user_id === p.id)!;
    expect(item.applicant.display_name).toBe("Ms. Submitted");
    expect(item.server_labels.profile_changed).toBe(true);
  });

  it("paginates with a cursor and filters by status, email domain and age", async () => {
    const t = H();
    const admin = await bootstrapAdmin(t);
    const ids: string[] = [];
    for (let i = 0; i < 5; i++) {
      t.advance(3600);
      ids.push((await applicant(t, `Ms. ${i}`)).id);
    }
    const a = await adminSession(t, admin.email); // five hours later: a fresh session
    const seen: string[] = [];
    let cursor: string | null = null;
    do {
      const q = await queueOf(t, a, `?limit=2${cursor ? `&cursor=${cursor}` : ""}`);
      seen.push(...q.items.map((x) => x.user_id));
      cursor = q.next_cursor;
    } while (cursor);
    expect(seen.filter((x) => ids.includes(x))).toEqual(ids);
    await approveUser(t, a, ids[0]!);
    expect((await queueOf(t, a, "?status=approved")).items.map((x) => x.user_id)).toContain(ids[0]);
    expect((await queueOf(t, a, "?status=open")).items.map((x) => x.user_id)).not.toContain(ids[0]);
    // Ages now: 4 h, 3 h, 2 h, 1 h, 0 h (ids[0] is approved, so not open).
    const old = (await queueOf(t, a, "?min_age_seconds=6000")).items.map((x) => x.user_id).filter((x) => ids.includes(x));
    expect(old).toEqual([ids[1], ids[2]]);
    const young = (await queueOf(t, a, "?max_age_seconds=1800")).items.map((x) => x.user_id).filter((x) => ids.includes(x));
    expect(young).toEqual([ids[4]]);
    expect((await queueOf(t, a, "?domain=other.example.org")).items).toEqual([]);
    expect((await queueOf(t, a, "?domain=school.example.org")).items.length).toBeGreaterThan(0);
    expect((await as(t, a, "GET", "/admin/queue?status=bogus")).status).toBe(400);
  });

  it("domain history comes from the precomputed counters and moves with each decision", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const before = (await env.DB.prepare("SELECT approved, rejected FROM domain_decisions WHERE domain = 'school.example.org'").first<{ approved: number; rejected: number }>()) ?? { approved: 0, rejected: 0 };
    const p1 = await applicant(t);
    const p2 = await applicant(t);
    await approveUser(t, a, p1.id);
    await as(t, a, "POST", "/admin/reject", rejectBody(p2.id, await openRequest(p2.id)));
    const after = await env.DB.prepare("SELECT approved, rejected FROM domain_decisions WHERE domain = 'school.example.org'").first<{ approved: number; rejected: number }>();
    expect(after).toEqual({ approved: before.approved + 1, rejected: before.rejected + 1 });
    const p3 = await applicant(t);
    const item = (await queueOf(t, a)).items.find((x) => x.user_id === p3.id)!;
    expect(item.server_labels.domain_history).toEqual(after);
  });
});

describe("decisions", () => {
  it("approve: needs the application id, note and evidence; upgrades trust live; audits; mails", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const p = await applicant(t);
    const rid = await openRequest(p.id);
    const before = await t.loginWeb(p.email);
    for (const bad of [
      { user_id: p.id, note: "n", evidence: "directory", evidence_note: "x" },
      { user_id: p.id, request_id: rid, evidence: "directory", evidence_note: "x" },
      { user_id: p.id, request_id: rid, note: "n", evidence: "a name match", evidence_note: "x" },
      { user_id: p.id, request_id: rid, note: "n", evidence: "directory" },
    ])
      expect((await as(t, a, "POST", "/admin/approve", bad)).status).toBe(400);
    expect((await as(t, a, "POST", "/admin/approve", approveBody(p.id, rid))).status).toBe(200);
    const me = (await (await t.call("GET", "/me", { bearer: before.access })).json()) as { trust: string; status: { state: string } };
    expect(me.trust).toBe("member");
    expect(me.status).toEqual({ state: "approved" });
    expect(t.mailer.last("application_approved", p.email)).toBeTruthy();
    const audit = await env.DB.prepare("SELECT * FROM auth_audit WHERE target_id = ? AND action = 'approve'").bind(p.id).all();
    expect(audit.results).toEqual([
      expect.objectContaining({ actor_id: a.id, actor_kind: "admin", prev_trust: "pending", new_trust: "member", evidence: "directory", evidence_note: "listed on the staff page", note: "checked" }),
    ]);
    expect((await as(t, a, "POST", "/admin/approve", approveBody(p.id, rid))).status).toBe(409);
    expect((await as(t, a, "POST", "/admin/reject", rejectBody(p.id, rid))).status).toBe(409);
  });

  it("concurrent approves and rejects of one application: exactly one wins, one audit row", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const p = await applicant(t);
    const rid = await openRequest(p.id);
    const res = await Promise.all(
      Array.from({ length: 12 }, (_, i) => (i % 2 ? as(t, a, "POST", "/admin/approve", approveBody(p.id, rid)) : as(t, a, "POST", "/admin/reject", rejectBody(p.id, rid)))),
    );
    expect(res.filter((x) => x.status === 200)).toHaveLength(1);
    expect(res.filter((x) => x.status === 409)).toHaveLength(11);
    const n = await env.DB.prepare("SELECT COUNT(*) AS n FROM auth_audit WHERE target_id = ? AND action IN ('approve', 'reject')").bind(p.id).first<{ n: number }>();
    expect(n?.n).toBe(1);
  });

  it("a stale queue page (the application was rejected and the applicant re-applied) cannot approve the new one", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const p = await applicant(t);
    const oldRid = await openRequest(p.id);
    await as(t, a, "POST", "/admin/reject", rejectBody(p.id, oldRid));
    t.advance(3 * 86400);
    const { access } = await t.loginNative(p.email);
    expect((await t.call("POST", "/account/reapply", { bearer: access, body: { display_name: "Ms. Again" } })).status).toBe(202);
    const a2 = await adminSession(t, a.email);
    // The admin's old page still says oldRid: approving with it must fail and change nothing.
    expect((await as(t, a2, "POST", "/admin/approve", approveBody(p.id, oldRid))).status).toBe(409);
    expect((await env.DB.prepare("SELECT trust FROM users WHERE id = ?").bind(p.id).first<{ trust: string }>())!.trust).toBe("pending");
    expect(await openRequest(p.id)).not.toBe(oldRid);
  });

  it("cannot approve: yourself, a deleted, an unactivated or a suspended account", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    expect((await as(t, a, "POST", "/admin/approve", approveBody(a.id, ZERO))).status).toBe(403);
    const un = uniqueEmail();
    await t.call("POST", "/auth/verify", { body: { token: await t.signup(un) } });
    expect((await approveUser(t, a, await idOf(un))).status).toBe(409);
    const s = await applicant(t);
    await env.DB.prepare("UPDATE users SET suspended_at = 1 WHERE id = ?").bind(s.id).run();
    expect((await approveUser(t, a, s.id)).status).toBe(409);
    const d = await applicant(t);
    const rid = await openRequest(d.id);
    const { access } = await t.loginNative(d.email);
    await t.call("DELETE", "/account", { native: true, bearer: access, body: { password: GOOD_PW } });
    expect((await as(t, a, "POST", "/admin/approve", approveBody(d.id, rid))).status).toBe(409);
  });

  it("bulk reject: at most N named applications, each gated and audited", async () => {
    const t = H({}, { bulkRejectMax: 3 });
    const a = await bootstrapAdmin(t);
    const ps = [await applicant(t), await applicant(t), await applicant(t)];
    const items = await Promise.all(ps.map(async (p) => ({ user_id: p.id, request_id: await openRequest(p.id) })));
    items[2] = { ...items[2]!, request_id: ZERO }; // a stale or wrong id is refused on its own
    const r = await as(t, a, "POST", "/admin/reject-bulk", { items, note: "spam wave", reason: "We could not confirm you are staff." });
    expect(((await r.json()) as { results: { status: string }[] }).results.map((x) => x.status)).toEqual(["rejected", "rejected", "not_pending"]);
    const n = await env.DB.prepare("SELECT COUNT(*) AS n FROM auth_audit WHERE action = 'reject' AND target_id IN (?, ?, ?)").bind(ps[0]!.id, ps[1]!.id, ps[2]!.id).first<{ n: number }>();
    expect(n?.n).toBe(2);
    const four = [...items, items[0]!];
    expect((await as(t, a, "POST", "/admin/reject-bulk", { items: four, note: "n", reason: "r" })).status).toBe(400);
  });

  it("reject: the applicant sees the reason (never the note); re-apply only after the cooldown, as a new application", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const p = await applicant(t);
    await as(t, a, "POST", "/admin/reject", { user_id: p.id, request_id: await openRequest(p.id), note: "SECRET-ADMIN-NOTE: student lookalike", reason: "We could not confirm you are staff." });
    expect(t.mailer.last("application_rejected", p.email)?.reason).toBe("We could not confirm you are staff.");
    const { access } = await t.loginNative(p.email);
    const meText = await (await t.call("GET", "/me", { bearer: access })).text();
    expect(meText).not.toContain("SECRET-ADMIN-NOTE");
    expect(JSON.parse(meText).status).toMatchObject({ state: "rejected", reason: "We could not confirm you are staff.", can_reapply: false });
    expect((await t.call("POST", "/account/reapply", { bearer: access, body: { display_name: "Ms. Real Again" } })).status).toBe(429);
    t.advance(3 * 86400);
    const { access: a2 } = await t.loginNative(p.email);
    expect((await t.call("POST", "/account/reapply", { bearer: a2, body: { display_name: "Ms. Real Again" } })).status).toBe(202);
    expect((await t.call("POST", "/account/reapply", { bearer: a2, body: { display_name: "Again" } })).status).toBe(409);
    const rows = await env.DB.prepare("SELECT id, decision FROM approval_requests WHERE user_id = ? ORDER BY requested_at").bind(p.id).all<{ id: string; decision: string | null }>();
    expect(rows.results.map((x) => x.decision)).toEqual(["rejected", null]);
    const a3 = await adminSession(t, a.email);
    const item = (await queueOf(t, a3)).items.find((x) => x.user_id === p.id)!;
    expect(item.server_labels.prior_rejections).toBe(1);
    expect(item.applicant.display_name).toBe("Ms. Real Again");
  });

  it("revoke: back to pending, sessions and device tokens die at once, the applicant sees the reason", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const p = await applicant(t);
    await approveUser(t, a, p.id);
    const s = await t.loginWeb(p.email);
    const dev = `__Host-device=${cookieValue(s.res, "__Host-device")}`;
    expect((await as(t, a, "POST", "/admin/revoke", { user_id: p.id, note: "n", reason: "Approved in error." })).status).toBe(200);
    expect((await t.call("GET", "/me", { bearer: s.access })).status).toBe(401);
    expect((await t.refreshWeb(s.refresh)).status).toBe(401);
    for (let i = 0; i < 31; i++) await t.call("POST", "/auth/login", { ip: `2001:db8:${i}::1`, body: { email: p.email, password: `g-${i}-xxxxxx` } });
    expect((await t.call("POST", "/auth/login", { cookie: dev, body: { email: p.email, password: GOOD_PW } })).status).toBe(429);
    expect((await as(t, a, "POST", "/admin/revoke", { user_id: p.id, note: "n", reason: "again" })).status).toBe(409);
  });

  it("concurrent revokes of one account: one wins, one audit row", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const p = await applicant(t);
    await approveUser(t, a, p.id);
    const res = await Promise.all(Array.from({ length: 8 }, () => as(t, a, "POST", "/admin/revoke", { user_id: p.id, note: "n", reason: "r" })));
    expect(res.filter((x) => x.status === 200)).toHaveLength(1);
    const n = await env.DB.prepare("SELECT COUNT(*) AS n FROM auth_audit WHERE target_id = ? AND action = 'revoke'").bind(p.id).first<{ n: number }>();
    expect(n?.n).toBe(1);
  });
});

describe("suspension", () => {
  it("blocks login (403 only with the right password), kills tokens immediately, runs the hook; restore undoes it", async () => {
    suspendedHook.length = 0;
    const t = H();
    const a = await bootstrapAdmin(t);
    const p = await applicant(t);
    await approveUser(t, a, p.id);
    const s = await t.loginNative(p.email);
    expect((await as(t, a, "POST", "/admin/suspend", { user_id: p.id, note: "n", reason: "Reported content under review." })).status).toBe(200);
    expect(suspendedHook).toContain(p.id);
    expect((await t.call("GET", "/me", { bearer: s.access })).status).toBe(401);
    expect((await t.refreshNative(s.refresh)).status).toBe(401);
    const right = await t.call("POST", "/auth/login", { body: { email: p.email, password: GOOD_PW } });
    expect(right.status).toBe(403);
    expect(await right.json()).toEqual({ error: "account_suspended" });
    expect((await t.call("POST", "/auth/login", { body: { email: p.email, password: "wrong-password-x" } })).status).toBe(401);
    expect(t.mailer.last("account_suspended", p.email)?.reason).toBe("Reported content under review.");
    expect((await as(t, a, "POST", "/admin/restore", { user_id: p.id, note: "cleared" })).status).toBe(200);
    expect((await t.call("POST", "/auth/login", { body: { email: p.email, password: GOOD_PW } })).status).toBe(200);
    expect((await as(t, a, "POST", "/admin/restore", { user_id: p.id, note: "again" })).status).toBe(409);
    // Audit: previous and new trust recorded on both, read inside the statement.
    const rows = await env.DB.prepare("SELECT action, prev_trust, new_trust FROM auth_audit WHERE target_id = ? AND action IN ('suspend', 'restore') ORDER BY rowid").bind(p.id).all();
    expect(rows.results).toEqual([
      { action: "suspend", prev_trust: "member", new_trust: "member" },
      { action: "restore", prev_trust: "member", new_trust: "member" },
    ]);
  });

  it("the live check alone blocks a suspended account, even if its sessions were not revoked", async () => {
    const t = H();
    const p = await applicant(t);
    const s = await t.loginNative(p.email);
    await env.DB.prepare("UPDATE users SET suspended_at = ? WHERE id = ?").bind(t.clock, p.id).run();
    expect((await t.call("GET", "/me", { bearer: s.access })).status).toBe(401);
    expect((await t.refreshNative(s.refresh)).status).toBe(401);
  });

  it("without the two-person rule: never yourself, and two admins suspending each other leave one active", async () => {
    const t = H({}, { twoPersonAdminSuspension: false });
    await onlyTheseAdmins();
    const a = await bootstrapAdmin(t);
    const b = await bootstrapAdmin(t);
    expect((await as(t, a, "POST", "/admin/suspend", { user_id: a.id, note: "n", reason: "r" })).status).toBe(403);
    const res = await Promise.all([
      as(t, a, "POST", "/admin/suspend", { user_id: b.id, note: "n", reason: "r" }),
      as(t, b, "POST", "/admin/suspend", { user_id: a.id, note: "n", reason: "r" }),
    ]);
    expect(res.filter((x) => x.status === 200)).toHaveLength(1);
    const active = await env.DB.prepare("SELECT COUNT(*) AS n FROM users WHERE id IN (?, ?) AND suspended_at IS NULL").bind(a.id, b.id).first<{ n: number }>();
    expect(active?.n).toBe(1);
  });
});

describe("two-person rule for actions on admins", () => {
  it("three admins: A proposes suspending B, B cannot confirm, A cannot confirm alone, C confirms; everyone is alerted", async () => {
    alerts.length = 0;
    const t = H();
    await onlyTheseAdmins();
    const a = await bootstrapAdmin(t);
    const b = await bootstrapAdmin(t);
    const c = await bootstrapAdmin(t);
    const prop = await as(t, a, "POST", "/admin/suspend", { user_id: b.id, note: "n", reason: "Unusual approvals." });
    expect(prop.status).toBe(202);
    const { proposal_id, self_confirm_after } = (await prop.json()) as { proposal_id: string; self_confirm_after: number | null };
    expect(self_confirm_after).toBeNull(); // C exists, so A may never confirm alone
    expect((await env.DB.prepare("SELECT suspended_at FROM users WHERE id = ?").bind(b.id).first<{ suspended_at: number | null }>())!.suspended_at).toBeNull();
    expect(alerts.map((x) => x.action)).toEqual(["admin_suspension_proposed"]);
    expect((await as(t, b, "POST", "/admin/confirm", { proposal_id })).status).toBe(403); // the target
    expect((await as(t, a, "POST", "/admin/confirm", { proposal_id })).status).toBe(403); // the proposer
    t.advance(30 * 3600);
    const a2 = await adminSession(t, a.email);
    expect((await as(t, a2, "POST", "/admin/confirm", { proposal_id })).status).toBe(403); // still not alone: C exists
    const c2 = await adminSession(t, c.email);
    expect((await as(t, c2, "POST", "/admin/confirm", { proposal_id })).status).toBe(200);
    expect((await env.DB.prepare("SELECT suspended_at FROM users WHERE id = ?").bind(b.id).first<{ suspended_at: number | null }>())!.suspended_at).not.toBeNull();
    expect(alerts.map((x) => x.action)).toEqual(["admin_suspension_proposed", "admin_suspended"]);
    expect((await as(t, c2, "POST", "/admin/confirm", { proposal_id })).status).toBe(409);
  });

  it("two admins: a sole possible confirmer is not stuck; the proposer may confirm alone after 24 h", async () => {
    const t = H();
    await onlyTheseAdmins();
    const a = await bootstrapAdmin(t);
    const b = await bootstrapAdmin(t);
    const prop = await as(t, a, "POST", "/admin/suspend", { user_id: b.id, note: "n", reason: "r" });
    const { proposal_id, self_confirm_after } = (await prop.json()) as { proposal_id: string; self_confirm_after: number };
    expect(self_confirm_after).toBe(t.clock + 24 * 3600 * 1000);
    expect((await as(t, a, "POST", "/admin/confirm", { proposal_id })).status).toBe(403); // not yet
    expect((await as(t, a, "POST", "/admin/suspend", { user_id: b.id, note: "n", reason: "r" })).status).toBe(409); // one open per target
    t.advance(24 * 3600);
    const a2 = await adminSession(t, a.email);
    expect((await as(t, a2, "POST", "/admin/confirm", { proposal_id })).status).toBe(200);
  });

  it("proposals expire after 72 h, and can be cancelled by any admin, including the target", async () => {
    const t = H();
    await onlyTheseAdmins();
    const a = await bootstrapAdmin(t);
    const b = await bootstrapAdmin(t);
    const c = await bootstrapAdmin(t);
    const p1 = ((await (await as(t, a, "POST", "/admin/suspend", { user_id: b.id, note: "n", reason: "r" })).json()) as { proposal_id: string }).proposal_id;
    expect((await as(t, b, "POST", "/admin/cancel", { proposal_id: p1 })).status).toBe(200);
    expect((await as(t, c, "POST", "/admin/confirm", { proposal_id: p1 })).status).toBe(409);
    const p2 = ((await (await as(t, a, "POST", "/admin/suspend", { user_id: b.id, note: "n", reason: "r" })).json()) as { proposal_id: string }).proposal_id;
    t.advance(72 * 3600);
    const c2 = await adminSession(t, c.email);
    expect((await as(t, c2, "POST", "/admin/confirm", { proposal_id: p2 })).status).toBe(409);
    // An expired proposal does not block a new one.
    const a2 = await adminSession(t, a.email);
    expect((await as(t, a2, "POST", "/admin/suspend", { user_id: b.id, note: "n", reason: "r" })).status).toBe(202);
  });
});

describe("the last admin and account deletion", () => {
  it("P1: the sole admin cannot delete their own account; an admin's deletion needs device and sudo, is audited and alerts", async () => {
    alerts.length = 0;
    const t = H();
    await onlyTheseAdmins();
    const a = await bootstrapAdmin(t);
    // Without device and sudo: refused before anything.
    expect((await t.call("DELETE", "/account", { bearer: a.access, body: { password: GOOD_PW } })).status).toBe(403);
    expect((await t.call("DELETE", "/account", { bearer: a.access, cookie: a.cookie, body: { password: GOOD_PW } })).status).toBe(403);
    // With both, but the last admin: 409, and still an admin.
    const r = await as(t, a, "DELETE", "/account", { password: GOOD_PW });
    expect(r.status).toBe(409);
    expect(await r.json()).toEqual({ error: "last_admin" });
    const n = await env.DB.prepare("SELECT COUNT(*) AS n FROM users WHERE trust = 'admin' AND deleted_at IS NULL AND suspended_at IS NULL").first<{ n: number }>();
    expect(n?.n).toBe(1);
    // With a second admin it works, is audited and alerts everyone.
    await bootstrapAdmin(t);
    const a2 = await adminSession(t, a.email);
    expect((await as(t, a2, "DELETE", "/account", { password: GOOD_PW })).status).toBe(204);
    expect(await env.DB.prepare("SELECT 1 FROM auth_audit WHERE target_id = ? AND action = 'account_deleted' AND actor_kind = 'user'").bind(a.id).first()).not.toBeNull();
    expect(alerts.map((x) => x.action)).toContain("admin_account_deleted");
  });

  it("A suspends B and then deletes themselves: refused, so B can still be restored", async () => {
    const t = H({}, { twoPersonAdminSuspension: false });
    await onlyTheseAdmins();
    const a = await bootstrapAdmin(t);
    const b = await bootstrapAdmin(t);
    expect((await as(t, a, "POST", "/admin/suspend", { user_id: b.id, note: "n", reason: "r" })).status).toBe(200);
    expect((await as(t, a, "DELETE", "/account", { password: GOOD_PW })).status).toBe(409);
  });

  it("recovery: when every admin is suspended, the bootstrap statements restore one (audited); demotion keeps the last admin", async () => {
    const t = H();
    await onlyTheseAdmins();
    const a = await bootstrapAdmin(t);
    await env.DB.prepare("UPDATE users SET suspended_at = 1, suspension_reason = 'x' WHERE id = ?").bind(a.id).run();
    await run(makeAdminStatements({ email: a.email, adminLevel: "admin", fromLevels: ["pending", "member"], now: t.clock }));
    const row = await env.DB.prepare("SELECT trust, suspended_at FROM users WHERE id = ?").bind(a.id).first();
    expect(row).toEqual({ trust: "admin", suspended_at: null });
    expect(await env.DB.prepare("SELECT 1 FROM auth_audit WHERE target_id = ? AND action = 'restore_admin' AND actor_kind = 'bootstrap'").bind(a.id).first()).not.toBeNull();
    // The last active admin cannot be demoted ...
    await run(demoteAdminStatements({ email: a.email, adminLevels: ["admin"], toLevel: "member", now: t.clock }));
    expect((await env.DB.prepare("SELECT trust FROM users WHERE id = ?").bind(a.id).first<{ trust: string }>())!.trust).toBe("admin");
    // ... but with another one it can, audited, with sessions revoked.
    const b = await bootstrapAdmin(t);
    await run(demoteAdminStatements({ email: b.email, adminLevels: ["admin"], toLevel: "member", now: t.clock }));
    expect((await env.DB.prepare("SELECT trust FROM users WHERE id = ?").bind(b.id).first<{ trust: string }>())!.trust).toBe("member");
    expect((await t.call("GET", "/me", { bearer: b.access })).status).toBe(401);
    expect(await env.DB.prepare("SELECT 1 FROM auth_audit WHERE target_id = ? AND action = 'demote_admin' AND actor_kind = 'bootstrap'").bind(b.id).first()).not.toBeNull();
    // An application closed by the bootstrap names the bootstrap as its decider.
    expect((await env.DB.prepare("SELECT decided_by FROM approval_requests WHERE user_id = ?").bind(b.id).first<{ decided_by: string }>())!.decided_by).toBe("bootstrap");
  });
});

describe("audit", () => {
  it("is append-only, readable, and survives deletion with notes AND reasons nulled (P2)", async () => {
    const t = H({}, { twoPersonAdminSuspension: false });
    const a = await bootstrapAdmin(t);
    const p = await applicant(t);
    await as(t, a, "POST", "/admin/approve", { ...approveBody(p.id, await openRequest(p.id)), note: "phoned the office of Jane Doe" });
    await as(t, a, "POST", "/admin/suspend", { user_id: p.id, note: "n", reason: "Jane Q Student, 10th grade at Fake High, phone 555-0100" });
    await as(t, a, "POST", "/admin/restore", { user_id: p.id, note: "n" });
    await expect(env.DB.prepare("DELETE FROM auth_audit WHERE target_id = ?").bind(p.id).run()).rejects.toThrow(/append-only/);
    await expect(env.DB.prepare("UPDATE auth_audit SET reason = NULL WHERE target_id = ?").bind(p.id).run()).rejects.toThrow(/append-only/);
    const log = (await (await as(t, a, "GET", "/admin/audit?limit=500")).json()) as { entries: { target_id: string; action: string; actor_kind: string }[] };
    expect(log.entries.some((x) => x.target_id === p.id && x.action === "approve")).toBe(true);
    expect(log.entries.some((x) => x.target_id === a.id && x.actor_kind === "bootstrap")).toBe(true);
    const { access } = await t.loginNative(p.email);
    expect((await t.call("DELETE", "/account", { native: true, bearer: access, body: { password: GOOD_PW } })).status).toBe(204);
    const rows = await env.DB.prepare("SELECT * FROM auth_audit WHERE target_id = ?").bind(p.id).all<Record<string, unknown>>();
    const text = JSON.stringify(rows.results);
    expect(text).not.toContain("Jane");
    expect(text).not.toContain("555-0100");
    expect(rows.results.find((x) => x.action === "approve")).toMatchObject({ actor_id: a.id, evidence: "directory", note: null, evidence_note: null, new_trust: "member" });
    expect(rows.results.some((x) => x.action === "account_deleted")).toBe(true);
    await expect(env.DB.prepare("UPDATE auth_audit SET action = 'x' WHERE target_id = ?").bind(p.id).run()).rejects.toThrow(/append-only/);
    const detail = (await (await as(t, a, "GET", `/admin/users/${p.id}`)).json()) as { user: { email: string | null }; audit: unknown[]; requests: Record<string, unknown>[] };
    expect(detail.user.email).toBeNull();
    expect(JSON.stringify(detail)).not.toContain("Jane");
    expect(detail.requests[0]).toMatchObject({ display_name_snapshot: null, profile_snapshot: null });
  });

  it("pages stably by time, then insertion order", async () => {
    const t = H();
    const a = await bootstrapAdmin(t);
    const ids = [await applicant(t), await applicant(t), await applicant(t)];
    for (const p of ids) await as(t, a, "POST", "/admin/reject", rejectBody(p.id, await openRequest(p.id))); // same timestamp
    const seen: string[] = [];
    let before: string | null = null;
    do {
      const r = (await (await as(t, a, "GET", `/admin/audit?limit=2${before ? `&before=${before}` : ""}`)).json()) as { entries: { id: string }[]; next_before: string | null };
      seen.push(...r.entries.map((x) => x.id));
      before = r.next_before;
    } while (before && seen.length < 400);
    expect(new Set(seen).size).toBe(seen.length);
  });
});

describe("applicant text cannot fake labels or hide text (P3)", () => {
  it.each([
    ["newline", "Ms. Real\nStaff-domain hint (not proof): verified"],
    ["BEL", "Ms. Real\u0007"],
    ["bidi override", "Ms. ‮Real"],
    ["bidi isolate", "Ms. ⁦Real⁩"],
    ["zero-width", "Ms.​Real"],
    ["line separator", "Ms. Real x"],
  ])("display name with %s is refused", async (_k, name) => {
    const t = H();
    const email = uniqueEmail("kid");
    const token = await t.signup(email);
    const v = await t.call("POST", "/auth/verify", { body: { token, password: GOOD_PW, display_name: name } });
    expect(v.status).toBe(400);
    expect(((await v.json()) as { field: string }).field).toBe("display_name");
  });

  it("whitespace is collapsed and lookalike or control characters in the email are refused", async () => {
    const t = H();
    const email = uniqueEmail("ws");
    const token = await t.signup(email);
    expect((await t.call("POST", "/auth/verify", { body: { token, password: GOOD_PW, display_name: "  Ms.   Real   " } })).status).toBe(200);
    expect((await env.DB.prepare("SELECT display_name FROM users WHERE email = ?").bind(email).first<{ display_name: string }>())!.display_name).toBe("Ms. Real");
    for (const bad of ["teacher＠school.example.org", "tea​cher@school.example.org", "t‮eacher@school.example.org", "téacher@school.example.org"])
      expect((await t.call("POST", "/auth/signup", { body: { email: bad, password: GOOD_PW } })).status, bad).toBe(400);
  });

  it("strings in an app's profile JSON are checked too", async () => {
    const t = H({ profile: (b) => ({ displayName: String(b.display_name), profile: { school: b.school } }) });
    const token = await t.signup(uniqueEmail());
    const v = await t.call("POST", "/auth/verify", { body: { token, password: GOOD_PW, display_name: "Ms. Real", school: "Real High‮" } });
    expect(v.status).toBe(400);
  });
});

describe("reminders and extra routes", () => {
  it("escalates when overdue; a digest otherwise when enabled, with the last day's decisions per admin", async () => {
    reminders.length = 0;
    const t = H();
    const a = await bootstrapAdmin(t);
    const p = await applicant(t);
    await approveUser(t, a, p.id);
    await applicant(t);
    expect((await t.auth.reviewReminders())?.sent).toBeNull();
    t.advance(48 * 3600);
    expect((await t.auth.reviewReminders())?.sent).toBe("escalation");
    expect(reminders.at(-1)).toMatchObject({ kind: "escalation" });
    const t2 = H({}, { digest: true });
    const a2 = await bootstrapAdmin(t2);
    const p2 = await applicant(t2);
    await approveUser(t2, a2, p2.id);
    expect((await t2.auth.reviewReminders())?.sent).toBe("digest");
    const last = reminders.at(-1)! as unknown as { lastDay: { approved: number; byAdmin: { admin_id: string; approved: number }[] } };
    expect(last.lastDay.byAdmin.find((x) => x.admin_id === a2.id)?.approved).toBeGreaterThanOrEqual(1);
  });

  it("app-supplied admin routes run behind the same guard and can audit", async () => {
    const t = new Harness({
      approval: approval({
        routes: [
          {
            method: "PUT",
            path: "/admin/things",
            write: true,
            handler: async (_req, c) => {
              await env.DB.batch([c.audit({ action: "thing.put", detail: { name: String(c.body?.name) } })]);
              return Response.json({ ok: true });
            },
          },
        ],
      }),
    });
    const a = await bootstrapAdmin(t);
    expect((await as(t, a, "PUT", "/admin/things", { name: "x" }, { sudo: false })).status).toBe(403);
    expect((await as(t, a, "PUT", "/admin/things", { name: "x" })).status).toBe(200);
    const row = await env.DB.prepare("SELECT detail FROM auth_audit WHERE action = 'thing.put' AND actor_id = ?").bind(a.id).first<{ detail: string }>();
    expect(JSON.parse(row!.detail)).toEqual({ name: "x" });
  });
});
