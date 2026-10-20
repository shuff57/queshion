// Author actions (visibility, delete with 14-day undo), reports, the counter-notice, and the
// moderation machinery: holds, takedown with quarantine, put-back, purge (docs/shared-pool.md
// "Reports", "Takedown, DMCA and moderation").
import { cleanText } from "../../../auth/src/validate.ts";
import { addBusinessDays } from "./bizdays.ts";
import { releaseOrphanHolds, selectedBlobs, selectedMatch, type BlockPlan } from "./blocking.ts";
import { deletePublishStatements } from "./publish.ts";
import {
  alertBudgeted,
  audit,
  countMiss,
  SHARED_SQL,
  usage,
  usageSql,
  DAY_MS,
  HOUR_MS,
  ID_RE,
  json,
  limitOf,
  missGuard,
  newId,
  notFound,
  PoolError,
  readJson,
  requireDaily,
  send,
  visibleTo,
  type Ctx,
  type PoolPrincipal,
} from "./shared.ts";

export const UNDO_MS = 14 * DAY_MS;
/** Privacy-blocked image bytes stay (served to nobody) this long before they are deleted. */
export const PRIVACY_BYTES_GRACE_MS = 14 * DAY_MS;
export const QUARANTINE_MS = 30 * DAY_MS;
export const STRIKE_WINDOW_MS = 365 * DAY_MS;
export const STRIKES_TO_SUSPEND = 3;
export const INSTANT_HOLDS_PER_DAY = 3;
/** Accounts younger than this cannot hide an item instantly (their reports wait for the admin). */
export const NEW_ACCOUNT_MS = 7 * DAY_MS;
export const THROTTLE_MS = 30 * DAY_MS;
export const REPORT_REASONS = ["wrong_answer", "copyright", "student_data", "inappropriate", "other"] as const;

async function idOrMiss(c: Ctx, id: string, me: string, ownOk = false): Promise<void> {
  // The day's miss lockout never stops an author managing their own items.
  if (ownOk && ID_RE.test(id) && (await c.db.prepare(`SELECT 1 AS x FROM items WHERE id = ?1 AND author_id = ?2`).bind(id, me).first())) return;
  await missGuard(c, me, "item_miss");
  if (!ID_RE.test(id)) {
    await countMiss(c, me, "item_miss");
    throw notFound();
  }
}
async function miss(c: Ctx, me: string): Promise<never> {
  await countMiss(c, me, "item_miss");
  throw notFound();
}

// ---- author actions ---------------------------------------------------------------------------------

/** PATCH /items/:id {visibility}: one UPDATE; public is refused while the current licence is circle-only. */
export async function patchItem(c: Ctx, req: Request, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me);
  const b = await readJson(req);
  const vis = b.visibility;
  if (vis !== "public" && vis !== "unlisted") throw new PoolError(400, "invalid_request", { field: "visibility" });
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE items SET visibility = ?3, updated_at = ?4 WHERE id = ?1 AND author_id = ?2 AND removed_at IS NULL AND current_publish_id IS NOT NULL
           AND (?3 = 'unlisted' OR license <> 'circle-only')`,
      )
      .bind(id, me, vis, now),
    audit(c.db, { actorId: me, actorKind: "user", action: "item.visibility", itemId: id, targetUserId: me, detail: { visibility: vis } }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) === 1) return json(200, { ok: true, visibility: vis });
  const own = await c.db.prepare(`SELECT license, removed_at FROM items WHERE id = ?1 AND author_id = ?2`).bind(id, me).first<{ license: string; removed_at: number | null }>();
  if (own && own.removed_at === null && own.license === "circle-only") throw new PoolError(422, "license_not_public");
  if (own && own.removed_at !== null) throw new PoolError(409, "item_removed");
  return miss(c, me);
}

/** DELETE /items/:id: hidden at once, purged after 14 days unless undone. */
export async function deleteItem(c: Ctx, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me, true);
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db
      .prepare(`UPDATE items SET removed_at = ?3, removed_reason = 'author_deleted', purge_after = ?4, updated_at = ?3 WHERE id = ?1 AND author_id = ?2 AND removed_at IS NULL`)
      .bind(id, me, now, now + UNDO_MS),
    audit(c.db, { actorId: me, actorKind: "user", action: "item.delete", itemId: id, targetUserId: me }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) === 1) return json(200, { ok: true, undo_until: now + UNDO_MS });
  const own = await c.db.prepare(`SELECT removed_reason, purge_after FROM items WHERE id = ?1 AND author_id = ?2 AND purged_at IS NULL`).bind(id, me).first<{ removed_reason: string | null; purge_after: number | null }>();
  // Already deleted: the same answer (the window is never extended). Privacy and DMCA removals
  // follow their own rules: deleting does not start or shorten anything.
  if (own?.removed_reason === "author_deleted") return json(200, { ok: true, undo_until: own.purge_after });
  if (own) throw new PoolError(409, "under_moderation");
  return miss(c, me);
}

/** POST /items/:id/undelete: only an author deletion, only within its 14 days. */
export async function undeleteItem(c: Ctx, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me, true);
  const now = c.now();
  const quota = await limitOf(c, me, "storage_bytes");
  const cap = await limitOf(c, me, "shared_questions");
  // Storage quota and the shared-question cap are part of the same UPDATE (critic A6): undoing a
  // deletion cannot take an account past either.
  const [upd] = await c.db.batch([
    c.db
      .prepare(
        `UPDATE items SET removed_at = NULL, removed_reason = NULL, purge_after = NULL, updated_at = ?2
          WHERE id = ?3 AND author_id = ?1 AND removed_reason = 'author_deleted' AND purge_after > ?2 AND purged_at IS NULL
            AND ${usageSql("?3")} <= ?4
            AND ${SHARED_SQL("?1", "?3")} + question_count <= ?5`,
      )
      .bind(me, now, id, quota, cap),
    audit(c.db, { actorId: me, actorKind: "user", action: "item.undelete", itemId: id, targetUserId: me }, now, `changes() = 1`),
  ]);
  if ((upd!.meta.changes ?? 0) === 1) return json(200, { ok: true });
  const it = await c.db
    .prepare(`SELECT question_count FROM items WHERE id = ?1 AND author_id = ?2 AND removed_reason = 'author_deleted' AND purge_after > ?3 AND purged_at IS NULL`)
    .bind(id, me, now)
    .first<{ question_count: number }>();
  if (!it) return miss(c, me);
  const after = await c.db.prepare(`SELECT ${usageSql("?3")} AS used, ${SHARED_SQL("?1", "?3")} AS shared`).bind(me, now, id).first<{ used: number; shared: number }>();
  if ((after?.used ?? 0) > quota) throw new PoolError(413, "quota_exceeded", { used: await usage(c, me), needed: (after?.used ?? 0) - (await usage(c, me)), limit: quota });
  throw new PoolError(422, "shared_questions_limit", { used: after?.shared ?? 0, needed: it.question_count, limit: cap });
}

// ---- reports ----------------------------------------------------------------------------------------

/** POST /items/:id/report: the reporter only ever hears "thanks". */
export async function report(c: Ctx, req: Request, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me);
  const b = await readJson(req);
  const reason = b.reason;
  if (typeof reason !== "string" || !(REPORT_REASONS as readonly string[]).includes(reason)) throw new PoolError(400, "invalid_request", { field: "reason" });
  let note: string | null = null;
  if (b.note !== undefined && b.note !== null && b.note !== "") {
    note = cleanText(b.note, 1000);
    if (!note) throw new PoolError(400, "invalid_request", { field: "note" });
  }
  const it = await c.db
    .prepare(`SELECT i.id, i.author_id, i.current_publish_id FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = ?1 AND ${visibleTo("?2")}`)
    .bind(id, me)
    .first<{ id: string; author_id: string; current_publish_id: string }>();
  if (!it) return miss(c, me);
  if (it.author_id === me) throw new PoolError(400, "own_item");
  await requireDaily(c, me, "reports");
  const now = c.now();
  const rid = newId();
  const threshold = await limitOf(c, "*", "report_threshold");
  const stmts: D1PreparedStatement[] = [
    c.db
      .prepare(`INSERT OR IGNORE INTO reports (id, item_id, publish_id, reporter_id, reason, note, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)`)
      .bind(rid, id, it.current_publish_id, me, reason, note, now),
  ];
  if (reason === "student_data")
    // One report hides the item at once, but one reporter can do that to at most
    // INSTANT_HOLDS_PER_DAY items a day, and not at all within THROTTLE_MS of having a student_data
    // report dismissed anywhere (the report is still recorded for the admin).
    stmts.push(
      c.db
        .prepare(
          `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by) SELECT ?1, 'student_data', ?2, NULL
            WHERE NOT EXISTS (SELECT 1 FROM reports WHERE reporter_id = ?3 AND reason = 'student_data' AND action = 'dismissed' AND resolved_at > ?2 - ?4)
              AND (SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5
                     AND EXISTS (SELECT 1 FROM reports r WHERE r.item_id = h.item_id AND r.reporter_id = ?3 AND r.reason = 'student_data' AND r.at = h.at)) < ?6
              -- a site-wide daily cap, and no instant holds from accounts younger than NEW_ACCOUNT_MS:
              -- those reports wait for the admin
              AND (SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5) < ?7
              -- past half the site cap, an instant hold needs two distinct reporters
              AND ((SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?5) < ?7 / 2
                   OR (SELECT COUNT(DISTINCT reporter_id) FROM reports WHERE item_id = ?1 AND reason = 'student_data' AND resolved_at IS NULL) >= 2)
              AND (SELECT created_at FROM users WHERE id = ?3) <= ?2 - ?8`,
        )
        .bind(id, now, me, THROTTLE_MS, Math.floor(now / DAY_MS) * DAY_MS, INSTANT_HOLDS_PER_DAY, await limitOf(c, "*", "instant_holds_global"), NEW_ACCOUNT_MS),
    );
  else
    // 3 distinct teacher reporters (open, non-privacy reasons) hide it pending review.
    stmts.push(
      c.db
        .prepare(
          `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by)
           SELECT ?1, 'reports', ?2, NULL WHERE (SELECT COUNT(DISTINCT r.reporter_id) FROM reports r JOIN users ru ON ru.id = r.reporter_id
             WHERE r.item_id = ?1 AND r.resolved_at IS NULL AND r.reason <> 'student_data' AND ru.trust IN ('teacher', 'admin') AND ru.deleted_at IS NULL AND ru.suspended_at IS NULL
               -- after an admin release, the same reporters cannot re-hide it for THROTTLE_MS
               AND NOT EXISTS (SELECT 1 FROM reports d WHERE d.item_id = ?1 AND d.reporter_id = r.reporter_id AND d.action = 'dismissed' AND d.resolved_at > ?2 - ?4)) >= ?3`,
        )
        .bind(id, now, threshold, THROTTLE_MS),
    );
  stmts.push(audit(c.db, { actorId: me, actorKind: "user", action: "item.report", itemId: id, detail: { reason, report_id: rid } }, now));
  const res = await c.db.batch(stmts);
  // student_data: the admin is mailed immediately, outside the notice budget.
  // Admin mail for an instant hold goes through the pool's hourly alert budget (the hold and the flag do not).
  if (reason === "student_data" && (res[1]!.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "student_data_report", itemId: id, reportId: rid });
  if (reason === "student_data" && (res[1]!.meta.changes ?? 0) === 0 && (res[0]!.meta.changes ?? 0) === 1) {
    // The site-wide cap was reached (or the reporter is new / throttled): the admin is told once a day.
    const today = Math.floor(now / DAY_MS);
    const flagged = await c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) SELECT ?1, 'instant_hold_cap', ?2, ?3, ?4, ?5
                 WHERE (SELECT COUNT(*) FROM item_holds h WHERE h.reason = 'student_data' AND h.source = '' AND h.at >= ?6) >= ?7`)
      .bind(newId(), id, JSON.stringify({ report_id: rid }), `instant_hold_cap:${today}`, now, today * DAY_MS, await limitOf(c, "*", "instant_holds_global"))
      .run();
    if ((flagged.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "student_data_report", itemId: id, reportId: rid });
  }
  return json(202, { ok: true, message: "Thanks, the admin will review it." });
}

// ---- appeal ---------------------------------------------------------------------------------------

/**
 * POST /items/:id/appeal {note}: the author of an item an admin removed or a hold hides asks for a
 * review. It raises an admin flag (at most one a day per item) and says nothing else.
 */
export async function appeal(c: Ctx, req: Request, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me, true);
  const b = await readJson(req);
  const n = cleanText(b.note, 500);
  if (!n) throw new PoolError(400, "invalid_request", { field: "note" });
  const it = await c.db
    .prepare(`SELECT removed_reason, EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = items.id) AS held FROM items WHERE id = ?1 AND author_id = ?2 AND purged_at IS NULL`)
    .bind(id, me)
    .first<{ removed_reason: string | null; held: number }>();
  if (!it) return miss(c, me);
  if (it.removed_reason !== "admin" && !it.held) throw new PoolError(409, "nothing_to_appeal");
  await requireDaily(c, me, "appeals");
  const now = c.now();
  // One flag a day per item; the audit row only when a flag was really raised (no row spam).
  await c.db.batch([
    c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, user_id, detail, dedup, at) VALUES (?1, 'appeal', ?2, ?3, ?4, ?5, ?6)`)
      .bind(newId(), id, me, JSON.stringify({ note: n, removed_reason: it.removed_reason, held: it.held === 1 }), `appeal:${id}:${Math.floor(now / DAY_MS)}`, now),
    audit(c.db, { actorId: me, actorKind: "user", action: "item.appeal", itemId: id, targetUserId: me, note: n }, now, `changes() = 1`),
  ]);
  return json(202, { ok: true, message: "The admin will review your item." });
}

// ---- counter-notice (512(g)(3)) ---------------------------------------------------------------------

/** POST /items/:id/counter-notice: the author of a taken-down item. Incomplete notices are refused. */
export async function counterNotice(c: Ctx, req: Request, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await idOrMiss(c, id, me);
  const b = await readJson(req);
  const action = await c.db
    .prepare(
      `SELECT a.id, a.notice_id FROM dmca_actions a JOIN items i ON i.id = a.item_id
        WHERE a.item_id = ?1 AND a.author_id = ?2 AND i.author_id = ?2 AND i.removed_reason = 'takedown'
          AND a.restored_at IS NULL AND a.purged_at IS NULL AND a.counter_notice_id IS NULL
          AND a.author_notified_at > ?3 - ?4 -- the 30-day window ends with the quarantine
        ORDER BY a.disabled_at DESC LIMIT 1`,
    )
    .bind(id, me, c.now(), QUARANTINE_MS)
    .first<{ id: string; notice_id: string }>();
  if (!action) return miss(c, me);
  const missing: string[] = [];
  const text = (k: string, max: number) => {
    const v = cleanText(b[k], max);
    if (!v) missing.push(k);
    return v ?? "";
  };
  const tick = (k: string) => {
    if (b[k] !== true) missing.push(k);
  };
  const signature = text("signature", 200);
  const material = text("material_desc", 2000);
  const name = text("name", 200);
  const address = text("address", 500);
  const phone = text("phone", 50);
  tick("perjury_stmt");
  tick("consent_jurisdiction");
  tick("accept_service");
  if (missing.length > 0) throw new PoolError(400, "counter_notice_incomplete", { missing });
  const now = c.now();
  const cid = newId();
  const notBefore = addBusinessDays(now, 10) - DAY_MS + 1; // the start of the 10th business day after receipt
  const by = addBusinessDays(now, 14);
  const res = await c.db.batch([
    c.db
      .prepare(
        `INSERT INTO dmca_counter_notices (id, action_id, received_at, signature, material_desc, perjury_stmt, name, address, phone, consent_jurisdiction, accept_service, forwarded_at)
         SELECT ?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, 1, 1, ?3 WHERE NOT EXISTS (SELECT 1 FROM dmca_counter_notices WHERE action_id = ?2)`,
      )
      .bind(cid, action.id, now, signature, material, name, address, phone),
    c.db
      .prepare(`UPDATE dmca_actions SET counter_notice_id = ?2, restore_not_before = ?3, restore_by = ?4 WHERE id = ?1 AND counter_notice_id IS NULL AND changes() = 1`)
      .bind(action.id, cid, notBefore, by),
    audit(c.db, { actorId: me, actorKind: "user", action: "dmca.counter_notice", itemId: id, targetUserId: me, detail: { action_id: action.id } }, now, `changes() = 1`),
  ]);
  if ((res[1]!.meta.changes ?? 0) !== 1) return miss(c, me);
  const notice = await c.db.prepare(`SELECT complainant_contact FROM dmca_notices WHERE id = ?1`).bind(action.notice_id).first<{ complainant_contact: string }>();
  // Forwarded to the complainant at once, with the restore date.
  if (notice) send(c, { kind: "counter_notice", actionId: action.id, complainantContact: notice.complainant_contact, restoreNotBefore: notBefore });
  return json(202, { ok: true, restore_not_before: notBefore, restore_by: by });
}

// ---- machinery --------------------------------------------------------------------------------------

/** Deletes blobs nothing references and nobody has pending (rows first, then the bytes). */
export async function gcBlobs(c: Ctx, only?: string[]): Promise<number> {
  const now = c.now();
  const del = await c.db
    .prepare(
      `DELETE FROM blobs WHERE sha256 IN (
         SELECT b.sha256 FROM blobs b
          WHERE (?1 IS NULL OR b.sha256 IN (SELECT value FROM json_each(?1)))
            AND NOT EXISTS (SELECT 1 FROM item_blobs ib WHERE ib.sha256 = b.sha256)
            AND NOT EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b.sha256 AND pb.until >= ?2)
          LIMIT 500)
       RETURNING sha256`,
    )
    .bind(only ? JSON.stringify(only) : null, now - HOUR_MS)
    .all<{ sha256: string }>();
  const gone = del.results.map((r) => r.sha256);
  if (gone.length === 0) return 0;
  // An upload that landed in between wrote a live pending row (and re-inserts the blobs row after
  // its R2 put): keep those bytes. An expired pending row protects nothing.
  const back = await c.db
    .prepare(
      `SELECT sha256 FROM pending_blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND until >= ?2
       UNION SELECT sha256 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1))`,
    )
    .bind(JSON.stringify(gone), now - HOUR_MS)
    .all<{ sha256: string }>();
  const keep = new Set(back.results.map((r) => r.sha256));
  const drop = gone.filter((s) => !keep.has(s));
  if (drop.length > 0) await c.r2.delete(drop.map((s) => `blobs/${s}`));
  return drop.length;
}

/** Purge: every publish's bodies and derived rows; the items row stays as a tombstone. */
export async function purgeItem(c: Ctx, itemId: string, actor: { id: string | null; kind: "admin" | "system" }, why: string): Promise<boolean> {
  const now = c.now();
  // Claimed first, and only an item whose purge is due: a put-back clears purge_after when it claims
  // its action, so the two can never both proceed (critic A8). Keys are read only after the claim.
  const claim = await c.db
    .prepare(
      `UPDATE items SET purged_at = ?2, current_publish_id = NULL, prev_publish_id = NULL, purge_after = NULL
        WHERE id = ?1 AND purged_at IS NULL AND removed_at IS NOT NULL AND purge_after IS NOT NULL AND purge_after <= ?2`,
    )
    .bind(itemId, now)
    .run();
  if ((claim.meta.changes ?? 0) !== 1) return false;
  const pubs = await c.db
    .prepare(`SELECT publish_id, full_key, stripped_key FROM item_publishes WHERE item_id = ?1`)
    .bind(itemId)
    .all<{ publish_id: string; full_key: string; stripped_key: string | null }>();
  const ids = pubs.results.map((r) => r.publish_id);
  const shas = ids.length
    ? (await c.db.prepare(`SELECT DISTINCT sha256 FROM item_blobs WHERE publish_id IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(ids)).all<{ sha256: string }>()).results.map((r) => r.sha256)
    : [];
  const wasTakedown = (await c.db.prepare(`SELECT removed_reason FROM items WHERE id = ?1`).bind(itemId).first<{ removed_reason: string }>())?.removed_reason === "takedown";
  await c.db.batch([
    // An upheld takedown (purged without a put-back): what was copied from it is no longer licence
    // or attribution evidence (round 5 E3c): the rightful author is not bound to the infringer's
    // licence nor credited to them.
    ...(wasTakedown ? [voidEvidence(c.db, { itemId }, now, false), audit(c.db, { actorId: actor.id, actorKind: actor.kind, action: "evidence.void", itemId, detail: { why: "takedown_upheld" } }, now)] : []),
    ...deletePublishStatements(c.db, ids),
    c.db.prepare(`DELETE FROM question_ids WHERE item_id = ?1`).bind(itemId),
    c.db.prepare(`UPDATE dmca_actions SET purged_at = ?2, outcome = COALESCE(outcome, 'purged') WHERE item_id = ?1 AND restored_at IS NULL AND purged_at IS NULL`).bind(itemId, now),
    audit(c.db, { actorId: actor.id, actorKind: actor.kind, action: "item.purge", itemId, detail: { why } }, now),
  ]);
  const keys = pubs.results.flatMap((r) => [r.full_key, ...(r.stripped_key ? [r.stripped_key] : [])]);
  if (keys.length) await c.r2.delete(keys);
  if (shas.length) await gcBlobs(c, shas);
  return true;
}

/** Voids (or restores) the evidence recorded from an item, or from everything an account was served. */
export function voidEvidence(db: D1Database, who: { itemId?: string; userId?: string }, now: number, restore: boolean): D1PreparedStatement {
  const col = who.itemId ? "item_id" : "user_id";
  return restore
    ? db.prepare(`UPDATE import_questions SET void_at = NULL WHERE ${col} = ?1 AND void_at IS NOT NULL`).bind(who.itemId ?? who.userId)
    : db.prepare(`UPDATE import_questions SET void_at = ?2 WHERE ${col} = ?1 AND void_at IS NULL`).bind(who.itemId ?? who.userId, now);
}

/** Moves every retained body of `itemId` under quarantine/ (or back), rewriting the keys in D1. */
async function moveBodies(c: Ctx, itemId: string, toQuarantine: boolean): Promise<string[]> {
  const written: string[] = [];
  const pubs = await c.db
    .prepare(`SELECT publish_id, full_key, stripped_key FROM item_publishes WHERE item_id = ?1`)
    .bind(itemId)
    .all<{ publish_id: string; full_key: string; stripped_key: string | null }>();
  for (const r of pubs.results) {
    const dest = (form: string) => (toQuarantine ? `quarantine/${itemId}/${r.publish_id}/${form}.json` : `items/${itemId}/${newId()}.json`);
    const move = async (key: string | null, form: string): Promise<string | null> => {
      if (!key) return null;
      if (key.startsWith("quarantine/") === toQuarantine) return key;
      const o = await c.r2.get(key);
      if (!o) return key;
      const to = dest(form);
      await c.r2.put(to, await o.arrayBuffer(), { httpMetadata: { contentType: "application/json" } });
      written.push(to);
      return to;
    };
    const nf = await move(r.full_key, "full");
    const ns = await move(r.stripped_key, "stripped");
    await c.db.prepare(`UPDATE item_publishes SET full_key = ?2, stripped_key = ?3 WHERE publish_id = ?1`).bind(r.publish_id, nf, ns).run();
    const old = [r.full_key, r.stripped_key].filter((k, i): k is string => k !== null && k !== [nf, ns][i]);
    if (old.length) await c.r2.delete(old);
  }
  return written;
}

/**
 * Items that copy `itemId`: any other item whose current publish has one of its questions
 * (match_hash, whatever the key) or records it as an origin. Listed for the admin on a takedown.
 */
export async function lineageCopies(c: Ctx, itemId: string, designated: string[] | null = null, includeUnlinked = false): Promise<{ item_id: string; author_id: string; title: string; visible: boolean }[]> {
  // A copy must have lineage (its author imported the item, or recorded it as an origin) AND copy
  // the whole item or a designated question (critic D2 5): an independent item that only shares a
  // common question is not a copy. An admin can widen it explicitly (includeUnlinked).
  const r = await c.db
    .prepare(
      `SELECT i.id AS item_id, i.author_id, i.title, (i.removed_at IS NULL) AS visible FROM items i
        WHERE i.id <> ?1 AND i.purged_at IS NULL AND i.current_publish_id IS NOT NULL
          AND (
            EXISTS (SELECT 1 FROM item_questions o WHERE o.publish_id = i.current_publish_id AND o.origin_item_id = ?1)
            OR ((?3 = 1 OR EXISTS (SELECT 1 FROM imports im WHERE im.user_id = i.author_id AND im.item_id = ?1 AND im.kind = 'import'))
                AND (CASE WHEN ?2 IS NULL
                       THEN NOT EXISTS (SELECT 1 FROM item_questions src JOIN items s ON s.current_publish_id = src.publish_id WHERE s.id = ?1
                                         AND NOT EXISTS (SELECT 1 FROM item_questions cq WHERE cq.publish_id = i.current_publish_id AND cq.match_hash = src.match_hash))
                       ELSE EXISTS (SELECT 1 FROM item_questions cq WHERE cq.publish_id = i.current_publish_id AND cq.match_hash IN (SELECT value FROM json_each(?2))) END)))
        LIMIT 500`,
    )
    .bind(itemId, designated ? JSON.stringify(designated) : null, includeUnlinked ? 1 : 0)
    .all<{ item_id: string; author_id: string; title: string; visible: number }>();
  return r.results.map((x) => ({ ...x, visible: x.visible === 1 }));
}

/** A DMCA takedown: disabled at once (D1), then the bodies go to quarantine/. */
export async function takedown(c: Ctx, adminId: string, noticeId: string, itemId: string, plan: BlockPlan, blockUntil: number | null): Promise<{ actionId: string; copies: Awaited<ReturnType<typeof lineageCopies>> }> {
  const now = c.now();
  const notice = await c.db.prepare(`SELECT status, has_signature, has_good_faith, has_perjury_stmt FROM dmca_notices WHERE id = ?1`).bind(noticeId).first<{ status: string; has_signature: number; has_good_faith: number; has_perjury_stmt: number }>();
  if (!notice) throw new PoolError(404, "notice_not_found");
  if (notice.status === "deficient" || notice.status === "withdrawn") throw new PoolError(409, "notice_not_actionable");
  const item = await c.db
    .prepare(`SELECT author_id, current_publish_id, removed_reason, purge_after FROM items WHERE id = ?1 AND purged_at IS NULL`)
    .bind(itemId)
    .first<{ author_id: string; current_publish_id: string | null; removed_reason: string | null; purge_after: number | null }>();
  if (!item) throw new PoolError(404, "not_found");
  // A live item, or one its author deleted (the takedown takes over: undelete is refused while it
  // stands, and a put-back returns it to "deleted", never to live). Items removed by an admin, an
  // account deletion or a privacy purge are already disabled for good.
  if (item.removed_reason !== null && item.removed_reason !== "author_deleted") throw new PoolError(409, "already_removed");
  const actionId = newId();
  // Keyed on THIS action (not the time): two takedowns in one millisecond cannot both strike.
  const guard = `EXISTS (SELECT 1 FROM dmca_actions WHERE id = ?3)`;
  await c.db.batch([
    c.db
      .prepare(
        `UPDATE items SET removed_at = ?2, removed_reason = 'takedown', quarantined_at = ?2, purge_after = NULL, updated_at = ?2
          WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason = 'author_deleted')`,
      )
      .bind(itemId, now),
    c.db
      .prepare(`INSERT INTO dmca_actions (id, notice_id, item_id, author_id, disabled_at, author_notified_at, prev_removed_reason, prev_purge_after) SELECT ?3, ?4, ?1, ?5, ?2, ?2, ?6, ?7 WHERE changes() = 1`)
      .bind(itemId, now, actionId, noticeId, item.author_id, item.removed_reason, item.removed_reason ? item.purge_after : null),
    c.db.prepare(`INSERT INTO strikes (id, user_id, action_id, at, upheld_at) SELECT ?6, ?4, ?3, ?2, ?2 WHERE ${guard}`).bind(itemId, now, actionId, item.author_id, null, newId()),
    // Re-uploads of the designated (previewed) questions and images are held for review, until the
    // admin's chosen end or until lifted (critic D2 1/7). A lifted older row is taken over.
    c.db
      .prepare(
        `INSERT INTO blocked_hashes (kind, hash, action_id, at, expires_at) SELECT j.value ->> '$.k', j.value ->> '$.h', ?3, ?2, ?4 FROM json_each(?5) j WHERE ${guard}
         ON CONFLICT (kind, hash) DO UPDATE SET action_id = excluded.action_id, at = excluded.at, expires_at = excluded.expires_at, lifted_at = NULL WHERE blocked_hashes.lifted_at IS NOT NULL`,
      )
      .bind(itemId, now, actionId, blockUntil, JSON.stringify([...selectedMatch(plan).map((h) => ({ k: "match", h })), ...selectedBlobs(plan).map((h) => ({ k: "blob", h }))])),
    c.db.prepare(`UPDATE dmca_notices SET status = 'actioned' WHERE id = ?3 AND ${guard}`).bind(itemId, now, noticeId),
    audit(c.db, { actorId: adminId, actorKind: "admin", action: "dmca.takedown", itemId, targetUserId: item.author_id, detail: { notice_id: noticeId, action_id: actionId } }, now),
  ]);
  const ok = await c.db.prepare(`SELECT 1 AS x FROM dmca_actions WHERE id = ?1`).bind(actionId).first();
  if (!ok) throw new PoolError(409, "already_removed");
  await moveBodies(c, itemId, true);
  send(c, { kind: "takedown_author", itemId, authorId: item.author_id, actionId });
  const strikes = await c.db
    .prepare(`SELECT COUNT(*) AS n FROM strikes WHERE user_id = ?1 AND upheld_at IS NOT NULL AND voided_at IS NULL AND at > ?2`)
    .bind(item.author_id, now - STRIKE_WINDOW_MS)
    .first<{ n: number }>();
  if ((strikes?.n ?? 0) >= STRIKES_TO_SUSPEND) {
    // The repeat-infringer policy: the admin suspends through the audited /admin/suspend route.
    await c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, user_id, detail, dedup, at) VALUES (?1, 'repeat_infringer', ?2, ?3, ?4, ?5)`)
      .bind(newId(), item.author_id, JSON.stringify({ strikes: strikes!.n }), `strikes:${item.author_id}:${strikes!.n}`, now)
      .run();
    send(c, { kind: "repeat_infringer", userId: item.author_id, strikes: strikes!.n });
  }
  return { actionId, copies: await lineageCopies(c, itemId, selectedMatch(plan)) };
}

/** Put-back: bodies return from quarantine, the item shows again, the strike and blocklist entries are lifted. */
export async function restore(c: Ctx, actionId: string, actor: { id: string | null; kind: "admin" | "system" }, outcome: string): Promise<boolean> {
  const now = c.now();
  // Claim first (critic A8): exactly one put-back wins, and clearing purge_after stops a purge from
  // claiming the item while its bodies are moved back.
  const claim = await c.db.batch([
    c.db.prepare(`UPDATE dmca_actions SET restored_at = ?2, outcome = ?3 WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL`).bind(actionId, now, outcome),
    c.db
      .prepare(`UPDATE items SET purge_after = NULL WHERE id = (SELECT item_id FROM dmca_actions WHERE id = ?1) AND removed_reason = 'takedown' AND purged_at IS NULL AND changes() = 1`)
      .bind(actionId),
  ]);
  if ((claim[0]!.meta.changes ?? 0) !== 1) return false;
  const a = (await c.db
    .prepare(`SELECT item_id, prev_removed_reason, prev_purge_after, disabled_at FROM dmca_actions WHERE id = ?1`)
    .bind(actionId)
    .first<{ item_id: string; prev_removed_reason: string | null; prev_purge_after: number | null; disabled_at: number }>())!;
  if ((claim[1]!.meta.changes ?? 0) !== 1) {
    // The item was purged first: the action is not a put-back after all.
    await c.db.prepare(`UPDATE dmca_actions SET restored_at = NULL, outcome = NULL WHERE id = ?1 AND restored_at = ?2`).bind(actionId, now).run();
    return false;
  }
  const written = await moveBodies(c, a.item_id, false);
  const wasDeleted = a.prev_removed_reason === "author_deleted";
  await c.db.batch([
    c.db
      .prepare(
        `UPDATE items SET quarantined_at = NULL, updated_at = ?2,
            removed_at = CASE WHEN ?3 = 1 THEN removed_at ELSE NULL END,
            removed_reason = CASE WHEN ?3 = 1 THEN 'author_deleted' ELSE NULL END,
            purge_after = CASE WHEN ?3 = 1 THEN ?4 ELSE NULL END
          WHERE id = ?1 AND removed_reason = 'takedown' AND purged_at IS NULL`,
      )
      // The author's undo window resumes with what was left of it when the takedown began (never purged in the same run).
      .bind(a.item_id, now, wasDeleted ? 1 : 0, wasDeleted ? now + Math.max(DAY_MS, (a.prev_purge_after ?? now) - a.disabled_at) : null),
    c.db.prepare(`UPDATE strikes SET voided_at = ?2 WHERE action_id = ?1 AND voided_at IS NULL`).bind(actionId, now),
    c.db.prepare(`UPDATE blocked_hashes SET lifted_at = ?2 WHERE action_id = ?1 AND lifted_at IS NULL`).bind(actionId, now),
    audit(c.db, { actorId: actor.id, actorKind: actor.kind, action: "dmca.restore", itemId: a.item_id, detail: { action_id: actionId, outcome, back_to: wasDeleted ? "author_deleted" : "live" } }, now),
  ]);
  // Holds that only this action's re-upload blocks caused go with them; copy holds an admin placed
  // (hold-copies) stay, flagged so they are offered for release.
  await releaseOrphanHolds(c, `dmca:${actionId}`, "blocklist");
  const left = await c.db.prepare(`SELECT COUNT(*) AS n FROM item_holds WHERE source = ?1`).bind(`dmca:${actionId}`).first<{ n: number }>();
  if ((left?.n ?? 0) > 0)
    await c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, item_id, detail, dedup, at) VALUES (?1, 'dmca_copy', ?2, ?3, ?4, ?5)`)
      .bind(newId(), a.item_id, JSON.stringify({ action_id: actionId, held_copies: left!.n, hint: "POST /admin/dmca/release-copies" }), `putback:${actionId}`, now)
      .run();
  // A purge that claimed the item while the bodies were moving (e.g. a privacy purge) wins: anything
  // this put-back wrote is removed, so no body outlives its rows.
  const gone = await c.db.prepare(`SELECT purged_at FROM items WHERE id = ?1`).bind(a.item_id).first<{ purged_at: number | null }>();
  if (gone?.purged_at != null) {
    if (written.length) await c.r2.delete(written);
    return false;
  }
  return true;
}

/** Court action resolved for the complainant: the material stays down and is purged; the strike stands. */
export async function upholdTakedown(c: Ctx, adminId: string, actionId: string): Promise<boolean> {
  const now = c.now();
  const [upd] = await c.db.batch([
    c.db.prepare(`UPDATE dmca_actions SET outcome = 'court_upheld' WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL`).bind(actionId),
    c.db.prepare(`UPDATE items SET purge_after = ?2 WHERE id = (SELECT item_id FROM dmca_actions WHERE id = ?1) AND removed_reason = 'takedown' AND purged_at IS NULL AND changes() = 1`).bind(actionId, now),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) return false;
  const a = (await c.db.prepare(`SELECT item_id FROM dmca_actions WHERE id = ?1`).bind(actionId).first<{ item_id: string }>())!;
  return purgeItem(c, a.item_id, { id: adminId, kind: "admin" }, "court_upheld");
}

/** Admin confirmation of a student-data report: immediate purge, image hashes blocklisted, importers told "privacy". */
export async function privacyPurge(c: Ctx, adminId: string, itemId: string, plan: BlockPlan): Promise<boolean> {
  const now = c.now();
  const mh = selectedMatch(plan);
  const bh = selectedBlobs(plan);
  const [upd] = await c.db.batch([
    c.db
      .prepare(`UPDATE items SET removed_at = ?2, removed_reason = 'privacy', purge_after = ?2, updated_at = ?2 WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason <> 'privacy')`)
      .bind(itemId, now),
    // Its own table (C1), and only what the admin designated and previewed (D2): by default every
    // distinctive question and every image, never a question K other authors also use.
    c.db.prepare(`INSERT OR IGNORE INTO privacy_blocks (kind, hash, item_id, at, by) SELECT 'match', value, ?1, ?2, ?3 FROM json_each(?4)`).bind(itemId, now, adminId, JSON.stringify(mh)),
    c.db
      .prepare(`INSERT OR IGNORE INTO privacy_blocks (kind, hash, item_id, at, by, bytes_purge_after) SELECT 'blob', value, ?1, ?2, ?3, ?5 FROM json_each(?4)`)
      .bind(itemId, now, adminId, JSON.stringify(bh), now + PRIVACY_BYTES_GRACE_MS),
    // Exactly the previewed items are held (any state, so an undelete or put-back keeps them held).
    c.db
      .prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT value, 'student_data', ?1, ?2, 'privacy' FROM json_each(?3)`)
      .bind(now, adminId, JSON.stringify(plan.affected.map((a) => a.item_id))),
    // Every pending hold on those images goes too.
    c.db.prepare(`DELETE FROM pending_blobs WHERE sha256 IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(bh)),
    c.db.prepare(`UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'purged' WHERE item_id = ?1 AND resolved_at IS NULL`).bind(itemId, now, adminId),
    c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1`).bind(itemId),
    audit(c.db, { actorId: adminId, actorKind: "admin", action: "privacy.purge", itemId, detail: { questions: mh.length, images: bh.length, held: plan.affected.length } }, now),
  ]);
  if ((upd!.meta.changes ?? 0) !== 1) return false;
  // The student's image is served from no item at once (getBlob refuses privacy blocks); its bytes
  // are deleted only after PRIVACY_BYTES_GRACE_MS (cron), so an unblock in that window loses nothing
  // (round 5 E5). The purged item's own bodies go now.
  return purgeItem(c, itemId, { id: adminId, kind: "admin" }, "privacy");
}

export { idOrMiss };
