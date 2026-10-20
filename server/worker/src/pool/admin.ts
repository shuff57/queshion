// Pool admin routes, served behind the auth package's admin guard (admin trust, a known device, and a
// sudo token for every write). The guard only matches exact paths, so ids travel in the body or the
// query string. Every write is audited twice: pool_audit and the auth package's auth_audit.
import type { AdminRoute } from "../../../auth/src/index.js";
import { cleanText } from "../../../auth/src/validate.ts";
import { audit, DEFAULT_LIMITS, ID_RE, json, limitOf, METRICS, newId, PoolError, SHARED_SQL, usageSql, type Ctx, type Metric } from "./shared.ts";
import { blockPlan, checkPreview, PREVIEW_MS, previewToken, readSelection, releaseOrphanHolds, selectedBlobs, selectedMatch } from "./blocking.ts";
import { voidEvidence } from "./moderation.ts";
import { lineageCopies, privacyPurge, purgeItem, restore, takedown, UNDO_MS, upholdTakedown } from "./moderation.ts";

const HOLD_REASONS = ["reports", "student_data", "blocklist", "admin"] as const;

function id(v: unknown, field: string): string {
  if (typeof v !== "string" || !ID_RE.test(v)) throw new PoolError(400, "invalid_request", { field });
  return v;
}
function note(v: unknown): string | null {
  if (v === undefined || v === null || v === "") return null;
  const s = cleanText(v, 1000);
  if (!s) throw new PoolError(400, "invalid_request", { field: "note" });
  return s;
}
const fail = (e: unknown): Response => {
  if (e instanceof PoolError) return json(e.status, { error: e.code, ...e.extra }, e.headers);
  throw e;
};

export function poolAdminRoutes(ctx: () => Ctx | null): AdminRoute[] {
  const wrap = (r: AdminRoute): AdminRoute => ({
    ...r,
    handler: async (req, h) => {
      try {
        if (!ctx()) return json(503, { error: "pool_not_configured" });
        return await r.handler(req, h);
      } catch (e) {
        return fail(e);
      }
    },
  });
  const C = () => ctx()!;
  const routes: AdminRoute[] = [
    {
      method: "GET",
      path: "/admin/pool/reports",
      write: false,
      // Open reports grouped by item, oldest first, with the reporters and their history.
      handler: async () => {
        const rows = await C()
          .db.prepare(
            `SELECT r.id, r.item_id, r.reason, r.note, r.at, r.reporter_id, ru.display_name AS reporter_name,
                    (SELECT COUNT(*) FROM reports r2 WHERE r2.reporter_id = r.reporter_id) AS reporter_reports,
                    (SELECT COUNT(*) FROM reports r3 WHERE r3.reporter_id = r.reporter_id AND r3.action = 'dismissed') AS reporter_dismissed,
                    i.title, i.author_id, (SELECT json_group_array(reason) FROM item_holds h WHERE h.item_id = r.item_id) AS holds
               FROM reports r JOIN items i ON i.id = r.item_id LEFT JOIN users ru ON ru.id = r.reporter_id
              WHERE r.resolved_at IS NULL ORDER BY r.at LIMIT 500`,
          )
          .all<Record<string, unknown>>();
        const by = new Map<string, { item_id: string; title: unknown; author_id: unknown; holds: unknown; reports: unknown[] }>();
        for (const r of rows.results) {
          const k = r.item_id as string;
          if (!by.has(k)) by.set(k, { item_id: k, title: r.title, author_id: r.author_id, holds: JSON.parse((r.holds as string) ?? "[]"), reports: [] });
          by.get(k)!.reports.push({ id: r.id, reason: r.reason, note: r.note, at: r.at, reporter_id: r.reporter_id, reporter_name: r.reporter_name, reporter_reports: r.reporter_reports, reporter_dismissed: r.reporter_dismissed });
        }
        return json(200, { items: [...by.values()] });
      },
    },
    {
      method: "GET",
      path: "/admin/pool/flags",
      write: false,
      handler: async () => {
        const rows = await C().db.prepare(`SELECT id, kind, item_id, publish_id, user_id, detail, at FROM admin_flags WHERE resolved_at IS NULL ORDER BY at LIMIT 500`).all();
        return json(200, { flags: rows.results });
      },
    },
    {
      method: "GET",
      path: "/admin/pool/item",
      write: false,
      // The full form, in any state (quarantine included). Rendered by the admin console as plain text.
      handler: async (req) => {
        const c = C();
        const itemId = id(new URL(req.url).searchParams.get("id"), "id");
        const item = await c.db.prepare(`SELECT * FROM items WHERE id = ?1`).bind(itemId).first<Record<string, unknown>>();
        if (!item) throw new PoolError(404, "not_found");
        const pubs = await c.db
          .prepare(`SELECT publish_id, version, status, hide_key, license, attested_at, attest_text_rev, full_key, committed_at FROM item_publishes WHERE item_id = ?1 ORDER BY version DESC`)
          .bind(itemId)
          .all<{ publish_id: string; full_key: string }>();
        const holds = await c.db.prepare(`SELECT reason, at, by FROM item_holds WHERE item_id = ?1`).bind(itemId).all();
        const current = pubs.results.find((p) => p.publish_id === item.current_publish_id);
        const body = current ? await c.r2.get(current.full_key) : null;
        const head = JSON.stringify({ item, publishes: pubs.results.map(({ full_key: _k, ...rest }) => rest), holds: holds.results });
        const payload = body ? await body.text() : "null";
        return new Response(`${head.slice(0, -1)},"payload_full":${payload}}`, {
          headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff", "Referrer-Policy": "no-referrer", Vary: "Authorization, Cookie, Origin" },
        });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/hold",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const n = note(h.body?.note);
        const [ins] = await c.db.batch([
          c.db.prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT ?1, 'admin', ?2, ?3, 'manual' WHERE EXISTS (SELECT 1 FROM items WHERE id = ?1)`).bind(itemId, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "item.hold", itemId, note: n }, h.now),
          h.audit({ action: "pool.hold", targetId: itemId, note: n }),
        ]);
        return json(200, { ok: true, changed: (ins!.meta.changes ?? 0) === 1 });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/release",
      write: true,
      // Lifting a hold; for student_data it is the admin rejecting the report.
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const reason = h.body?.reason;
        if (typeof reason !== "string" || !(HOLD_REASONS as readonly string[]).includes(reason)) throw new PoolError(400, "invalid_request", { field: "reason" });
        const n = note(h.body?.note);
        const resolve =
          reason === "student_data"
            ? `reason = 'student_data'`
            : reason === "reports"
              ? `reason <> 'student_data'`
              : `0`;
        await c.db.batch([
          c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1 AND reason = ?2`).bind(itemId, reason),
          c.db.prepare(`UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'dismissed' WHERE item_id = ?1 AND resolved_at IS NULL AND ${resolve}`).bind(itemId, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "item.release", itemId, detail: { reason }, note: n }, h.now),
          h.audit({ action: "pool.release", targetId: itemId, detail: { reason }, note: n }),
        ]);
        return json(200, { ok: true });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/dismiss",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const rid = id(h.body?.report_id, "report_id");
        const n = note(h.body?.note);
        const [upd] = await c.db.batch([
          c.db.prepare(`UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'dismissed' WHERE id = ?1 AND resolved_at IS NULL`).bind(rid, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "report.dismiss", detail: { report_id: rid }, note: n }, h.now),
          h.audit({ action: "pool.report_dismiss", targetId: rid, note: n }),
        ]);
        return json((upd!.meta.changes ?? 0) === 1 ? 200 : 404, (upd!.meta.changes ?? 0) === 1 ? { ok: true } : { error: "not_found" });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/remove",
      write: true,
      // Removal for other policy reasons: hidden now, purged after 14 days.
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const n = note(h.body?.note);
        const [upd] = await c.db.batch([
          c.db
            .prepare(
              `UPDATE items SET prior_removed_reason = removed_reason, prior_purge_after = purge_after, removed_at = COALESCE(removed_at, ?2),
                 removed_reason = 'admin', purge_after = ?3, updated_at = ?2 WHERE id = ?1 AND purged_at IS NULL AND (removed_reason IS NULL OR removed_reason = 'author_deleted')`,
            )
            .bind(itemId, h.now, h.now + UNDO_MS),
          c.db.prepare(`UPDATE reports SET resolved_at = ?2, resolved_by = ?3, action = 'removed' WHERE item_id = ?1 AND resolved_at IS NULL AND changes() = 1`).bind(itemId, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "item.remove", itemId, note: n }, h.now),
          h.audit({ action: "pool.remove", targetId: itemId, note: n }),
        ]);
        return (upd!.meta.changes ?? 0) === 1 ? json(200, { ok: true }) : json(409, { error: "not_removable" });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/unremove",
      write: true,
      // Reverses an admin removal within its 14 days, back to what it replaced (live, or the
      // author's own deletion). Holds stay; a privacy block on its content adds one; going live is
      // subject to the author's quota and question cap, like an undelete (critic C6).
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const n = note(h.body?.note);
        const it = await c.db.prepare(`SELECT author_id FROM items WHERE id = ?1 AND removed_reason = 'admin' AND purge_after > ?2 AND purged_at IS NULL`).bind(itemId, h.now).first<{ author_id: string }>();
        if (!it) return json(404, { error: "not_found" });
        const quota = await limitOf(c, it.author_id, "storage_bytes");
        const cap = await limitOf(c, it.author_id, "shared_questions");
        const [upd] = await c.db.batch([
          c.db
            .prepare(
              `UPDATE items SET removed_reason = prior_removed_reason, purge_after = prior_purge_after,
                 removed_at = CASE WHEN prior_removed_reason IS NULL THEN NULL ELSE removed_at END,
                 prior_removed_reason = NULL, prior_purge_after = NULL, updated_at = ?2
               WHERE id = ?3 AND author_id = ?1 AND removed_reason = 'admin' AND purge_after > ?2 AND purged_at IS NULL
                 AND (prior_removed_reason IS NOT NULL OR (${usageSql("?3")} <= ?4 AND ${SHARED_SQL("?1", "?3")} + question_count <= ?5))`,
            )
            .bind(it.author_id, h.now, itemId, quota, cap),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "item.unremove", itemId, targetUserId: it.author_id, note: n }, h.now, `changes() = 1`),
          c.db
            .prepare(
              `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
               SELECT ?1, 'student_data', ?2, ?3, 'privacy' WHERE changes() = 1 AND EXISTS (
                 SELECT 1 FROM item_publishes p WHERE p.item_id = ?1 AND (
                   EXISTS (SELECT 1 FROM item_questions iq JOIN privacy_blocks pb ON pb.kind = 'match' AND pb.hash = iq.match_hash WHERE iq.publish_id = p.publish_id)
                   OR EXISTS (SELECT 1 FROM item_blobs ib JOIN privacy_blocks pb ON pb.kind = 'blob' AND pb.hash = ib.sha256 WHERE ib.publish_id = p.publish_id)))`,
            )
            .bind(itemId, h.now, h.admin.userId),
          h.audit({ action: "pool.unremove", targetId: itemId, note: n }),
        ]);
        if ((upd!.meta.changes ?? 0) === 1) return json(200, { ok: true });
        return json(409, { error: "over_limits", message: "Restoring it would take the author over their storage quota or question cap." });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/purge-preview",
      write: true,
      // What a privacy purge would block and hold; the purge must quote its confirm_preview token.
      handler: async (_req, h) => {
        const c = C();
        const plan = await blockPlan(c, id(h.body?.item_id, "item_id"), readSelection(h.body));
        return json(200, { ...plan, confirm_preview: await previewToken(c, "privacy", plan), expires_in: PREVIEW_MS / 1000 });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/takedown-preview",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const plan = await blockPlan(c, id(h.body?.item_id, "item_id"), readSelection(h.body));
        return json(200, { ...plan, copies: await lineageCopies(c, plan.item_id, selectedMatch(plan)), confirm_preview: await previewToken(c, "dmca", plan), expires_in: PREVIEW_MS / 1000 });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/evidence",
      write: true,
      // Void or restore licence/attribution evidence: everything copied from an item, or everything
      // an account was served (e.g. an infringer's item, or a sock puppet's reads). Audited.
      handler: async (_req, h) => {
        const c = C();
        const itemId = h.body?.item_id === undefined ? undefined : id(h.body.item_id, "item_id");
        const userId = h.body?.user_id;
        if (userId !== undefined && (typeof userId !== "string" || !/^[0-9a-f-]{36}$/.test(userId))) throw new PoolError(400, "invalid_request", { field: "user_id" });
        if ((itemId === undefined) === (userId === undefined)) throw new PoolError(400, "invalid_request", { field: "item_id" });
        const act = h.body?.action;
        if (act !== "void" && act !== "restore") throw new PoolError(400, "invalid_request", { field: "action" });
        const n = note(h.body?.note);
        const [upd] = await c.db.batch([
          voidEvidence(c.db, { itemId, userId: userId as string | undefined }, h.now, act === "restore"),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: `evidence.${act}`, itemId: itemId ?? null, targetUserId: (userId as string | undefined) ?? null, note: n }, h.now),
          h.audit({ action: `pool.evidence_${act}`, targetId: itemId ?? (userId as string), note: n }),
        ]);
        return json(200, { ok: true, rows: upd!.meta.changes ?? 0 });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/unblock",
      write: true,
      // Lifts one privacy block (a false positive); holds that only it caused are released.
      handler: async (_req, h) => {
        const c = C();
        const kind = h.body?.kind;
        const hash = h.body?.hash;
        if ((kind !== "match" && kind !== "blob") || typeof hash !== "string" || !/^[0-9a-f]{64}$/.test(hash)) throw new PoolError(400, "invalid_request", { field: "hash" });
        const n = note(h.body?.note);
        const blk = await c.db.prepare(`SELECT bytes_purged_at FROM privacy_blocks WHERE kind = ?1 AND hash = ?2`).bind(kind, hash).first<{ bytes_purged_at: number | null }>();
        if (!blk) return json(404, { error: "not_found" });
        // Never a silent loss (round 5 E5): within the grace window the bytes are intact and the unblock
        // restores everything; after it, every copy that used the image is held as needs_reupload,
        // flagged for the admin, and named in the answer.
        const lost = blk.bytes_purged_at !== null;
        const needs = lost
          ? (
              await c.db
                .prepare(
                  `SELECT DISTINCT i.id FROM items i JOIN item_blobs ib ON ib.publish_id = i.current_publish_id WHERE ib.sha256 = ?1 AND i.purged_at IS NULL`,
                )
                .bind(hash)
                .all<{ id: string }>()
            ).results.map((r) => r.id)
          : [];
        await c.db.batch([
          c.db.prepare(`DELETE FROM privacy_blocks WHERE kind = ?1 AND hash = ?2`).bind(kind, hash),
          c.db
            .prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT value, 'admin', ?1, ?2, 'needs_reupload' FROM json_each(?3)`)
            .bind(h.now, h.admin.userId, JSON.stringify(needs)),
          c.db
            .prepare(`INSERT INTO admin_flags (id, kind, item_id, detail, at) SELECT ?1, 'needs_reupload', NULL, ?2, ?3 WHERE ?4 > 0`)
            .bind(newId(), JSON.stringify({ hash, items: needs }), h.now, needs.length),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "privacy.unblock", detail: { kind, hash, bytes_lost: lost, needs_reupload: needs.length }, note: n }, h.now),
          h.audit({ action: "pool.privacy_unblock", targetId: hash, note: n }),
        ]);
        return json(200, { ok: true, released: await releaseOrphanHolds(c, "privacy", "student_data"), bytes_lost: lost, needs_reupload: needs });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/unblock",
      write: true,
      // Lifts an action's re-upload blocks (all, or one hash), upheld or not; holds that only they caused are released.
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const hash = h.body?.hash;
        if (hash !== undefined && (typeof hash !== "string" || !/^[0-9a-f]{64}$/.test(hash))) throw new PoolError(400, "invalid_request", { field: "hash" });
        const n = note(h.body?.note);
        const [upd] = await c.db.batch([
          c.db.prepare(`UPDATE blocked_hashes SET lifted_at = ?2 WHERE action_id = ?1 AND lifted_at IS NULL AND (?3 IS NULL OR hash = ?3)`).bind(aid, h.now, hash ?? null),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.unblock", detail: { action_id: aid, hash: hash ?? "all" }, note: n }, h.now, `changes() > 0`),
          h.audit({ action: "pool.dmca_unblock", targetId: aid, note: n }),
        ]);
        return json(200, { ok: true, lifted: upd!.meta.changes ?? 0, released: await releaseOrphanHolds(c, `dmca:${aid}`, "blocklist") });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/purge",
      write: true,
      // Confirmed student data: purged immediately (no quarantine), image hashes blocklisted.
      handler: async (_req, h) => {
        const c = C();
        const itemId = id(h.body?.item_id, "item_id");
        const n = note(h.body?.note);
        await c.db.batch([h.audit({ action: "pool.purge_privacy", targetId: itemId, note: n })]);
        const plan = await blockPlan(c, itemId, readSelection(h.body));
        await checkPreview(c, "privacy", plan, h.body?.confirm_preview);
        const ok = await privacyPurge(c, h.admin.userId, itemId, plan);
        return ok ? json(200, { ok: true, blocked: { questions: selectedMatch(plan).length, images: selectedBlobs(plan).length }, held: plan.affected.map((a) => a.item_id) }) : json(409, { error: "not_purgeable" });
      },
    },
    {
      method: "POST",
      path: "/admin/pool/resolve-flag",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const fid = id(h.body?.flag_id, "flag_id");
        await c.db.batch([
          c.db.prepare(`UPDATE admin_flags SET resolved_at = ?2, resolved_by = ?3 WHERE id = ?1 AND resolved_at IS NULL`).bind(fid, h.now, h.admin.userId),
          h.audit({ action: "pool.flag_resolve", targetId: fid }),
        ]);
        return json(200, { ok: true });
      },
    },
    {
      method: "PUT",
      path: "/admin/pool/limits",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const user = h.body?.user_id;
        if (user !== "*" && (typeof user !== "string" || !/^[0-9a-f-]{36}$/.test(user))) throw new PoolError(400, "invalid_request", { field: "user_id" });
        const metric = h.body?.metric;
        if (typeof metric !== "string" || !(METRICS as string[]).includes(metric)) throw new PoolError(400, "invalid_request", { field: "metric" });
        const value = h.body?.value;
        if (typeof value !== "number" || !Number.isInteger(value) || value < 0 || value > 1e12) throw new PoolError(400, "invalid_request", { field: "value" });
        await c.db.batch([
          c.db
            .prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (user_id, metric) DO UPDATE SET value = ?3, updated_at = ?4, updated_by = ?5`)
            .bind(user, metric, value, h.now, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "limits.set", targetUserId: user === "*" ? null : user, detail: { metric, value, default: DEFAULT_LIMITS[metric as Metric] } }, h.now),
          h.audit({ action: "pool.limits", targetId: user === "*" ? null : user, detail: { metric, value } }),
        ]);
        return json(200, { ok: true });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/notices",
      write: true,
      // One field or tick per 512(c)(3)(A) element; a notice missing one is stored as deficient.
      handler: async (_req, h) => {
        const c = C();
        const b = h.body ?? {};
        const text = (k: string, max: number) => {
          const v = cleanText(b[k], max);
          if (!v) throw new PoolError(400, "invalid_request", { field: k });
          return v;
        };
        const name = text("complainant_name", 200);
        const contact = text("complainant_contact", 500);
        const work = text("work_desc", 2000);
        const material = text("material_desc", 2000);
        const sig = b.has_signature === true;
        const gf = b.has_good_faith === true;
        const pj = b.has_perjury_stmt === true;
        const nid = newId();
        const status = sig && gf && pj ? "open" : "deficient";
        await c.db.batch([
          c.db
            .prepare(
              `INSERT INTO dmca_notices (id, received_at, complainant_name, complainant_contact, work_desc, material_desc, has_signature, has_good_faith, has_perjury_stmt, status, recorded_by)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)`,
            )
            .bind(nid, h.now, name, contact, work, material, sig ? 1 : 0, gf ? 1 : 0, pj ? 1 : 0, status, h.admin.userId),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.notice", detail: { notice_id: nid, status } }, h.now),
          h.audit({ action: "pool.dmca_notice", targetId: nid, detail: { status } }),
        ]);
        return json(201, { notice_id: nid, status });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/takedown",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const nid = id(h.body?.notice_id, "notice_id");
        const itemId = id(h.body?.item_id, "item_id");
        const plan = await blockPlan(c, itemId, readSelection(h.body));
        await checkPreview(c, "dmca", plan, h.body?.confirm_preview);
        const days = h.body?.block_days;
        if (days !== undefined && days !== null && (typeof days !== "number" || !Number.isInteger(days) || days < 1 || days > 3650)) throw new PoolError(400, "invalid_request", { field: "block_days" });
        await c.db.batch([h.audit({ action: "pool.takedown", targetId: itemId, detail: { notice_id: nid, block_days: days ?? null } })]);
        const r = await takedown(c, h.admin.userId, nid, itemId, plan, typeof days === "number" ? h.now + days * 86_400_000 : null);
        // Copies (same questions, or recorded as derived from it) are listed; "hold-copies" hides them.
        return json(200, { ok: true, action_id: r.actionId, copies: r.copies });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/court-action",
      write: true,
      // The complainant's notice of a court action keeps the material disabled until resolved.
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const [upd] = await c.db.batch([
          c.db.prepare(`UPDATE dmca_actions SET court_action_at = ?2 WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL AND court_action_at IS NULL`).bind(aid, h.now),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.court_action", detail: { action_id: aid } }, h.now),
          h.audit({ action: "pool.dmca_court_action", targetId: aid }),
        ]);
        return (upd!.meta.changes ?? 0) === 1 ? json(200, { ok: true }) : json(404, { error: "not_found" });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/resolve",
      write: true,
      // The way out of a court action: the court sided with the author (restore) or the complainant (remove).
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const outcome = h.body?.outcome;
        if (outcome !== "restore" && outcome !== "remove") throw new PoolError(400, "invalid_request", { field: "outcome" });
        const n = note(h.body?.note);
        const act = await c.db.prepare(`SELECT court_action_at FROM dmca_actions WHERE id = ?1 AND restored_at IS NULL AND purged_at IS NULL`).bind(aid).first<{ court_action_at: number | null }>();
        if (!act) return json(404, { error: "not_found" });
        await c.db.batch([
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.resolve", detail: { action_id: aid, outcome }, note: n }, h.now),
          h.audit({ action: "pool.dmca_resolve", targetId: aid, detail: { outcome }, note: n }),
        ]);
        const ok = outcome === "restore" ? await restore(c, aid, { id: h.admin.userId, kind: "admin" }, "court_restore") : await upholdTakedown(c, h.admin.userId, aid);
        return ok ? json(200, { ok: true }) : json(409, { error: "not_resolvable" });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/hold-copies",
      write: true,
      // One click: hold every copy of the taken-down item, tagged with the action.
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const a = await c.db.prepare(`SELECT item_id FROM dmca_actions WHERE id = ?1`).bind(aid).first<{ item_id: string }>();
        if (!a) return json(404, { error: "not_found" });
        const designated = Array.isArray(h.body?.questions) ? ((h.body!.questions as unknown[]).filter((x) => typeof x === "string" && /^[0-9a-f]{64}$/.test(x)) as string[]) : null;
        const copies = await lineageCopies(c, a.item_id, designated, h.body?.include_unlinked === true);
        await c.db.batch([
          c.db
            .prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT value, 'admin', ?2, ?3, ?1 FROM json_each(?4)`)
            .bind(`dmca:${aid}`, h.now, h.admin.userId, JSON.stringify(copies.map((x) => x.item_id))),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.hold_copies", itemId: a.item_id, detail: { action_id: aid, copies: copies.length } }, h.now),
          h.audit({ action: "pool.dmca_hold_copies", targetId: aid, detail: { copies: copies.length } }),
        ]);
        return json(200, { ok: true, held: copies.map((x) => x.item_id) });
      },
    },
    {
      method: "GET",
      path: "/admin/dmca/copies",
      write: false,
      // The copies held because of an action (offered for release together after a put-back).
      handler: async (req) => {
        const c = C();
        const aid = id(new URL(req.url).searchParams.get("action_id"), "action_id");
        const a = await c.db.prepare(`SELECT item_id, restored_at FROM dmca_actions WHERE id = ?1`).bind(aid).first<{ item_id: string; restored_at: number | null }>();
        if (!a) return json(404, { error: "not_found" });
        const held = await c.db.prepare(`SELECT item_id, reason FROM item_holds WHERE source = ?1`).bind(`dmca:${aid}`).all();
        return json(200, { restored: a.restored_at !== null, held: held.results, candidates: await lineageCopies(c, a.item_id) });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/release-copies",
      write: true,
      handler: async (_req, h) => {
        const c = C();
        const aid = id(h.body?.action_id, "action_id");
        const [del] = await c.db.batch([
          c.db.prepare(`DELETE FROM item_holds WHERE source = ?1`).bind(`dmca:${aid}`),
          audit(c.db, { actorId: h.admin.userId, actorKind: "admin", action: "dmca.release_copies", detail: { action_id: aid } }, h.now),
          h.audit({ action: "pool.dmca_release_copies", targetId: aid }),
        ]);
        return json(200, { ok: true, released: del!.meta.changes ?? 0 });
      },
    },
    {
      method: "POST",
      path: "/admin/dmca/withdraw",
      write: true,
      // A withdrawn notice: everything taken down under it is put back.
      handler: async (_req, h) => {
        const c = C();
        const nid = id(h.body?.notice_id, "notice_id");
        await c.db.batch([
          c.db.prepare(`UPDATE dmca_notices SET status = 'withdrawn' WHERE id = ?1`).bind(nid),
          h.audit({ action: "pool.dmca_withdraw", targetId: nid }),
        ]);
        const acts = await c.db.prepare(`SELECT id FROM dmca_actions WHERE notice_id = ?1 AND restored_at IS NULL AND purged_at IS NULL`).bind(nid).all<{ id: string }>();
        for (const a of acts.results) await restore(c, a.id, { id: h.admin.userId, kind: "admin" }, "withdrawn");
        return json(200, { ok: true, restored: acts.results.length });
      },
    },
  ];
  return routes.map(wrap);
}

export { purgeItem };
