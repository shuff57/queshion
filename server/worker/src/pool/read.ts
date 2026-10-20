// Every read path. One rule decides what a caller is served (servedForm): the author gets the full
// form; anyone else gets the stripped form of a hide_key publish and the full form otherwise. Every
// non-author read joins items.current_publish_id and the author's standing (VISIBLE_TO_OTHERS).
// full_sha256 / full_content / full_key never leave this module for a non-author of a hide_key publish.
import { allowedForAll, LICENSES, type Grant } from "./license.ts";
import { admissible, authorContext, grantsFor, importHits } from "./provenance.ts";
import { subjectFor } from "./visible.ts";
import {
  countMiss,
  DEFAULT_LIMITS,
  ID_RE,
  json,
  missGuard,
  notFound,
  PoolError,
  rawJson,
  readCursor,
  readOptionalJson,
  requireDaily,
  SHA_RE,
  signCursor,
  usage,
  limitOf,
  READ_COST,
  reserveBytes,
  USAGE_SQL,
  DAY_MS,
  visibleTo,
  VISIBLE_TO_OTHERS,
  type Ctx,
  type PoolPrincipal,
} from "./shared.ts";

const KINDS: [string, number][] = [
  ["Mc", 1],
  ["Tf", 2],
  ["Matching", 4],
  ["Blank", 8],
  ["Short", 16],
  ["Multipart", 32],
];
const kindsOf = (mask: number) => KINDS.filter(([, b]) => mask & b).map(([k]) => k);

interface ItemRow {
  id: string;
  author_id: string;
  kind: string;
  visibility: string;
  version: number;
  title: string;
  subject: string;
  course: string;
  grade: string;
  kinds_mask: number;
  question_count: number;
  license: string;
  has_images: number;
  committed_at: number;
  current_publish_id: string;
  full_key: string;
  stripped_key: string | null;
  p_hide: number;
  p_license: string;
  p_version: number;
  p_public_version: number;
  public_at: number | null;
  public_publish_id: string;
  schema_version: number;
  author_name: string | null;
}

/** The item and its current publish, iff the caller may see it (one statement). */
async function loadVisible(c: Ctx, id: string, me: string): Promise<ItemRow | null> {
  return c.db
    .prepare(
      `SELECT i.id, i.author_id, i.kind, i.visibility, i.version, i.title, i.subject, i.course, i.grade, i.kinds_mask, i.question_count,
              i.license, i.has_images, i.committed_at, i.current_publish_id,
              p.full_key, p.stripped_key, p.hide_key AS p_hide, p.license AS p_license, p.version AS p_version, p.schema_version,
              p.public_version AS p_public_version, i.public_at, COALESCE(i.public_publish_id, i.current_publish_id) AS public_publish_id,
              u.display_name AS author_name
         FROM items i JOIN users u ON u.id = i.author_id JOIN item_publishes p ON p.publish_id = i.current_publish_id
        WHERE i.id = ?1 AND ${visibleTo("?2")}`,
    )
    .bind(id, me)
    .first<ItemRow>();
}

/** THE served-form rule. */
export function servedForm(row: { author_id: string; p_hide: number }, me: string): "full" | "stripped" {
  return row.author_id !== me && row.p_hide === 1 ? "stripped" : "full";
}

function itemView(r: ItemRow, me: string, form: "full" | "stripped") {
  const isAuthor = r.author_id === me;
  return {
    id: r.id,
    kind: r.kind,
    title: r.title,
    subject: r.subject,
    course: r.course,
    grade: r.grade,
    kinds: kindsOf(r.kinds_mask),
    question_count: r.question_count,
    // Non-authors see the public version: it does not move for a change they are not served.
    version: isAuthor ? r.p_version : r.p_public_version,
    // Non-authors only ever see the public publish id: a hidden-only republish moves nothing (D4).
    publish_id: isAuthor ? r.current_publish_id : r.public_publish_id,
    license: r.p_license,
    hide_key: r.p_hide === 1,
    has_images: r.has_images === 1,
    author_name: r.author_name ?? "deleted teacher",
    committed_at: isAuthor ? r.committed_at : (r.public_at ?? r.committed_at),
    form,
    is_author: isAuthor,
    ...(isAuthor ? { visibility: r.visibility } : {}),
  };
}

interface QRow {
  qid: string;
  pos: number;
  match_hash: string;
  full_sha256: string;
  stripped_sha256: string | null;
  chain_json: string;
}

/** Per-question provenance for the served form (never the full hash of a stripped serve). */
async function questionsFor(c: Ctx, publishId: string, form: "full" | "stripped"): Promise<object[]> {
  const rows = await c.db
    .prepare(`SELECT qid, pos, match_hash, full_sha256, stripped_sha256, chain_json FROM item_questions WHERE publish_id = ?1 ORDER BY pos`)
    .bind(publishId)
    .all<QRow>();
  // Chains store the display name at publish time; an author who has since deleted their account is
  // shown as "deleted teacher" (resolved now, from the item's author, for every link).
  const chains = rows.results.map((q) => JSON.parse(q.chain_json) as { author_name: string; item_id: string }[]);
  const linked = [...new Set(chains.flat().map((l) => l.item_id).filter((x) => ID_RE.test(x)))];
  const gone = new Set<string>();
  if (linked.length > 0) {
    const r = await c.db
      .prepare(`SELECT i.id FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND u.deleted_at IS NOT NULL`)
      .bind(JSON.stringify(linked))
      .all<{ id: string }>();
    for (const x of r.results) gone.add(x.id);
  }
  return rows.results.map((q, k) => ({
    pos: q.pos,
    qid: q.qid,
    served_sha256: form === "stripped" ? q.stripped_sha256 : q.full_sha256,
    match_hash: q.match_hash,
    chain: chains[k]!.map((l) => (gone.has(l.item_id) ? { ...l, author_name: "deleted teacher" } : l)),
  }));
}

/** The stored body's R2 object (a handle: its bytes are only streamed, never held whole). */
async function body(c: Ctx, key: string | null): Promise<R2ObjectBody | null> {
  if (!key) return null;
  return c.r2.get(key);
}

/**
 * `{...head, "payload": <the stored canonical bytes, unchanged>}` as a STREAM (round 5 E6): the head,
 * then the R2 body piped through, then "}". Nothing concatenates the body into a string; the read's
 * byte reservation is released when the stream ends or is cancelled.
 */
function streamedPayload(head: Record<string, unknown>, obj: R2ObjectBody, release: () => void): Response {
  const h = JSON.stringify(head);
  const pre = new TextEncoder().encode(`${h.slice(0, -1)}${h.length > 2 ? "," : ""}"payload":`);
  const tail = new TextEncoder().encode("}");
  const src = obj.body.getReader();
  let started = false;
  const stream = new ReadableStream<Uint8Array>({
    async pull(ctrl) {
      try {
        if (!started) {
          started = true;
          ctrl.enqueue(pre);
          return;
        }
        const { done, value } = await src.read();
        if (done) {
          ctrl.enqueue(tail);
          ctrl.close();
          release();
        } else ctrl.enqueue(value);
      } catch (e) {
        release();
        ctrl.error(e);
      }
    },
    cancel(reason) {
      release();
      return src.cancel(reason);
    },
  });
  return new Response(stream, { status: 200, headers: rawJson(200, "").headers });
}

async function itemIdOrMiss(c: Ctx, id: string, me: string): Promise<void> {
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

// ---- GET /items/:id ---------------------------------------------------------------------------------

export async function getItem(c: Ctx, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await itemIdOrMiss(c, id, me);
  await requireDaily(c, me, "item_reads");
  const release = reserveBytes(READ_COST);
  try {
    const r = await loadVisible(c, id, me);
    if (!r) return await miss(c, me);
    const form = servedForm(r, me);
    const obj = await body(c, form === "stripped" ? r.stripped_key : r.full_key);
    if (obj === null) return await miss(c, me);
    // A read is licence evidence too (critic X6): a verbatim copy of something only read is still a
    // copy, so the hard check must see it. Recorded as a 'view', which never counts as an import.
    if (r.author_id !== me) await recordServed(c, me, r, form, "view");
    const questions = await questionsFor(c, r.current_publish_id, form);
    return streamedPayload({ item: { ...itemView(r, me, form), ...(await tagsFor(c, r.current_publish_id)) }, questions }, obj, release);
  } catch (e) {
    release();
    throw e;
  }
}

// ---- POST /items/:id/import -------------------------------------------------------------------------

export async function importItem(c: Ctx, req: Request, p: PoolPrincipal, id: string): Promise<Response> {
  const me = p.userId;
  await itemIdOrMiss(c, id, me);
  await readOptionalJson(req);
  await requireDaily(c, me, "imports");
  await requireDaily(c, me, "item_reads");
  const release = reserveBytes(READ_COST);
  try {
    return await importInner(c, me, id, release);
  } catch (e) {
    release();
    throw e;
  }
}

async function importInner(c: Ctx, me: string, id: string, release: () => void): Promise<Response> {
  const r = await loadVisible(c, id, me);
  if (!r) return miss(c, me);
  const form = servedForm(r, me);
  const isAuthor = r.author_id === me;
  // circle-only material is copied only inside its circles (phase 3); in phase 2 only its author.
  if (!isAuthor && r.p_license === "circle-only") return miss(c, me);
  let images: { held: number; until?: number; reason?: string } = { held: 0 };
  const before = await c.db
    .prepare(`SELECT version FROM imports WHERE user_id = ?1 AND item_id = ?2 AND kind = 'import' ORDER BY version`)
    .bind(me, id)
    .all<{ version: number }>();
  const obj = await body(c, form === "stripped" ? r.stripped_key : r.full_key);
  if (obj === null) return miss(c, me);
  if (!isAuthor) {
    // Recorded only if the item is STILL visible after the body was read: a takedown or hold that
    // landed meanwhile turns the import into a 404 with nothing recorded (one atomic batch).
    const res = await recordServed(c, me, r, form, "import");
    if (!res) return miss(c, me);
    images = await holdImportedImages(c, me, r.current_publish_id, form);
  }
  const questions = await questionsFor(c, r.current_publish_id, form);
  const head = {
    item: { ...itemView(r, me, form), ...(await tagsFor(c, r.current_publish_id)) },
    questions,
    license: r.p_license,
    // The same function the publish check uses, over the same grants, for a verbatim copy (critic A4).
    may_republish_as: isAuthor ? [...LICENSES] : await mayRepublishAs(c, me, r.current_publish_id, form),
    previously_imported_versions: before.results.map((x) => x.version),
    // Copy-on-import (IMG-1): the images served are held for this account (charged to its quota)
    // for IMPORT_IMAGE_HOLD_MS, so a republish of the copy needs no re-upload even if the source changes.
    images,
    recorded: !isAuthor,
  };
  return streamedPayload(head, obj, release);
}

// ---- POST /items/versions ---------------------------------------------------------------------------

export async function versions(c: Ctx, req: Request, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  const b = await readOptionalJson(req);
  const ids = b.ids;
  if (!Array.isArray(ids) || ids.length === 0 || ids.length > 200 || !ids.every((x) => typeof x === "string" && x.length <= 64)) throw new PoolError(400, "invalid_request", { field: "ids" });
  await requireDaily(c, me, "item_reads");
  const uniq = [...new Set(ids as string[])].filter((x) => ID_RE.test(x));
  const rows = await c.db
    .prepare(
      `SELECT j.value AS id, i.author_id, CASE WHEN i.author_id = ?1 THEN i.version ELSE i.public_version END AS version, i.removed_reason,
              (SELECT MAX(version) FROM imports im WHERE im.user_id = ?1 AND im.item_id = j.value AND im.kind = 'import') AS mine,
              EXISTS (SELECT 1 FROM imports im WHERE im.user_id = ?1 AND im.item_id = j.value) AS seen,
              EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = j.value AND h.source = 'privacy') AS privacy_held,
              CASE WHEN i.id IS NULL THEN 0
                   WHEN i.author_id = ?1 THEN (i.removed_at IS NULL AND i.current_publish_id IS NOT NULL)
                   ELSE (${VISIBLE_TO_OTHERS}) END AS visible
         FROM json_each(?2) j LEFT JOIN items i ON i.id = j.value LEFT JOIN users u ON u.id = i.author_id`,
    )
    .bind(me, JSON.stringify(uniq))
    .all<{ id: string; author_id: string | null; version: number | null; removed_reason: string | null; mine: number | null; seen: number; privacy_held: number; visible: number }>();
  const by = new Map(rows.results.map((r) => [r.id, r]));
  const out = (ids as string[]).map((id) => {
    const r = by.get(id);
    // Only for an item the caller authored or imported: no oracle for arbitrary ids.
    // Only for an item the caller authored, imported or read: no oracle for arbitrary ids. A preview
    // read never counts as having the newest version (it must not hide an update notice).
    if (!r || r.author_id === null || (r.author_id !== me && !r.seen)) return { id, status: "unavailable" };
    // A copy held because it carries confirmed student data: the client is told to delete its copy.
    if (r.author_id !== me && r.privacy_held) return { id, status: "removed", reason: "privacy" };
    if (r.removed_reason !== null) {
      const reason = r.removed_reason === "takedown" ? "takedown" : r.removed_reason === "privacy" ? "privacy" : "deleted";
      return { id, status: "removed", reason };
    }
    if (!r.visible) return { id, status: "unavailable" };
    // "newer" only against an imported copy; a caller who only read it holds no copy to update.
    if (r.author_id !== me && r.mine !== null && r.mine < (r.version ?? 0)) return { id, status: "newer", version: r.version };
    return { id, status: "current", version: r.version };
  });
  return json(200, { items: out });
}

// ---- GET /items -------------------------------------------------------------------------------------

export async function list(c: Ctx, url: URL, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  const q = url.searchParams;
  const scope = q.get("scope") ?? "public";
  if (scope !== "public" && scope !== "mine") throw new PoolError(400, "invalid_request", { field: "scope" });
  const limitRaw = q.get("limit");
  const limit = limitRaw === null ? 20 : Number(limitRaw);
  if (!Number.isInteger(limit) || limit < 1) throw new PoolError(400, "invalid_request", { field: "limit" });
  const lim = Math.min(limit, 50);
  const subject = q.get("subject");
  if (subject !== null && (subject.length === 0 || subject.length > 100)) throw new PoolError(400, "invalid_request", { field: "subject" });
  const kind = q.get("kind");
  const bit = kind === null ? 0 : (KINDS.find(([k]) => k === kind)?.[1] ?? -1);
  if (bit < 0) throw new PoolError(400, "invalid_request", { field: "kind" });
  const tagQ = q.get("tag");
  if (tagQ !== null && (tagQ.length === 0 || tagQ.length > 64)) throw new PoolError(400, "invalid_request", { field: "tag" });
  const tag = tagQ?.trim().toLowerCase() ?? null;
  const stdQ = q.get("standard");
  if (stdQ !== null && (stdQ.length === 0 || stdQ.length > 64)) throw new PoolError(400, "invalid_request", { field: "standard" });
  const standard = stdQ?.replace(/\s+/g, " ").trim().toUpperCase() ?? null;
  const prefix = q.get("q");
  if (prefix !== null && (prefix.length === 0 || prefix.length > 100)) throw new PoolError(400, "invalid_request", { field: "q" });
  // A case-insensitive title prefix (LIKE is ASCII-case-insensitive in SQLite); % and _ escaped.
  const titleLike = prefix === null ? null : `${prefix.replace(/[\\%_]/g, (x) => `\\${x}`)}%`;
  let cur: [number, string] | null = null;
  const cs = q.get("cursor");
  if (cs !== null) {
    cur = await readCursor(c, cs);
    if (!cur) throw new PoolError(400, "invalid_request", { field: "cursor" });
  }
  await requireDaily(c, me, "browse");
  // Visibility, holds, removal, author standing, the filters and the LIMIT are one statement.
  const where = [scope === "mine" ? `i.author_id = ?1 AND i.removed_at IS NULL AND i.current_publish_id IS NOT NULL` : `i.visibility = 'public' AND ${visibleTo("?1")}`];
  // Pages on the immutable creation time (a republish never moves an item across a cursor, PAGE-1).
  where.push(
    `(?2 IS NULL OR i.subject = ?2 COLLATE NOCASE)`,
    `(?3 = 0 OR (i.kinds_mask & ?3) <> 0)`,
    `(?4 IS NULL OR i.created_at < ?4 OR (i.created_at = ?4 AND i.id < ?5))`,
    `(?7 IS NULL OR EXISTS (SELECT 1 FROM item_tags t WHERE t.publish_id = i.current_publish_id AND t.tag = ?7))`,
    `(?8 IS NULL OR EXISTS (SELECT 1 FROM item_standards s WHERE s.publish_id = i.current_publish_id AND s.code = ?8))`,
    `(?9 IS NULL OR i.title LIKE ?9 ESCAPE '\\')`,
  );
  const rows = await c.db
    .prepare(
      `SELECT i.id, i.kind, i.title, i.subject, i.course, i.grade, i.kinds_mask, i.question_count,
              CASE WHEN i.author_id = ?1 THEN i.version ELSE i.public_version END AS version, i.hide_key, i.license,
              i.has_images, CASE WHEN i.author_id = ?1 THEN i.committed_at ELSE COALESCE(i.public_at, i.committed_at) END AS committed_at,
              i.created_at, i.visibility, i.author_id, u.display_name AS author_name
         FROM items i JOIN users u ON u.id = i.author_id
        WHERE ${where.join(" AND ")}
        ORDER BY i.created_at DESC, i.id DESC LIMIT ?6`,
    )
    .bind(me, subject, bit, cur?.[0] ?? null, cur?.[1] ?? "", lim + 1, tag, standard, titleLike)
    .all<{ id: string; kind: string; title: string; subject: string; course: string; grade: string; kinds_mask: number; question_count: number; version: number; hide_key: number; license: string; has_images: number; committed_at: number; created_at: number; visibility: string; author_id: string; author_name: string | null }>();
  const page = rows.results.slice(0, lim);
  const next = rows.results.length > lim ? await signCursor(c, page[page.length - 1]!.created_at, page[page.length - 1]!.id) : null;
  return json(200, {
    items: page.map((r) => ({
      id: r.id,
      kind: r.kind,
      title: r.title,
      subject: r.subject,
      course: r.course,
      grade: r.grade,
      kinds: kindsOf(r.kinds_mask),
      question_count: r.question_count,
      version: r.version,
      hide_key: r.hide_key === 1,
      license: r.license,
      has_images: r.has_images === 1,
      author_name: r.author_name ?? "deleted teacher",
      committed_at: r.committed_at,
      ...(r.author_id === me ? { visibility: r.visibility, is_author: true } : {}),
    })),
    next_cursor: next,
  });
}

// ---- GET /items/mine --------------------------------------------------------------------------------

export async function mine(c: Ctx, p: PoolPrincipal): Promise<Response> {
  const me = p.userId;
  const rows = await c.db
    .prepare(
      `SELECT i.id, i.kind, i.title, i.subject, i.version, i.visibility, i.license, i.hide_key, i.question_count, i.committed_at,
              i.removed_at, i.removed_reason, i.purge_after, i.current_publish_id,
              EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id) AS held,
              EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id AND h.source = 'needs_reupload') AS needs_reupload,
              (SELECT COUNT(DISTINCT im.user_id) FROM imports im WHERE im.item_id = i.id AND im.user_id <> i.author_id AND im.kind = 'import') AS import_count,
              (SELECT a.id FROM dmca_actions a WHERE a.item_id = i.id AND a.restored_at IS NULL AND a.purged_at IS NULL ORDER BY a.disabled_at DESC LIMIT 1) AS action_id,
              (SELECT a.counter_notice_id FROM dmca_actions a WHERE a.item_id = i.id AND a.restored_at IS NULL AND a.purged_at IS NULL ORDER BY a.disabled_at DESC LIMIT 1) AS counter_notice_id
         FROM items i WHERE i.author_id = ?1 AND i.purged_at IS NULL
        ORDER BY i.updated_at DESC, i.id DESC LIMIT 1000`,
    )
    .bind(me)
    .all<{ id: string; kind: string; title: string; subject: string; version: number; visibility: string; license: string; hide_key: number; question_count: number; committed_at: number | null; removed_at: number | null; removed_reason: string | null; purge_after: number | null; current_publish_id: string | null; held: number; needs_reupload: number; import_count: number; action_id: string | null; counter_notice_id: string | null }>();
  const now = c.now();
  const items = rows.results
    .filter((r) => r.current_publish_id !== null)
    .map((r) => {
      let state = "live";
      const extra: Record<string, unknown> = {};
      if (r.removed_reason === "author_deleted") {
        state = "deleted";
        if (r.purge_after !== null && r.purge_after > now) extra.undo_until = r.purge_after;
      } else if (r.removed_reason === "takedown") {
        state = "taken_down"; // "taken down after a copyright notice", with the counter-notice form
        extra.counter_notice = r.counter_notice_id !== null ? "received" : r.action_id !== null ? "available" : "closed";
      } else if (r.removed_reason === "privacy") state = "removed_privacy";
      else if (r.removed_reason !== null) {
        state = "removed";
        extra.removed_reason = r.removed_reason;
        if (r.removed_reason === "admin") extra.appeal = "available";
      }
      // A hold never says why or who reported: "held for review".
      else if (r.needs_reupload) state = "needs_reupload"; // an image was lost: upload it again and republish
      else if (r.held) {
        state = "held_for_review";
        extra.appeal = "available";
      }
      return {
        id: r.id,
        kind: r.kind,
        title: r.title,
        subject: r.subject,
        version: r.version,
        visibility: r.visibility,
        license: r.license,
        hide_key: r.hide_key === 1,
        question_count: r.question_count,
        committed_at: r.committed_at,
        import_count: r.import_count,
        state,
        ...extra,
      };
    });
  return json(200, { items, storage: { used: await usage(c, me), limit: await limitOf(c, me, "storage_bytes") } });
}

// ---- GET /items/:id/blobs/:sha256 -------------------------------------------------------------------

const EXT: Record<string, string> = { "image/png": "png", "image/jpeg": "jpg", "image/gif": "gif", "image/webp": "webp" };

export async function getBlob(c: Ctx, p: PoolPrincipal, id: string, sha: string): Promise<Response> {
  const me = p.userId;
  await missGuard(c, me, "blob_miss");
  const missBlob = async (): Promise<never> => {
    await countMiss(c, me, "blob_miss");
    throw notFound();
  };
  if (!ID_RE.test(id) || !SHA_RE.test(sha)) return missBlob();
  // Readable iff the caller may see the item AND the blob is listed under the caller's served form of
  // the CURRENT publish. A miss and a forbidden read are the same 404.
  const row = await c.db
    .prepare(
      `SELECT b.content_type FROM items i JOIN users u ON u.id = i.author_id
         JOIN item_publishes p ON p.publish_id = i.current_publish_id
         JOIN item_blobs ib ON ib.publish_id = i.current_publish_id AND ib.sha256 = ?3
              AND ib.form = CASE WHEN i.author_id = ?2 OR p.hide_key = 0 THEN 'full' ELSE 'stripped' END
         JOIN blobs b ON b.sha256 = ib.sha256
        WHERE i.id = ?1 AND ${visibleTo("?2")}
          -- An image confirmed as student data is served to nobody, from any item (critic A5).
          AND NOT EXISTS (SELECT 1 FROM privacy_blocks pb WHERE pb.kind = 'blob' AND pb.hash = ?3)`,
    )
    .bind(id, me, sha)
    .first<{ content_type: string }>();
  if (!row) return missBlob();
  const o = await c.r2.get(`blobs/${sha}`);
  if (!o) return missBlob();
  return new Response(o.body, {
    status: 200,
    headers: {
      // Only ever the type verified from the bytes at upload.
      "Content-Type": row.content_type,
      "X-Content-Type-Options": "nosniff",
      "Content-Security-Policy": "sandbox; default-src 'none'",
      "Content-Disposition": `attachment; filename="${sha}.${EXT[row.content_type] ?? "bin"}"`,
      "Cache-Control": "no-store",
      "Cross-Origin-Resource-Policy": "same-site",
      Vary: "Authorization, Cookie, Origin",
    },
  });
}

export { DEFAULT_LIMITS };

/** Tags (incl. merged topics) and standards of a publish, served with the item and kept on import copies. */
async function tagsFor(c: Ctx, publishId: string): Promise<{ tags: string[]; standards: string[] }> {
  const [t, s] = await c.db.batch([
    c.db.prepare(`SELECT tag FROM item_tags WHERE publish_id = ?1 ORDER BY tag`).bind(publishId),
    c.db.prepare(`SELECT code FROM item_standards WHERE publish_id = ?1 ORDER BY code`).bind(publishId),
  ]);
  return { tags: (t!.results as { tag: string }[]).map((x) => x.tag), standards: (s!.results as { code: string }[]).map((x) => x.code) };
}

/**
 * Licences under which this account may publish a verbatim copy of what it was just served: per
 * question, every import it holds of a question with that match_hash (this one included) with the
 * chains they carry, combined by the one rule in license.ts. Exactly what publish computes.
 */
async function mayRepublishAs(c: Ctx, me: string, publishId: string, form: "full" | "stripped"): Promise<string[]> {
  const rows = await c.db
    .prepare(`SELECT match_hash, CASE WHEN ?2 = 'stripped' THEN stripped_content ELSE full_content END AS full, keyfree_content AS keyfree FROM item_questions WHERE publish_id = ?1 ORDER BY pos`)
    .bind(publishId, form)
    .all<{ match_hash: string; full: string; keyfree: string }>();
  const hashes = rows.results.map((r) => r.match_hash);
  const actx = await authorContext(c, me, hashes);
  const hits = await importHits(c, me, hashes);
  // A verbatim copy of what was served, published the only way it can be (hidden when the serve
  // was stripped): the same Subject, rules and comparison publish uses (visible.ts, provenance.ts).
  const per: Grant[][] = rows.results.map((q) => {
    const s = subjectFor(form === "stripped", q.keyfree, q.full);
    return grantsFor(hits.filter((h) => h.match_hash === q.match_hash && admissible(h, actx, s)), s);
  });
  return allowedForAll(per);
}

/**
 * Records what a non-author was served (an import, or a 'view' for a plain read), only if the item
 * is STILL visible after the body was read: a takedown or hold that landed meanwhile turns it into
 * a 404 with nothing recorded (one atomic batch). A view never downgrades an import.
 */
async function recordServed(c: Ctx, me: string, r: ItemRow, form: "full" | "stripped", kind: "import" | "view"): Promise<boolean> {
  const now = c.now();
  const cond = `EXISTS (SELECT 1 FROM items i JOIN users u ON u.id = i.author_id WHERE i.id = ?2 AND ${VISIBLE_TO_OTHERS})`;
  // Keyed on the PUBLIC publish id, so re-importing after a hidden-only republish is the same row.
  const stmts = [
    c.db
      .prepare(
        `INSERT INTO imports (user_id, item_id, publish_id, version, form, license, author_id, at, kind, real_publish_id, licenses_json)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, json_array(?6) WHERE ${cond}
         ON CONFLICT (user_id, item_id, publish_id) DO UPDATE SET at = excluded.at, real_publish_id = excluded.real_publish_id, license = excluded.license,
           licenses_json = (SELECT json_group_array(v) FROM (SELECT DISTINCT value AS v FROM json_each(COALESCE(imports.licenses_json, json_array(imports.license)))
                                                               UNION SELECT excluded.license)),
           kind = CASE WHEN excluded.kind = 'import' THEN 'import' ELSE imports.kind END`,
      )
      .bind(me, r.id, r.public_publish_id, r.p_public_version, form, r.p_license, r.author_id, now, kind, r.current_publish_id),
  ];
  // The question rows are written once per (account, publish) for a read (INSERT OR IGNORE: no
  // write amplification on repeated reads) and refreshed on an import; durable either way.
  stmts.push(
      c.db
        .prepare(
          `INSERT OR ${kind === "import" ? "REPLACE" : "IGNORE"} INTO import_questions (user_id, publish_id, qid, item_id, match_hash, content_hash, keyfree_hash, chain_json, author_name, source_first_seq, stem_hash)
           SELECT ?1, ?6, iq.qid, ?2, iq.match_hash, CASE WHEN ?4 = 'stripped' THEN iq.stripped_content ELSE iq.full_content END, iq.keyfree_content, iq.chain_json, ?5,
                  (SELECT qi.seq FROM question_ids qi WHERE qi.qid = iq.qid), iq.stem_hash
             FROM item_questions iq WHERE iq.publish_id = ?3 AND ${cond}`,
        )
        .bind(me, r.id, r.current_publish_id, form, r.author_name ?? "a former member", r.public_publish_id),
    );
  stmts.push(c.db.prepare(`SELECT 1 AS ok WHERE ${cond.replaceAll("?2", "?1")}`).bind(r.id));
  const res = await c.db.batch(stmts);
  return (res[res.length - 1]!.results as unknown[]).length > 0;
}

export const IMPORT_IMAGE_HOLD_MS = 30 * DAY_MS;

/** Holds the served images of an import for the importer, if their quota allows; one statement. */
async function holdImportedImages(c: Ctx, me: string, publishId: string, form: "full" | "stripped"): Promise<{ held: number; until?: number; reason?: string }> {
  const now = c.now();
  const until = now + IMPORT_IMAGE_HOLD_MS;
  const quota = await limitOf(c, me, "storage_bytes");
  const n = await c.db.prepare(`SELECT COUNT(*) AS n FROM item_blobs WHERE publish_id = ?1 AND form = ?2`).bind(publishId, form).first<{ n: number }>();
  if (!n?.n) return { held: 0 };
  const r = await c.db
    .prepare(
      `INSERT INTO pending_blobs (sha256, uploader_id, size, until)
       SELECT b.sha256, ?1, b.size, ?5 FROM item_blobs ib JOIN blobs b ON b.sha256 = ib.sha256
        WHERE ib.publish_id = ?3 AND ib.form = ?4
          AND ${USAGE_SQL} + (SELECT COALESCE(SUM(b2.size), 0) FROM item_blobs ib2 JOIN blobs b2 ON b2.sha256 = ib2.sha256
                               WHERE ib2.publish_id = ?3 AND ib2.form = ?4
                                 AND NOT EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b2.sha256 AND pb.uploader_id = ?1 AND pb.until >= ?2)) <= ?6
       ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = MAX(until, excluded.until)`,
    )
    .bind(me, now, publishId, form, until, quota)
    .run();
  return (r.meta.changes ?? 0) > 0 ? { held: n.n, until } : { held: 0, reason: "quota" };
}
