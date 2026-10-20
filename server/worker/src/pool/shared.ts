// Shared plumbing for the pool routes: responses, body reading, ids, the visibility rule, daily caps,
// the storage quota, rate limits and the audit log.
import { b64url, deriveKeys, fromB64url, hmac, utf8 } from "../../../auth/src/crypto.ts";
import type { RateLimitRule, RateLimitStore } from "../../../auth/src/types.ts";
import type { Core } from "./core.ts";

export const MB = 1024 * 1024;
export const DAY_MS = 86_400_000;
export const HOUR_MS = 3_600_000;

export interface PoolPrincipal {
  userId: string;
  trust: string;
}

/** Something the pool wants a human to hear about. Wired to mail in src/index.ts. */
export type PoolNotice =
  | { kind: "student_data_report"; itemId: string; reportId: string }
  | { kind: "takedown_author"; itemId: string; authorId: string; actionId: string }
  | { kind: "counter_notice"; actionId: string; complainantContact: string; restoreNotBefore: number }
  | { kind: "cap"; userId: string; metric: string }
  | { kind: "repeat_infringer"; userId: string; strikes: number };

export interface PoolDeps {
  db: D1Database;
  /** The R2 bucket for bodies and images. Missing (not configured yet) -> every pool route answers 503. */
  r2: R2Bucket | undefined;
  core: Core;
  /** AUTH_SECRET: rate-limit keys are derived exactly as the auth package derives them, so deleting an account purges the pool's counters too. */
  secret: string;
  allowedOrigins: string[];
  /** The auth package's authenticate (revocation and current trust checked against D1). */
  authenticate(req: Request): Promise<PoolPrincipal | null>;
  now?: () => number;
  /** The auth package's D1RateLimitStore in the Worker (injected, so this module loads under node's strip-only TypeScript). */
  rateStore: RateLimitStore;
  /** The auth package's ipBucket (IPv6 grouped by /64). */
  ipBucket: (ip: string) => string;
  notify?: (n: PoolNotice) => Promise<void>;
  /** Publish body cap in bytes (default 4 MB). See README "Limits" for the measured memory per publish. */
  maxPublishBytes?: number;
  /** How long a publish body may take to arrive (default 15 s). */
  bodyDeadlineMs?: number;
  /** Runs a promise after the response (ctx.waitUntil). */
  background?: (p: Promise<unknown>) => void;
}

export interface Ctx {
  db: D1Database;
  r2: R2Bucket;
  core: Core;
  now: () => number;
  deps: PoolDeps;
  keys: Promise<{ limit: CryptoKey; cursor: CryptoKey; preview: CryptoKey }>;
  store: RateLimitStore;
}

// ---- responses -------------------------------------------------------------------------------------

const BASE: Record<string, string> = {
  "Content-Type": "application/json; charset=utf-8",
  "Cache-Control": "no-store",
  Pragma: "no-cache",
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "no-referrer",
  // A shared school browser or proxy must never cache one user's body for the next (ASVS V14).
  Vary: "Authorization, Cookie, Origin",
};

export function json(status: number, body: unknown, headers: Record<string, string> = {}): Response {
  return rawJson(status, JSON.stringify(body), headers);
}

/** A JSON response whose text was assembled by the caller (to embed share-core's bytes as they are). */
export function rawJson(status: number, text: string, headers: Record<string, string> = {}): Response {
  const h = new Headers(BASE);
  for (const [k, v] of Object.entries(headers)) h.set(k, v);
  return new Response(text, { status, headers: h });
}

export class PoolError extends Error {
  status: number;
  code: string;
  extra: Record<string, unknown>;
  headers: Record<string, string>;
  constructor(status: number, code: string, extra: Record<string, unknown> = {}, headers: Record<string, string> = {}) {
    super(code);
    this.status = status;
    this.code = code;
    this.extra = extra;
    this.headers = headers;
  }
}

export const notFound = () => new PoolError(404, "not_found");
export const bad = (field: string) => new PoolError(400, "invalid_request", { field });

// ---- bodies ----------------------------------------------------------------------------------------

function contentType(req: Request): string {
  return (req.headers.get("Content-Type") ?? "").split(";")[0]!.trim().toLowerCase();
}

/** Reads at most `max` bytes; a longer body (declared or actual) is a 413 and reading stops there. */
export async function readBytes(req: Request, max: number, deadlineMs?: number): Promise<Uint8Array> {
  const declared = req.headers.get("Content-Length");
  if (declared !== null && (!/^\d+$/.test(declared) || Number(declared) > max)) throw new PoolError(413, "body_too_large", { limit: max });
  if (!req.body) return new Uint8Array(0);
  const reader = req.body.getReader();
  const parts: Uint8Array[] = [];
  let total = 0;
  // A body that never finishes cannot pin anything: past the deadline the request is refused.
  const until = deadlineMs === undefined ? Infinity : Date.now() + deadlineMs;
  for (;;) {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const next =
      until === Infinity
        ? reader.read()
        : Promise.race([
            reader.read(),
            new Promise<never>((_, rej) => {
              timer = setTimeout(() => rej(new PoolError(408, "body_timeout", { message: "The request body did not arrive in time." })), Math.max(0, until - Date.now()));
            }),
          ]);
    let chunk: ReadableStreamReadResult<Uint8Array>;
    try {
      chunk = await next;
    } catch (e) {
      await reader.cancel().catch(() => {});
      throw e;
    } finally {
      if (timer) clearTimeout(timer);
    }
    const { done, value } = chunk;
    if (done) break;
    total += value.byteLength;
    if (total > max) {
      await reader.cancel().catch(() => {});
      throw new PoolError(413, "body_too_large", { limit: max });
    }
    parts.push(value);
  }
  const out = new Uint8Array(total);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.byteLength;
  }
  return out;
}

/** A small JSON object body (`application/json` only, so an HTML form cannot send it cross-site). */
export async function readJson(req: Request, max = 64 * 1024): Promise<Record<string, unknown>> {
  if (contentType(req) !== "application/json") throw new PoolError(415, "unsupported_media_type");
  const b = await readBytes(req, max);
  let v: unknown;
  try {
    v = JSON.parse(new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(b));
  } catch {
    throw new PoolError(400, "invalid_json");
  }
  if (typeof v !== "object" || v === null || Array.isArray(v)) throw new PoolError(400, "invalid_json");
  return v as Record<string, unknown>;
}

/** An optional small JSON body: none at all is `{}`. */
export async function readOptionalJson(req: Request): Promise<Record<string, unknown>> {
  if (req.headers.get("Content-Type") === null && (req.headers.get("Content-Length") ?? "0") === "0") return {};
  return readJson(req);
}

export const PUBLISH_CONTENT_TYPE = "application/vnd.vershion.publish";

/**
 * A publish request: `<meta JSON>\n<payload bytes>`. The payload goes to share-core exactly as sent
 * (JavaScript never parses it first: a JSON.parse round trip would silently resolve duplicate keys
 * and replace lone surrogates).
 */
export async function readPublish(req: Request, maxPayload: number, maxMeta: number, deadlineMs?: number): Promise<{ raw: Uint8Array; meta: Record<string, unknown>; payload: Uint8Array }> {
  if (contentType(req) !== PUBLISH_CONTENT_TYPE) throw new PoolError(415, "unsupported_media_type");
  const raw = await readBytes(req, maxPayload + maxMeta + 1, deadlineMs);
  const nl = raw.indexOf(0x0a);
  if (nl < 0 || nl > maxMeta) throw new PoolError(400, "invalid_request", { field: "meta" });
  if (raw.length - nl - 1 > maxPayload) throw new PoolError(413, "body_too_large", { limit: maxPayload });
  let meta: unknown;
  try {
    meta = JSON.parse(new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(raw.subarray(0, nl)));
  } catch {
    throw new PoolError(400, "invalid_json", { field: "meta" });
  }
  if (typeof meta !== "object" || meta === null || Array.isArray(meta)) throw new PoolError(400, "invalid_json", { field: "meta" });
  return { raw, meta: meta as Record<string, unknown>, payload: raw.subarray(nl + 1) };
}

// ---- ids and hashes --------------------------------------------------------------------------------

/** 128 random bits, hex: item ids, qids, R2 key parts, report ids. */
export function newId(): string {
  const b = crypto.getRandomValues(new Uint8Array(16));
  return [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
}
export const ID_RE = /^[0-9a-f]{32}$/;
/** Client-generated publish ids: at least 128 bits of base64url (or hex). */
export const PUBLISH_ID_RE = /^[A-Za-z0-9_-]{22,64}$/;
export const SHA_RE = /^[0-9a-f]{64}$/;

export async function sha256Hex(b: Uint8Array): Promise<string> {
  const d = new Uint8Array(await crypto.subtle.digest("SHA-256", b));
  return [...d].map((x) => x.toString(16).padStart(2, "0")).join("");
}

export function dayOf(ms: number): number {
  return Math.floor(ms / DAY_MS);
}

// ---- visibility ------------------------------------------------------------------------------------

/** The author's standing (docs/shared-pool.md "Content follows the author's standing"). `u` = the author's users row. */
export const STANDING = `u.trust IN ('teacher', 'admin') AND u.suspended_at IS NULL AND u.deleted_at IS NULL`;

/** Visible to someone other than its author: public, committed, not removed, not held, author in good standing. */
export const VISIBLE_TO_OTHERS = `i.visibility = 'public' AND i.current_publish_id IS NOT NULL AND i.removed_at IS NULL
  AND NOT EXISTS (SELECT 1 FROM item_holds h WHERE h.item_id = i.id)
  AND ((${STANDING}) OR (i.anonymized = 1 AND u.deleted_at IS NOT NULL AND u.trust IN ('teacher', 'admin') AND u.suspended_at IS NULL))`;

/** Visible to the caller bound at `me` (a parameter reference such as `?2`). */
export const visibleTo = (me: string) =>
  `((i.author_id = ${me} AND i.removed_at IS NULL AND i.current_publish_id IS NOT NULL) OR (i.author_id <> ${me} AND ${VISIBLE_TO_OTHERS}))`;

// ---- limits ----------------------------------------------------------------------------------------

/** Defaults (docs/shared-pool.md "Limits"); each can be raised per user or site-wide by an admin. */
export const DEFAULT_LIMITS = {
  publishes: 50,
  imports: 200,
  item_reads: 2000,
  browse: 500,
  blob_uploads: 500,
  blob_bytes: 200 * MB,
  blob_miss: 100,
  item_miss: 100,
  reports: 20,
  storage_bytes: 200 * MB,
  shared_questions: 5000,
  report_threshold: 3,
  /** Bytes of removed items not purged yet (they leave the quota at once, but their bodies stay 14 days). */
  removed_bytes: 200 * MB,
  /** Refused publishes per day: each costs the server a full validation. */
  publish_refusals: 300,
  /** A question or image used by at least this many OTHER authors is never blocked unless selected. */
  block_common_k: 3,
  /** Appeals per account per day. */
  appeals: 10,
  /** Instant student-data holds per day across all accounts (then they wait for the admin). */
  instant_holds_global: 50,
} as const;
export type Metric = keyof typeof DEFAULT_LIMITS;
export const METRICS = Object.keys(DEFAULT_LIMITS) as Metric[];

export async function limitOf(c: Ctx, userId: string, metric: Metric): Promise<number> {
  const r = await c.db
    .prepare(`SELECT value FROM pool_limits WHERE metric = ?1 AND user_id IN (?2, '*') ORDER BY user_id = '*' LIMIT 1`)
    .bind(metric, userId)
    .first<{ value: number }>();
  return r ? r.value : DEFAULT_LIMITS[metric];
}

/**
 * Takes `n` from today's allowance of `metric`; false (and nothing taken) when it would cross the
 * cap. One atomic UPSERT, so concurrent requests cannot overshoot.
 */
export async function takeDaily(c: Ctx, userId: string, metric: Metric, n = 1): Promise<boolean> {
  const cap = await limitOf(c, userId, metric);
  const day = dayOf(c.now());
  const r = await c.db
    .prepare(
      `INSERT INTO usage_daily (user_id, day, metric, count) SELECT ?1, ?2, ?3, ?4 WHERE ?4 <= ?5
       ON CONFLICT (user_id, day, metric) DO UPDATE SET count = count + excluded.count WHERE count + excluded.count <= ?5
       RETURNING count`,
    )
    .bind(userId, day, metric, n, cap)
    .first<{ count: number }>();
  if (r) return true;
  await capAlert(c, userId, metric);
  return false;
}

/** Throws 429 when today's allowance of `metric` is used up. */
export async function requireDaily(c: Ctx, userId: string, metric: Metric, n = 1): Promise<void> {
  if (!(await takeDaily(c, userId, metric, n))) throw new PoolError(429, "daily_limit", { metric }, { "Retry-After": String(secondsToMidnight(c.now())) });
}

function secondsToMidnight(now: number): number {
  return Math.max(1, Math.ceil(((dayOf(now) + 1) * DAY_MS - now) / 1000));
}

/** Counts a miss (404) of `metric` (item_miss / blob_miss). */
export async function countMiss(c: Ctx, userId: string, metric: "item_miss" | "blob_miss" | "publish_refusals"): Promise<void> {
  await c.db
    .prepare(
      `INSERT INTO usage_daily (user_id, day, metric, count) VALUES (?1, ?2, ?3, 1)
       ON CONFLICT (user_id, day, metric) DO UPDATE SET count = count + 1`,
    )
    .bind(userId, dayOf(c.now()), metric)
    .run();
}

// ---- memory: one byte budget per isolate (reads and publishes together, round 5 E6) ---------------

/** JS bytes the isolate may commit to requests at once (128 MB isolate - ~25 MB wasm - headroom). */
export const ISOLATE_BYTES_BUDGET = 96 * MB;
/** What a streamed read costs (chunks in flight, the head): it never holds the whole body. */
export const READ_COST = 1 * MB;
/** What a publish costs per payload byte at its peak (measured: about 49 MB for a 3.8 MB bank). */
export const PUBLISH_COST_PER_BYTE = 13;
let bytesInFlight = 0;

/** Reserves `n` bytes of the isolate budget, or 503 busy; returns the release (idempotent). */
export function reserveBytes(n: number): () => void {
  if (bytesInFlight + n > ISOLATE_BYTES_BUDGET) throw new PoolError(503, "busy", { message: "The server is busy; try again shortly." }, { "Retry-After": "5" });
  bytesInFlight += n;
  let done = false;
  return () => {
    if (done) return;
    done = true;
    bytesInFlight -= n;
  };
}
export function bytesReserved(): number {
  return bytesInFlight;
}

/** Takes back one counted miss (a reserved refusal that turned out not to be one). */
export async function refundMiss(c: Ctx, userId: string, metric: "publish_refusals"): Promise<void> {
  await c.db.prepare(`UPDATE usage_daily SET count = count - 1 WHERE user_id = ?1 AND day = ?2 AND metric = ?3 AND count > 0`).bind(userId, dayOf(c.now()), metric).run();
}

/** Past the day's miss allowance, every id-taking route answers 429 for the rest of the day. */
export async function missGuard(c: Ctx, userId: string, metric: "item_miss" | "blob_miss" | "publish_refusals"): Promise<void> {
  const r = await c.db
    .prepare(`SELECT count FROM usage_daily WHERE user_id = ?1 AND day = ?2 AND metric = ?3`)
    .bind(userId, dayOf(c.now()), metric)
    .first<{ count: number }>();
  if (r && r.count >= (await limitOf(c, userId, metric))) {
    await capAlert(c, userId, metric);
    throw new PoolError(429, "daily_limit", { metric }, { "Retry-After": String(secondsToMidnight(c.now())) });
  }
}

/** One admin flag (and at most a few mails an hour) per user, day and metric. */
export async function capAlert(c: Ctx, userId: string, metric: string): Promise<void> {
  const now = c.now();
  const dedup = `cap:${userId}:${dayOf(now)}:${metric}`;
  const [ins] = await c.db.batch([
    c.db
      .prepare(`INSERT OR IGNORE INTO admin_flags (id, kind, user_id, detail, dedup, at) VALUES (?1, 'cap', ?2, ?3, ?4, ?5)`)
      .bind(newId(), userId, JSON.stringify({ metric }), dedup, now),
    audit(c.db, { actorKind: "system", action: "cap_reached", targetUserId: userId, detail: { metric } }, now, `changes() = 1`),
  ]);
  if ((ins!.meta.changes ?? 0) === 1) await alertBudgeted(c, { kind: "cap", userId, metric });
}

export const ALERTS_PER_HOUR = 5;

/** Admin alert mails (cap crossings and the like), at most ALERTS_PER_HOUR; the rest stay in admin_flags. */
export async function alertBudgeted(c: Ctx, n: PoolNotice): Promise<void> {
  if (!c.deps.notify) return;
  const row = await c.db
    .prepare(`INSERT INTO pool_alert_budget (hour, sent) VALUES (?1, 1) ON CONFLICT (hour) DO UPDATE SET sent = sent + 1 RETURNING sent`)
    .bind(Math.floor(c.now() / HOUR_MS))
    .first<{ sent: number }>();
  if ((row?.sent ?? 1) <= ALERTS_PER_HOUR) send(c, n);
}

/** Sends a notice in the background (a failure never fails the request). */
export function send(c: Ctx, n: PoolNotice): void {
  if (!c.deps.notify) return;
  const p = c.deps.notify(n).catch((e) => console.error(`pool: notice ${n.kind} failed: ${String(e)}`));
  if (c.deps.background) c.deps.background(p);
}

// ---- storage quota ---------------------------------------------------------------------------------

/**
 * Bytes charged to `?1` at time `?2`: the distinct blobs referenced by the retained (and staged)
 * publishes of their items that are not removed, plus their live pending uploads, the body bytes of
 * those publishes, and 1 KB per item not removed (removal releases the charge, as the spec says). Computed, not counted, so it cannot drift; one statement, so it is consistent.
 */
export function usageSql(include: string | null = null): string {
  // `include`: an item id (parameter reference) counted as live although removed (the undelete check).
  const live = (alias: string) => `NOT EXISTS (SELECT 1 FROM items it WHERE it.id = ${alias}.item_id AND it.removed_at IS NOT NULL${include ? ` AND it.id <> ${include}` : ""})`;
  return `(
  COALESCE((SELECT SUM(size) FROM (
      SELECT ib.sha256, b.size FROM item_blobs ib
        JOIN item_publishes p ON p.publish_id = ib.publish_id
        JOIN blobs b ON b.sha256 = ib.sha256
       WHERE p.author_id = ?1 AND ${live("p")}
      UNION
      SELECT sha256, size FROM pending_blobs WHERE uploader_id = ?1 AND until >= ?2)), 0)
  + COALESCE((SELECT SUM(p.full_bytes + p.stripped_bytes) FROM item_publishes p WHERE p.author_id = ?1 AND ${live("p")}), 0)
  + 1024 * (SELECT COUNT(*) FROM items WHERE author_id = ?1 AND (removed_at IS NULL${include ? ` OR id = ${include}` : ""}))
)`;
}
export const USAGE_SQL = usageSql();

/** Body and image bytes still held by `?1`'s removed-but-unpurged items (storage amplification bound). */
export const REMOVED_SQL = `(
  COALESCE((SELECT SUM(p.full_bytes + p.stripped_bytes) FROM item_publishes p JOIN items it ON it.id = p.item_id
             WHERE p.author_id = ?1 AND it.removed_at IS NOT NULL AND it.purged_at IS NULL), 0))`;

/**
 * Questions shared by `me` across items other than `except`: committed live items, plus every
 * other item's publish still in flight (staging/staged): a new item's whole count, a live item's
 * republish only what it adds (MAX(0, new - current), critic C3).
 */
export const SHARED_SQL = (me: string, except: string) => `(
  (SELECT COALESCE(SUM(question_count), 0) FROM items WHERE author_id = ${me} AND removed_at IS NULL AND id <> ${except})
  + (SELECT COALESCE(SUM(question_count), 0) FROM item_publishes WHERE author_id = ${me} AND status IN ('staging', 'staged') AND item_id <> ${except}
       AND NOT EXISTS (SELECT 1 FROM items x WHERE x.id = item_publishes.item_id))
  + (SELECT COALESCE(SUM(MAX(0, p.question_count - x.question_count)), 0) FROM item_publishes p JOIN items x ON x.id = p.item_id
      WHERE p.author_id = ${me} AND p.status IN ('staging', 'staged') AND p.item_id <> ${except} AND x.removed_at IS NULL))`;

export async function usage(c: Ctx, userId: string): Promise<number> {
  const r = await c.db.prepare(`SELECT ${USAGE_SQL} AS used`).bind(userId, c.now()).first<{ used: number }>();
  return r?.used ?? 0;
}

// ---- rate limits -----------------------------------------------------------------------------------

/** Burst limits on every pool route, per account and per client IP (the daily caps come on top). */
export const RATE = {
  account: [{ limit: 300, windowSeconds: 300 }] as RateLimitRule[],
  ip: [{ limit: 1200, windowSeconds: 300 }] as RateLimitRule[],
  /** Read routes, per client IP per day: generous, because a whole school can share one address. */
  ipReadDaily: [{ limit: 50_000, windowSeconds: 86_400 }] as RateLimitRule[],
};

export async function rateLimit(c: Ctx, req: Request, userId: string, read = false): Promise<void> {
  const ip = req.headers.get("CF-Connecting-IP");
  if (!ip || !ip.trim()) throw new PoolError(403, "client_ip_required");
  const { limit } = await c.keys;
  const now = c.now();
  // Same key scheme as the auth package (`a:<hmac(user:<id>)>:`), so account deletion purges these too.
  const acct = `a:${await hmac(limit, `user:${userId}`)}:pool:`;
  const ipH = `i:${await hmac(limit, c.deps.ipBucket(ip.trim()))}:pool:`;
  const hits = [
    ...RATE.account.map((r) => ({ key: `${acct}${r.windowSeconds}`, rule: r })),
    ...RATE.ip.map((r) => ({ key: `${ipH}${r.windowSeconds}`, rule: r })),
    ...(read ? RATE.ipReadDaily.map((r) => ({ key: `${ipH}read:${r.windowSeconds}`, rule: r })) : []),
  ];
  const res = await Promise.all(hits.map((h) => c.store.hit(h.key, h.rule, now)));
  const denied = res.filter((r) => !r.allowed);
  if (denied.length > 0) {
    if (c.store.undo) await Promise.all(hits.map((h) => c.store.undo!(h.key, h.rule, now)));
    throw new PoolError(429, "rate_limited", {}, { "Retry-After": String(Math.max(...denied.map((d) => d.retryAfter))) });
  }
}

export function makeCtx(d: PoolDeps, r2: R2Bucket): Ctx {
  return {
    db: d.db,
    r2,
    core: d.core,
    now: d.now ?? Date.now,
    deps: d,
    store: d.rateStore,
    keys: (async () => {
      const k = await deriveKeys(d.secret);
      const ikm = await crypto.subtle.importKey("raw", utf8(d.secret), "HKDF", false, ["deriveKey"]);
      const cursor = await crypto.subtle.deriveKey(
        { name: "HKDF", hash: "SHA-256", salt: utf8("vershion-pool/v1"), info: utf8("browse-cursor") },
        ikm,
        { name: "HMAC", hash: "SHA-256", length: 256 },
        false,
        ["sign", "verify"],
      );
      // Preview tokens get their own HKDF purpose (never interchangeable with a cursor).
      const preview = await crypto.subtle.deriveKey(
        { name: "HKDF", hash: "SHA-256", salt: utf8("vershion-pool/v1"), info: utf8("block-preview") },
        ikm,
        { name: "HMAC", hash: "SHA-256", length: 256 },
        false,
        ["sign", "verify"],
      );
      return { limit: k.limit, cursor, preview };
    })(),
  };
}

// ---- cursors ---------------------------------------------------------------------------------------

export async function signCursor(c: Ctx, at: number, id: string): Promise<string> {
  const body = b64url(utf8(JSON.stringify([at, id])));
  return `${body}.${await hmac((await c.keys).cursor, body)}`;
}

export async function readCursor(c: Ctx, s: string): Promise<[number, string] | null> {
  const m = /^([A-Za-z0-9_-]{1,200})\.([A-Za-z0-9_-]{43})$/.exec(s);
  if (!m) return null;
  const ok = await crypto.subtle.verify("HMAC", (await c.keys).cursor, fromB64url(m[2]!), utf8(m[1]!));
  if (!ok) return null;
  try {
    const v = JSON.parse(new TextDecoder().decode(fromB64url(m[1]!))) as unknown;
    if (Array.isArray(v) && typeof v[0] === "number" && typeof v[1] === "string" && ID_RE.test(v[1])) return [v[0], v[1]];
  } catch {
    /* refused below */
  }
  return null;
}

// ---- audit -----------------------------------------------------------------------------------------

export interface AuditEntry {
  actorId?: string | null;
  actorKind: "user" | "admin" | "system";
  action: string;
  itemId?: string | null;
  targetUserId?: string | null;
  detail?: Record<string, unknown> | null;
  note?: string | null;
}

/** A pool_audit insert. `when` adds a SQL condition (for "only if this batch won"). */
export function audit(db: D1Database, e: AuditEntry, now: number, when?: string): D1PreparedStatement {
  return db
    .prepare(
      `INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, item_id, target_user_id, detail, note)
       SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9 ${when ? `WHERE ${when}` : ""}`,
    )
    .bind(newId(), now, e.actorId ?? null, e.actorKind, e.action, e.itemId ?? null, e.targetUserId ?? null, e.detail ? JSON.stringify(e.detail) : null, e.note ?? null);
}

/** Splits `xs` into chunks of `n`. */
export function chunks<T>(xs: T[], n: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < xs.length; i += n) out.push(xs.slice(i, i + n));
  return out;
}
