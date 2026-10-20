// POST /items (new item) and PUT /items/:id (republish): docs/shared-pool.md "Publish order".
//
// validate (share-core) -> server rewrite (ids, times, image refs from the verified blobs, client
// origin/published dropped) -> validate again -> strip (share-core, hide_key only) -> hashes ->
// licence hard check against the author's own imports, soft check, blocklist -> staged rows (quota
// checked in the same INSERT) -> R2 bodies -> derived rows -> one compare-and-set commit batch ->
// retention (current + previous). A lost CAS is a 409 and the staged rows were never reachable.
import { isFail, kindTag, questionsOf, type Fail, type LabelDecision, type PayloadJson, type Pos, type QuestionJson, type Stripped, type Validated } from "./core.ts";
import { EXT } from "./image.ts";
import { allowedFor, allowedForAll, ATTEST_TEXT_REVS, isLicense, type Grant, type License } from "./license.ts";
import { sameAs, subjectFor } from "./visible.ts";
import { admissible, authorContext, carriedImportHits, declaredImportHits, grantsFor, importHits, keyfreeHash, linkGrants, ownItemSet, stemHash, stemHits, type ChainLink, type ImportHit } from "./provenance.ts";
import {
  REMOVED_SQL,
  audit,
  chunks,
  countMiss,
  refundMiss,
  DAY_MS,
  missGuard,
  ID_RE,
  json,
  limitOf,
  MB,
  newId,
  PoolError,
  PUBLISH_ID_RE,
  dayOf,
  capAlert,
  readPublish,
  PUBLISH_COST_PER_BYTE,
  reserveBytes,
  requireDaily,
  sha256Hex,
  SHARED_SQL,
  usage,
  USAGE_SQL,
  VISIBLE_TO_OTHERS,
  type Ctx,
  type PoolPrincipal,
} from "./shared.ts";

export const MAX_PAYLOAD = 4 * MB;
export const MAX_META = 1 * MB;
/** A staging/staged publish younger than this is assumed to be in flight (a concurrent retry gets 409). */
export const IN_FLIGHT_MS = 60_000;
export const CHAIN_MAX = 32;

const KIND_BITS: Record<string, number> = { Mc: 1, Tf: 2, Matching: 4, Blank: 8, Short: 16, Multipart: 32 };
const MEDIA_RE = /^media\/([0-9a-f]{64})\.(png|jpg|gif|webp)$/;
const enc = new TextEncoder();


interface PublishMeta {
  publishId: string;
  base: string | null;
  hideKey: boolean;
  license: License;
  attestRev: string;
  labels: LabelDecision[];
  orderAcks: Pos[];
  imageOk: number[];
  visibility: "public" | "unlisted" | null;
  meta: unknown;
}

const isNat = (x: unknown): x is number => typeof x === "number" && Number.isInteger(x) && x >= 0 && x < 1_000_000;

function readMeta(m: Record<string, unknown>, isNew: boolean): PublishMeta {
  const bad = (field: string) => new PoolError(400, "invalid_request", { field });
  if (typeof m.publish_id !== "string" || !PUBLISH_ID_RE.test(m.publish_id)) throw bad("publish_id");
  let base: string | null = null;
  if (isNew) {
    if (m.base_publish_id !== undefined && m.base_publish_id !== null) throw bad("base_publish_id");
  } else {
    if (typeof m.base_publish_id !== "string" || !PUBLISH_ID_RE.test(m.base_publish_id)) throw bad("base_publish_id");
    base = m.base_publish_id;
  }
  if (typeof m.hide_key !== "boolean") throw bad("hide_key");
  // Nothing is preselected: the licence must be chosen explicitly on every publish.
  if (!isLicense(m.license)) throw bad("license");
  const at = m.attestation as Record<string, unknown> | undefined;
  if (!at || typeof at !== "object" || at.accepted !== true || !(ATTEST_TEXT_REVS as readonly unknown[]).includes(at.text_rev)) throw bad("attestation");
  const labels = m.labels ?? [];
  if (!Array.isArray(labels) || labels.length > 60_000) throw bad("labels");
  const outLabels: LabelDecision[] = [];
  for (const l of labels as Record<string, unknown>[]) {
    if (!l || typeof l !== "object" || !isNat(l.question) || (l.part !== undefined && !isNat(l.part)) || (l.choice !== "strip" && l.choice !== "keep")) throw bad("labels");
    outLabels.push(l.part === undefined ? { question: l.question, choice: l.choice } : { question: l.question, part: l.part as number, choice: l.choice });
  }
  const acks = m.order_acks ?? [];
  if (!Array.isArray(acks) || acks.length > 60_000) throw bad("order_acks");
  const outAcks: Pos[] = [];
  for (const a of acks as Record<string, unknown>[]) {
    if (!a || typeof a !== "object" || !isNat(a.question) || (a.part !== undefined && !isNat(a.part))) throw bad("order_acks");
    outAcks.push(a.part === undefined ? { question: a.question } : { question: a.question, part: a.part as number });
  }
  const img = m.image_ok ?? [];
  if (!Array.isArray(img) || img.length > 1000 || !img.every(isNat)) throw bad("image_ok");
  let visibility: PublishMeta["visibility"] = null;
  if (isNew) {
    if (m.visibility !== "public" && m.visibility !== "unlisted") throw bad("visibility");
    visibility = m.visibility;
    if (visibility === "public" && m.license === "circle-only") throw new PoolError(422, "license_not_public");
  }
  return {
    publishId: m.publish_id,
    base,
    hideKey: m.hide_key,
    license: m.license,
    attestRev: at.text_rev as string,
    labels: outLabels,
    orderAcks: outAcks,
    imageOk: img as number[],
    visibility,
    meta: m.meta ?? {},
  };
}

function invalid(f: Fail, status = 422): PoolError {
  const extra: Record<string, unknown> = { errors: f.errors };
  if (f.label_findings) extra.label_findings = f.label_findings;
  if (f.needs_order_ack) extra.needs_order_ack = f.needs_order_ack;
  if (f.order_reasons) extra.order_reasons = f.order_reasons;
  const code = f.errors.some((e) => e.code === "order_ack_required") ? "order_ack_required" : f.errors.some((e) => e.code.startsWith("labels_")) ? "labels_need_decision" : "invalid_payload";
  return new PoolError(status, code, extra);
}

/**
 * The payload with everything that is not content neutralised: what "modified" is decided on.
 * Done in place and restored (no structuredClone: a second tree of a 4 MB bank is tens of MB).
 */
function neutral(p: PayloadJson): Uint8Array {
  const qs = questionsOf(p);
  const saved = qs.map((q) => [q.id, q.created, q.modified, q.origin, q.published] as const);
  const bank = p.bank ? ([p.bank.id, p.bank.created, p.bank.modified, p.bank.origin, p.bank.published] as const) : null;
  for (const q of qs) {
    q.id = "q";
    q.created = 0;
    q.modified = 0;
    delete q.origin;
    delete q.published;
  }
  if (p.bank) {
    p.bank.id = "b";
    p.bank.created = 0;
    p.bank.modified = 0;
    delete p.bank.origin;
    delete p.bank.published;
  }
  try {
    return enc.encode(JSON.stringify(p));
  } finally {
    qs.forEach((q, k) => {
      const [id, cr, mo, or, pu] = saved[k]!;
      q.id = id;
      q.created = cr;
      q.modified = mo;
      if (or !== undefined) q.origin = or;
      if (pu !== undefined) q.published = pu;
    });
    if (p.bank && bank) {
      p.bank.id = bank[0];
      p.bank.created = bank[1];
      p.bank.modified = bank[2];
      if (bank[3] !== undefined) p.bank.origin = bank[3];
      if (bank[4] !== undefined) p.bank.published = bank[4];
    }
  }
}

function hashesOrThrow(x: string[] | Fail): string[] {
  if (isFail(x)) throw invalid(x);
  return x;
}

/** Statements deleting a publish's derived rows and its row (bodies are deleted by the caller). */
export function deletePublishStatements(db: D1Database, publishIds: string[]): D1PreparedStatement[] {
  if (publishIds.length === 0) return [];
  const ids = JSON.stringify(publishIds);
  const inIds = `IN (SELECT value FROM json_each(?1))`;
  return [
    // Committed ids stay reserved forever (a staged id that never committed may be retried).
    db
      .prepare(`INSERT OR IGNORE INTO retired_publish_ids (publish_id, author_id) SELECT publish_id, author_id FROM item_publishes WHERE publish_id ${inIds} AND status IN ('current', 'previous')`)
      .bind(ids),
    db.prepare(`DELETE FROM item_questions WHERE publish_id ${inIds}`).bind(ids),
    db.prepare(`DELETE FROM item_tags WHERE publish_id ${inIds}`).bind(ids),
    db.prepare(`DELETE FROM item_standards WHERE publish_id ${inIds}`).bind(ids),
    db.prepare(`DELETE FROM item_blobs WHERE publish_id ${inIds}`).bind(ids),
    db.prepare(`DELETE FROM item_publishes WHERE publish_id ${inIds}`).bind(ids),
  ];
}

/** qids first introduced by a publish that never committed, of that publish's own item, used nowhere. */
export function discardQidsStatement(db: D1Database, publishId: string, itemId: string): D1PreparedStatement {
  return db
    .prepare(`DELETE FROM question_ids WHERE first_publish_id = ?1 AND item_id = ?2 AND NOT EXISTS (SELECT 1 FROM item_questions iq WHERE iq.qid = question_ids.qid)`)
    .bind(publishId, itemId);
}

/** Removes a staged publish that will never commit: its rows, its new qids and its bodies. */
async function discardStaged(c: Ctx, row: { publish_id: string; item_id: string; full_key: string; stripped_key: string | null; created_at: number }): Promise<boolean> {
  // Claiming the row first (only a never-committed one) makes two concurrent discards safe.
  const del = await c.db
    .prepare(`DELETE FROM item_publishes WHERE publish_id = ?1 AND status IN ('staging', 'staged') AND created_at = ?2`)
    .bind(row.publish_id, row.created_at)
    .run();
  if ((del.meta.changes ?? 0) !== 1) return false;
  await c.db.batch([
    ...deletePublishStatements(c.db, [row.publish_id]),
    // qids first introduced by a publish that never committed belong to nothing.
    discardQidsStatement(c.db, row.publish_id, row.item_id),
  ]);
  await c.r2.delete([row.full_key, ...(row.stripped_key ? [row.stripped_key] : [])]);
  return true;
}

interface PubRow {
  publish_id: string;
  item_id: string;
  author_id: string;
  status: string;
  request_sha256: string;
  result_json: string | null;
  full_key: string;
  stripped_key: string | null;
  created_at: number;
}

/**
 * Refused publishes are counted per day (each costs a full validation): past the allowance the
 * account gets 429 before any work is done. Conflicts, retries and rate limits do not count.
 */
export async function publish(c: Ctx, req: Request, p: PoolPrincipal, pathItemId: string | null): Promise<Response> {
  const me = p.userId;
  await missGuard(c, me, "publish_refusals");
  // The day's publish allowance is checked BEFORE any work (cheap); a request refused for it still
  // counts as a refused publish (it is not refunded), so retries cannot buy free validations.
  const used = await c.db
    .prepare(`SELECT count FROM usage_daily WHERE user_id = ?1 AND day = ?2 AND metric = 'publishes'`)
    .bind(me, dayOf(c.now()))
    .first<{ count: number }>();
  if ((used?.count ?? 0) >= (await limitOf(c, me, "publishes"))) {
    await countMiss(c, me, "publish_refusals");
    await capAlert(c, me, "publishes");
    throw new PoolError(429, "daily_limit", { metric: "publishes" }, { "Retry-After": "3600" });
  }
  // Reserved BEFORE any work and refunded only on a definite non-refusal: a killed isolate, a wasm
  // trap or any unexpected exception still counts (critic D1d).
  await countMiss(c, me, "publish_refusals");
  const refund = () => refundMiss(c, me, "publish_refusals");
  // One publish at a time per account, across isolates (critic D1a).
  const lock = await acquireAccountLock(c, me);
  if (!lock) {
    await refund();
    throw new PoolError(409, "publish_in_progress", { message: "Another publish from this account is still running." }, { "Retry-After": "5" });
  }
  const heavy = { release: () => {} };
  const budget: (() => void)[] = [];
  const refresh = () => refreshAccountLock(c, me, lock);
  try {
    // The body is read in full BEFORE any heavy slot is taken (round 5 E7): its length must be
    // declared (it is reserved against the isolate's byte budget first), and it must arrive within
    // the deadline, so a hung upload pins nothing but its own reservation for a bounded time.
    const declared = req.headers.get("Content-Length");
    if (declared === null) throw new PoolError(411, "length_required", { message: "Send the publish with a Content-Length." });
    const max = (c.deps.maxPublishBytes ?? MAX_PAYLOAD) + MAX_META + 1;
    if (!/^\d+$/.test(declared) || Number(declared) > max) throw new PoolError(413, "body_too_large", { limit: c.deps.maxPublishBytes ?? MAX_PAYLOAD });
    budget.push(reserveBytes(Number(declared)));
    const read = await readPublish(req, c.deps.maxPublishBytes ?? MAX_PAYLOAD, MAX_META, c.deps.bodyDeadlineMs ?? BODY_DEADLINE_MS);
    await refresh();
    const res = await publishOnce(c, p, pathItemId, heavy, read, async () => {
      // The heavy phase: one per isolate, and its measured peak reserved against the byte budget.
      heavy.release = await acquireHeavy();
      budget.push(reserveBytes(Number(declared) * (PUBLISH_COST_PER_BYTE - 1)));
      await refresh();
    }, refresh);
    await refund();
    return res;
  } catch (e) {
    if (e instanceof PoolError && (e.status === 409 || e.status === 503)) await refund();
    throw e;
  } finally {
    heavy.release();
    for (const r of budget) r();
    await releaseAccountLock(c, me, lock);
  }
}

/** How long a publish body may take to arrive. */
export const BODY_DEADLINE_MS = 15_000;

/** Keeps the account lock alive while this request is (round 5 E7): refreshed at every phase. */
async function refreshAccountLock(c: Ctx, me: string, token: string): Promise<void> {
  await c.db.prepare(`UPDATE pool_locks SET until = ?3 WHERE user_id = ?1 AND token = ?2`).bind(me, token, c.now() + ACCOUNT_LOCK_MS).run();
}

// ---- memory: one heavy publish per isolate ----------------------------------------------------------

/** Concurrent publishes that may parse, validate and strip at once in one isolate (critic D1a). */
export const HEAVY_CONCURRENCY = 1;
/** Publishes allowed to wait for it; beyond that, 503 with Retry-After. */
export const HEAVY_QUEUE_MAX = 4;
let heavyActive = 0;
const heavyQueue: (() => void)[] = [];

/** Waits for the isolate's heavy-work slot; returns its release (idempotent). */
export async function acquireHeavy(): Promise<() => void> {
  if (heavyActive >= HEAVY_CONCURRENCY) {
    if (heavyQueue.length >= HEAVY_QUEUE_MAX) throw new PoolError(503, "busy", { message: "The server is busy with large publishes; try again shortly." }, { "Retry-After": "10" });
    await new Promise<void>((r) => heavyQueue.push(r));
  }
  heavyActive++;
  let done = false;
  return () => {
    if (done) return;
    done = true;
    heavyActive--;
    heavyQueue.shift()?.();
  };
}

export const ACCOUNT_LOCK_MS = 60_000;

async function acquireAccountLock(c: Ctx, me: string): Promise<string | null> {
  const token = newId();
  const now = c.now();
  const r = await c.db
    .prepare(
      `INSERT INTO pool_locks (user_id, token, until) VALUES (?1, ?2, ?3)
       ON CONFLICT (user_id) DO UPDATE SET token = excluded.token, until = excluded.until WHERE pool_locks.until < ?4
       RETURNING token`,
    )
    .bind(me, token, now + ACCOUNT_LOCK_MS, now)
    .first<{ token: string }>();
  return r?.token === token ? token : null;
}

async function releaseAccountLock(c: Ctx, me: string, token: string): Promise<void> {
  await c.db.prepare(`DELETE FROM pool_locks WHERE user_id = ?1 AND token = ?2`).bind(me, token).run();
}

async function publishOnce(
  c: Ctx,
  p: PoolPrincipal,
  pathItemId: string | null,
  heavy: { release: () => void },
  read: { raw: Uint8Array; meta: Record<string, unknown>; payload: Uint8Array },
  enterHeavy: () => Promise<void>,
  refresh: () => Promise<void>,
): Promise<Response> {
  const isNew = pathItemId === null;
  if (!isNew) await missGuard(c, p.userId, "item_miss");
  if (!isNew && !ID_RE.test(pathItemId)) {
    await countMiss(c, p.userId, "item_miss");
    throw new PoolError(404, "not_found");
  }
  await enterHeavy();
  const rawMeta = read.meta;
  let payload: Uint8Array | null = read.payload;
  const m = readMeta(rawMeta, isNew);
  const requestSha = await sha256Hex(read.raw);
  (read as { raw: Uint8Array | null }).raw = null;
  const me = p.userId;

  // ---- idempotency ---------------------------------------------------------------------------------
  const prior = await c.db
    .prepare(`SELECT publish_id, item_id, author_id, status, request_sha256, result_json, full_key, stripped_key, created_at FROM item_publishes WHERE publish_id = ?1`)
    .bind(m.publishId)
    .first<PubRow>();
  const retired = await c.db.prepare(`SELECT 1 AS x FROM retired_publish_ids WHERE publish_id = ?1`).bind(m.publishId).first();
  if (retired) throw new PoolError(409, "publish_id_conflict");
  if (prior) {
    // Someone else's id, or the same id with a different body: never resumed, never revealed.
    if (prior.author_id !== me || prior.request_sha256 !== requestSha || (!isNew && prior.item_id !== pathItemId)) throw new PoolError(409, "publish_id_conflict");
    if ((prior.status === "current" || prior.status === "previous") && prior.result_json) return json(200, { ...(JSON.parse(prior.result_json) as object), replayed: true });
    if (prior.created_at > c.now() - IN_FLIGHT_MS) throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
    // An abandoned attempt: start again from scratch with the same publish_id.
    if (!(await discardStaged(c, prior))) throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
  }

  // ---- the item ------------------------------------------------------------------------------------
  const now = c.now();
  let itemId: string;
  let version: number;
  let visibility: "public" | "unlisted";
  let itemKind: string | null = null;
  let baseOf: string | null = null;
  let prevPublic = { version: 0, at: null as number | null };
  let prevHidden = false;
  if (isNew) {
    itemId = newId();
    version = 1;
    visibility = m.visibility!;
  } else {
    const it = await c.db
      .prepare(`SELECT id, kind, visibility, current_publish_id, version, removed_at, purged_at, public_version, public_at, hide_key FROM items WHERE id = ?1 AND author_id = ?2`)
      .bind(pathItemId, me)
      .first<{ id: string; kind: string; visibility: "public" | "unlisted"; current_publish_id: string | null; version: number; removed_at: number | null; purged_at: number | null; public_version: number; public_at: number | null; hide_key: number }>();
    if (!it) {
      await countMiss(c, me, "item_miss");
      throw new PoolError(404, "not_found");
    }
    if (it.removed_at !== null)
      throw new PoolError(409, "item_removed", it.purged_at !== null ? { purged: true, message: "This item was deleted for good. Publish it again as a new item." } : { message: "This item is removed. Undo the deletion first, or publish it as a new item." });
    if (it.current_publish_id !== m.base) throw new PoolError(409, "conflict", { current_version: it.version, current_publish_id: it.current_publish_id });
    if (it.visibility === "public" && m.license === "circle-only") throw new PoolError(422, "license_not_public");
    itemId = it.id;
    version = it.version + 1;
    visibility = it.visibility;
    itemKind = it.kind;
    baseOf = it.current_publish_id;
    prevPublic = { version: it.public_version, at: it.public_at };
    prevHidden = it.hide_key === 1;
  }

  // ---- validate, rewrite, validate again -----------------------------------------------------------
  const meta = c.core.validateMeta(m.meta);
  if (isFail(meta)) throw invalid(meta, 422);
  const v1 = c.core.validateForPublish(payload!, { hideKey: m.hideKey, labels: m.labels });
  payload = null;
  (read as { payload: Uint8Array | null }).payload = null;
  if (isFail(v1)) throw invalid(v1);
  const kind = v1.payload.bank ? "bank" : "question";
  // A republish must be schema v2: v1 cannot carry `published`, so its questions would lose their
  // identity (and the hidden-change and provenance rules with it).
  if (!isNew && v1.payload.schema_version < 2)
    throw new PoolError(422, "schema_upgrade_required", { message: "Republishing needs schema version 2 (update verSHion)." });
  if (itemKind !== null && itemKind !== kind) throw new PoolError(422, "kind_mismatch");
  const qs = questionsOf(v1.payload);
  const n = qs.length;
  if (n === 0) throw new PoolError(422, "invalid_payload", { errors: [{ code: "no_questions", path: "bank.questions" }] });


  // qids: a sent qid is kept only if it already belongs to this item and appears once.
  const sent = qs.map((q) => (typeof q.published?.qid === "string" ? q.published.qid : null));
  const owned = new Set<string>();
  if (!isNew && sent.some((x) => x !== null)) {
    const rows = await c.db
      .prepare(`SELECT qid FROM question_ids WHERE item_id = ?1 AND qid IN (SELECT value FROM json_each(?2))`)
      .bind(itemId, JSON.stringify([...new Set(sent.filter((x): x is string => x !== null))]))
      .all<{ qid: string }>();
    for (const r of rows.results) owned.add(r.qid);
  }
  const counts = new Map<string, number>();
  for (const s of sent) if (s) counts.set(s, (counts.get(s) ?? 0) + 1);
  const qids = sent.map((s) => (s && owned.has(s) && counts.get(s) === 1 ? s : newId()));
  const newQids = qids.filter((q, i) => q !== sent[i] || !owned.has(q));

  // Images: every referenced blob must be verified and the author's (pending, or in one of their publishes).
  const imgOf = (q: QuestionJson) => (q.image ? MEDIA_RE.exec(q.image.file)?.[1] ?? null : null);
  const shas = [...new Set(qs.map(imgOf).filter((x): x is string => x !== null))];
  const blobInfo = new Map<string, { content_type: keyof typeof EXT; width: number; height: number }>();
  if (shas.length > 0) {
    const rows = await c.db
      .prepare(
        `SELECT b.sha256, b.content_type, b.width, b.height FROM blobs b
          WHERE b.sha256 IN (SELECT value FROM json_each(?1))
            AND (EXISTS (SELECT 1 FROM pending_blobs pb WHERE pb.sha256 = b.sha256 AND pb.uploader_id = ?2 AND pb.until >= ?3)
              OR EXISTS (SELECT 1 FROM item_blobs ib JOIN item_publishes ip ON ip.publish_id = ib.publish_id WHERE ib.sha256 = b.sha256 AND ip.author_id = ?2))`,
      )
      .bind(JSON.stringify(shas), me, now)
      .all<{ sha256: string; content_type: keyof typeof EXT; width: number; height: number }>();
    for (const r of rows.results) blobInfo.set(r.sha256, r);
    const missing = qs.map((q, i) => [i, imgOf(q)] as const).filter(([, s]) => s !== null && !blobInfo.has(s!)).map(([i]) => i);
    if (missing.length > 0) throw new PoolError(422, "blob_missing", { positions: missing.slice(0, 100) });
    // Validation extends the author's hold on every referenced blob (spec "Publish order" step 4),
    // so the GC cannot collect one between this check and the commit. Already charged: no quota change.
    await c.db
      .prepare(
        `INSERT INTO pending_blobs (sha256, uploader_id, size, until) SELECT sha256, ?2, size, ?3 FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1))
         ON CONFLICT (sha256, uploader_id) DO UPDATE SET until = MAX(until, excluded.until)`,
      )
      .bind(JSON.stringify(shas), me, now + DAY_MS)
      .run();
    // A blob GC could have raced an upload: the bytes must really be in R2.
    for (const s of shas)
      if (!(await c.r2.head(`blobs/${s}`))) {
        await c.db.prepare(`UPDATE blobs SET missing_at = COALESCE(missing_at, ?2) WHERE sha256 = ?1`).bind(s, now).run();
        throw new PoolError(422, "blob_missing", { reason: "reupload", positions: qs.map((q, i) => (imgOf(q) === s ? i : -1)).filter((i) => i >= 0).slice(0, 100) });
      }
  }

  // Declared origins are recorded for the hard check, then dropped: provenance is the server's.
  const declared = qs.map((q) => (q.origin && typeof q.origin.item_id === "string" ? { item: q.origin.item_id, qid: typeof q.origin.qid === "string" ? q.origin.qid : "", version: typeof q.origin.version === "number" ? q.origin.version : -1 } : null));
  const day = Math.floor(now / DAY_MS) * DAY_MS;
  let rewritten: PayloadJson | null = structuredClone(v1.payload);
  const fixQ = (q: QuestionJson, i: number) => {
    q.id = qids[i]!;
    q.created = day;
    q.modified = day;
    delete q.origin;
    delete q.published;
    const s = imgOf(q);
    if (s && q.image) {
      const b = blobInfo.get(s)!;
      q.image = { file: `media/${s}.${EXT[b.content_type]}`, width: b.width, height: b.height };
    }
  };
  if (rewritten!.bank) {
    rewritten.bank.id = itemId;
    rewritten.bank.created = day;
    rewritten.bank.modified = day;
    delete rewritten.bank.origin;
    delete rewritten.bank.published;
    rewritten.bank.questions.forEach(fixQ);
  } else fixQ(rewritten.question!, 0);
  let rBytes: Uint8Array | null = enc.encode(JSON.stringify(rewritten));
  // Peak memory (a 4 MB bank is ~100 MB of JS objects per copy): every intermediate is dropped as
  // soon as it is used. The first validation's tree goes now; the rewrite after the second pass.
  rewritten = null;
  (v1 as { payload: unknown }).payload = null;
  qs.length = 0;

  const full = c.core.validateForPublish(rBytes!, { hideKey: m.hideKey, labels: m.labels });
  if (isFail(full)) throw invalid(full);
  let st: Stripped | null = null;
  if (m.hideKey) {
    const s = c.core.strip(rBytes!, { labels: m.labels, orderAcks: m.orderAcks, imageOk: m.imageOk });
    if (isFail(s)) throw invalid(s);
    st = s;
  }
  rBytes = null;
  // A hidden republish whose served form is unchanged (a key correction, an explanation edit) keeps
  // the previous stripped body byte for byte and the public version: nothing a non-author sees,
  // not even the shuffle or a "newer" notice, reveals that something hidden changed (HOLD-1).
  let reused: { bytes: Uint8Array; hashes: string[]; contents: string[] } | null = null;
  if (st && prevHidden && baseOf) {
    const prevQ = await c.db
      .prepare(`SELECT qid, stripped_sha256, stripped_content FROM item_questions WHERE publish_id = ?1 ORDER BY pos`)
      .bind(baseOf)
      .all<{ qid: string; stripped_sha256: string; stripped_content: string }>();
    const prevKey = (await c.db.prepare(`SELECT stripped_key FROM item_publishes WHERE publish_id = ?1`).bind(baseOf).first<{ stripped_key: string | null }>())?.stripped_key;
    if (prevKey && prevQ.results.length === qids.length && prevQ.results.every((q, k) => q.qid === qids[k])) {
      const o = await c.r2.get(prevKey);
      if (o) {
        const text = await o.text();
        if (sameServed(JSON.parse(text) as PayloadJson, st.payload, st.servedOrders)) reused = { bytes: enc.encode(text), hashes: prevQ.results.map((q) => q.stripped_sha256), contents: prevQ.results.map((q) => q.stripped_content) };
      }
    }
  }
  const publicVersion = isNew ? 1 : reused ? prevPublic.version : prevPublic.version + 1;
  const publicAt = reused ? prevPublic.at : null; // null: this commit's time
  const fullQs = questionsOf(full.payload);
  const mh = st ? st.matchHashes : hashesOrThrow(c.core.matchHashes(full.bytes));
  const fh = hashesOrThrow(c.core.fullHashes(full.bytes));
  const fullContent = hashesOrThrow(c.core.fullHashes(neutral(full.payload)));
  const strippedContent = reused ? reused.contents : st ? hashesOrThrow(c.core.fullHashes(neutral(st.payload))) : null;
  if (mh.length !== n || fh.length !== n || fullContent.length !== n) throw new Error("share-core hash count mismatch");

  // Key-free facts per question as served to non-authors: what "modified" is decided on when this
  // publish hides its key, so nothing a non-author sees can depend on the key (critic A1).
  const servedQs = st ? questionsOf(st.payload) : fullQs;
  const keyfree = await Promise.all(servedQs.map((q, i) => keyfreeHash(mh[i]!, q, imgOf)));
  // Two different questions are asked of every source:
  // * shown `modified` (in the chain, seen by everyone): key-free facts whenever this publish hides its
  //   key, so the flag can never reveal a key (critic A1);
  // * licence `modified` (decides the grant, seen only by this publisher): against exactly what the
  //   source served, explanation, points and image included (critic C4), key-free only for a
  //   keyless (stripped) serve.
  // Every non-author-visible derivation below compares through visible.ts: a hidden publish's
  // Subject has no full-content hash, so nothing visible can depend on its key (round 5).
  const subjects = keyfree.map((k, i) => subjectFor(m.hideKey, k, fullContent[i]!));
  const differs = (i: number, h: ImportHit) => !sameAs(subjects[i]!, h);

  // A loose stem-only hash: a near copy of something served (edited choices, punctuation) raises an
  // "origin_suspect" flag for the admin. Never a refusal.
  const stems = await Promise.all(servedQs.map((q) => stemHash((q as { stem?: unknown }).stem)));

  // ---- licence and provenance: built by the server, never taken from the client --------------------
  // Evidence rules (critic D3): a source counts only if it is not the publisher's own work (nor
  // derived from it) and was first published before the publisher first published such a question.
  const actx = await authorContext(c, me, mh);
  const hitsAll = await importHits(c, me, mh);
  const byHash = new Map<string, ImportHit[]>();
  for (const r of hitsAll) byHash.set(r.match_hash, [...(byHash.get(r.match_hash) ?? []), r]);
  // A declared origin is a hint: it counts only if it names a question this account imported, at
  // the version it imported (critic A2, "validate declared origin qid/version").
  const declaredHits = await declaredImportHits(c, me, declared);
  // A kept qid carries its previous provenance: dropping `origin` cannot clear it (critic A3).
  const prev = new Map<string, { chain_json: string; origin_item_id: string | null; origin_qid: string | null; full_content: string; keyfree_content: string }>();
  if (!isNew && baseOf !== null) {
    const kept = qids.filter((q, i) => q === sent[i]);
    if (kept.length > 0) {
      const rows = await c.db
        .prepare(`SELECT qid, chain_json, origin_item_id, origin_qid, full_content, keyfree_content FROM item_questions WHERE publish_id = ?1 AND qid IN (SELECT value FROM json_each(?2))`)
        .bind(baseOf, JSON.stringify(kept))
        .all<{ qid: string; chain_json: string; origin_item_id: string | null; origin_qid: string | null; full_content: string; keyfree_content: string }>();
      for (const r of rows.results) prev.set(r.qid, r);
    }
  }
  const carriedSources = await carriedImportHits(c, me, [...prev.values()].filter((x) => x.origin_item_id && x.origin_qid).map((x) => ({ item: x.origin_item_id!, qid: x.origin_qid! })));
  const ownItems = await ownItemSet(c, me, declared.filter((d) => d !== null).map((d) => d!.item));
  const incompatible: number[] = [];
  const perQuestion: Grant[][] = [];
  const unverified: { pos: number; item: string }[] = [];
  const declaredModified: { pos: number; item: string }[] = [];
  const derived: { chain: ChainLink[]; originItem: string | null; originQid: string | null; modified: boolean | null }[] = [];
  const linkOf = (h: ImportHit, modified: boolean): ChainLink => ({ author_name: h.author_name, item_id: h.item_id, qid: h.qid, version: h.version, license: h.license, modified });
  const byKey = (a: ImportHit, b: ImportHit) => (a.item_id + a.qid + String(a.version).padStart(9, "0") < b.item_id + b.qid + String(b.version).padStart(9, "0") ? -1 : 1);
  const chainLen = (h: ImportHit) => (JSON.parse(h.chain_json) as ChainLink[]).length;
  for (let i = 0; i < n; i++) {
    const ok = (h: ImportHit) => admissible(h, actx, subjects[i]!);
    const hits = (byHash.get(mh[i]!) ?? []).filter(ok);
    const d = declared[i];
    const dRaw = d ? declaredHits.get(`${d.item}/${d.qid}/${d.version}`) ?? null : null;
    const dHit = dRaw && ok(dRaw) ? dRaw : null;
    if (d && !dHit && !hits.some((h) => h.item_id === d.item) && !ownItems.has(d.item)) unverified.push({ pos: i, item: ID_RE.test(d.item) ? d.item : "" });
    // A declared origin whose content differs from what was imported is kept (an edit) but the admin sees it.
    if (dHit && dHit.keyfree_hash !== keyfree[i]) declaredModified.push({ pos: i, item: dHit.item_id });
    const pv = prev.get(qids[i]!);
    const cRaw = pv && pv.origin_item_id && pv.origin_qid ? carriedSources.get(`${pv.origin_item_id}/${pv.origin_qid}`) ?? null : null;
    const carried = cRaw && ok(cRaw) ? cRaw : null;
    const seen = new Set<string>();
    const sources = [...hits, ...(dHit ? [dHit] : []), ...(carried ? [carried] : [])].filter((h) => {
      const k = `${h.item_id}/${h.qid}/${h.version}`;
      return seen.has(k) ? false : (seen.add(k), true);
    });
    const grants: Grant[] = grantsFor(sources, subjects[i]!);
    let entry: (typeof derived)[number];
    if (sources.length > 0) {
      // Attribution (critic C5): never by version numbers across items. A source whose key-free
      // content is identical wins (the longest chain among them); otherwise every candidate's chain
      // is merged, deduplicated by (author, item, version). Deterministic whatever the import order.
      // Attribution: the identical sources (visible.ts: key-free when hidden) are all credited, in the
      // order they were first published; with none identical, every candidate is merged. Never by
      // version numbers across items, never by import order, never by the hidden key.
      const sorted = [...sources].sort((a, b) => a.source_first_seq - b.source_first_seq || byKey(a, b));
      const identical = sorted.filter((h) => !differs(i, h));
      const pool = identical.length > 0 ? identical : sorted;
      const src = pool.reduce((a, b) => (chainLen(b) > chainLen(a) ? b : a));
      let chain: ChainLink[] = [];
      const have = new Set<string>();
      for (const h of pool)
        for (const l of [...(JSON.parse(h.chain_json) as ChainLink[]), linkOf(h, differs(i, h))]) {
          const k = `${l.author_name}|${l.item_id}|${l.version}`;
          if (!have.has(k)) {
            have.add(k);
            chain.push(l);
          }
        }
      if (chain.length > CHAIN_MAX) chain = [{ author_name: "and earlier contributors", item_id: "", qid: "", version: 0, license: "", modified: true }, ...chain.slice(chain.length - (CHAIN_MAX - 1))];
      entry = { chain, originItem: src.item_id, originQid: src.qid, modified: differs(i, src) };
    } else if (pv && pv.chain_json !== "[]") {
      // Provenance whose import record is gone: kept as it was, and an edit counts as a modification.
      // Key-free comparison whenever either version hides its key: a public v2 must not reveal
      // whether a hidden v1's key differed (critic round 3, latent leak).
      const chain = JSON.parse(pv.chain_json) as ChainLink[];
      const last = chain[chain.length - 1];
      const changed = m.hideKey || prevHidden ? pv.keyfree_content !== keyfree[i] : pv.full_content !== fullContent[i];
      if (last && changed) last.modified = true;
      grants.push(...linkGrants(chain.slice(0, -1), last ? { item: last.item_id, license: last.license, modified: last.modified } : null));
      entry = { chain, originItem: pv.origin_item_id, originQid: pv.origin_qid, modified: last ? last.modified : null };
    } else entry = { chain: [], originItem: null, originQid: null, modified: null };
    derived.push(entry);
    perQuestion.push(grants);
    if (!allowedFor(grants).includes(m.license)) incompatible.push(i);
  }
  if (incompatible.length > 0)
    // Which questions force which licences: a mixed bank can be split, or published under the licence they all allow.
    throw new PoolError(422, "license_incompatible", {
      positions: incompatible.slice(0, 100),
      allowed: allowedForAll(perQuestion),
      per_question: incompatible.slice(0, 100).map((pos) => ({ pos, allowed: allowedFor(perQuestion[pos]!) })),
    });
  const suspectStems = await stemHits(c, me, stems.filter((x, k): x is string => x !== null && derived[k]!.chain.length === 0));
  const suspect = stems.map((x, k) => (x !== null && derived[k]!.chain.length === 0 && suspectStems.has(x) ? k : -1)).filter((k) => k >= 0);

  // ---- soft check, blocklist, duplicates ------------------------------------------------------------
  const mhJson = JSON.stringify([...new Set(mh)]);
  const soft = await c.db
    .prepare(
      `SELECT DISTINCT i.id FROM item_questions iq
         JOIN items i ON i.current_publish_id = iq.publish_id JOIN users u ON u.id = i.author_id
        WHERE iq.match_hash IN (SELECT value FROM json_each(?2)) AND i.author_id <> ?1 AND ${VISIBLE_TO_OTHERS}
          AND NOT EXISTS (SELECT 1 FROM imports im WHERE im.user_id = ?1 AND im.item_id = i.id AND im.kind = 'import') LIMIT 50`,
    )
    .bind(me, mhJson)
    .all<{ id: string }>();
  // Holds that follow content (none is ever a refusal: common questions would give false positives):
  // privacy blocks (student data, critic C2), DMCA re-upload blocks, and a hold on one of this
  // author's own items with the same questions (a held item deleted and published again, critic X5).
  // Images only in the full form of a hidden publish are never checked against blocks: a hit on
  // material nobody else is served must not create state its author can see (round 5 E4).
  const servedShas = [...new Set(servedQs.map(imgOf).filter((x): x is string => x !== null))];
  const privacyHit = await c.db
    .prepare(`SELECT 1 AS x FROM privacy_blocks WHERE (kind = 'match' AND hash IN (SELECT value FROM json_each(?1))) OR (kind = 'blob' AND hash IN (SELECT value FROM json_each(?2))) LIMIT 1`)
    .bind(mhJson, JSON.stringify(servedShas))
    .first();
  const blocked = await c.db
    .prepare(
      `SELECT kind, hash, action_id FROM blocked_hashes WHERE lifted_at IS NULL AND (expires_at IS NULL OR expires_at > ?3)
         AND ((kind = 'match' AND hash IN (SELECT value FROM json_each(?1))) OR (kind = 'blob' AND hash IN (SELECT value FROM json_each(?2)))) LIMIT 1`,
    )
    .bind(mhJson, JSON.stringify(servedShas), now)
    .first<{ kind: string; hash: string; action_id: string | null }>();
  const carried = await c.db
    .prepare(
      `SELECT h.reason, h.item_id FROM item_holds h JOIN items i ON i.id = h.item_id AND i.author_id = ?1 AND i.purged_at IS NULL
        WHERE h.item_id <> ?2 AND h.source IN ('', 'manual') -- human judgements; content blocks are re-checked anyway
          AND EXISTS (SELECT 1 FROM item_publishes p JOIN item_questions iq ON iq.publish_id = p.publish_id
                                         WHERE p.item_id = h.item_id AND iq.match_hash IN (SELECT value FROM json_each(?3)))
        ORDER BY h.reason LIMIT 1`,
    )
    .bind(me, itemId, mhJson)
    .first<{ reason: string; item_id: string }>();
  let possibleDuplicate: string | null = null;
  if (isNew) {
    const dup = await c.db
      .prepare(
        `SELECT i.id FROM items i JOIN item_questions iq ON iq.publish_id = i.current_publish_id
          WHERE i.author_id = ?1 AND i.removed_at IS NULL
          GROUP BY i.id HAVING COUNT(*) = ?2 AND SUM(iq.match_hash IN (SELECT value FROM json_each(?3))) = ?2 LIMIT 1`,
      )
      .bind(me, n, mhJson)
      .first<{ id: string }>();
    possibleDuplicate = dup?.id ?? null;
  }

  // ---- stage ---------------------------------------------------------------------------------------
  const kindsMask = fullQs.reduce((acc, q) => acc | (KIND_BITS[kindTag(q)] ?? 0), 0);
  const hasImages = servedQs.some((q) => q.image) ? 1 : 0;
  // Everything later needs only these: the payload trees (tens of MB for a 4 MB bank) go now, and
  // the isolate's heavy slot is handed to the next publish.
  const lite = fullQs.map((q) => ({ kind: kindTag(q), difficulty: typeof q.difficulty === "string" ? q.difficulty : null, topics: (q.topics ?? []) as string[] }));
  const bankName = full.payload.bank ? String(full.payload.bank.name ?? "") : "";
  const schemaVersion = full.payload.schema_version;
  const strippedShas = st ? [...new Set(questionsOf(st.payload).map(imgOf).filter((x): x is string => x !== null))] : [];
  const fullTags = tagsOf(meta.tags, fullQs);
  (full as { payload: unknown }).payload = null;
  if (st) (st as { payload: unknown }).payload = null;
  fullQs.length = 0;
  heavy.release();
  await refresh();
  const fullKey = `items/${itemId}/${newId()}.json`;
  const strippedKey = st ? `items/${itemId}/${newId()}.json` : null;
  const fullBytes = full.bytes.length;
  const strippedBytes = reused ? reused.bytes.length : st ? st.bytes.length : 0;
  const charge = fullBytes + strippedBytes + (isNew ? 1024 : 0);
  const quota = await limitOf(c, me, "storage_bytes");
  const title = meta.title || bankName;
  const acks = JSON.stringify({ labels: st ? st.labelFindings : [], order_acks: m.orderAcks, image_ok: m.imageOk, order_reasons: st ? st.orderReasons : [] });
  // Tags (with the merged topics) are never silently dropped: too many is an error.
  if (fullTags.size > MAX_TAGS) throw new PoolError(422, "too_many_tags", { limit: MAX_TAGS, message: "Too many distinct tags and topics in this item." });
  // Charged only now: a payload refused by any check above does not use up the day's publishes.
  await requireDaily(c, me, "publishes");
  const sharedCap = await limitOf(c, me, "shared_questions");
  const removedCap = await limitOf(c, me, "removed_bytes");
  const token = newId();
  // Quota, the shared-question cap (counting other items' in-flight publishes) and the id reservation
  // are all in this one INSERT, so concurrent publishes cannot race past them (critic A7).
  const staged = await c.db
    .prepare(
      `INSERT OR IGNORE INTO item_publishes (publish_id, item_id, author_id, base_publish_id, version, status, schema_version, hide_key, license,
         attested_at, attest_text_rev, acks_json, request_sha256, full_key, stripped_key, full_bytes, stripped_bytes,
         title, subject, course, grade, kinds_mask, question_count, has_images, created_at, stage_token, public_version)
       SELECT ?3, ?4, ?1, ?5, ?6, 'staging', ?7, ?8, ?9, ?2, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?2, ?26, ?29
        WHERE ${USAGE_SQL} + ?24 <= ?25 AND NOT EXISTS (SELECT 1 FROM retired_publish_ids WHERE publish_id = ?3)
          AND ${SHARED_SQL("?1", "?4")} + ?22 <= ?27 AND ${REMOVED_SQL} <= ?28`,
    )
    .bind(
      me,
      now,
      m.publishId,
      itemId,
      m.base,
      version,
      schemaVersion,
      m.hideKey ? 1 : 0,
      m.license,
      m.attestRev,
      acks,
      requestSha,
      fullKey,
      strippedKey,
      fullBytes,
      strippedBytes,
      title,
      meta.subject,
      meta.course,
      meta.grade,
      kindsMask,
      n,
      hasImages,
      charge,
      quota,
      token,
      sharedCap,
      removedCap,
      publicVersion,
    )
    .run();
  if ((staged.meta.changes ?? 0) !== 1) {
    const again = await c.db.prepare(`SELECT 1 AS x FROM item_publishes WHERE publish_id = ?1`).bind(m.publishId).first();
    if (again) throw new PoolError(409, "publish_in_progress", {}, { "Retry-After": "5" });
    const used = await usage(c, me);
    if (used + charge > quota) throw new PoolError(413, "quota_exceeded", { used, needed: charge, limit: quota });
    const removed = (await c.db.prepare(`SELECT ${REMOVED_SQL} AS n`).bind(me).first<{ n: number }>())?.n ?? 0;
    if (removed > removedCap)
      throw new PoolError(413, "removed_storage_exceeded", { used: removed, limit: removedCap, message: "Deleted items still hold storage until they are purged (14 days). Undo a deletion or wait." });
    const q = await c.db.prepare(`SELECT ${SHARED_SQL("?1", "?2")} AS n`).bind(me, itemId).first<{ n: number }>();
    throw new PoolError(422, "shared_questions_limit", { used: q?.n ?? 0, needed: n, limit: sharedCap });
  }
  const mine = `EXISTS (SELECT 1 FROM item_publishes WHERE publish_id = ?1 AND stage_token = ?3 AND status = 'staging')`;
  const stagedRow = { publish_id: m.publishId, item_id: itemId, full_key: fullKey, stripped_key: strippedKey, created_at: now };
  try {
    await c.r2.put(fullKey, full.bytes, { httpMetadata: { contentType: "application/json" } });
    if (st && strippedKey) await c.r2.put(strippedKey, reused ? reused.bytes : st.bytes, { httpMetadata: { contentType: "application/json" } });

    const qRows = lite.map((q, i) => ({
      qid: qids[i],
      pos: i,
      qkind: q.kind,
      difficulty: q.difficulty,
      mh: mh[i],
      fh: fh[i],
      sh: reused ? reused.hashes[i] : st ? st.strippedHashes[i] : null,
      fc: fullContent[i],
      kf: keyfree[i],
      st: stems[i],
      sc: strippedContent ? strippedContent[i] : null,
      chain: JSON.stringify(derived[i]!.chain),
      oi: derived[i]!.originItem,
      oq: derived[i]!.originQid,
      mod: derived[i]!.modified === null ? null : derived[i]!.modified ? 1 : 0,
    }));
    const tags = fullTags;
    const standards = new Set(meta.standards.map((s) => s.replace(/\s+/g, " ").trim().toUpperCase()).filter(Boolean));
    const stmts: D1PreparedStatement[] = [];
    for (const ch of chunks(qRows, 150))
      stmts.push(
        c.db
          .prepare(
            `INSERT INTO item_questions (publish_id, qid, pos, qkind, difficulty, match_hash, full_sha256, stripped_sha256, full_content, stripped_content, keyfree_content, stem_hash, chain_json, origin_item_id, origin_qid, modified)
             SELECT ?1, j.value ->> '$.qid', j.value ->> '$.pos', j.value ->> '$.qkind', j.value ->> '$.difficulty', j.value ->> '$.mh', j.value ->> '$.fh',
                    j.value ->> '$.sh', j.value ->> '$.fc', j.value ->> '$.sc', j.value ->> '$.kf', j.value ->> '$.st', j.value ->> '$.chain', j.value ->> '$.oi', j.value ->> '$.oq', j.value ->> '$.mod'
               FROM json_each(?2) j WHERE ${mine}`,
          )
          .bind(m.publishId, JSON.stringify(ch), token),
      );
    for (const ch of chunks([...tags], 200))
      stmts.push(c.db.prepare(`INSERT OR IGNORE INTO item_tags (publish_id, tag) SELECT ?1, value FROM json_each(?2) WHERE ${mine}`).bind(m.publishId, JSON.stringify(ch), token));
    if (standards.size > 0) stmts.push(c.db.prepare(`INSERT OR IGNORE INTO item_standards (publish_id, code) SELECT ?1, value FROM json_each(?2) WHERE ${mine}`).bind(m.publishId, JSON.stringify([...standards]), token));
    if (shas.length > 0) stmts.push(c.db.prepare(`INSERT OR IGNORE INTO item_blobs (publish_id, form, sha256) SELECT ?1, 'full', value FROM json_each(?2) WHERE ${mine}`).bind(m.publishId, JSON.stringify(shas), token));
    if (strippedShas.length > 0)
      stmts.push(c.db.prepare(`INSERT OR IGNORE INTO item_blobs (publish_id, form, sha256) SELECT ?1, 'stripped', value FROM json_each(?2) WHERE ${mine}`).bind(m.publishId, JSON.stringify(strippedShas), token));
    if (newQids.length > 0)
      stmts.push(
        c.db
          .prepare(`INSERT OR IGNORE INTO question_ids (qid, item_id, first_publish_id, created_at, seq)
                    SELECT value, ?4, ?1, ?5, (SELECT COALESCE(MAX(seq), 0) + 1 FROM question_ids) FROM json_each(?2) WHERE ${mine}`)
          .bind(m.publishId, JSON.stringify(newQids), token, itemId, now),
      );
    for (const ch of chunks(stmts, 20)) await c.db.batch(ch);
    await c.db.prepare(`UPDATE item_publishes SET status = 'staged' WHERE publish_id = ?1 AND status = 'staging' AND stage_token = ?2`).bind(m.publishId, token).run();
  } catch (e) {
    await discardStaged(c, stagedRow).catch(() => {});
    throw e;
  }

  // ---- commit (one batch, compare-and-set) ---------------------------------------------------------
  const result = {
    item_id: itemId,
    publish_id: m.publishId,
    version,
    kind,
    visibility,
    license: m.license,
    hide_key: m.hideKey,
    qids,
    public_version: publicVersion,
    possible_duplicate_of: possibleDuplicate,
    // The author's review of what non-authors will be served (positions and orders, no text).
    served_orders: st?.servedOrders ?? [],
    order_reasons: st?.orderReasons ?? [],
    labels_removed: st?.labelsRemoved ?? [],
    images_kept: st?.imagesKept ?? [],
    images_removed: st?.imagesRemoved ?? [],
    figures_removed: st?.figuresRemoved ?? [],
    dropped: full.dropped,
  };
  const resultJson = JSON.stringify(result);
  const listing = [title, meta.subject, meta.course, meta.grade, kindsMask, n, m.hideKey ? 1 : 0, m.license, hasImages];
  const guard = (a: string, b: string) => `EXISTS (SELECT 1 FROM items WHERE id = ${a} AND current_publish_id = ${b})`;
  const flags = [
    ...soft.results.map((r) => ({ kind: "license_soft", detail: JSON.stringify({ matches_item: r.id }) })),
    ...unverified.map((u) => ({ kind: "origin_unverified", detail: JSON.stringify({ pos: u.pos, declared_item: u.item }) })),
    ...declaredModified.map((u) => ({ kind: "origin_modified", detail: JSON.stringify({ pos: u.pos, declared_item: u.item }) })),
    ...(suspect.length > 0 ? [{ kind: "origin_suspect", detail: JSON.stringify({ positions: suspect.slice(0, 50) }) }] : []),
    ...(blocked ? [{ kind: "blocklist", detail: JSON.stringify({ hash_kind: (blocked as { kind: string }).kind }) }] : []),
  ].map((f) => ({ id: newId(), ...f }));
  const stmts: D1PreparedStatement[] = [];
  if (isNew)
    stmts.push(
      c.db
        .prepare(
          `INSERT INTO items (id, author_id, kind, visibility, current_publish_id, prev_publish_id, version, title, subject, course, grade,
             kinds_mask, question_count, hide_key, license, has_images, committed_at, created_at, updated_at, public_version, public_at, public_publish_id)
           SELECT ?1, ?2, ?3, ?4, ?5, NULL, 1, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15, ?15, 1, ?15, ?5
            WHERE EXISTS (SELECT 1 FROM item_publishes WHERE publish_id = ?5 AND status = 'staged' AND author_id = ?2 AND stage_token = ?16)
              AND NOT EXISTS (SELECT 1 FROM items WHERE id = ?1)`,
        )
        .bind(itemId, me, kind, visibility, m.publishId, ...listing, now, token),
    );
  else
    stmts.push(
      c.db
        .prepare(
          `UPDATE items SET current_publish_id = ?4, prev_publish_id = current_publish_id, version = ?5, title = ?6, subject = ?7, course = ?8, grade = ?9,
             kinds_mask = ?10, question_count = ?11, hide_key = ?12, license = ?13, has_images = ?14, committed_at = ?15, updated_at = ?15,
             public_version = ?17, public_at = COALESCE(?18, ?15), public_publish_id = CASE WHEN ?19 = 1 THEN COALESCE(public_publish_id, ?4) ELSE ?4 END
           WHERE id = ?1 AND author_id = ?2 AND current_publish_id IS ?3 AND removed_at IS NULL
             AND EXISTS (SELECT 1 FROM item_publishes WHERE publish_id = ?4 AND status = 'staged' AND author_id = ?2 AND stage_token = ?16)`,
        )
        .bind(itemId, me, m.base, m.publishId, version, ...listing, now, token, publicVersion, publicAt, reused ? 1 : 0),
    );
  stmts.push(
    c.db.prepare(`UPDATE item_publishes SET status = 'previous' WHERE item_id = ?1 AND status = 'current' AND publish_id <> ?2 AND ${guard("?1", "?2")}`).bind(itemId, m.publishId),
    c.db
      .prepare(`UPDATE item_publishes SET status = 'current', committed_at = ?3, result_json = ?4 WHERE publish_id = ?2 AND status = 'staged' AND ${guard("?1", "?2")}`)
      .bind(itemId, m.publishId, now, resultJson),
    c.db.prepare(`DELETE FROM pending_blobs WHERE uploader_id = ?3 AND sha256 IN (SELECT value FROM json_each(?4)) AND ${guard("?1", "?2")}`).bind(itemId, m.publishId, me, JSON.stringify(shas)),
    // The images were checked present in R2 above: a needs_reupload hold has been answered.
    c.db.prepare(`DELETE FROM item_holds WHERE item_id = ?1 AND source = 'needs_reupload' AND ${guard("?1", "?2")}`).bind(itemId, m.publishId),
  );
  // Privacy and DMCA blocks are re-checked INSIDE the commit batch (below), against this publish's
  // own rows: a block confirmed after the checks above still holds it (no gap). Carried holds are
  // computed above (they depend on other items, not on blocks).
  void privacyHit;
  void blocked;
  const holds: { reason: string; source: string }[] = [...(carried ? [{ reason: carried.reason, source: `carried:${carried.item_id}` }] : [])];
  const served = `ib.form = CASE WHEN p.hide_key = 1 THEN 'stripped' ELSE 'full' END`;
  stmts.push(
    c.db
      .prepare(
        `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
         SELECT ?1, 'student_data', ?3, NULL, 'privacy' WHERE ${guard("?1", "?2")} AND EXISTS (SELECT 1 FROM privacy_blocks pb WHERE
           (pb.kind = 'match' AND pb.hash IN (SELECT match_hash FROM item_questions WHERE publish_id = ?2))
           OR (pb.kind = 'blob' AND pb.hash IN (SELECT ib.sha256 FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id WHERE ib.publish_id = ?2 AND ${served})))`,
      )
      .bind(itemId, m.publishId, now),
    c.db
      .prepare(
        `INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source)
         SELECT DISTINCT ?1, 'blocklist', ?3, NULL, 'dmca:' || bh.action_id FROM blocked_hashes bh
          WHERE ${guard("?1", "?2")} AND bh.lifted_at IS NULL AND (bh.expires_at IS NULL OR bh.expires_at > ?3) AND bh.action_id IS NOT NULL AND (
            (bh.kind = 'match' AND bh.hash IN (SELECT match_hash FROM item_questions WHERE publish_id = ?2))
            OR (bh.kind = 'blob' AND bh.hash IN (SELECT ib.sha256 FROM item_blobs ib JOIN item_publishes p ON p.publish_id = ib.publish_id WHERE ib.publish_id = ?2 AND ${served})))`,
      )
      .bind(itemId, m.publishId, now),
  );
  if (holds.length > 0)
    stmts.push(
      c.db
        .prepare(`INSERT OR IGNORE INTO item_holds (item_id, reason, at, by, source) SELECT ?1, j.value ->> '$.reason', ?3, NULL, j.value ->> '$.source' FROM json_each(?4) j WHERE ${guard("?1", "?2")}`)
        .bind(itemId, m.publishId, now, JSON.stringify(holds)),
    );
  if (flags.length > 0)
    stmts.push(
      c.db
        .prepare(
          `INSERT INTO admin_flags (id, kind, item_id, publish_id, user_id, detail, at)
           SELECT j.value ->> '$.id', j.value ->> '$.kind', ?1, ?2, ?3, j.value ->> '$.detail', ?4 FROM json_each(?5) j WHERE ${guard("?1", "?2")}`,
        )
        .bind(itemId, m.publishId, me, now, JSON.stringify(flags)),
    );
  stmts.push(
    audit(c.db, { actorId: me, actorKind: "user", action: isNew ? "item.publish" : "item.republish", itemId, targetUserId: me, detail: { publish_id: m.publishId, version, license: m.license, hide_key: m.hideKey } }, now, guard(`'${itemId}'`, `'${m.publishId}'`)), // both match [A-Za-z0-9_-]+
    c.db.prepare(`SELECT current_publish_id, version FROM items WHERE id = ?1`).bind(itemId),
  );
  let won = false;
  let current: { current_publish_id: string | null; version: number } | undefined;
  try {
    const res = await c.db.batch(stmts);
    current = (res[res.length - 1]!.results as { current_publish_id: string | null; version: number }[])[0];
    won = current?.current_publish_id === m.publishId;
  } catch (e) {
    // e.g. the CHECK (public needs a public licence) after a concurrent visibility change.
    console.error(`pool: commit batch failed: ${String(e)}`);
  }
  if (!won) {
    const discarded = await discardStaged(c, { ...stagedRow, created_at: now }).catch(() => false);
    if (!discarded) {
      // A retry re-staged this publish_id meanwhile: our bodies belong to nothing (its row has its own keys).
      const row = await c.db.prepare(`SELECT full_key, stripped_key FROM item_publishes WHERE publish_id = ?1`).bind(m.publishId).first<{ full_key: string; stripped_key: string | null }>();
      const held = new Set([row?.full_key, row?.stripped_key].filter(Boolean));
      const ours = [fullKey, strippedKey].filter((k): k is string => k !== null && !held.has(k));
      if (ours.length) await c.r2.delete(ours);
    }
    // Staged rows were never reachable: every read joins items.current_publish_id.
    throw new PoolError(409, "conflict", { current_version: current?.version ?? null, current_publish_id: current?.current_publish_id ?? null });
  }

  // ---- after commit: retention (current + previous) ------------------------------------------------
  await retain(c, itemId);
  return json(isNew ? 201 : 200, result);
}

/** Deletes every publish of `itemId` that is neither current nor the previous one (rows and bodies). */
export async function retain(c: Ctx, itemId: string): Promise<void> {
  const old = await c.db
    .prepare(
      `SELECT p.publish_id, p.full_key, p.stripped_key FROM item_publishes p JOIN items i ON i.id = p.item_id
        WHERE p.item_id = ?1 AND p.status = 'previous' AND p.publish_id IS NOT i.prev_publish_id`,
    )
    .bind(itemId)
    .all<{ publish_id: string; full_key: string; stripped_key: string | null }>();
  if (old.results.length === 0) return;
  await c.db.batch(deletePublishStatements(c.db, old.results.map((r) => r.publish_id)));
  await c.r2.delete(old.results.flatMap((r) => [r.full_key, ...(r.stripped_key ? [r.stripped_key] : [])]));
}

export type { Validated };

export const MAX_TAGS = 200;

function tagsOf(metaTags: string[], qs: QuestionJson[]): Set<string> {
  const tags = new Set(metaTags.map((t) => t.toLowerCase()));
  for (const q of qs) for (const t of q.topics ?? []) if (typeof t === "string" && t.trim()) tags.add(t.trim().toLowerCase());
  return tags;
}

/**
 * Equal as served to non-authors, ignoring only ids, times and what strip shuffles (critic D5):
 * order matters everywhere share-core preserves it (questions, Multipart parts, locked choices,
 * acknowledged / kept orders, Matching left items); only the choices strip actually shuffled
 * (`servedOrders`: unlocked Mc choices of that question or part) and the Matching word bank are
 * compared as multisets.
 */
export function sameServed(a: PayloadJson, b: PayloadJson, servedOrders: unknown[]): boolean {
  const shuffled = new Set(
    (servedOrders as { question: number; part?: number; matching?: boolean }[]).filter((o) => !o.matching).map((o) => (o.part === undefined ? `${o.question}` : `${o.question}/${o.part}`)),
  );
  const sortKeys = (v: unknown): unknown => {
    if (Array.isArray(v)) return v.map(sortKeys);
    if (v && typeof v === "object")
      return Object.fromEntries(Object.entries(v as Record<string, unknown>).filter(([k]) => !["id", "created", "modified"].includes(k)).sort(([x], [y]) => (x < y ? -1 : 1)).map(([k, x]) => [k, sortKeys(x)]));
    return v;
  };
  const answers = (list: { locked?: boolean }[], isShuffled: boolean) =>
    isShuffled
      ? { locked: list.map((x, k) => (x.locked ? [k, JSON.stringify(sortKeys(x))] : null)).filter(Boolean), free: list.filter((x) => !x.locked).map((x) => JSON.stringify(sortKeys(x))).sort() }
      : list.map((x) => JSON.stringify(sortKeys(x)));
  const kindCanon = (kind: Record<string, any>, key: string): unknown => {
    if (kind.Mc) return { Mc: answers(kind.Mc.answers, shuffled.has(key)) };
    if (kind.Matching) return { Matching: { pairs: sortKeys(kind.Matching.pairs), bank: [...kind.Matching.distractors].sort() } };
    if (kind.Multipart) return { Multipart: (kind.Multipart.parts as { stem: string; kind: Record<string, any> }[]).map((p, k) => ({ stem: p.stem, kind: kindCanon(p.kind, `${key}/${k}`) })) };
    return sortKeys(kind);
  };
  const qCanon = (q: QuestionJson, i: number) => JSON.stringify({ ...(sortKeys({ ...q, kind: null }) as object), kind: kindCanon(q.kind as Record<string, any>, `${i}`) });
  const qa = questionsOf(a);
  const qb = questionsOf(b);
  if (qa.length !== qb.length || !qa.every((q, i) => qCanon(q, i) === qCanon(qb[i]!, i))) return false;
  const rest = (p: PayloadJson) => JSON.stringify(sortKeys(p.bank ? { ...p.bank, questions: null } : { ...p, question: null }));
  return rest(a) === rest(b);
}
