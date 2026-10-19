// Run: npm test (node, no dependencies; node:sqlite is SQLite, which is what D1 runs).
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import { execFileSync } from "node:child_process";
import { demoteAdminStatements, makeAdminStatements } from "../../auth/src/bootstrap.ts";
import { registrableDomain, staffDomainLabels } from "../src/admin.ts";
import { adminAlert, makeMailer, queueReminder } from "../src/mail.ts";
import { flushNoticeBacklog, noticeNewApplication, NOTICES_PER_HOUR } from "../src/notices.ts";

const here = new URL("..", import.meta.url).pathname;

function freshDb(): DatabaseSync {
  const db = new DatabaseSync(":memory:");
  for (const f of readdirSync(`${here}migrations`).filter((x) => x.endsWith(".sql")).sort()) db.exec(readFileSync(`${here}migrations/${f}`, "utf8"));
  return db;
}

/** Exactly what scripts/make-admin.sh feeds to wrangler. */
function scriptSql(cmd: "make" | "demote", email: string): string {
  return execFileSync("node", ["--experimental-strip-types", "--no-warnings", `${here}scripts/admin-sql.ts`, cmd, email], { encoding: "utf8" });
}

/** A minimal D1 over node:sqlite (?NNN parameters, RETURNING, batch). */
function d1(db: DatabaseSync): D1Database {
  const stmt = (sql: string, args: unknown[] = []): unknown => ({
    bind: (...a: unknown[]) => stmt(sql, a),
    first: async () => {
      const r = db.prepare(sql).get(...(args as never[]));
      return r ? { ...r } : null;
    },
    all: async () => ({ results: db.prepare(sql).all(...(args as never[])).map((x) => ({ ...x })) }),
    run: async () => ({ meta: { changes: Number(db.prepare(sql).run(...(args as never[])).changes) } }),
  });
  return {
    prepare: (sql: string) => stmt(sql),
    batch: async (ss: { run: () => Promise<unknown> }[]) => {
      const out = [];
      for (const x of ss) out.push(await x.run());
      return out;
    },
  } as unknown as D1Database;
}

function capture(): { logs: string[]; restore: () => void } {
  const logs: string[] = [];
  const orig = console.log;
  console.log = (m: string) => void logs.push(m);
  return { logs, restore: () => (console.log = orig) };
}

test("the worker's copies of the package migrations match the package (scripts/sync-migrations.sh)", () => {
  for (const f of readdirSync(`${here}../auth/migrations`))
    assert.equal(readFileSync(`${here}migrations/${f}`, "utf8"), readFileSync(`${here}../auth/migrations/${f}`, "utf8"), f);
});

test("make-admin promotes only a verified, active account, audited as bootstrap, once; and recovers a suspended admin", () => {
  const db = freshDb();
  const add = (id: string, email: string, trust: string, verified: boolean, pw: boolean, suspended = false) =>
    db
      .prepare("INSERT INTO users (id, email, trust, created_at, email_verified_at, pw_hash, suspended_at) VALUES (?, ?, ?, 1, ?, ?, ?)")
      .run(id, email, trust, verified ? 1 : null, pw ? "$argon2id$x" : null, suspended ? 5 : null);
  add("u1", "ok@school.example.org", "pending", true, true);
  add("u2", "squat@school.example.org", "unverified", false, true);
  add("u3", "nopw@school.example.org", "unverified", true, false);
  add("u4", "susp@school.example.org", "teacher", true, true, true);
  add("u5", "admin@school.example.org", "admin", true, true, true);
  db.prepare("INSERT INTO approval_requests (id, user_id, requested_at) VALUES ('r1', 'u1', 1)").run();
  // The script's own output, twice (idempotent), then the others.
  db.exec(scriptSql("make", "ok@school.example.org"));
  db.exec(scriptSql("make", "ok@school.example.org"));
  for (const e of ["squat@school.example.org", "nopw@school.example.org", "susp@school.example.org", "admin@school.example.org"]) db.exec(scriptSql("make", e));
  const row = (id: string) => ({ ...(db.prepare("SELECT trust, suspended_at FROM users WHERE id = ?").get(id) as object) });
  assert.deepEqual(row("u1"), { trust: "admin", suspended_at: null });
  assert.deepEqual(row("u2"), { trust: "unverified", suspended_at: null });
  assert.deepEqual(row("u3"), { trust: "unverified", suspended_at: null });
  assert.deepEqual(row("u4"), { trust: "teacher", suspended_at: 5 }); // a suspended NON-admin is never promoted
  assert.deepEqual(row("u5"), { trust: "admin", suspended_at: null }); // the all-admins-suspended recovery
  const audit = db.prepare("SELECT actor_id, actor_kind, action, target_id, prev_trust, new_trust FROM auth_audit ORDER BY rowid").all().map((x) => ({ ...x }));
  assert.deepEqual(audit, [
    { actor_id: "bootstrap", actor_kind: "bootstrap", action: "make_admin", target_id: "u1", prev_trust: "pending", new_trust: "admin" },
    { actor_id: "bootstrap", actor_kind: "bootstrap", action: "restore_admin", target_id: "u5", prev_trust: "admin", new_trust: "admin" },
  ]);
  assert.deepEqual({ ...(db.prepare("SELECT decision, decided_by FROM approval_requests WHERE id = 'r1'").get() as object) }, { decision: "bootstrap", decided_by: "bootstrap" });
  assert.throws(() => db.exec("DELETE FROM auth_audit"), /append-only/);
  // The generator and the package function are the same statements.
  assert.equal(scriptSql("make", "ok@school.example.org").includes("restore_admin"), true);
  assert.throws(() => makeAdminStatements({ email: "x'y@school.example.org", adminLevel: "admin", fromLevels: ["pending"] }), /refusing/);
});

test("demote-admin demotes with sessions revoked, audited, never the last active admin", () => {
  const db = freshDb();
  for (const [id, email] of [["a1", "a1@school.example.org"], ["a2", "a2@school.example.org"]])
    db.prepare("INSERT INTO users (id, email, trust, created_at, email_verified_at, pw_hash) VALUES (?, ?, 'admin', 1, 1, 'x')").run(id, email);
  db.prepare("INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at) VALUES ('s1', 'f1', 'a2', 'h', 1, 1, 99999999999999)").run();
  db.exec(scriptSql("demote", "a2@school.example.org"));
  db.exec(scriptSql("demote", "a1@school.example.org")); // now the last active admin: refused
  const trust = (id: string) => (db.prepare("SELECT trust FROM users WHERE id = ?").get(id) as { trust: string }).trust;
  assert.equal(trust("a2"), "teacher");
  assert.equal(trust("a1"), "admin");
  assert.notEqual((db.prepare("SELECT revoked_at FROM sessions WHERE id = 's1'").get() as { revoked_at: number | null }).revoked_at, null);
  assert.equal(db.prepare("SELECT COUNT(*) AS n FROM auth_audit WHERE action = 'demote_admin'").get()!.n, 1);
  void demoteAdminStatements;
});

test("new-application notices: a few per hour, the rest rolled into one summary", async () => {
  const db = freshDb();
  const c = capture();
  try {
    const env = { MAIL_MODE: "log", APP_URL: "http://localhost:5173", ADMIN_EMAIL: "a1@x.org", DB: d1(db) };
    const t0 = 1_800_000_000_000;
    const results = [];
    for (let i = 0; i < NOTICES_PER_HOUR + 7; i++) results.push(await noticeNewApplication(env, `u${i}`, t0));
    assert.equal(results.filter((x) => x === "sent").length, NOTICES_PER_HOUR);
    assert.equal(c.logs.length, NOTICES_PER_HOUR);
    assert.equal(await flushNoticeBacklog(env, t0), 7);
    assert.equal(c.logs.length, NOTICES_PER_HOUR + 1);
    assert.match(c.logs.at(-1)!, /7 more account/);
    assert.equal(await flushNoticeBacklog(env, t0), 0);
  } finally {
    c.restore();
  }
});

test("admin alerts reach every admin and the escalation contact", async () => {
  const c = capture();
  try {
    await adminAlert(
      { MAIL_MODE: "log", APP_URL: "http://localhost:5173", ADMIN_EMAIL: "a1@x.org,a2@x.org", ESCALATION_EMAIL: "office@x.org" },
      { action: "admin_suspension_proposed", actorId: "A", targetId: "B", at: 0, proposalId: "P" },
    );
  } finally {
    c.restore();
  }
  assert.deepEqual(c.logs.map((l) => /to=([^ ]+)/.exec(l)![1]), ["a1@x.org", "a2@x.org", "office@x.org"]);
  assert.match(c.logs[0]!, /ALERT/);
});

test("staff-domain hints accept registrable domains only", () => {
  for (const ok of ["pvusd.net", "Staff.PVUSD.org", "sanjuan.k12.ca.us", "xn--bcher-kva.example"]) assert.ok(registrableDomain(ok), ok);
  for (const bad of ["com", "k12.ca.us", "co.uk", "gmail.com", "localhost", "-bad.org", "a..b.org", "1.2.3.4", "x", 42, null, "a".repeat(300) + ".org"])
    assert.equal(registrableDomain(bad), null, String(bad));
});

test("staff-domain labels say they are hints, not proof", async () => {
  const db = {
    prepare: () => ({ bind: () => ({ all: async () => ({ results: [{ domain: "pvusd.net", note: "Pleasant Valley USD" }] }) }) }),
  } as unknown as D1Database;
  const labels = await staffDomainLabels(db)(["pvusd.net"]);
  assert.match(labels["pvusd.net"]!, /hint \(not proof\)/);
});

test("decision mails render, with the reason the reviewer gave the applicant", async () => {
  const logs: string[] = [];
  const orig = console.log;
  console.log = (m: string) => void logs.push(m);
  try {
    const mailer = makeMailer({ MAIL_MODE: "log", APP_URL: "http://localhost:5173" });
    for (const kind of ["application_approved", "application_rejected", "approval_revoked", "account_suspended", "account_restored"] as const)
      await mailer.send({ kind, to: "a@b.org", reason: "REASON-TEXT" });
  } finally {
    console.log = orig;
  }
  assert.equal(logs.length, 5);
  assert.match(logs[1]!, /REASON-TEXT/);
  assert.match(logs[3]!, /REASON-TEXT/);
});

test("an overdue queue and the digest reach every admin and the escalation contact", async () => {
  const logs: string[] = [];
  const orig = console.log;
  console.log = (m: string) => void logs.push(m);
  try {
    const env = { MAIL_MODE: "log", APP_URL: "http://localhost:5173", ADMIN_EMAIL: "a1@x.org, a2@x.org", ESCALATION_EMAIL: "office@x.org" };
    await queueReminder(env, { open: 3, oldestAgeSeconds: 50 * 3600, kind: "escalation" });
    await queueReminder(env, { open: 3, oldestAgeSeconds: 3600, kind: "digest", lastDay: { approved: 4, rejected: 1, byAdmin: [{ admin_id: "A", approved: 4, rejected: 1 }] } });
  } finally {
    console.log = orig;
  }
  const to = logs.map((l) => /to=([^ ]+)/.exec(l)![1]);
  // Both go to the escalation contact too: the digest carries the last day's decisions per admin.
  assert.deepEqual(to, ["a1@x.org", "a2@x.org", "office@x.org", "a1@x.org", "a2@x.org", "office@x.org"]);
  assert.match(logs[0]!, /oldest for 50 h/);
});

test("migration 0005 moves an old free-text reason out of audit.detail into the nullable column", () => {
  const db = new DatabaseSync(":memory:");
  const files = readdirSync(`${here}migrations`).filter((x) => x.endsWith(".sql")).sort();
  for (const f of files.filter((x) => x < "0005")) db.exec(readFileSync(`${here}migrations/${f}`, "utf8"));
  db.prepare("INSERT INTO auth_audit (id, at, actor_kind, action, target_id, detail) VALUES ('x', 1, 'admin', 'suspend', 'u', ?)").run(JSON.stringify({ reason: "Jane Q Student" }));
  db.prepare("INSERT INTO auth_audit (id, at, actor_kind, action, target_id, detail) VALUES ('y', 1, 'admin', 'thing', 'u', ?)").run(JSON.stringify({ reason: "r", domain: "d.org" }));
  for (const f of files.filter((x) => x >= "0005")) db.exec(readFileSync(`${here}migrations/${f}`, "utf8"));
  assert.deepEqual({ ...(db.prepare("SELECT reason, detail FROM auth_audit WHERE id = 'x'").get() as object) }, { reason: "Jane Q Student", detail: null });
  assert.deepEqual({ ...(db.prepare("SELECT reason, detail FROM auth_audit WHERE id = 'y'").get() as object) }, { reason: "r", detail: '{"domain":"d.org"}' });
  assert.throws(() => db.exec("UPDATE auth_audit SET reason = NULL WHERE id = 'x'"), /append-only/); // target not deleted
});
