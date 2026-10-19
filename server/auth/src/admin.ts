// The approval workflow: a review queue, decisions, suspension (with a two-person rule for admins),
// and an append-only audit log. Generic: trust levels, evidence kinds, labels and side effects are
// all configured.

import { fromB64url, hmac, utf8, type Keys } from "./crypto.js";
import { HttpError, json, type ClientInfo } from "./http.js";
import type { Action, AdminAlert, AdminAuditEntry, AdminPrincipal, ApprovalConfig, MailMessage, Principal, QueueStats, WaitUntil } from "./types.js";

type Handler = (req: Request, client: ClientInfo, ctx: WaitUntil) => Promise<Response>;

export interface AdminDeps {
  db: D1Database;
  cfg: ApprovalConfig;
  /** The level an activated account waits at (the auth config's trust.afterVerify). */
  pendingLevel: string;
  keys: Promise<Keys>;
  now: () => number;
  readJson: (req: Request) => Promise<Record<string, unknown>>;
  authenticate: (req: Request) => Promise<Principal | null>;
  limit: (action: Action, req: Request, account: string | null) => Promise<() => Promise<void>>;
  hasValidDevice: (req: Request, client: ClientInfo, user: { id: string; device_epoch: number }) => Promise<boolean>;
  mail: (ctx: WaitUntil, msg: MailMessage) => void;
  background: (ctx: WaitUntil, p: Promise<unknown>, what: string) => void;
  emit: (e: { type: "admin_action"; userId?: string }) => void;
}

const SUDO_TTL_S = 5 * 60;
const MAX_TEXT = 2000;
const DEFAULT_EVIDENCE = ["directory", "school_confirmed"];
const ID = /^[0-9a-f-]{36}$/;
const DAY_MS = 86_400_000;

// ---- sudo tokens -----------------------------------------------------------------------------
// `s1.<userId>.<sessionFamily>.<deviceEpoch>.<expSeconds>.<mac>`: 5 minutes, bound to one session
// family (so a token lifted from another session is useless) and to the device epoch (so a reset or
// logout-everywhere voids it). Stateless; its own HKDF key.

export async function issueSudo(keys: Keys, p: Principal, epoch: number, nowMs: number): Promise<{ token: string; expiresIn: number }> {
  const payload = `s1.${p.userId}.${p.sessionFamily}.${epoch}.${Math.floor(nowMs / 1000) + SUDO_TTL_S}`;
  return { token: `${payload}.${await hmac(keys.sudo, payload)}`, expiresIn: SUDO_TTL_S };
}

export async function sudoValid(keys: Keys, token: string | null, p: Principal, epoch: number, nowMs: number): Promise<boolean> {
  if (!token) return false;
  const m = /^(s1\.([0-9a-f-]{36})\.([0-9a-f-]{36})\.(\d{1,9})\.(\d{1,12}))\.([A-Za-z0-9_-]{43})$/.exec(token);
  if (!m) return false;
  const [, payload, uid, sid, e, exp, mac] = m as unknown as string[];
  if (uid !== p.userId || sid !== p.sessionFamily || Number(e) !== epoch || Number(exp) <= Math.floor(nowMs / 1000)) return false;
  return crypto.subtle.verify("HMAC", keys.sudo, fromB64url(mac!), utf8(payload!));
}

// ---- helpers ---------------------------------------------------------------------------------

function text(body: Record<string, unknown>, key: string, required = true): string | null {
  const v = body[key];
  if (v === undefined || v === null || (typeof v === "string" && v.trim() === "")) {
    if (required) throw new HttpError(400, "invalid_request", { field: key });
    return null;
  }
  if (typeof v !== "string" || v.length > MAX_TEXT) throw new HttpError(400, "invalid_request", { field: key });
  return v.trim();
}

function idField(body: Record<string, unknown>, key: string): string {
  const v = body[key];
  if (typeof v !== "string" || !ID.test(v)) throw new HttpError(400, "invalid_request", { field: key });
  return v;
}

const domainOf = (email: string | null) => (email ? email.slice(email.indexOf("@") + 1) : null);

export function createAdmin(d: AdminDeps) {
  const { db, cfg } = d;
  const admins = cfg.adminLevels;
  const evidenceKinds = cfg.evidenceKinds ?? DEFAULT_EVIDENCE;
  const cooldownMs = (cfg.reapplyCooldownSeconds ?? 7 * 86400) * 1000;
  const escalateMs = (cfg.escalateAfterSeconds ?? 48 * 3600) * 1000;
  const twoPerson = cfg.twoPersonAdminSuspension ?? true;
  const proposalTtlMs = (cfg.adminProposalTtlSeconds ?? 72 * 3600) * 1000;
  const selfConfirmMs = (cfg.adminSelfConfirmAfterSeconds ?? 24 * 3600) * 1000;
  const bulkMax = cfg.bulkRejectMax ?? 50;
  /** The admin levels as one JSON parameter (D1 caps a statement at 100 bound parameters). */
  const adminsJson = JSON.stringify(admins);
  const isAdminLevel = (trust: string) => admins.includes(trust);

  /** Admins that are active and not one of `exclude`, counted in SQL. `?A` is the admin JSON. */
  const otherActiveAdmins = (excludeSql: string, a: string) =>
    `(SELECT COUNT(*) FROM users o WHERE o.trust IN (SELECT value FROM json_each(${a})) AND o.deleted_at IS NULL
        AND o.suspended_at IS NULL AND o.id NOT IN (${excludeSql}))`;

  /** True unless `users` (the row being changed) is an admin with no other active admin left. */
  const notLastAdmin = (a: string) => `(users.trust NOT IN (SELECT value FROM json_each(${a})) OR ${otherActiveAdmins("users.id", a)} >= 1)`;

  /** Mails every admin and the escalation contact, in the background. */
  function alert(ctx: WaitUntil, e: AdminAlert) {
    if (cfg.onAdminAlert) d.background(ctx, cfg.onAdminAlert(e), "onAdminAlert");
  }

  async function guard(req: Request, client: ClientInfo, write: boolean): Promise<AdminPrincipal> {
    const p = await d.authenticate(req);
    if (!p) throw new HttpError(401, "unauthorized", {}, { "WWW-Authenticate": 'Bearer error="invalid_token"' });
    await d.limit("admin", req, `user:${p.userId}`);
    if (!isAdminLevel(p.trust)) throw new HttpError(403, "forbidden");
    const u = await db
      .prepare(`SELECT id, device_epoch FROM users WHERE id = ?1 AND deleted_at IS NULL AND suspended_at IS NULL`)
      .bind(p.userId)
      .first<{ id: string; device_epoch: number }>();
    if (!u) throw new HttpError(401, "unauthorized");
    if (!(await d.hasValidDevice(req, client, u))) throw new HttpError(403, "known_device_required");
    let sudo = false;
    if (write) {
      sudo = await sudoValid(await d.keys, req.headers.get("X-Sudo-Token"), p, u.device_epoch, d.now());
      if (!sudo) throw new HttpError(403, "sudo_required");
    }
    return { ...p, sudo };
  }

  type AuditFields = AdminAuditEntry & {
    prevTrust?: string | null;
    newTrust?: string | null;
    evidence?: string | null;
    evidenceNote?: string | null;
    reason?: string | null;
  };

  /**
   * An audit insert. `trustFromRow` takes the previous and new trust from the target row INSIDE the
   * statement (an action that does not change trust), so they cannot race; `when` adds a condition
   * (for "only if this batch won").
   */
  function audit(actor: { userId: string } | null, kind: "admin" | "system" | "user", e: AuditFields, now: number, when?: string, trustFromRow = false): D1PreparedStatement {
    const cols = `id, at, actor_id, actor_kind, action, target_id, prev_trust, new_trust, evidence, evidence_note, note, reason, detail`;
    const fixed = [crypto.randomUUID(), now, actor?.userId ?? null, kind, e.action, e.targetId ?? null];
    const rest = [e.evidence ?? null, e.evidenceNote ?? null, e.note ?? null, e.reason ?? null, e.detail ? JSON.stringify(e.detail) : null];
    if (trustFromRow)
      return db
        .prepare(
          `INSERT INTO auth_audit (${cols})
           SELECT ?1, ?2, ?3, ?4, ?5, ?6, u.trust, u.trust, ?7, ?8, ?9, ?10, ?11 FROM users u WHERE u.id = ?6 ${when ? `AND ${when}` : ""}`,
        )
        .bind(...fixed, ...rest);
    return db
      .prepare(`INSERT INTO auth_audit (${cols}) SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13 ${when ? `WHERE ${when}` : ""}`)
      .bind(...fixed, e.prevTrust ?? null, e.newTrust ?? null, ...rest);
  }

  async function emailOf(id: string): Promise<string | null> {
    const row = await db.prepare(`SELECT email FROM users WHERE id = ?1 AND deleted_at IS NULL`).bind(id).first<{ email: string | null }>();
    return row?.email ?? null;
  }

  async function notify(ctx: WaitUntil, id: string, msg: Omit<MailMessage, "to">) {
    const to = await emailOf(id);
    if (to) d.mail(ctx, { ...msg, to });
  }

  // ---- decisions on an open application ------------------------------------------------------

  /**
   * One decision on one NAMED application. The gate is a single UPDATE on (request id, user id,
   * still open, account still pending, active, unsuspended, not deleted), so concurrent decisions
   * cannot both win and a stale queue page (after a re-application) changes nothing.
   */
  async function decideOne(
    admin: AdminPrincipal,
    ctx: WaitUntil,
    target: string,
    requestId: string,
    decision: "approved" | "rejected",
    f: { note: string; evidence?: string | null; evidenceNote?: string | null; reason?: string | null },
  ): Promise<boolean> {
    if (target === admin.userId) throw new HttpError(403, "self_action");
    const now = d.now();
    const nonce = crypto.randomUUID();
    const won = `EXISTS (SELECT 1 FROM approval_requests WHERE decision_nonce = '${nonce}')`;
    const stmts: D1PreparedStatement[] = [
      db
        .prepare(
          `UPDATE approval_requests SET decided_at = ?1, decided_by = ?2, decision = ?3, evidence = ?4, evidence_note = ?5,
             note = ?6, reason = ?7, decision_nonce = ?8
           WHERE id = ?11 AND user_id = ?9 AND decided_at IS NULL
             AND EXISTS (SELECT 1 FROM users WHERE id = ?9 AND trust = ?10 AND deleted_at IS NULL AND suspended_at IS NULL
                           AND email_verified_at IS NOT NULL AND pw_hash IS NOT NULL)
           RETURNING id`,
        )
        .bind(now, admin.userId, decision, f.evidence ?? null, f.evidenceNote ?? null, f.note, f.reason ?? null, nonce, target, d.pendingLevel, requestId),
    ];
    if (decision === "approved")
      stmts.push(
        db.prepare(`UPDATE users SET trust = ?1 WHERE id = ?2 AND trust = ?3 AND ${won}`).bind(cfg.approvedLevel, target, d.pendingLevel),
      );
    stmts.push(
      // The per-domain counters move with the decision, in the same batch.
      db
        .prepare(
          `INSERT INTO domain_decisions (domain, approved, rejected)
           SELECT email_domain, ?1, ?2 FROM approval_requests WHERE decision_nonce = ?3 AND email_domain IS NOT NULL
           ON CONFLICT (domain) DO UPDATE SET approved = approved + ?1, rejected = rejected + ?2`,
        )
        .bind(decision === "approved" ? 1 : 0, decision === "approved" ? 0 : 1, nonce),
      audit(
        admin,
        "admin",
        {
          action: decision === "approved" ? "approve" : "reject",
          targetId: target,
          prevTrust: d.pendingLevel,
          newTrust: decision === "approved" ? cfg.approvedLevel : d.pendingLevel,
          evidence: f.evidence ?? null,
          evidenceNote: f.evidenceNote ?? null,
          note: f.note,
          reason: f.reason ?? null,
          detail: { request_id: requestId },
        },
        now,
        won,
      ),
    );
    const res = await db.batch(stmts);
    if (res[0]!.results.length !== 1) return false;
    d.emit({ type: "admin_action", userId: target });
    await notify(ctx, target, decision === "approved" ? { kind: "application_approved" } : { kind: "application_rejected", reason: f.reason! });
    return true;
  }

  const approve: Handler = async (req, client, ctx) => {
    const admin = await guard(req, client, true);
    const body = await d.readJson(req);
    const target = idField(body, "user_id");
    const requestId = idField(body, "request_id");
    const note = text(body, "note")!;
    const ev = body.evidence;
    if (typeof ev !== "string" || !evidenceKinds.includes(ev)) throw new HttpError(400, "invalid_request", { field: "evidence" });
    const evidenceNote = text(body, "evidence_note")!;
    if (!(await decideOne(admin, ctx, target, requestId, "approved", { note, evidence: ev, evidenceNote }))) throw new HttpError(409, "not_pending");
    return json(200, { ok: true, user_id: target, request_id: requestId, decision: "approved" });
  };

  const reject: Handler = async (req, client, ctx) => {
    const admin = await guard(req, client, true);
    const body = await d.readJson(req);
    const target = idField(body, "user_id");
    const requestId = idField(body, "request_id");
    const note = text(body, "note")!;
    const reason = text(body, "reason")!;
    if (!(await decideOne(admin, ctx, target, requestId, "rejected", { note, reason }))) throw new HttpError(409, "not_pending");
    return json(200, { ok: true, user_id: target, request_id: requestId, decision: "rejected" });
  };

  /** Reject up to `bulkRejectMax` named applications with one note and reason; each is gated and audited on its own. */
  const rejectBulk: Handler = async (req, client, ctx) => {
    const admin = await guard(req, client, true);
    const body = await d.readJson(req);
    const items = body.items;
    if (!Array.isArray(items) || items.length === 0 || items.length > bulkMax) throw new HttpError(400, "invalid_request", { field: "items", max: bulkMax });
    const note = text(body, "note")!;
    const reason = text(body, "reason")!;
    const parsed = items.map((x) => {
      if (!x || typeof x !== "object") throw new HttpError(400, "invalid_request", { field: "items" });
      const o = x as Record<string, unknown>;
      return { user_id: idField(o, "user_id"), request_id: idField(o, "request_id") };
    });
    const results = [];
    for (const it of parsed) {
      const ok = it.user_id !== admin.userId && (await decideOne(admin, ctx, it.user_id, it.request_id, "rejected", { note, reason }));
      results.push({ ...it, status: ok ? "rejected" : "not_pending" });
    }
    return json(200, { results });
  };

  /** Revoke the approved level: back to pending, sessions and device tokens revoked, applicant may re-apply after the cooldown. */
  const revoke: Handler = async (req, client, ctx) => {
    const admin = await guard(req, client, true);
    const body = await d.readJson(req);
    const target = idField(body, "user_id");
    if (target === admin.userId) throw new HttpError(403, "self_action");
    const note = text(body, "note")!;
    const reason = text(body, "reason")!;
    const now = d.now();
    const nonce = crypto.randomUUID();
    const won = `EXISTS (SELECT 1 FROM approval_requests WHERE decision_nonce = '${nonce}')`;
    const res = await db.batch([
      // The gate: only an account at the approved level, flipped once. changes() in the next
      // statement is this UPDATE's own result, so only the request that flipped it records it.
      db
        .prepare(`UPDATE users SET trust = ?1, device_epoch = device_epoch + 1 WHERE id = ?2 AND trust = ?3 AND deleted_at IS NULL RETURNING id`)
        .bind(d.pendingLevel, target, cfg.approvedLevel),
      db
        .prepare(
          `INSERT INTO approval_requests (id, user_id, requested_at, decided_at, decided_by, decision, note, reason, decision_nonce, email_domain)
           SELECT ?1, ?2, ?3, ?3, ?4, 'revoked', ?5, ?6, ?7, (SELECT substr(email, instr(email, '@') + 1) FROM users WHERE id = ?2)
           WHERE changes() = 1`,
        )
        .bind(crypto.randomUUID(), target, now, admin.userId, note, reason, nonce),
      db.prepare(`UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL AND ${won}`).bind(now, target),
      db
        .prepare(
          `INSERT INTO domain_decisions (domain, approved, rejected)
           SELECT email_domain, 0, 1 FROM approval_requests WHERE decision_nonce = ?1 AND email_domain IS NOT NULL
           ON CONFLICT (domain) DO UPDATE SET rejected = rejected + 1`,
        )
        .bind(nonce),
      audit(admin, "admin", { action: "revoke", targetId: target, prevTrust: cfg.approvedLevel, newTrust: d.pendingLevel, note, reason }, now, won),
    ]);
    if (res[0]!.results.length !== 1) throw new HttpError(409, "not_approved");
    d.emit({ type: "admin_action", userId: target });
    await notify(ctx, target, { kind: "approval_revoked", reason });
    return json(200, { ok: true, user_id: target });
  };

  // ---- suspension, with the two-person rule for admins -----------------------------------

  async function targetRow(id: string) {
    return db
      .prepare(`SELECT id, trust, suspended_at FROM users WHERE id = ?1 AND deleted_at IS NULL`)
      .bind(id)
      .first<{ id: string; trust: string; suspended_at: number | null }>();
  }

  /**
   * Suspends now. The gate, the last-admin rule and the audit (with the previous trust read from the
   * row inside the statement) are one batch, so nothing about it can race.
   */
  async function executeSuspend(actor: AdminPrincipal, ctx: WaitUntil, target: string, note: string | null, reason: string, proposalId?: string): Promise<boolean> {
    const now = d.now();
    const proposalOpen = proposalId
      ? `AND EXISTS (SELECT 1 FROM admin_proposals WHERE id = '${proposalId}' AND decided_at IS NULL AND expires_at > ${now})`
      : "";
    const res = await db.batch([
      db
        .prepare(
          `UPDATE users SET suspended_at = ?1, suspension_reason = ?2, device_epoch = device_epoch + 1
           WHERE id = ?3 AND suspended_at IS NULL AND deleted_at IS NULL AND ${notLastAdmin("?4")} ${proposalOpen} RETURNING trust`,
        )
        .bind(now, reason, target, adminsJson),
      audit(
        actor,
        "admin",
        { action: "suspend", targetId: target, note, reason, detail: proposalId ? { proposal_id: proposalId } : null },
        now,
        `changes() = 1`,
        true,
      ),
      ...(proposalId
        ? [
            db
              .prepare(
                `UPDATE admin_proposals SET decided_by = ?1, decided_at = ?2, outcome = 'confirmed'
                 WHERE id = ?3 AND decided_at IS NULL AND EXISTS (SELECT 1 FROM users WHERE id = ?4 AND suspended_at = ?2)`,
              )
              .bind(actor.userId, now, proposalId, target),
          ]
        : []),
    ]);
    if (res[0]!.results.length !== 1) return false;
    const wasAdmin = isAdminLevel((res[0]!.results[0] as { trust: string }).trust);
    // The flip above already blocks every login, refresh and live check; this ends the sessions too.
    await db.batch([
      db.prepare(`UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL`).bind(now, target),
      ...(cfg.onSuspend ? cfg.onSuspend(target, now) : []),
    ]);
    d.emit({ type: "admin_action", userId: target });
    if (wasAdmin) alert(ctx, { action: "admin_suspended", actorId: actor.userId, targetId: target, at: now, proposalId });
    await notify(ctx, target, { kind: "account_suspended", reason });
    return true;
  }

  /**
   * Suspend. Against an admin-level account (with the two-person rule on, the default) it only
   * PROPOSES: a different admin must confirm. If no other admin could confirm, the proposer may
   * confirm alone after `adminSelfConfirmAfterSeconds`. Every step alerts all admins and the
   * escalation contact, so a compromised admin cannot quietly suspend the others one by one.
   */
  const suspend: Handler = async (req, client, ctx) => {
    const admin = await guard(req, client, true);
    const body = await d.readJson(req);
    const target = idField(body, "user_id");
    if (target === admin.userId) throw new HttpError(403, "self_action");
    const note = text(body, "note")!;
    const reason = text(body, "reason")!;
    const t = await targetRow(target);
    if (!t) throw new HttpError(404, "not_found");
    if (twoPerson && isAdminLevel(t.trust)) {
      const now = d.now();
      const id = crypto.randomUUID();
      const res = await db.batch([
        db
          .prepare(`UPDATE admin_proposals SET decided_at = ?1, outcome = 'expired' WHERE target_id = ?2 AND decided_at IS NULL AND expires_at <= ?1`)
          .bind(now, target),
        // Self-confirmation is allowed later only when nobody but the proposer and the target is an active admin.
        db
          .prepare(
            `INSERT OR IGNORE INTO admin_proposals (id, action, target_id, proposed_by, proposed_at, expires_at, self_confirm_after, note, reason)
             SELECT ?1, 'suspend', ?2, ?3, ?4, ?5,
                    CASE WHEN ${otherActiveAdmins("?2, ?3", "?8")} = 0 THEN ?6 ELSE NULL END, ?7, ?9
             WHERE EXISTS (SELECT 1 FROM users WHERE id = ?2 AND suspended_at IS NULL AND deleted_at IS NULL)
             RETURNING self_confirm_after, expires_at`,
          )
          .bind(id, target, admin.userId, now, now + proposalTtlMs, now + selfConfirmMs, note, adminsJson, reason),
        audit(admin, "admin", { action: "suspend_proposed", targetId: target, note, reason, detail: { proposal_id: id } }, now, `changes() = 1`, true),
      ]);
      const row = res[1]!.results[0] as { self_confirm_after: number | null; expires_at: number } | undefined;
      if (!row) throw new HttpError(409, "proposal_exists");
      alert(ctx, { action: "admin_suspension_proposed", actorId: admin.userId, targetId: target, at: now, proposalId: id });
      return json(202, { ok: true, proposal_id: id, expires_at: row.expires_at, self_confirm_after: row.self_confirm_after });
    }
    if (!(await executeSuspend(admin, ctx, target, note, reason))) {
      const now2 = await targetRow(target);
      if (now2 && now2.suspended_at === null && isAdminLevel(now2.trust)) throw new HttpError(409, "last_admin");
      throw new HttpError(409, "not_suspendable");
    }
    return json(200, { ok: true, user_id: target });
  };

  /** Confirm a proposal: a different admin (never the target), or the proposer alone once self-confirmation is open. */
  const confirm: Handler = async (req, client, ctx) => {
    const admin = await guard(req, client, true);
    const body = await d.readJson(req);
    const id = idField(body, "proposal_id");
    const now = d.now();
    const p = await db
      .prepare(`SELECT * FROM admin_proposals WHERE id = ?1 AND decided_at IS NULL AND expires_at > ?2`)
      .bind(id, now)
      .first<{ target_id: string; proposed_by: string; self_confirm_after: number | null; note: string | null; reason: string | null }>();
    if (!p) throw new HttpError(409, "no_open_proposal");
    if (admin.userId === p.target_id) throw new HttpError(403, "self_action");
    if (admin.userId === p.proposed_by) {
      // Alone only if nobody else could confirm, and only after the delay everyone was alerted about.
      const others = await db
        .prepare(`SELECT ${otherActiveAdmins("?1, ?2", "?3")} AS n`)
        .bind(p.target_id, p.proposed_by, adminsJson)
        .first<{ n: number }>();
      if (p.self_confirm_after === null || now < p.self_confirm_after || (others?.n ?? 0) > 0) throw new HttpError(403, "second_admin_required");
    }
    if (!(await executeSuspend(admin, ctx, p.target_id, p.note, p.reason ?? "", id))) throw new HttpError(409, "not_suspendable");
    return json(200, { ok: true, user_id: p.target_id, proposal_id: id });
  };

  /** Cancel an open proposal (any admin, including its target). */
  const cancel: Handler = async (req, client, ctx) => {
    const admin = await guard(req, client, true);
    const body = await d.readJson(req);
    const id = idField(body, "proposal_id");
    const now = d.now();
    const row = await db
      .prepare(`UPDATE admin_proposals SET decided_by = ?1, decided_at = ?2, outcome = 'cancelled' WHERE id = ?3 AND decided_at IS NULL RETURNING target_id`)
      .bind(admin.userId, now, id)
      .first<{ target_id: string }>();
    if (!row) throw new HttpError(409, "no_open_proposal");
    await db.batch([audit(admin, "admin", { action: "suspend_proposal_cancelled", targetId: row.target_id, detail: { proposal_id: id } }, now)]);
    alert(ctx, { action: "admin_suspension_cancelled", actorId: admin.userId, targetId: row.target_id, at: now, proposalId: id });
    return json(200, { ok: true });
  };

  const proposals: Handler = async (req, client) => {
    await guard(req, client, false);
    const rows = await db
      .prepare(
        `SELECT id, action, target_id, proposed_by, proposed_at, expires_at, self_confirm_after, decided_by, decided_at, outcome
         FROM admin_proposals ORDER BY proposed_at DESC, id DESC LIMIT 100`,
      )
      .all();
    return json(200, { proposals: rows.results });
  };

  const restore: Handler = async (req, client, ctx) => {
    const admin = await guard(req, client, true);
    const body = await d.readJson(req);
    const target = idField(body, "user_id");
    if (target === admin.userId) throw new HttpError(403, "self_action");
    const note = text(body, "note")!;
    const now = d.now();
    const res = await db.batch([
      db
        .prepare(`UPDATE users SET suspended_at = NULL, suspension_reason = NULL WHERE id = ?1 AND suspended_at IS NOT NULL AND deleted_at IS NULL RETURNING trust`)
        .bind(target),
      audit(admin, "admin", { action: "restore", targetId: target, note }, now, `changes() = 1`, true),
    ]);
    if (res[0]!.results.length !== 1) throw new HttpError(409, "not_suspended");
    if (cfg.onRestore) {
      const extra = cfg.onRestore(target, now);
      if (extra.length > 0) await db.batch(extra);
    }
    d.emit({ type: "admin_action", userId: target });
    await notify(ctx, target, { kind: "account_restored" });
    return json(200, { ok: true, user_id: target });
  };

  // ---- reads ---------------------------------------------------------------------------------

  const STATUS: Record<string, string> = {
    open: "r.decided_at IS NULL",
    approved: "r.decision = 'approved'",
    rejected: "r.decision = 'rejected'",
    revoked: "r.decision = 'revoked'",
    all: "1 = 1",
  };

  async function stats(now: number): Promise<QueueStats> {
    const row = await db
      .prepare(
        `SELECT COUNT(*) AS n, MIN(r.requested_at) AS oldest FROM approval_requests r JOIN users u ON u.id = r.user_id
         WHERE r.decided_at IS NULL AND u.deleted_at IS NULL`,
      )
      .first<{ n: number; oldest: number | null }>();
    return { open: row?.n ?? 0, oldestAgeSeconds: row?.oldest ? Math.max(0, Math.floor((now - row.oldest) / 1000)) : 0 };
  }

  /**
   * The review queue, oldest first. Applicant-typed fields (display name, profile) are the SNAPSHOT
   * submitted with that application; `profile_changed` says whether the live profile differs now.
   * Server-generated labels are in `server_labels`, never mixed into applicant text. Render every
   * applicant field as plain text.
   */
  const queue: Handler = async (req, client) => {
    await guard(req, client, false);
    const url = new URL(req.url);
    const status = url.searchParams.get("status") ?? "open";
    const where = STATUS[status];
    if (!where) throw new HttpError(400, "invalid_request", { field: "status" });
    const limit = Math.min(200, Math.max(1, Number(url.searchParams.get("limit") ?? 50) || 50));
    const domain = url.searchParams.get("domain");
    if (domain !== null && !/^[a-z0-9.-]{1,253}$/.test(domain)) throw new HttpError(400, "invalid_request", { field: "domain" });
    const age = (k: string) => {
      const v = url.searchParams.get(k);
      if (v === null) return null;
      if (!/^\d{1,10}$/.test(v)) throw new HttpError(400, "invalid_request", { field: k });
      return Number(v) * 1000;
    };
    const minAge = age("min_age_seconds");
    const maxAge = age("max_age_seconds");
    const cursor = url.searchParams.get("cursor");
    let afterAt = -1;
    let afterId = "";
    if (cursor) {
      const m = /^(\d{1,15})\.([0-9a-f-]{1,36})$/.exec(cursor);
      if (!m) throw new HttpError(400, "invalid_request", { field: "cursor" });
      afterAt = Number(m[1]);
      afterId = m[2]!;
    }
    const now = d.now();
    const rows = await db
      .prepare(
        `SELECT r.id AS request_id, r.user_id, r.requested_at, r.decided_at, r.decision, r.email_domain,
                r.display_name_snapshot, r.profile_snapshot, u.email, u.display_name, u.profile, u.trust, u.suspended_at,
                (SELECT COUNT(*) FROM approval_requests x WHERE x.user_id = r.user_id AND x.id != r.id
                   AND x.decision IN ('rejected', 'revoked')) AS prior_rejections
         FROM approval_requests r JOIN users u ON u.id = r.user_id
         WHERE u.deleted_at IS NULL AND ${where}
           AND (?4 IS NULL OR r.email_domain = ?4)
           AND (?5 IS NULL OR r.requested_at <= ?6 - ?5)
           AND (?7 IS NULL OR r.requested_at >= ?6 - ?7)
           AND (r.requested_at > ?1 OR (r.requested_at = ?1 AND r.id > ?2))
         ORDER BY r.requested_at, r.id LIMIT ?3`,
      )
      .bind(afterAt, afterId, limit + 1, domain, minAge, now, maxAge)
      .all<{
        request_id: string;
        user_id: string;
        requested_at: number;
        decided_at: number | null;
        decision: string | null;
        email_domain: string | null;
        display_name_snapshot: string | null;
        profile_snapshot: string | null;
        email: string;
        display_name: string | null;
        profile: string | null;
        trust: string;
        suspended_at: number | null;
        prior_rejections: number;
      }>();
    const page = rows.results.slice(0, limit);
    const domains = [...new Set(page.map((x) => x.email_domain ?? domainOf(x.email)).filter((x): x is string => !!x))];
    const history: Record<string, { approved: number; rejected: number }> = {};
    if (domains.length > 0) {
      const marks = domains.map((_, i) => `?${i + 1}`).join(", ");
      const h = await db
        .prepare(`SELECT domain, approved, rejected FROM domain_decisions WHERE domain IN (${marks})`)
        .bind(...domains)
        .all<{ domain: string; approved: number; rejected: number }>();
      for (const x of h.results) history[x.domain] = { approved: x.approved, rejected: x.rejected };
    }
    const labels = cfg.domainLabels && domains.length > 0 ? await cfg.domainLabels(domains) : {};
    const last = page[page.length - 1];
    return json(200, {
      ...(await stats(now)),
      items: page.map((x) => {
        const dom = x.email_domain ?? domainOf(x.email);
        return {
          request_id: x.request_id,
          user_id: x.user_id,
          status: x.decided_at === null ? "open" : x.decision,
          activated_at: x.requested_at,
          age_seconds: Math.max(0, Math.floor((now - x.requested_at) / 1000)),
          decided_at: x.decided_at,
          email: x.email,
          email_domain: dom,
          // Server-generated; never mixed with applicant text.
          server_labels: {
            domain_hint: dom ? (labels[dom] ?? null) : null,
            domain_history: dom ? (history[dom] ?? { approved: 0, rejected: 0 }) : null,
            prior_rejections: x.prior_rejections,
            profile_changed: x.display_name !== x.display_name_snapshot || x.profile !== x.profile_snapshot,
            suspended: x.suspended_at !== null,
          },
          // Applicant-typed, as submitted with this application. Render as plain text.
          applicant: {
            display_name: x.display_name_snapshot,
            profile: x.profile_snapshot === null ? null : JSON.parse(x.profile_snapshot),
          },
          trust: x.trust,
        };
      }),
      next_cursor: rows.results.length > limit && last ? `${last.requested_at}.${last.request_id}` : null,
    });
  };

  const userDetail = async (req: Request, client: ClientInfo, id: string): Promise<Response> => {
    await guard(req, client, false);
    if (!ID.test(id)) throw new HttpError(404, "not_found");
    const u = await db
      .prepare(
        `SELECT id, email, display_name, profile, trust, created_at, email_verified_at, suspended_at, suspension_reason, deleted_at
         FROM users WHERE id = ?1`,
      )
      .bind(id)
      .first<Record<string, unknown> & { profile: string | null }>();
    if (!u) throw new HttpError(404, "not_found");
    const requests = await db
      .prepare(
        `SELECT id, requested_at, decided_at, decided_by, decision, evidence, evidence_note, note, reason, email_domain,
                display_name_snapshot, profile_snapshot
         FROM approval_requests WHERE user_id = ?1 ORDER BY requested_at, rowid`,
      )
      .bind(id)
      .all<Record<string, unknown> & { profile_snapshot: string | null }>();
    const trail = await db
      .prepare(`SELECT * FROM auth_audit WHERE target_id = ?1 ORDER BY at DESC, rowid DESC LIMIT 200`)
      .bind(id)
      .all<Record<string, unknown> & { detail: string | null }>();
    return json(200, {
      user: { ...u, profile: u.profile === null ? null : JSON.parse(u.profile) },
      requests: requests.results.map((x) => ({ ...x, profile_snapshot: x.profile_snapshot === null ? null : JSON.parse(x.profile_snapshot) })),
      audit: trail.results.map((x) => ({ ...x, detail: x.detail === null ? null : JSON.parse(x.detail) })),
    });
  };

  /** The audit log, newest first; ties broken by insertion order (rowid), so paging is stable. */
  const auditLog: Handler = async (req, client) => {
    await guard(req, client, false);
    const url = new URL(req.url);
    const limit = Math.min(500, Math.max(1, Number(url.searchParams.get("limit") ?? 100) || 100));
    const before = url.searchParams.get("before");
    let at = Number.MAX_SAFE_INTEGER;
    let seq = Number.MAX_SAFE_INTEGER;
    if (before) {
      const m = /^(\d{1,15})\.(\d{1,15})$/.exec(before);
      if (!m) throw new HttpError(400, "invalid_request", { field: "before" });
      at = Number(m[1]);
      seq = Number(m[2]);
    }
    const rows = await db
      .prepare(`SELECT rowid AS seq, * FROM auth_audit WHERE (at < ?1 OR (at = ?1 AND rowid < ?2)) ORDER BY at DESC, rowid DESC LIMIT ?3`)
      .bind(at, seq, limit + 1)
      .all<Record<string, unknown> & { at: number; seq: number; detail: string | null }>();
    const page = rows.results.slice(0, limit);
    const last = page[page.length - 1];
    return json(200, {
      entries: page.map((x) => ({ ...x, detail: x.detail === null ? null : JSON.parse(x.detail) })),
      next_before: rows.results.length > limit && last ? `${last.at}.${last.seq}` : null,
    });
  };

  // ---- the applicant's side ----------------------------------------------------------------

  /** The applicant's own status, with the reviewer's reason but never the reviewer's notes. */
  async function statusOf(id: string, trust: string, suspendedAt: number | null, suspensionReason: string | null, now: number) {
    if (suspendedAt !== null) return { state: "suspended", reason: suspensionReason };
    if (isAdminLevel(trust) || trust === cfg.approvedLevel) return { state: "approved" };
    if (trust !== d.pendingLevel) return { state: "unverified" };
    const last = await db
      .prepare(`SELECT decided_at, decision, reason FROM approval_requests WHERE user_id = ?1 ORDER BY requested_at DESC, rowid DESC LIMIT 1`)
      .bind(id)
      .first<{ decided_at: number | null; decision: string | null; reason: string | null }>();
    if (!last || last.decided_at === null) return { state: "pending" };
    if (last.decision === "rejected" || last.decision === "revoked") {
      const after = last.decided_at + cooldownMs;
      return { state: last.decision, reason: last.reason, reapply_after: after, can_reapply: now >= after };
    }
    return { state: "pending" };
  }

  /**
   * After a rejection or revocation and the cooldown, the applicant applies again with a fresh
   * profile. It is a NEW application row (with its own snapshot); the old decision stays as it was.
   */
  async function reapply(p: Principal, profile: { displayName: string | null; profile: Record<string, unknown> | null }): Promise<Response> {
    const now = d.now();
    const u = await db
      .prepare(`SELECT trust, suspended_at FROM users WHERE id = ?1 AND deleted_at IS NULL`)
      .bind(p.userId)
      .first<{ trust: string; suspended_at: number | null }>();
    if (!u || u.suspended_at !== null || u.trust !== d.pendingLevel) throw new HttpError(409, "not_eligible");
    const last = await db
      .prepare(`SELECT decided_at, decision FROM approval_requests WHERE user_id = ?1 ORDER BY requested_at DESC, rowid DESC LIMIT 1`)
      .bind(p.userId)
      .first<{ decided_at: number | null; decision: string | null }>();
    if (!last || last.decided_at === null || (last.decision !== "rejected" && last.decision !== "revoked")) throw new HttpError(409, "not_eligible");
    if (now < last.decided_at + cooldownMs)
      throw new HttpError(429, "cooldown", { reapply_after: last.decided_at + cooldownMs }, { "Retry-After": String(Math.ceil((last.decided_at + cooldownMs - now) / 1000)) });
    const reqId = crypto.randomUUID();
    const profileJson = profile.profile === null ? null : JSON.stringify(profile.profile);
    const res = await db.batch([
      // OR IGNORE + the one-open-application index: concurrent re-applications make one row.
      db
        .prepare(
          `INSERT OR IGNORE INTO approval_requests (id, user_id, requested_at, email_domain, display_name_snapshot, profile_snapshot)
           SELECT ?1, id, ?2, substr(email, instr(email, '@') + 1), ?4, ?5 FROM users WHERE id = ?3`,
        )
        .bind(reqId, now, p.userId, profile.displayName, profileJson),
      db
        .prepare(`UPDATE users SET display_name = ?1, profile = ?2 WHERE id = ?3 AND EXISTS (SELECT 1 FROM approval_requests WHERE id = ?4)`)
        .bind(profile.displayName, profileJson, p.userId, reqId),
      audit(p, "user", { action: "reapply", targetId: p.userId, detail: { request_id: reqId } }, now, `EXISTS (SELECT 1 FROM approval_requests WHERE id = '${reqId}')`),
    ]);
    if ((res[0]!.meta.changes ?? 0) !== 1) throw new HttpError(409, "not_eligible");
    return json(202, { ok: true, request_id: reqId });
  }

  /** For the cron: queue state; an escalation when overdue, or a digest (with the last day's decisions per admin) if enabled. */
  async function reviewReminders(): Promise<QueueStats & { sent: "digest" | "escalation" | null }> {
    const now = d.now();
    const s = await stats(now);
    let kind: "digest" | "escalation" | null = null;
    if (s.open > 0 && s.oldestAgeSeconds * 1000 >= escalateMs) kind = "escalation";
    else if (cfg.digest) kind = "digest";
    if (kind && cfg.onQueueReminder) {
      const rows = await db
        .prepare(
          `SELECT decided_by, SUM(decision = 'approved') AS approved, SUM(decision IN ('rejected', 'revoked')) AS rejected
           FROM approval_requests WHERE decided_at > ?1 AND decided_by IS NOT NULL GROUP BY decided_by`,
        )
        .bind(now - DAY_MS)
        .all<{ decided_by: string; approved: number; rejected: number }>();
      const byAdmin = rows.results.map((x) => ({ admin_id: x.decided_by, approved: x.approved, rejected: x.rejected }));
      await cfg.onQueueReminder({
        ...s,
        kind,
        lastDay: {
          approved: byAdmin.reduce((n, x) => n + x.approved, 0),
          rejected: byAdmin.reduce((n, x) => n + x.rejected, 0),
          byAdmin,
        },
      });
    }
    return { ...s, sent: kind && cfg.onQueueReminder ? kind : null };
  }

  /** Wraps an app-supplied admin route behind the same guard. */
  function extra(route: NonNullable<ApprovalConfig["routes"]>[number]): Handler {
    return async (req, client) => {
      const admin = await guard(req, client, route.write);
      const body = route.method === "GET" || req.headers.get("Content-Type") === null ? null : await d.readJson(req);
      return route.handler(req, {
        admin,
        body,
        now: d.now(),
        audit: (e) => audit(admin, "admin", e, d.now()),
      });
    };
  }

  /**
   * Account deletion when the workflow is on. An admin-level account needs a known device and a sudo
   * token, can never delete itself as the last active admin (the rule is in the tombstone UPDATE
   * itself), and alerts everyone. Every deletion writes an audit row in the same batch.
   */
  async function deletionGuard(req: Request, client: ClientInfo, p: Principal, user: { id: string; trust: string; device_epoch: number }) {
    if (!isAdminLevel(user.trust)) return;
    if (!(await d.hasValidDevice(req, client, user))) throw new HttpError(403, "known_device_required");
    if (!(await sudoValid(await d.keys, req.headers.get("X-Sudo-Token"), p, user.device_epoch, d.now()))) throw new HttpError(403, "sudo_required");
  }

  return {
    queue,
    approve,
    reject,
    rejectBulk,
    revoke,
    suspend,
    confirm,
    cancel,
    proposals,
    restore,
    auditLog,
    userDetail,
    statusOf,
    reapply,
    reviewReminders,
    extra,
    stats,
    isAdminLevel,
    adminsJson,
    notLastAdmin,
    deletionGuard,
    alert,
    auditDeletion: (userId: string, trust: string, now: number, when: string) =>
      audit({ userId }, "user", { action: "account_deleted", targetId: userId, prevTrust: trust, newTrust: null }, now, when),
  };
}

export type Admin = ReturnType<typeof createAdmin>;
