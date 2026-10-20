// The pool's scheduled work. Ages, not states, guard in-flight work (docs/shared-pool.md "GC").
import { gcBlobs, purgeItem, QUARANTINE_MS, restore } from "./moderation.ts";
import { deletePublishStatements, discardQidsStatement } from "./publish.ts";
import { DAY_MS, HOUR_MS, type Ctx } from "./shared.ts";

export interface HourlyReport {
  staged: number;
  pending: number;
  blobs: number;
  missing: number;
}
export const RECONCILE_BATCH = 200;

/** Hourly: abandoned staged publishes, expired pending uploads, unreferenced blobs. */
export async function hourly(c: Ctx): Promise<HourlyReport> {
  const now = c.now();
  const stale = await c.db
    .prepare(`SELECT publish_id, item_id, full_key, stripped_key, created_at FROM item_publishes WHERE status IN ('staging', 'staged') AND created_at < ?1 LIMIT 200`)
    .bind(now - HOUR_MS)
    .all<{ publish_id: string; item_id: string; full_key: string; stripped_key: string | null; created_at: number }>();
  let staged = 0;
  for (const r of stale.results) {
    const del = await c.db.prepare(`DELETE FROM item_publishes WHERE publish_id = ?1 AND status IN ('staging', 'staged') AND created_at = ?2`).bind(r.publish_id, r.created_at).run();
    if ((del.meta.changes ?? 0) !== 1) continue;
    await c.db.batch([...deletePublishStatements(c.db, [r.publish_id]), discardQidsStatement(c.db, r.publish_id, r.item_id)]);
    await c.r2.delete([r.full_key, ...(r.stripped_key ? [r.stripped_key] : [])]);
    staged++;
  }
  const pend = await c.db.prepare(`DELETE FROM pending_blobs WHERE until < ?1`).bind(now - HOUR_MS).run();
  const blobs = await gcBlobs(c);
  // Reconciler: a blob row whose bytes are gone (a GC that raced an upload) is marked missing, so
  // the next publish names the positions and the author uploads again; found again, it is cleared.
  const check = await c.db.prepare(`SELECT sha256 FROM blobs ORDER BY COALESCE(checked_at, 0) LIMIT ?1`).bind(RECONCILE_BATCH).all<{ sha256: string }>();
  let missing = 0;
  for (const r of check.results) {
    const there = (await c.r2.head(`blobs/${r.sha256}`)) !== null;
    if (!there) missing++;
    await c.db
      .prepare(`UPDATE blobs SET checked_at = ?2, missing_at = CASE WHEN ?3 = 1 THEN NULL ELSE COALESCE(missing_at, ?2) END WHERE sha256 = ?1`)
      .bind(r.sha256, now, there ? 1 : 0)
      .run();
  }
  return { staged, pending: pend.meta.changes ?? 0, blobs, missing };
}

export interface DailyReport {
  purged: number;
  restored: number;
  quarantineExpired: number;
}

/** Daily: DMCA put-backs and quarantine expiry, then the purge of everything past purge_after. */
export async function daily(c: Ctx): Promise<DailyReport> {
  const now = c.now();
  // A complete counter-notice and no court action: restored no sooner than 10 business days after receipt.
  const due = await c.db
    .prepare(`SELECT id FROM dmca_actions WHERE counter_notice_id IS NOT NULL AND restore_not_before <= ?1 AND court_action_at IS NULL AND restored_at IS NULL AND purged_at IS NULL LIMIT 100`)
    .bind(now)
    .all<{ id: string }>();
  let restored = 0;
  for (const a of due.results) if (await restore(c, a.id, { id: null, kind: "system" }, "put_back")) restored++;
  // No counter-notice within 30 days of notifying the author: purge.
  const expired = await c.db
    .prepare(
      `UPDATE items SET purge_after = ?1 WHERE removed_reason = 'takedown' AND purged_at IS NULL AND purge_after IS NULL
         AND id IN (SELECT item_id FROM dmca_actions WHERE counter_notice_id IS NULL AND court_action_at IS NULL AND restored_at IS NULL AND purged_at IS NULL AND author_notified_at <= ?2)`,
    )
    .bind(now, now - QUARANTINE_MS)
    .run();
  // An account deletion whose content hook failed after the tombstone committed: caught up here.
  await c.db
    .prepare(
      `UPDATE items SET removed_at = ?1, removed_reason = 'account_deleted', purge_after = ?2, updated_at = ?1
        WHERE removed_at IS NULL AND anonymized = 0 AND author_id IN (SELECT id FROM users WHERE deleted_at IS NOT NULL)`,
    )
    .bind(now, now + 14 * DAY_MS)
    .run();
  // Privacy-blocked images past their grace window: the bytes go, every copy is marked missing.
  const doomed = await c.db
    .prepare(`UPDATE privacy_blocks SET bytes_purged_at = ?1 WHERE kind = 'blob' AND bytes_purged_at IS NULL AND bytes_purge_after <= ?1 RETURNING hash`)
    .bind(now)
    .all<{ hash: string }>();
  if (doomed.results.length > 0) {
    const hs = doomed.results.map((r) => r.hash);
    await c.db.prepare(`UPDATE blobs SET missing_at = ?2 WHERE sha256 IN (SELECT value FROM json_each(?1))`).bind(JSON.stringify(hs), now).run();
    await c.r2.delete(hs.map((h) => `blobs/${h}`));
  }
  const items = await c.db.prepare(`SELECT id FROM items WHERE purge_after IS NOT NULL AND purge_after <= ?1 AND purged_at IS NULL AND removed_at IS NOT NULL LIMIT 200`).bind(now).all<{ id: string }>();
  let purged = 0;
  for (const it of items.results) if (await purgeItem(c, it.id, { id: null, kind: "system" }, "purge_after")) purged++;
  await c.db.batch([
    c.db.prepare(`DELETE FROM usage_daily WHERE day < ?1`).bind(Math.floor(now / DAY_MS) - 2),
    c.db.prepare(`DELETE FROM pool_alert_budget WHERE hour < ?1`).bind(Math.floor(now / HOUR_MS) - 48),
  ]);
  return { purged, restored, quarantineExpired: expired.meta.changes ?? 0 };
}
