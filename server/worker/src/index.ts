// verSHion pool API Worker. Phase 1: accounts and auth, via the generic package in ../auth.
import wasm from "../../share-core/pkg/share_core_bg.wasm";
import { argon2id, initSync } from "../../share-core/pkg/share_core.js";
import { cleanText, createAuth, InputError, type Argon2idHasher, type Auth, type Principal } from "../../auth/src/index.js";
import { staffDomainLabels, staffDomainRoutes } from "./admin.js";
import { onDeleteContent, onSuspendContent } from "./cleanup.js";
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
}

/** The trust levels of docs/shared-pool.md, lowest first. */
export const TRUST = ["unverified", "pending", "teacher", "admin"] as const;
export type Trust = (typeof TRUST)[number];

let wasmReady = false;
const shareCoreHasher: Argon2idHasher = {
  argon2id(password, salt, p) {
    if (!wasmReady) {
      initSync({ module: wasm });
      wasmReady = true;
    }
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

function auth(env: Env): Auth {
  const mailer = makeMailer(env);
  return createAuth({
    db: env.DB,
    hasher: shareCoreHasher,
    mailer,
    secret: env.AUTH_SECRET,
    allowedOrigins: env.ALLOWED_ORIGINS.split(",").map((s) => s.trim()).filter(Boolean),
    issuer: "vershion-pool",
    audience: "vershion-pool",
    argon2: { m: 19456, t: 2, p: 1 }, // docs/phase1-argon2-spike.md
    trust: { levels: TRUST, afterVerify: "pending", requestApproval: true },
    profile,
    // Every verified account lands in the admin queue and the admin is emailed (no automatic promotion).
    onVerified: async (userId) => void (await noticeNewApplication(env, userId)),
    // Logs loudly until phase 2 adds the content tables (see cleanup.ts).
    onDelete: onDeleteContent,
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
      onSuspend: onSuspendContent,
      routes: staffDomainRoutes(env.DB),
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
    const a = auth(env);
    const handled = await a.handle(req, ctx);
    if (handled) return handled;
    const pre = a.preflight(req, "GET, POST, PUT, DELETE");
    if (pre) return pre;
    return a.withCors(req, Response.json({ error: "not_found" }, { status: 404, headers: { "Cache-Control": "no-store" } }));
  },

  async scheduled(c: ScheduledController, env: Env): Promise<void> {
    const a = auth(env);
    // Hourly: the summary for sign-up notices over the budget.
    await flushNoticeBacklog(env, c.scheduledTime);
    // Daily (04:xx UTC): purge, and escalate an overdue queue or send the digest.
    if (new Date(c.scheduledTime).getUTCHours() === 4) {
      await a.purgeExpired();
      await a.reviewReminders();
    }
  },
} satisfies ExportedHandler<Env>;
