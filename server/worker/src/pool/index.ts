// The shared pool's routes (docs/shared-pool.md, phase 2). Every route needs `teacher` (or `admin`)
// trust, read from D1 on every request by the auth package's authenticate; `pending` and
// `unverified` accounts are refused. Items the caller may not see are 404, never 403.
import { poolAdminRoutes } from "./admin.ts";
import { putBlob } from "./blobs.ts";
import { daily, hourly } from "./cron.ts";
import { appeal, counterNotice, deleteItem, patchItem, report, undeleteItem } from "./moderation.ts";
import { publish } from "./publish.ts";
import { getBlob, getItem, importItem, list, mine, versions } from "./read.ts";
import { json, makeCtx, PoolError, rateLimit, readJson, type Ctx, type PoolDeps, type PoolPrincipal } from "./shared.ts";

/** GET/PUT /items/prefs {keep_on_delete}: what happens to my shared items if I delete my account. */
async function getPrefs(c: Ctx, p: PoolPrincipal): Promise<Response> {
  const r = await c.db.prepare(`SELECT keep_on_delete FROM pool_prefs WHERE user_id = ?1`).bind(p.userId).first<{ keep_on_delete: number }>();
  return json(200, { keep_on_delete: r?.keep_on_delete === 1 });
}
async function putPrefs(c: Ctx, req: Request, p: PoolPrincipal): Promise<Response> {
  const b = await readJson(req);
  if (typeof b.keep_on_delete !== "boolean") throw new PoolError(400, "invalid_request", { field: "keep_on_delete" });
  await c.db
    .prepare(`INSERT INTO pool_prefs (user_id, keep_on_delete, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT (user_id) DO UPDATE SET keep_on_delete = ?2, updated_at = ?3`)
    .bind(p.userId, b.keep_on_delete ? 1 : 0, c.now())
    .run();
  return json(200, { keep_on_delete: b.keep_on_delete });
}

export const POOL_METHODS = "GET, POST, PUT, PATCH, DELETE";
const POOL_TRUST = new Set(["teacher", "admin"]);

type Route = (c: Ctx, req: Request, p: PoolPrincipal, m: string[], url: URL) => Promise<Response>;

const ROUTES: { re: RegExp; methods: Record<string, Route> }[] = [
  { re: /^\/items$/, methods: { GET: (c, _r, p, _m, u) => list(c, u, p), POST: (c, r, p) => publish(c, r, p, null) } },
  { re: /^\/items\/mine$/, methods: { GET: (c, _r, p) => mine(c, p) } },
  { re: /^\/items\/prefs$/, methods: { GET: (c, _r, p) => getPrefs(c, p), PUT: (c, r, p) => putPrefs(c, r, p) } },
  { re: /^\/items\/versions$/, methods: { POST: (c, r, p) => versions(c, r, p) } },
  {
    re: /^\/items\/([^/]{1,80})$/,
    methods: {
      GET: (c, _r, p, m) => getItem(c, p, m[1]!),
      PUT: (c, r, p, m) => publish(c, r, p, m[1]!),
      PATCH: (c, r, p, m) => patchItem(c, r, p, m[1]!),
      DELETE: (c, _r, p, m) => deleteItem(c, p, m[1]!),
    },
  },
  { re: /^\/items\/([^/]{1,80})\/undelete$/, methods: { POST: (c, _r, p, m) => undeleteItem(c, p, m[1]!) } },
  { re: /^\/items\/([^/]{1,80})\/import$/, methods: { POST: (c, r, p, m) => importItem(c, r, p, m[1]!) } },
  { re: /^\/items\/([^/]{1,80})\/report$/, methods: { POST: (c, r, p, m) => report(c, r, p, m[1]!) } },
  { re: /^\/items\/([^/]{1,80})\/appeal$/, methods: { POST: (c, r, p, m) => appeal(c, r, p, m[1]!) } },
  { re: /^\/items\/([^/]{1,80})\/counter-notice$/, methods: { POST: (c, r, p, m) => counterNotice(c, r, p, m[1]!) } },
  { re: /^\/items\/([^/]{1,80})\/blobs\/([^/]{1,80})$/, methods: { GET: (c, _r, p, m) => getBlob(c, p, m[1]!, m[2]!) } },
  { re: /^\/blobs\/([^/]{1,80})$/, methods: { PUT: (c, r, p, m) => putBlob(c, r, p, m[1]!) } },
];

export interface Pool {
  /** Handles a pool route; null for any other path. */
  handle(req: Request): Promise<Response | null>;
  /** Admin routes for the auth package's approval config (`routes`). */
  adminRoutes: ReturnType<typeof poolAdminRoutes>;
  hourly(): Promise<unknown>;
  daily(): Promise<unknown>;
}

export function createPool(d: PoolDeps): Pool {
  const ctx = d.r2 ? makeCtx(d, d.r2) : null;
  const allowed = new Set(d.allowedOrigins);
  async function handle(req: Request): Promise<Response | null> {
    const url = new URL(req.url);
    const route = ROUTES.find((r) => r.re.test(url.pathname));
    if (!route) return null;
    try {
      const h = route.methods[req.method];
      if (!h) return json(405, { error: "method_not_allowed" }, { Allow: Object.keys(route.methods).join(", ") });
      const origin = req.headers.get("Origin");
      if (origin !== null && !allowed.has(origin)) throw new PoolError(403, "origin_not_allowed");
      if (req.headers.get("X-Client") !== null && origin !== null) throw new PoolError(400, "native_client_with_origin");
      if (!ctx) throw new PoolError(503, "pool_not_configured");
      const p = await d.authenticate(req);
      if (!p) throw new PoolError(401, "unauthorized", {}, { "WWW-Authenticate": 'Bearer error="invalid_token"' });
      // pending and unverified accounts (and anything else) cannot use the pool at all.
      if (!POOL_TRUST.has(p.trust))
        throw new PoolError(403, "account_not_approved", {
          message: p.trust === "pending" ? "Your account is waiting for approval. The shared pool opens once an admin approves it." : "This account cannot use the shared pool.",
        });
      await rateLimit(ctx, req, p.userId, req.method === "GET" || /\/(import|versions)$/.test(url.pathname));
      return await h(ctx, req, p, route.re.exec(url.pathname)!, url);
    } catch (e) {
      if (e instanceof PoolError) return json(e.status, { error: e.code, ...e.extra }, e.headers);
      console.error(`pool: unhandled error on ${req.method} ${url.pathname}: ${e instanceof Error ? e.message : "?"}`);
      return json(500, { error: "server_error" });
    }
  }
  return {
    handle,
    adminRoutes: poolAdminRoutes(() => ctx),
    hourly: async () => (ctx ? hourly(ctx) : null),
    daily: async () => (ctx ? daily(ctx) : null),
  };
}

/**
 * Account deletion (the auth package's onDelete, run once the account is certainly gone): the
 * author's items are removed at once and purged after 14 days (an item under a DMCA action follows
 * the same date), filed reports are kept anonymised, pending uploads, counters, limits and the
 * importer's per-question records go, and audit notes about the account are nulled.
 */
export function deleteContentStatements(db: D1Database, userId: string, now: number): D1PreparedStatement[] {
  const purgeAt = now + 14 * 86_400_000;
  const keep = `EXISTS (SELECT 1 FROM pool_prefs WHERE user_id = ?1 AND keep_on_delete = 1)`;
  return [
    // The author chose to keep their shared items: they stay, shown as by "deleted teacher", with
    // licences and attribution intact (the chains already hold the display name).
    db.prepare(`UPDATE items SET anonymized = 1, updated_at = ?2 WHERE author_id = ?1 AND removed_at IS NULL AND ${keep}`).bind(userId, now),
    db
      .prepare(`UPDATE items SET removed_at = ?2, removed_reason = 'account_deleted', purge_after = ?3, updated_at = ?2 WHERE author_id = ?1 AND removed_at IS NULL AND NOT ${keep}`)
      .bind(userId, now, purgeAt),
    db
      .prepare(`UPDATE items SET purge_after = MIN(COALESCE(purge_after, ?2), ?2) WHERE author_id = ?1 AND removed_at IS NOT NULL AND purged_at IS NULL`)
      .bind(userId, purgeAt),
    db.prepare(`UPDATE reports SET reporter_id = NULL WHERE reporter_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM pending_blobs WHERE uploader_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM import_questions WHERE user_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM usage_daily WHERE user_id = ?1`).bind(userId),
    db.prepare(`DELETE FROM pool_limits WHERE user_id = ?1`).bind(userId),
    db.prepare(`UPDATE pool_audit SET note = NULL WHERE target_user_id = ?1 AND note IS NOT NULL`).bind(userId),
    db
      .prepare(`INSERT INTO pool_audit (id, at, actor_id, actor_kind, action, target_user_id) VALUES (?1, ?2, ?3, 'user', 'account_deleted.content_removed', ?3)`)
      .bind(crypto.randomUUID(), now, userId),
  ];
}

/**
 * Suspension and restore change nothing in the pool: every non-author read joins the author's
 * standing, so the items hide and show at once. Only the audit row.
 */
export function standingStatements(db: D1Database, userId: string, now: number, action: "author_suspended" | "author_restored"): D1PreparedStatement[] {
  return [db.prepare(`INSERT INTO pool_audit (id, at, actor_kind, action, target_user_id) VALUES (?1, ?2, 'system', ?3, ?4)`).bind(crypto.randomUUID(), now, action, userId)];
}

export type { PoolDeps, PoolPrincipal } from "./shared.ts";
export { makeCore, type ShareCoreExports } from "./core.ts";
