// Block plans for privacy purges and DMCA takedowns (critic D2): which question texts (match_hash)
// and images a block covers, chosen per question by the admin, never a common question by default,
// with a preview of every item the block would hold, confirmed by a short-lived token so the
// confirm acts on exactly what was previewed.
import { b64url, fromB64url, hmac, timingSafeEqual, utf8 } from "../../../auth/src/crypto.ts";
import { limitOf, PoolError, type Ctx } from "./shared.ts";

export interface BlockSelection {
  /** Question positions in the item's current publish; absent = every distinctive question. */
  questions?: number[];
  /** Image hashes; absent = every image of the item. */
  images?: string[];
}

export interface BlockPlan {
  item_id: string;
  match: { pos: number; hash: string; other_authors: number; introduced: boolean; selected: boolean }[];
  blob: { sha256: string; other_authors: number; introduced: boolean; selected: boolean }[];
  /** Other items (any state) the block would hold: they use a selected question or image. */
  affected: { item_id: string; author_id: string; title: string; via: "question" | "image" }[];
}

export const PREVIEW_MS = 10 * 60_000;

/** The plan for blocking `itemId` with `sel`, as of now. */
export async function blockPlan(c: Ctx, itemId: string, sel: BlockSelection): Promise<BlockPlan> {
  const item = await c.db
    .prepare(`SELECT author_id, current_publish_id, created_at FROM items WHERE id = ?1 AND purged_at IS NULL`)
    .bind(itemId)
    .first<{ author_id: string; current_publish_id: string | null; created_at: number }>();
  if (!item) throw new PoolError(404, "not_found");
  const k = await limitOf(c, "*", "block_common_k");
  // Each question's seq (who published it first): an item's own introductions are the default.
  const qs = await c.db
    .prepare(`SELECT iq.pos, iq.match_hash, qi.seq FROM item_questions iq LEFT JOIN question_ids qi ON qi.qid = iq.qid WHERE iq.publish_id = ?1 ORDER BY iq.pos`)
    .bind(item.current_publish_id)
    .all<{ pos: number; match_hash: string; seq: number | null }>();
  // Only what non-authors were SERVED (round 5 E4): an image only in a hidden publish's full form
  // is never blocked, so no block or hold can ever turn a guess at it into an oracle.
  const imgs = await c.db
    .prepare(
      `SELECT DISTINCT ib.sha256 FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id
        WHERE p.item_id = ?1 AND ib.form = CASE WHEN p.hide_key = 1 THEN 'stripped' ELSE 'full' END`,
    )
    .bind(itemId)
    .all<{ sha256: string }>();
  // How many OTHER authors' VISIBLE items (live, public) use each question / image, and the
  // earliest of them: unlisted or removed items (sock puppets) never influence the default.
  const visible = `i.removed_at IS NULL AND i.visibility = 'public' AND i.purged_at IS NULL AND i.author_id <> ?2`;
  const stats = async (sql: string, hashes: string[]) => {
    if (hashes.length === 0) return new Map<string, { n: number; first: number | null }>();
    const r = await c.db.prepare(sql).bind(JSON.stringify(hashes), item.author_id).all<{ h: string; n: number; first: number | null }>();
    return new Map(r.results.map((x) => [x.h, { n: x.n, first: x.first }]));
  };
  const mCommon = await stats(
    `SELECT iq.match_hash AS h, COUNT(DISTINCT i.author_id) AS n, MIN(qi.seq) AS first FROM item_questions iq
       JOIN items i ON i.current_publish_id = iq.publish_id LEFT JOIN question_ids qi ON qi.qid = iq.qid
      WHERE iq.match_hash IN (SELECT value FROM json_each(?1)) AND ${visible} GROUP BY iq.match_hash`,
    qs.results.map((x) => x.match_hash),
  );
  const bCommon = await stats(
    `SELECT ib.sha256 AS h, COUNT(DISTINCT i.author_id) AS n, MIN(i.created_at) AS first FROM item_blobs ib
       JOIN items i ON i.current_publish_id = ib.publish_id
      WHERE ib.sha256 IN (SELECT value FROM json_each(?1)) AND ${visible}
        AND ib.form = CASE WHEN i.hide_key = 1 THEN 'stripped' ELSE 'full' END GROUP BY ib.sha256`,
    imgs.results.map((x) => x.sha256),
  );
  if (sel.questions && !sel.questions.every((p) => Number.isInteger(p) && p >= 0 && p < qs.results.length)) throw new PoolError(400, "invalid_request", { field: "questions" });
  const known = new Set(imgs.results.map((x) => x.sha256));
  if (sel.images && !sel.images.every((h) => known.has(h))) throw new PoolError(400, "invalid_request", { field: "images" });
  // Default: only what this item introduced (no visible item of another author had it first) and
  // what is distinctive (fewer than K other authors). An explicit selection overrides both.
  const match = qs.results.map((q) => {
    const st = mCommon.get(q.match_hash);
    const other = st?.n ?? 0;
    const copied = st?.first != null && q.seq != null && st.first < q.seq;
    return { pos: q.pos, hash: q.match_hash, other_authors: other, introduced: !copied, selected: sel.questions ? sel.questions.includes(q.pos) : other < k && !copied };
  });
  const blob = imgs.results.map((b) => {
    const st = bCommon.get(b.sha256);
    const other = st?.n ?? 0;
    const copied = st?.first != null && st.first < item.created_at;
    return { sha256: b.sha256, other_authors: other, introduced: !copied, selected: sel.images ? sel.images.includes(b.sha256) : other < k && !copied };
  });
  const mh = [...new Set(match.filter((x) => x.selected).map((x) => x.hash))];
  const bh = blob.filter((x) => x.selected).map((x) => x.sha256);
  const aff = await c.db
    .prepare(
      `SELECT DISTINCT i.id AS item_id, i.author_id, i.title,
              CASE WHEN EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id WHERE p.item_id = i.id AND iq.match_hash IN (SELECT value FROM json_each(?2)))
                   THEN 'question' ELSE 'image' END AS via
         FROM items i WHERE i.id <> ?1 AND i.purged_at IS NULL AND EXISTS (SELECT 1 FROM item_publishes p WHERE p.item_id = i.id AND (
           EXISTS (SELECT 1 FROM item_questions iq WHERE iq.publish_id = p.publish_id AND iq.match_hash IN (SELECT value FROM json_each(?2)))
           OR EXISTS (SELECT 1 FROM item_blobs ib WHERE ib.publish_id = p.publish_id AND ib.sha256 IN (SELECT value FROM json_each(?3))
                        AND ib.form = CASE WHEN p.hide_key = 1 THEN 'stripped' ELSE 'full' END)))
        LIMIT 500`,
    )
    .bind(itemId, JSON.stringify(mh), JSON.stringify(bh))
    .all<{ item_id: string; author_id: string; title: string; via: "question" | "image" }>();
  return { item_id: itemId, match, blob, affected: aff.results };
}

export const selectedMatch = (p: BlockPlan) => [...new Set(p.match.filter((x) => x.selected).map((x) => x.hash))];
export const selectedBlobs = (p: BlockPlan) => p.blob.filter((x) => x.selected).map((x) => x.sha256);

const digest = (kind: string, p: BlockPlan) => JSON.stringify([kind, p.item_id, selectedMatch(p).sort(), selectedBlobs(p).sort(), p.affected.map((a) => a.item_id).sort()]);

/** A token binding a confirm to exactly the previewed plan, valid PREVIEW_MS. */
export async function previewToken(c: Ctx, kind: string, p: BlockPlan): Promise<string> {
  const exp = c.now() + PREVIEW_MS;
  const key = (await c.keys).preview;
  const body = b64url(utf8(JSON.stringify([exp, await hmac(key, digest(kind, p))])));
  return `${body}.${await hmac(key, body)}`;
}

/** Throws 428 unless `token` was issued for this very plan (same selection, same affected items) and is unexpired. */
export async function checkPreview(c: Ctx, kind: string, p: BlockPlan, token: unknown): Promise<void> {
  const fail = () => new PoolError(428, "preview_required", { message: "Preview the block first and confirm with its confirm_preview token (it binds the confirm to what you saw)." });
  if (typeof token !== "string") throw fail();
  const m = /^([A-Za-z0-9_-]{1,400})\.([A-Za-z0-9_-]{43})$/.exec(token);
  const key = (await c.keys).preview;
  const same = (a: string, b: string) => a.length === b.length && timingSafeEqual(utf8(a), utf8(b));
  if (!m || !same(await hmac(key, m[1]!), m[2]!)) throw fail();
  let exp = 0;
  let h = "";
  try {
    [exp, h] = JSON.parse(new TextDecoder().decode(fromB64url(m[1]!))) as [number, string];
  } catch {
    throw fail();
  }
  if (exp < c.now() || !same(h, await hmac(key, digest(kind, p)))) throw fail();
}

/** Reads a selection from an admin request body. */
export function readSelection(b: Record<string, unknown> | null): BlockSelection {
  const sel: BlockSelection = {};
  if (b?.questions !== undefined) {
    if (!Array.isArray(b.questions) || b.questions.length > 1000) throw new PoolError(400, "invalid_request", { field: "questions" });
    sel.questions = b.questions as number[];
  }
  if (b?.images !== undefined) {
    if (!Array.isArray(b.images) || b.images.length > 1000 || !b.images.every((x) => typeof x === "string")) throw new PoolError(400, "invalid_request", { field: "images" });
    sel.images = b.images as string[];
  }
  return sel;
}

/**
 * Holds of `source` (reason `reason`) on items that no longer use any still-active block of that
 * source are released: what an unblock leaves behind.
 */
export async function releaseOrphanHolds(c: Ctx, source: string, reason: string): Promise<number> {
  const blocks =
    source === "privacy"
      ? `SELECT kind, hash FROM privacy_blocks`
      : `SELECT kind, hash FROM blocked_hashes WHERE action_id = '${source.slice(5).replace(/[^0-9a-f]/g, "")}' AND lifted_at IS NULL AND (expires_at IS NULL OR expires_at > ${c.now()})`;
  const r = await c.db
    .prepare(
      `DELETE FROM item_holds WHERE source = ?1 AND reason = ?2 AND NOT EXISTS (
         SELECT 1 FROM item_publishes p WHERE p.item_id = item_holds.item_id AND (
           EXISTS (SELECT 1 FROM item_questions iq JOIN (${blocks}) bl ON bl.kind = 'match' AND bl.hash = iq.match_hash WHERE iq.publish_id = p.publish_id)
           OR EXISTS (SELECT 1 FROM item_blobs ib JOIN (${blocks}) bl ON bl.kind = 'blob' AND bl.hash = ib.sha256 WHERE ib.publish_id = p.publish_id)))`,
    )
    .bind(source, reason)
    .run();
  return r.meta.changes ?? 0;
}
