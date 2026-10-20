// verSHion pool API Worker. Phase 1: accounts and auth, via the generic package in ../auth.
// Phase 2: the shared pool (src/pool/), with share-core's wasm behind one adapter (src/pool/core.ts).
import wasm from "../../share-core/pkg/share_core_bg.wasm";
import { argon2id, initSync } from "../../share-core/pkg/share_core.js";
import * as shareCore from "../../share-core/pkg/share_core.js";
import { assertProductionShareCore } from "./share-core-guard.js";
import { cleanText, createAuth, D1RateLimitStore, InputError, ipBucket, type Argon2idHasher, type Auth, type Principal } from "../../auth/src/index.js";
import { staffDomainLabels, staffDomainRoutes } from "./admin.js";
import { onDeleteContent, onRestoreContent, onSuspendContent } from "./cleanup.js";
import { createPool, makeCore, POOL_METHODS, type Pool, type ShareCoreExports } from "./pool/index.ts";
import { poolNotifier } from "./poolmail.ts";
import { adminAlert, makeMailer, queueReminder, type MailEnv } from "./mail.js";
import { flushNoticeBacklog, noticeNewApplication } from "./notices.js";

export interface Env extends MailEnv {
  DB: D1Database;
  /** 32+ random characters; `wrangler secret put AUTH_SECRET`. */
  AUTH_SECRET: string;
  /** Comma-separated exact origins of the web build, e.g. "https://app.example.org". */
  ALLOWED_ORIGINS: string;
  /** "1" sends the admin a daily approval digest even when nothing is overdue. */
  APPROVAL_DIGEST?: string;
  /** R2 bucket for publish bodies, quarantine and images (wrangler.toml [[r2_buckets]] binding "POOL"). */
  POOL?: R2Bucket;
}

/** The trust levels of docs/shared-pool.md, lowest first. */
export const TRUST = ["unverified", "pending", "teacher", "admin"] as const;
export type Trust = (typeof TRUST)[number];

// Refuse to start with a test build of share-core (it would export a caller-seeded strip).
assertProductionShareCore(shareCore);

let wasmReady = false;
function ensureWasm(): void {
  if (!wasmReady) {
    initSync({ module: wasm });
    wasmReady = true;
  }
}
const shareCoreHasher: Argon2idHasher = {
  argon2id(password, salt, p) {
    ensureWasm();
    return argon2id(password, salt, p.m, p.t, p.p);
  },
};

/** Sign-up fields beyond email and password: display name, school, and the adult-employee statement (COPPA/SOPIPA). */
function profile(body: Record<string, unknown>) {
  // Plain text only: no control, format or bidi characters, whitespace collapsed (the package checks
  // again whatever this returns).
  const name = cleanText(body.display_name, 100);
  if (!name) throw new InputError("required_plain_text", "display_name");
  const school = cleanText(body.school, 200);
  if (!school) throw new InputError("required_plain_text", "school");
  if (body.adult_school_employee !== true) throw new InputError("must_confirm", "adult_school_employee");
  return { displayName: name, profile: { school, adult_school_employee_at: new Date().toISOString() } };
}

const origins = (env: Env) => env.ALLOWED_ORIGINS.split(",").map((s) => s.trim()).filter(Boolean);

/** The pool and the auth package, wired together: the pool authenticates through auth, and its admin routes sit behind auth's admin guard. */
function build(env: Env, ctx?: ExecutionContext): { a: Auth; pool: Pool } {
  ensureWasm();
  let a: Auth | null = null;
  const pool = createPool({
    db: env.DB,
    r2: env.POOL,
    core: makeCore(shareCore as unknown as ShareCoreExports),
    secret: env.AUTH_SECRET,
    allowedOrigins: origins(env),
    authenticate: (req) => a!.authenticate(req),
    rateStore: new D1RateLimitStore(env.DB),
    ipBucket: (ip) => ipBucket(ip),
    notify: poolNotifier(env),
    background: (p) => ctx?.waitUntil(p),
  });
  a = auth(env, pool);
  return { a, pool };
}

function auth(env: Env, pool: Pool): Auth {
  const mailer = makeMailer(env);
  return createAuth({
    db: env.DB,
    hasher: shareCoreHasher,
    mailer,
    secret: env.AUTH_SECRET,
    allowedOrigins: origins(env),
    issuer: "vershion-pool",
    audience: "vershion-pool",
    argon2: { m: 19456, t: 2, p: 1 }, // docs/phase1-argon2-spike.md
    trust: { levels: TRUST, afterVerify: "pending", requestApproval: true },
    profile,
    // Every verified account lands in the admin queue and the admin is emailed (no automatic promotion).
    onVerified: async (userId) => void (await noticeNewApplication(env, userId)),
    // The author's pool items are removed at once and purged after 14 days (cleanup.ts).
    onDelete: (userId, now) => onDeleteContent(env.DB, userId, now),
    // Never-confirmed accounts are removed after a week, freeing a squatted address.
    purgeUnverifiedAfterSeconds: 7 * 86400,
    onRefreshReuse: (e) => console.warn(`auth alert: ${e.type} user=${e.userId} family=${e.sessionFamily}`),
    paths: { adminRevoke: "/revoke-teacher" },
    // docs/shared-pool.md "Trust levels": no automatic promotion; an admin approves only on the
    // verified address being on the school's staff directory, or the school office confirming
    // through a contact the admin initiated. A name match is never evidence.
    approval: {
      approvedLevel: "teacher",
      adminLevels: ["admin"],
      evidenceKinds: ["directory", "school_confirmed"],
      reapplyCooldownSeconds: 7 * 86400,
      escalateAfterSeconds: 48 * 3600,
      digest: env.APPROVAL_DIGEST === "1",
      domainLabels: staffDomainLabels(env.DB),
      onQueueReminder: (e) => queueReminder(env, e),
      onAdminAlert: (e) => adminAlert(env, e),
      twoPersonAdminSuspension: true,
      onSuspend: (userId, now) => onSuspendContent(env.DB, userId, now),
      onRestore: (userId, now) => onRestoreContent(env.DB, userId, now),
      routes: [...staffDomainRoutes(env.DB), ...pool.adminRoutes],
    },
  });
}

/** Route guard for later phases: every route other than the account routes needs `teacher`. */
export async function requireTrust(a: Auth, req: Request, min: Trust): Promise<Principal | Response> {
  const p = await a.authenticate(req); // always checks revocation and reads current trust
  if (!p) return Response.json({ error: "unauthorized" }, { status: 401 });
  if (TRUST.indexOf(p.trust as Trust) < TRUST.indexOf(min)) return Response.json({ error: "forbidden" }, { status: 403 });
  return p;
}

export default {
  async fetch(req: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const { a, pool } = build(env, ctx);
    const handled = await a.handle(req, ctx);
    if (handled) return handled;
    const pre = a.preflight(req, POOL_METHODS);
    if (pre) return pre;
    const pooled = await pool.handle(req);
    if (pooled) return a.withCors(req, pooled);
    return a.withCors(req, Response.json({ error: "not_found" }, { status: 404, headers: { "Cache-Control": "no-store" } }));
  },

  async scheduled(c: ScheduledController, env: Env, ctx: ExecutionContext): Promise<void> {
    const { a, pool } = build(env, ctx);
    // Hourly: the summary for sign-up notices over the budget, and the pool's GC.
    await flushNoticeBacklog(env, c.scheduledTime);
    await pool.hourly();
    // Daily (04:xx UTC): purge, and escalate an overdue queue or send the digest.
    if (new Date(c.scheduledTime).getUTCHours() === 4) {
      await a.purgeExpired();
      await a.reviewReminders();
      await pool.daily();
    }
  },
} satisfies ExportedHandler<Env>;
