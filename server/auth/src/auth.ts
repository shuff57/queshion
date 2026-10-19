import { b64url, deriveKeys, fromB64url, hmac, randomToken, sha256, TOKEN_RE, utf8 } from "./crypto.js";
import {
  bearer,
  clearCookie,
  clientInfo,
  corsHeaders,
  empty,
  errorResponse,
  HttpError,
  json,
  normalizeEmail,
  preflight,
  readCookie,
  readJson,
  refreshCookie,
  str,
  withHeaders,
  type ClientInfo,
} from "./http.js";
import { createAdmin, issueSudo, type Admin } from "./admin.js";
import { cleanText, profileIsSafe } from "./validate.js";
import { signAccessToken, verifyAccessToken } from "./jwt.js";
import {
  checkPasswordPolicy,
  dummyVerify,
  hashPassword,
  MAX_PASSWORD_CHARS,
  normalizePassword,
  paramsJson,
  parsePhc,
  sameParams,
  verifyPassword,
} from "./password.js";
import { D1RateLimitStore, ipBucket } from "./ratelimit.js";
import {
  InputError,
  type Action,
  type ActionLimits,
  type Argon2Params,
  type AuthConfig,
  type AuthEvent,
  type MailMessage,
  type Paths,
  type Principal,
  type ProfileInput,
  type QueueStats,
  type RateLimitRule,
  type RateLimitStore,
  type TrustConfig,
  type WaitUntil,
} from "./types.js";

export const DEFAULT_ARGON2: Argon2Params = { m: 19456, t: 2, p: 1 };

export const DEFAULT_PATHS: Paths = {
  signup: "/auth/signup",
  login: "/auth/login",
  refresh: "/auth/refresh",
  logout: "/auth/logout",
  verify: "/auth/verify",
  resend: "/auth/resend",
  forgot: "/auth/forgot",
  reset: "/auth/reset",
  me: "/me",
  account: "/account",
  export: "/account/export",
  enrol: "/auth/device/enrol",
  enrolmentCode: "/auth/device/enrolment-code",
  reauth: "/auth/reauth",
  reapply: "/account/reapply",
  admin: "/admin",
  adminRevoke: "/revoke",
};

const MIN = 60;
const HOUR = 3600;
/** How long a claim on a one-time code lasts before another request may claim it. */
const LEASE_MS = 30_000;
/** Wrong passwords allowed with one enrolment code before it is burned. */
const ENROL_MAX_FAILURES = 5;
/** Enrolment codes per account per 24 h (all sources). */
const ENROL_DAILY = 5;
const DEFAULT_DEVICE_RULES: RateLimitRule[] = [
  { limit: 10, windowSeconds: 15 * MIN },
  { limit: 20, windowSeconds: 24 * HOUR },
];
export const DEFAULT_LIMITS: Record<Action, ActionLimits> = {
  // Generous per IP because a school's staff may share one NAT address.
  signup: { ip: [{ limit: 20, windowSeconds: HOUR }], account: [{ limit: 5, windowSeconds: HOUR }] },
  login: {
    // Every attempt counts per IP (this bounds hashing CPU). Per account only failures count.
    ip: [{ limit: 60, windowSeconds: 5 * MIN }, { limit: 300, windowSeconds: HOUR }],
    // Every failed login (known address or not) gets its per-IP charge back within this separate
    // allowance, so floods of wrong passwords cannot starve real logins on a shared address, and the
    // effect on the budget never depends on whether the address has an account.
    ipFailed: [{ limit: 120, windowSeconds: 5 * MIN }],
    failures: {
      perAccountIp: { limit: 5, windowSeconds: 15 * MIN },
      // 30 an hour stops bursts; 100 a day caps a patient attacker who stays under the hourly rule.
      perAccount: [
        { limit: 30, windowSeconds: HOUR },
        { limit: 100, windowSeconds: 24 * HOUR },
      ],
      perDevice: DEFAULT_DEVICE_RULES,
      // All of an account's devices together; never part of the lockdown that refuses other logins.
      perAccountDevice: [{ limit: 40, windowSeconds: 24 * HOUR }],
    },
  },
  refresh: { ip: [{ limit: 120, windowSeconds: MIN }] },
  // Verify and reset charge the IP only for an invalid token, so junk never blocks a valid link.
  verify: { ip: [{ limit: 30, windowSeconds: HOUR }] },
  // Resend and forgot: only per (IP, address), so junk aimed at other addresses from a shared
  // school address never uses up an owner's allowance. No per-IP cap: no hashing happens there, and
  // a per-IP cap could be filled with random addresses. Per account they use the mail gap and
  // ceiling, never a 429. Volumetric floods belong to Cloudflare WAF / rate-limiting rules, and an
  // optional global `mailBudget` bounds total mail.
  resend: { ipAccount: [{ limit: 5, windowSeconds: HOUR }] },
  forgot: { ipAccount: [{ limit: 5, windowSeconds: HOUR }] },
  // Charged only for an unknown enrolment code.
  enrol: { ip: [{ limit: 20, windowSeconds: HOUR }] },
  // Re-entering the password for a sudo token: its own per-account limiter, like deletion.
  reauth: { ip: [{ limit: 20, windowSeconds: HOUR }], account: [{ limit: 5, windowSeconds: HOUR }, { limit: 10, windowSeconds: 24 * HOUR }] },
  admin: { ip: [{ limit: 600, windowSeconds: 5 * MIN }], account: [{ limit: 600, windowSeconds: 5 * MIN }] },
  reapply: { ip: [{ limit: 20, windowSeconds: HOUR }], account: [{ limit: 5, windowSeconds: 24 * HOUR }] },
  reset: { ip: [{ limit: 20, windowSeconds: HOUR }] },
  delete: { ip: [{ limit: 20, windowSeconds: HOUR }], account: [{ limit: 5, windowSeconds: HOUR }, { limit: 10, windowSeconds: 24 * HOUR }] },
};

const DAY = 86400;
const UA_MAX = 256;
const OK_BODY = { ok: true } as const;

export interface PurgeReport {
  rateLimits: number;
  sessions: number;
  deviceEnrolments: number;
  emailTokens: number;
  tombstoneHashes: number;
  unverifiedAccounts: number;
}

export interface Auth {
  /** Handles an auth route. Returns null for any path that is not one of ours. */
  handle(req: Request, ctx: WaitUntil): Promise<Response | null>;
  /**
   * Checks the bearer access token, then that its session has not been revoked and the account
   * still exists (one indexed D1 read), and returns the trust level as it is now in D1. There is
   * deliberately no token-only mode: nothing can read a stale trust level or a revoked session.
   */
  authenticate(req: Request): Promise<Principal | null>;
  /** CORS for the app's own routes: a preflight answer, or null when `req` is not a preflight. */
  preflight(req: Request, methods: string): Response | null;
  /** Adds the exact-origin CORS headers to a response from one of the app's routes. */
  withCors(req: Request, res: Response): Response;
  /** Deletes expired counters, sessions and tokens, and tombstone email hashes past their time. Run from a cron trigger. */
  purgeExpired(): Promise<PurgeReport>;
  /** Finds a deleted account whose email hash is still held (for re-linking content). */
  findTombstoneByEmail(email: string): Promise<string | null>;
  /**
   * For a cron trigger: the review queue's state; calls `approval.onQueueReminder` when the oldest
   * open application is overdue (escalation), or with a daily digest if enabled. Null without an
   * approval workflow.
   */
  reviewReminders(): Promise<(QueueStats & { sent: "digest" | "escalation" | null }) | null>;
}

interface Resolved {
  db: D1Database;
  cfg: AuthConfig;
  argon2: Argon2Params;
  trust: TrustConfig;
  initialTrust: string;
  paths: Paths;
  limits: Record<Action, ActionLimits>;
  store: RateLimitStore;
  allowed: ReadonlySet<string>;
  cookie: string;
  issuer: string;
  audience: string;
  accessTtl: number;
  refreshTtl: number;
  sessionMax: number;
  graceMs: number;
  deviceTtl: number;
  deviceCookie: string;
  mailGapMs: number;
  mailCeiling: number;
  outstanding: number;
  enrolTtl: number;
  verifyTtl: number;
  resetTtl: number;
  tombstoneTtl: number;
  minPassword: number;
  extraCommon: ReadonlySet<string>;
  now: () => number;
}

function resolve(cfg: AuthConfig): Resolved {
  if (typeof cfg.secret !== "string" || cfg.secret.length < 32) throw new Error("auth: secret must be at least 32 characters");
  for (const o of cfg.allowedOrigins) {
    let parsed: URL;
    try {
      parsed = new URL(o);
    } catch {
      throw new Error(`auth: allowed origin is not a URL: ${o}`);
    }
    if (o.includes("*") || parsed.origin !== o) throw new Error(`auth: allowed origin must be an exact origin: ${o}`);
  }
  const levels = cfg.trust?.levels ?? ["unverified", "verified"];
  const trust: TrustConfig = {
    levels,
    afterVerify: cfg.trust?.afterVerify ?? levels[1] ?? levels[0]!,
    requestApproval: cfg.trust?.requestApproval ?? false,
  };
  if (levels.length === 0 || !levels.includes(trust.afterVerify)) throw new Error("auth: trust.afterVerify must be one of trust.levels");
  const limits = { ...DEFAULT_LIMITS };
  for (const [k, v] of Object.entries(cfg.rateLimits ?? {})) limits[k as Action] = v!;
  return {
    db: cfg.db,
    cfg,
    argon2: cfg.argon2 ?? DEFAULT_ARGON2,
    trust,
    initialTrust: levels[0]!,
    paths: { ...DEFAULT_PATHS, ...cfg.paths },
    limits,
    store: cfg.rateLimitStore ?? new D1RateLimitStore(cfg.db),
    allowed: new Set(cfg.allowedOrigins),
    cookie: cfg.cookieName ?? "__Host-refresh",
    issuer: cfg.issuer ?? "d1-auth",
    audience: cfg.audience ?? "d1-auth",
    accessTtl: cfg.accessTokenTtlSeconds ?? 15 * MIN,
    refreshTtl: cfg.refreshTokenTtlSeconds ?? 30 * DAY,
    sessionMax: cfg.sessionMaxAgeSeconds ?? 90 * DAY,
    graceMs: (cfg.reuseGraceSeconds ?? 10) * 1000,
    deviceTtl: cfg.knownDeviceTtlSeconds ?? 90 * DAY,
    deviceCookie: cfg.deviceCookieName ?? "__Host-device",
    mailGapMs: (cfg.mailGapSeconds ?? 120) * 1000,
    mailCeiling: cfg.mailCeiling ?? 10,
    outstanding: Math.max(1, cfg.outstandingLinks ?? 3),
    enrolTtl: cfg.enrolCodeTtlSeconds ?? 15 * MIN,
    verifyTtl: cfg.verifyTokenTtlSeconds ?? DAY,
    resetTtl: cfg.resetTokenTtlSeconds ?? HOUR,
    tombstoneTtl: cfg.tombstoneEmailTtlSeconds ?? 14 * DAY,
    minPassword: cfg.minPasswordLength ?? 10,
    extraCommon: new Set((cfg.extraCommonPasswords ?? []).map((s) => s.toLowerCase())),
    now: cfg.now ?? Date.now,
  };
}

type Handler = (req: Request, client: ClientInfo, ctx: WaitUntil) => Promise<Response>;

export function createAuth(config: AuthConfig): Auth {
  const r = resolve(config);
  const { db } = r;
  const keysP = deriveKeys(config.secret);

  const emit = (e: AuthEvent) => {
    try {
      config.onEvent?.(e);
    } catch {
      /* audit hooks must never break auth */
    }
  };

  /** A replay after the grace window (a likely stolen token): audit stream plus the alert hook. */
  const reuse = (e: AuthEvent) => {
    emit(e);
    try {
      config.onRefreshReuse?.(e);
    } catch {
      /* alert hooks must never break auth */
    }
  };

  const background = (ctx: WaitUntil, p: Promise<unknown>, what: string) => {
    ctx.waitUntil(
      p.catch(() => {
        // Never log the error object: a mail provider error can echo the address or token.
        console.error(`auth: background ${what} failed`);
      }),
    );
  };

  const mail = (ctx: WaitUntil, msg: MailMessage) => {
    let p: Promise<void>;
    try {
      p = config.mailer.send(msg);
    } catch (e) {
      p = Promise.reject(e);
    }
    background(ctx, p, `mail:${msg.kind}`);
  };

  /** The client's IP, or a 403: requests with no known IP are never pooled into one shared bucket. */
  const ipOf = (req: Request): string => {
    const ip = config.clientIp ? config.clientIp(req) : req.headers.get("CF-Connecting-IP");
    if (!ip || !ip.trim()) throw new HttpError(403, "client_ip_required");
    return ip.trim();
  };

  /**
   * Every counter that can identify an account starts with this prefix, so deleting the account
   * purges them all with one range delete.
   */
  async function acctPrefix(account: string): Promise<string> {
    const keys = await keysP;
    return `a:${await hmac(keys.limit, account)}:`;
  }

  /**
   * Rate limit BEFORE any expensive work; throws 429. Returns a refund: a caller that ends up doing
   * no expensive work (for example a login the failure check refuses) gives the hits back, so a
   * shared address's budget only counts work actually done.
   * - `ip`: per client IP (IPv6 /64).
   * - `account`: per account, every attempt.
   * - `ipAccount`: per (IP, account) pair, so junk aimed at other addresses from a shared IP never
   *   uses up a real owner's allowance.
   */
  async function limit(action: Action, req: Request, account: string | null): Promise<() => Promise<void>> {
    const keys = await keysP;
    const rules = r.limits[action];
    const now = r.now();
    const hits: { key: string; rule: RateLimitRule }[] = [];
    const ipH = await hmac(keys.limit, ipBucket(ipOf(req))); // fails closed before any counting
    for (const rule of rules.ip ?? []) hits.push({ key: `i:${ipH}:${action}:${rule.windowSeconds}`, rule });
    if (account !== null) {
      const pre = await acctPrefix(account);
      for (const rule of rules.account ?? []) hits.push({ key: `${pre}${action}:${rule.windowSeconds}`, rule });
      for (const rule of rules.ipAccount ?? []) hits.push({ key: `${pre}${action}:ip:${rule.windowSeconds}:${ipH}`, rule });
    }
    const refund = async () => {
      if (r.store.undo && hits.length > 0) await Promise.all(hits.map((h) => r.store.undo!(h.key, h.rule, now)));
    };
    if (hits.length === 0) return refund;
    const results = await Promise.all(hits.map((h) => r.store.hit(h.key, h.rule, now)));
    const denied = results.filter((x) => !x.allowed);
    if (denied.length === 0) return refund;
    await refund();
    emit({ type: "rate_limited", action });
    const retry = Math.max(...denied.map((d) => d.retryAfter));
    throw new HttpError(429, "rate_limited", {}, { "Retry-After": String(retry) });
  }

  // ---- known devices -------------------------------------------------------------------
  // A stateless token, `v1.<userId>.<epoch>.<expSeconds>.<nonce>.<mac>`, issued at each successful
  // login, reset, activation and enrolment. The MAC key is separate from every other key. Bumping
  // users.device_epoch revokes them all. users.last_device holds the nonce of the most recently
  // issued one: the owner's current device.

  interface Device {
    token: string;
    nonce: string;
  }

  async function issueDeviceToken(userId: string, epoch: number): Promise<Device> {
    const keys = await keysP;
    // The random nonce makes every issued token distinct, so two devices that log in during the
    // same second never share a token (or its per-device failure counter).
    const nonce = b64url(crypto.getRandomValues(new Uint8Array(9)));
    const payload = `v1.${userId}.${epoch}.${Math.floor(r.now() / 1000) + r.deviceTtl}.${nonce}`;
    return { token: `${payload}.${await hmac(keys.device, payload)}`, nonce };
  }

  async function parseDevice(token: string | null, user: { id: string; device_epoch: number } | null): Promise<Device | null> {
    if (!token || !user) return null;
    const m = /^(v1\.([0-9a-f-]{36})\.(\d{1,9})\.(\d{1,12})\.([A-Za-z0-9_-]{12}))\.([A-Za-z0-9_-]{43})$/.exec(token);
    if (!m) return null;
    const [, payload, uid, epoch, exp, nonce, mac] = m as unknown as [string, string, string, string, string, string, string];
    // Per account: a token for one account never unlocks another.
    if (uid !== user.id || Number(epoch) !== user.device_epoch || Number(exp) <= Math.floor(r.now() / 1000)) return null;
    const keys = await keysP;
    return (await crypto.subtle.verify("HMAC", keys.device, fromB64url(mac), utf8(payload))) ? { token, nonce } : null;
  }

  function presentedDevice(req: Request, client: ClientInfo): string | null {
    return client.mode === "native" ? req.headers.get("X-Device-Token") : readCookie(req, r.deviceCookie);
  }

  type Slot = { key: string; rule: RateLimitRule };
  const rulesOf = (x: RateLimitRule | RateLimitRule[] | undefined, fallback: RateLimitRule[] = []): RateLimitRule[] =>
    x === undefined ? fallback : Array.isArray(x) ? x : [x];

  /**
   * Login failure limits, checked before the hash. A failure slot is reserved atomically before
   * the hash and given back on success, so concurrent guesses cannot overshoot and only failures
   * count.
   * - Without a device token: the (account, network /48) pair and every per-account rule must
   *   hold. Past a per-account rule the account is locked down for every network.
   * - With a valid known-device token for this account: none of those apply, so the owner is never
   *   refused for other people's failures. The device's own failures count per device
   *   (`perDevice`). Devices other than the owner's current one (`last_device`) also share
   *   `perAccountDevice`, which bounds several stolen tokens together; the current device is exempt,
   *   so stolen tokens cannot use that shared cap to lock the owner out. None of these feed the
   *   lockdown that refuses everyone else.
   * Every key includes the account's `device_epoch`. A reset, activation or logout-everywhere bumps
   * it in its own batch, which re-keys all of the account's counters at once: no separate clearing
   * step and no race. Unknown addresses use epoch 0, so refusals look the same for them.
   */
  async function reserveFailure(
    req: Request,
    account: string,
    epoch: number,
    device: Device | null,
    lastDevice: string | null,
  ): Promise<() => Promise<void>> {
    const f = r.limits.login.failures;
    if (!f) return async () => {};
    const keys = await keysP;
    const now = r.now();
    const pre = await acctPrefix(account);
    const e = `e${epoch}`;
    let slots: Slot[];
    if (device) {
      const dh = await hmac(keys.limit, device.token);
      slots = rulesOf(f.perDevice, DEFAULT_DEVICE_RULES).map((rule) => ({ key: `${pre}lf:dev:${dh}:${rule.windowSeconds}`, rule }));
      if (device.nonce !== lastDevice)
        slots.push(...rulesOf(f.perAccountDevice).map((rule) => ({ key: `${pre}lf:devacct:${e}:${rule.windowSeconds}`, rule })));
    } else {
      const ipH = await hmac(keys.limit, ipBucket(ipOf(req), 48));
      slots = [
        { key: `${pre}lf:pair:${e}:${f.perAccountIp.windowSeconds}:${ipH}`, rule: f.perAccountIp },
        ...rulesOf(f.perAccount).map((rule) => ({ key: `${pre}lf:acct:${e}:${rule.windowSeconds}`, rule })),
      ];
    }
    const results = await Promise.all(slots.map((x) => r.store.hit(x.key, x.rule, now)));
    const release = async () => {
      if (r.store.undo) await Promise.all(slots.map((x) => r.store.undo!(x.key, x.rule, now)));
    };
    const denied = results.filter((x) => !x.allowed);
    if (denied.length === 0) return release;
    await release();
    emit({ type: "rate_limited", action: "login" });
    throw new HttpError(429, "rate_limited", {}, { "Retry-After": String(Math.max(...denied.map((d) => d.retryAfter))) });
  }

  /** The presented device token, if it is valid for this user; else null. */
  function validDevice(req: Request, client: ClientInfo, user: { id: string; device_epoch: number } | null): Promise<Device | null> {
    return parseDevice(presentedDevice(req, client), user);
  }

  /** Issues a device token and records it as the account's current device. */
  async function newCurrentDevice(userId: string, epoch: number): Promise<Device> {
    const d = await issueDeviceToken(userId, epoch);
    await db.prepare(`UPDATE users SET last_device = ?1 WHERE id = ?2 AND device_epoch = ?3`).bind(d.nonce, userId, epoch).run();
    return d;
  }

  /** Puts a freshly issued device token on a response: web cookie, or native body field. */
  async function withDevice(res: Response, client: ClientInfo, body: Record<string, unknown>, userId: string, epoch: number): Promise<Response> {
    const { token } = await newCurrentDevice(userId, epoch);
    if (client.mode === "native") {
      const out = json(res.status, { ...body, device_token: token });
      res.headers.forEach((v, k) => {
        if (k.toLowerCase() !== "content-type" && k.toLowerCase() !== "content-length") out.headers.set(k, v);
      });
      return out;
    }
    res.headers.append("Set-Cookie", refreshCookie(r.deviceCookie, token, r.deviceTtl));
    return res;
  }

  // ---- leases on one-time codes ----------------------------------------------------------
  // A valid code is claimed with a short lease BEFORE any password hash, so N concurrent requests
  // with one code cost one hash. Only the code's holder (the mailbox owner) can claim it, and the
  // lease lasts LEASE_MS, so it cannot keep the owner out.

  async function claimEmailToken(tokenHash: string, kind: "verify" | "reset", now: number): Promise<void> {
    const got = await db
      .prepare(
        `UPDATE email_tokens SET claimed_until = ?3
         WHERE token_hash = ?1 AND kind = ?4 AND used_at IS NULL AND expires_at > ?2 AND (claimed_until IS NULL OR claimed_until <= ?2)
         RETURNING id`,
      )
      .bind(tokenHash, now, now + LEASE_MS, kind)
      .first();
    if (got) return;
    // Still valid but leased: another request with this same token is being processed right now
    // (a double click). Only someone holding the token can see this.
    const held = await db
      .prepare(`SELECT 1 FROM email_tokens WHERE token_hash = ?1 AND used_at IS NULL AND claimed_until > ?2`)
      .bind(tokenHash, now)
      .first();
    throw held ? new HttpError(409, "in_progress") : new HttpError(400, "invalid_token");
  }

  async function releaseEmailToken(tokenHash: string): Promise<void> {
    await db.prepare(`UPDATE email_tokens SET claimed_until = NULL WHERE token_hash = ?1 AND used_at IS NULL`).bind(tokenHash).run();
  }

  /**
   * Mails a one-time enrolment code that a native app redeems (with the password) for a device
   * token. With `limited`, the insert itself enforces the mail gap and the daily cap (so concurrent
   * requests cannot exceed them) and returns whether a code was issued.
   */
  async function issueEnrolment(ctx: WaitUntil, userId: string, email: string, epoch: number, limited = false): Promise<boolean> {
    const code = randomToken();
    const now = r.now();
    const expiresAt = now + r.enrolTtl * 1000;
    const guard = limited
      ? `WHERE NOT EXISTS (SELECT 1 FROM device_enrolments g WHERE g.user_id = ?2 AND g.created_at > ?5 - ?7)
           AND (SELECT COUNT(*) FROM device_enrolments d WHERE d.user_id = ?2 AND d.created_at > ?5 - ?8) < ?9`
      : "";
    const ins = await db
      .prepare(`INSERT INTO device_enrolments (id, user_id, code_hash, epoch, created_at, expires_at) SELECT ?1, ?2, ?3, ?4, ?5, ?6 ${guard} RETURNING id`)
      .bind(crypto.randomUUID(), userId, await sha256(code), epoch, now, expiresAt, ...(limited ? [r.mailGapMs, DAY * 1000, ENROL_DAILY] : []))
      .first();
    if (!ins) return false;
    mail(ctx, { kind: "device_enrol", to: email, token: code, expiresAt });
    return true;
  }

  function ua(req: Request): string | null {
    const v = req.headers.get("User-Agent");
    return v ? v.slice(0, UA_MAX) : null;
  }

  function readPassword(body: Record<string, unknown>, key = "password"): string {
    // Bound before normalizing; NFKC can grow a string.
    return normalizePassword(str(body, key, MAX_PASSWORD_CHARS * 4)!);
  }

  function policy(password: string, email: string): void {
    const why = checkPasswordPolicy(password, email, r.minPassword, r.extraCommon);
    if (why) throw new HttpError(400, "weak_password", { reason: why });
  }

  async function extraPasswordCheck(password: string, email: string): Promise<void> {
    if (!config.passwordCheck) return;
    const why = await config.passwordCheck(password, email);
    if (why) throw new HttpError(400, "weak_password", { reason: why });
  }

  /**
   * Profile fields, validated whatever the app's hook returns: the display name is NFKC-normalized,
   * whitespace-collapsed, at most 100 characters and free of control and format characters (no line
   * breaks, bidi overrides or zero-width characters to fake labels or hide text in an admin's
   * queue); every string in the app's profile JSON is free of them too, and the whole stays small.
   */
  function readProfile(body: Record<string, unknown>): ProfileInput {
    let out: ProfileInput;
    try {
      if (config.profile) out = config.profile(body);
      else {
        const raw = body.display_name;
        out = { displayName: raw === undefined || raw === null || raw === "" ? null : (raw as string), profile: null };
      }
    } catch (e) {
      if (e instanceof InputError) throw new HttpError(400, "invalid_request", e.field ? { field: e.field, reason: e.code } : { reason: e.code });
      throw e;
    }
    let displayName: string | null = null;
    if (out.displayName !== null) {
      displayName = cleanText(out.displayName, 100);
      if (displayName === null) throw new HttpError(400, "invalid_request", { field: "display_name", reason: "invalid_characters_or_length" });
    }
    if (!profileIsSafe(out.profile)) throw new HttpError(400, "invalid_request", { field: "profile", reason: "invalid_characters_or_length" });
    return { displayName, profile: out.profile };
  }

  // ---- tokens ----------------------------------------------------------------------------

  async function accessToken(userId: string, family: string, trust: string): Promise<string> {
    const keys = await keysP;
    const iat = Math.floor(r.now() / 1000);
    return signAccessToken(keys.access, {
      iss: r.issuer,
      aud: r.audience,
      sub: userId,
      sid: family,
      trust,
      iat,
      exp: iat + r.accessTtl,
      jti: crypto.randomUUID(),
    });
  }

  /** Builds the token response. Web: refresh token only in the cookie. Native: in the body. */
  async function tokenResponse(
    client: ClientInfo,
    user: { id: string; trust: string },
    family: string,
    refresh: string,
    refreshExpiresAt: number,
    deviceToken: string | null = null,
  ): Promise<Response> {
    const at = await accessToken(user.id, family, user.trust);
    const body: Record<string, unknown> = {
      access_token: at,
      token_type: "Bearer",
      expires_in: r.accessTtl,
      user: { id: user.id, trust: user.trust },
    };
    const maxAge = Math.max(0, Math.floor((refreshExpiresAt - r.now()) / 1000));
    if (client.mode === "native") {
      body.refresh_token = refresh;
      body.refresh_expires_in = maxAge;
      if (deviceToken) body.device_token = deviceToken; // keep in the keychain; send as X-Device-Token
      return json(200, body);
    }
    const res = json(200, body, { "Set-Cookie": refreshCookie(r.cookie, refresh, maxAge) });
    if (deviceToken) res.headers.append("Set-Cookie", refreshCookie(r.deviceCookie, deviceToken, r.deviceTtl));
    return res;
  }

  function refreshFailure(client: ClientInfo): HttpError {
    return new HttpError(401, "invalid_refresh_token", {}, client.mode === "web" ? { "Set-Cookie": clearCookie(r.cookie) } : {});
  }

  /** Cookie-carrying endpoints in web mode must come from an allowed browser origin (CSRF defence in depth). */
  function requireOriginForCookie(client: ClientInfo): void {
    if (client.mode === "web" && client.origin === null) throw new HttpError(403, "origin_required");
  }

  async function presentedRefresh(req: Request, client: ClientInfo, body: Record<string, unknown> | null): Promise<string | null> {
    // Web clients never send the token in a body and native clients never use the cookie.
    const raw = client.mode === "native" ? (body?.refresh_token ?? null) : readCookie(req, r.cookie);
    return typeof raw === "string" && TOKEN_RE.test(raw) ? raw : null;
  }

  // ---- handlers --------------------------------------------------------------------------

  const signup: Handler = async (req, _client, ctx) => {
    const body = await readJson(req);
    const email = normalizeEmail(str(body, "email", 320)!);
    const password = readPassword(body);
    const prof = readProfile(body);
    policy(password, email);
    await limit("signup", req, `email:${email}`);
    await extraPasswordCheck(password, email);
    // Always hash, whether or not the address has an account, so timing is the same.
    const hashed = await hashPassword(config.hasher, password, r.argon2);
    const now = r.now();
    const userId = crypto.randomUUID();
    const token = randomToken();
    const expiresAt = now + r.verifyTtl * 1000;
    // One round trip in both cases: the insert is a no-op for an existing address.
    const [ins, , existing] = await db.batch<{ id: string; email_verified_at: number | null }>([
      db
        .prepare(
          `INSERT INTO users (id, email, pw_hash, pw_params, display_name, profile, trust, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT (email) DO NOTHING RETURNING id`,
        )
        .bind(
          userId,
          email,
          hashed.phc,
          paramsJson(hashed.params),
          prof.displayName,
          prof.profile === null ? null : JSON.stringify(prof.profile),
          r.initialTrust,
          now,
        ),
      db
        .prepare(
          `INSERT INTO email_tokens (id, user_id, kind, token_hash, created_at, expires_at)
           SELECT ?1, id, 'verify', ?2, ?3, ?4 FROM users WHERE id = ?5`,
        )
        .bind(crypto.randomUUID(), await sha256(token), now, expiresAt, userId),
      db.prepare(`SELECT id, email_verified_at FROM users WHERE email = ?1`).bind(email),
    ]);
    if (ins!.results.length === 1) {
      mail(ctx, { kind: "verify", to: email, token, expiresAt });
      emit({ type: "signup", userId });
    } else {
      const row = existing!.results[0];
      mail(ctx, { kind: "signup_existing", to: email, accountVerified: row ? row.email_verified_at !== null : false });
    }
    return json(202, OK_BODY);
  };

  /** Confirms the address of the account picked by `userIdSql` (idempotent). */
  function confirmEmail(userIdSql: string, binds: unknown[], now: number): D1PreparedStatement {
    const n = binds.length;
    return db
      .prepare(`UPDATE users SET email_verified_at = COALESCE(email_verified_at, ?${n + 1}) WHERE id = (${userIdSql}) AND deleted_at IS NULL`)
      .bind(...binds, now);
  }

  /**
   * Activates an account: confirmed address, a password and profile set by the mailbox owner. Moves
   * it from the initial trust level to `afterVerify` and opens its approval request. The UPDATE's
   * RETURNING says whether this request did it, so onVerified runs exactly once per account.
   */
  function activate(userIdSql: string, binds: unknown[], now: number): D1PreparedStatement[] {
    const n = binds.length;
    const stmts = [
      db
        .prepare(
          `UPDATE users SET trust = ?${n + 1}
           WHERE id = (${userIdSql}) AND deleted_at IS NULL AND trust = ?${n + 2}
             AND email_verified_at IS NOT NULL AND pw_hash IS NOT NULL RETURNING id`,
        )
        .bind(...binds, r.trust.afterVerify, r.initialTrust),
    ];
    if (r.trust.requestApproval)
      stmts.push(
        db
          .prepare(
            `INSERT OR IGNORE INTO approval_requests (id, user_id, requested_at, email_domain, display_name_snapshot, profile_snapshot)
             SELECT ?${n + 3}, id, ?${n + 1}, substr(email, instr(email, '@') + 1), display_name, profile
             FROM users WHERE id = (${userIdSql}) AND deleted_at IS NULL AND trust = ?${n + 2}
               AND NOT EXISTS (SELECT 1 FROM approval_requests WHERE user_id = users.id)`,
          )
          .bind(...binds, now, r.trust.afterVerify, crypto.randomUUID()),
      );
    return stmts;
  }

  const activated = (ctx: WaitUntil, userId: string) => {
    emit({ type: "activated", userId });
    if (config.onVerified) background(ctx, config.onVerified(userId), "onVerified");
  };

  /**
   * Confirms an address. Whoever registered the account may not be whoever owns the mailbox (a
   * student can sign up with a teacher's address), so confirming discards everything chosen before
   * confirmation: the password, the display name and profile, every session and device token. The
   * person who clicked the link either sends `password` plus the profile fields with the token
   * (activating the account in one step) or, without them, completes it through forgot/reset.
   */
  const verify: Handler = async (req, client, ctx) => {
    const body = await readJson(req);
    const token = body.token;
    if (typeof token !== "string" || !TOKEN_RE.test(token)) throw new HttpError(400, "invalid_token");
    const password = body.password === undefined || body.password === null ? null : readPassword(body);
    // Choosing a password here means entering the profile here too (the old one may be a student's).
    const prof = password === null ? null : readProfile(body);
    ipOf(req); // fail closed with no client IP
    const now = r.now();
    const th = await sha256(token);
    // The token is looked up first (by its hash). Only an invalid token is charged to the IP, so
    // junk from a shared school address can never make a valid link fail; work for valid links
    // is bounded by the mail gap and ceiling, and by the lease below.
    const row = await db
      .prepare(
        `SELECT u.email FROM email_tokens t JOIN users u ON u.id = t.user_id
         WHERE t.token_hash = ?1 AND t.kind = 'verify' AND t.used_at IS NULL AND t.expires_at > ?2 AND u.deleted_at IS NULL`,
      )
      .bind(th, now)
      .first<{ email: string }>();
    if (!row) {
      await limit("verify", req, null);
      throw new HttpError(400, "invalid_token");
    }
    let hashed: { phc: string; params: Argon2Params } | null = null;
    let leased = false;
    try {
      if (password !== null) {
        policy(password, row.email);
        await extraPasswordCheck(password, row.email);
        await claimEmailToken(th, "verify", now); // concurrent duplicates stop here, unhashed
        leased = true;
        hashed = await hashPassword(config.hasher, password, r.argon2);
      }
      const nonce = crypto.randomUUID();
      const consumed = `SELECT user_id FROM email_tokens WHERE used_nonce = ?1`;
      const results = await db.batch<{ user_id?: string; id?: string }>([
        db
          .prepare(
            `UPDATE email_tokens SET used_at = ?3, used_nonce = ?1
             WHERE token_hash = ?2 AND kind = 'verify' AND used_at IS NULL AND expires_at > ?3 RETURNING user_id`,
          )
          .bind(nonce, th, now),
        // Everything set before confirmation is replaced (or dropped), in one statement.
        db
          .prepare(
            `UPDATE users SET pw_hash = ?2, pw_params = ?3, display_name = ?4, profile = ?5, device_epoch = device_epoch + 1
             WHERE id = (${consumed}) AND deleted_at IS NULL AND email_verified_at IS NULL RETURNING device_epoch`,
          )
          .bind(
            nonce,
            hashed?.phc ?? null,
            hashed ? paramsJson(hashed.params) : null,
            prof?.displayName ?? null,
            prof?.profile == null ? null : JSON.stringify(prof.profile),
          ),
        confirmEmail(consumed, [nonce], now),
        ...activate(consumed, [nonce], now),
        // Any session from before confirmation ends.
        db.prepare(`UPDATE sessions SET revoked_at = ?2 WHERE revoked_at IS NULL AND user_id = (${consumed})`).bind(nonce, now),
        // Every other outstanding link stops working too, including reset links requested before
        // confirmation (whoever completes the account does it here or with a fresh link).
        db.prepare(`UPDATE email_tokens SET used_at = ?2 WHERE used_at IS NULL AND user_id = (${consumed})`).bind(nonce, now),
      ]);
      const userId = results[0]!.results[0]?.user_id;
      if (!userId) throw new HttpError(400, "invalid_token");
      leased = false; // consumed
      emit({ type: "verified", userId });
      const reply = { ok: true, password_set: hashed !== null };
      if (results[3]!.results.length !== 1) return json(200, reply);
      activated(ctx, userId);
      // The mailbox owner just set the password: the epoch bump in this batch re-keyed every failure
      // counter (ending any lockdown planted before activation), this becomes a known device, and
      // an enrolment code is mailed for the owner's other devices (the native app).
      const epoch = (results[1]!.results[0] as { device_epoch?: number } | undefined)?.device_epoch;
      if (epoch === undefined) return json(200, reply);
      await issueEnrolment(ctx, userId, row.email, epoch);
      return withDevice(json(200, reply), client, reply, userId, epoch);
    } finally {
      if (leased) await releaseEmailToken(th);
    }
  };

  /**
   * Shared body of resend and forgot: one batch whatever the address, and always the same reply.
   * There is no per-IP cap (no hashing happens here, and a per-IP cap could be filled from a shared
   * school address with random addresses to block the owner). The only rate limit is per (IP,
   * address). Per account there is no 429 to fill up: a mail goes out unless one of this kind went
   * out in the last `mailGapSeconds` or `mailCeiling` went out within one link lifetime, and the
   * newest `outstandingLinks` links all stay valid. An optional global `mailBudget` (not keyed by
   * IP) bounds total mail; volumetric floods belong to Cloudflare WAF / rate-limiting rules.
   */
  async function issueMailToken(
    req: Request,
    ctx: WaitUntil,
    action: "resend" | "forgot",
    kind: "verify" | "reset",
    userFilter: string,
    ttlSeconds: number,
  ): Promise<Response> {
    const body = await readJson(req);
    const email = normalizeEmail(str(body, "email", 320)!);
    await limit(action, req, `email:${email}`);
    const now = r.now();
    const token = randomToken();
    const tokenId = crypto.randomUUID();
    const expiresAt = now + ttlSeconds * 1000;
    const target = `SELECT id FROM users WHERE email = ?1 AND deleted_at IS NULL ${userFilter}`;
    // The global mail budget is taken BEFORE anything changes. Over budget, nothing happens at all:
    // no new link, and every link already mailed stays valid (the owner still has those).
    const budget = config.mailBudget;
    if (budget && !(await r.store.hit("g:mail", budget, now)).allowed) {
      await r.store.undo?.("g:mail", budget, now);
      emit({ type: "rate_limited", action });
      return json(202, OK_BODY);
    }
    const [ins] = await db.batch<{ user_id: string }>([
      db
        .prepare(
          `INSERT INTO email_tokens (id, user_id, kind, token_hash, created_at, expires_at)
           SELECT ?2, u.id, ?3, ?4, ?5, ?6 FROM users u WHERE u.email = ?1 AND u.deleted_at IS NULL ${userFilter}
             AND NOT EXISTS (SELECT 1 FROM email_tokens g WHERE g.user_id = u.id AND g.kind = ?3 AND g.created_at > ?5 - ?7)
             AND (SELECT COUNT(*) FROM email_tokens d WHERE d.user_id = u.id AND d.kind = ?3 AND d.created_at > ?5 - ?8) < ?9
           RETURNING user_id`,
        )
        .bind(email, tokenId, kind, await sha256(token), now, expiresAt, r.mailGapMs, ttlSeconds * 1000, r.mailCeiling),
      // Only the newest few links work; older ones are retired.
      db
        .prepare(
          `UPDATE email_tokens SET used_at = ?2 WHERE kind = ?3 AND used_at IS NULL AND user_id = (${target})
             AND rowid NOT IN (SELECT rowid FROM email_tokens WHERE kind = ?3 AND used_at IS NULL AND user_id = (${target})
                               ORDER BY created_at DESC, rowid DESC LIMIT ?4)`,
        )
        .bind(email, now, kind, r.outstanding),
    ]);
    const userId = ins!.results[0]?.user_id;
    // No mail went out (no such account, or the gap or ceiling held): the budget unit goes back, in
    // the background so the reply's timing does not depend on it.
    if (!userId && budget && r.store.undo) background(ctx, r.store.undo("g:mail", budget, now), "mailBudget");
    if (userId) {
      mail(ctx, { kind, to: email, token, expiresAt });
      if (kind === "reset") emit({ type: "reset_requested", userId });
    }
    return json(202, OK_BODY);
  }

  const resend: Handler = (req, _c, ctx) => issueMailToken(req, ctx, "resend", "verify", "AND email_verified_at IS NULL", r.verifyTtl);

  // A reset is allowed for an account not yet active too: completing it proves control of the
  // address, and it is how a real owner takes back an address someone else registered.
  const forgot: Handler = (req, _c, ctx) => issueMailToken(req, ctx, "forgot", "reset", "", r.resetTtl);

  const reset: Handler = async (req, client, ctx) => {
    const body = await readJson(req);
    const token = body.token;
    if (typeof token !== "string" || !TOKEN_RE.test(token)) throw new HttpError(400, "invalid_token");
    const password = readPassword(body);
    ipOf(req); // fail closed with no client IP
    const now = r.now();
    const th = await sha256(token);
    // Looked up first: a bad token never costs a hash, and only a bad token is charged to the IP,
    // so junk from a shared school address can never make a valid reset link fail.
    const row = await db
      .prepare(
        `SELECT t.user_id, u.email, u.trust FROM email_tokens t JOIN users u ON u.id = t.user_id
         WHERE t.token_hash = ?1 AND t.kind = 'reset' AND t.used_at IS NULL AND t.expires_at > ?2 AND u.deleted_at IS NULL`,
      )
      .bind(th, now)
      .first<{ user_id: string; email: string; trust: string }>();
    if (!row) {
      await limit("reset", req, null);
      throw new HttpError(400, "invalid_token");
    }
    // An account not yet active gets its profile from the mailbox owner here, never from sign-up.
    // Trust only ever moves up, so "already active" read here is still true at the batch.
    const needsProfile = row.trust === r.initialTrust;
    const prof = needsProfile ? readProfile(body) : null;
    policy(password, row.email);
    await extraPasswordCheck(password, row.email);
    await claimEmailToken(th, "reset", now); // concurrent duplicates stop here, unhashed
    let leased = true;
    try {
      const hashed = await hashPassword(config.hasher, password, r.argon2);
      const nonce = crypto.randomUUID();
      const consumed = `SELECT user_id FROM email_tokens WHERE used_nonce = ?1`;
      const setProfile = prof ? ", display_name = ?4, profile = ?5" : "";
      const profBinds = prof ? [prof.displayName, prof.profile == null ? null : JSON.stringify(prof.profile)] : [];
      const results = await db.batch<{ user_id?: string; id?: string }>([
        db
          .prepare(
            `UPDATE email_tokens SET used_at = ?3, used_nonce = ?1
             WHERE token_hash = ?2 AND kind = 'reset' AND used_at IS NULL AND expires_at > ?3 RETURNING user_id`,
          )
          .bind(nonce, th, now),
        db
          .prepare(
            `UPDATE users SET pw_hash = ?2, pw_params = ?3${setProfile}, device_epoch = device_epoch + 1
             WHERE id = (${consumed}) AND deleted_at IS NULL RETURNING device_epoch`,
          )
          .bind(nonce, hashed.phc, paramsJson(hashed.params), ...profBinds),
        confirmEmail(consumed, [nonce], now),
        ...(needsProfile ? activate(consumed, [nonce], now) : []),
        // Every session ends, on every device.
        db.prepare(`UPDATE sessions SET revoked_at = ?2 WHERE revoked_at IS NULL AND user_id = (${consumed})`).bind(nonce, now),
        // Every other outstanding link (reset or verify) stops working.
        db.prepare(`UPDATE email_tokens SET used_at = ?2 WHERE used_at IS NULL AND user_id = (${consumed})`).bind(nonce, now),
      ]);
      const userId = results[0]!.results[0]?.user_id;
      if (!userId) throw new HttpError(400, "invalid_token");
      leased = false; // consumed
      emit({ type: "password_reset", userId });
      if (needsProfile && results[3]!.results.length === 1) activated(ctx, userId);
      mail(ctx, { kind: "password_changed", to: row.email });
      // Completing a reset proves control of the mailbox, so it restores login even during a
      // lockdown: the epoch bump in the batch above re-keyed every failure counter of the account
      // (every network, every device), old device tokens died with it, this device becomes known,
      // and an enrolment code is mailed for the owner's other devices (the native app).
      const res = json(200, OK_BODY);
      if (client.mode === "web") res.headers.append("Set-Cookie", clearCookie(r.cookie));
      const epoch = (results[1]!.results[0] as { device_epoch?: number } | undefined)?.device_epoch;
      if (epoch === undefined) return res;
      await issueEnrolment(ctx, userId, row.email, epoch);
      return withDevice(res, client, OK_BODY, userId, epoch);
    } finally {
      if (leased) await releaseEmailToken(th);
    }
  };

  const login: Handler = async (req, client, ctx) => {
    const body = await readJson(req);
    const email = normalizeEmail(str(body, "email", 320)!);
    const password = readPassword(body);
    const account = `email:${email}`;
    // Per IP every attempt counts (CPU bound); per account only failures, checked before the hash.
    ipOf(req); // fail closed with no client IP, device or not
    const user = await db
      .prepare(`SELECT id, pw_hash, trust, email_verified_at, device_epoch, last_device, suspended_at FROM users WHERE email = ?1 AND deleted_at IS NULL`)
      .bind(email)
      .first<{
        id: string;
        pw_hash: string | null;
        trust: string;
        email_verified_at: number | null;
        device_epoch: number;
        last_device: string | null;
        suspended_at: number | null;
      }>();
    const device = await validDevice(req, client, user);
    // The owner's known device is exempt from the per-IP limit (a shared school address may be
    // saturated by others); its own failures are counted per device instead.
    const refundIp = device ? async () => {} : await limit("login", req, account);
    // Reserve a failure slot (refused, without a hash, when over the limits); given back on success.
    // A refusal also refunds the IP hit, so a shared address's budget counts only hashed attempts.
    let release: () => Promise<void>;
    try {
      release = await reserveFailure(req, account, user?.device_epoch ?? 0, device, user?.last_device ?? null);
    } catch (e) {
      await refundIp();
      throw e;
    }
    // Unknown address or no password: a full-cost hash anyway, so every failure takes the same time.
    // Known accounts hash with their own stored parameters, so right after a cost raise an old hash
    // is cheaper than the dummy (see README, "Raising the cost").
    const matches = user?.pw_hash ? await verifyPassword(config.hasher, user.pw_hash, password) : await dummyVerify(config.hasher, password, r.argon2);
    const fail = async (): Promise<never> => {
      // Every failed login, for a known address or not, was charged to the shared per-IP budget like
      // any login; within a separate per-IP "failed login" allowance the charge is given back. So a
      // flood of wrong passwords from a school NAT cannot starve real teachers there, and the effect
      // on the budget is the same whether or not the address has an account (no existence oracle).
      // Done in the background, so the response and its timing are the same either way.
      if (!device) background(ctx, refundFailed(req, refundIp), "refundFailed");
      emit({ type: "login_failed", userId: user?.id });
      throw new HttpError(401, "invalid_credentials");
    };
    // An account that is not active never logs in: before confirmation its password may belong to
    // someone who does not own the address. Same answer as a wrong password, so this reveals nothing.
    if (!matches || !user || !user.pw_hash || user.email_verified_at === null || user.trust === r.initialTrust) return fail();
    if (user.suspended_at !== null) {
      // Only reachable with the right password, so it reveals nothing to anyone else.
      await release();
      throw new HttpError(403, "account_suspended");
    }

    const now = r.now();
    const family = crypto.randomUUID();
    const sessionId = crypto.randomUUID();
    const refresh = randomToken();
    const expiresAt = Math.min(now + r.refreshTtl * 1000, now + r.sessionMax * 1000);
    const dev = await issueDeviceToken(user.id, user.device_epoch);
    const stmts: D1PreparedStatement[] = [
      // The session exists only if the account is exactly as it was when the password was checked:
      // a reset or deletion that committed in between leaves no row, and the login fails.
      db
        .prepare(
          `INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, user_agent)
           SELECT ?1, ?2, id, ?3, ?4, ?4, ?5, ?6 FROM users
           WHERE id = ?7 AND pw_hash = ?8 AND deleted_at IS NULL AND email_verified_at IS NOT NULL AND trust != ?9 AND suspended_at IS NULL`,
        )
        .bind(sessionId, family, await sha256(refresh), now, expiresAt, ua(req), user.id, user.pw_hash, r.initialTrust),
      // This device becomes the account's current device.
      db
        .prepare(`UPDATE users SET last_device = ?1 WHERE id = ?2 AND EXISTS (SELECT 1 FROM sessions WHERE id = ?3)`)
        .bind(dev.nonce, user.id, sessionId),
    ];
    const stored = parsePhc(user.pw_hash);
    if (stored && !sameParams(stored.params, r.argon2)) {
      // Parameters were raised since this hash was made: upgrade it now that we have the password.
      const h = await hashPassword(config.hasher, password, r.argon2);
      stmts.push(
        db
          .prepare(`UPDATE users SET pw_hash = ?1, pw_params = ?2 WHERE id = ?3 AND pw_hash = ?4 AND EXISTS (SELECT 1 FROM sessions WHERE id = ?5)`)
          .bind(h.phc, paramsJson(h.params), user.id, user.pw_hash, sessionId),
      );
    }
    if (client.mode === "web") {
      // A browser that still holds an older session gets that session ended, not orphaned.
      const old = readCookie(req, r.cookie);
      if (old && TOKEN_RE.test(old))
        stmts.push(
          db
            .prepare(
              `UPDATE sessions SET revoked_at = ?1 WHERE revoked_at IS NULL AND family_id != ?3
                 AND family_id = (SELECT family_id FROM sessions WHERE refresh_hash = ?2)
                 AND EXISTS (SELECT 1 FROM sessions WHERE id = ?4)`,
            )
            .bind(now, await sha256(old), family, sessionId),
        );
    }
    const [ins] = await db.batch(stmts);
    if ((ins!.meta.changes ?? 0) !== 1) return fail();
    await release(); // a success never counts against the account
    emit({ type: "login_succeeded", userId: user.id, sessionFamily: family });
    return tokenResponse(client, user, family, refresh, expiresAt, dev.token);
  };

  /** Gives back a failed login's per-IP charge if it fits the separate per-IP allowance. */
  async function refundFailed(req: Request, refundIp: () => Promise<void>): Promise<void> {
    const rules = r.limits.login.ipFailed ?? [];
    if (rules.length === 0) return;
    const keys = await keysP;
    const now = r.now();
    const ipH = await hmac(keys.limit, ipBucket(ipOf(req)));
    const hits = rules.map((rule) => ({ key: `i:${ipH}:login-failed:${rule.windowSeconds}`, rule }));
    const res = await Promise.all(hits.map((h) => r.store.hit(h.key, h.rule, now)));
    if (res.every((x) => x.allowed)) await refundIp();
    else if (r.store.undo) await Promise.all(hits.map((h) => r.store.undo!(h.key, h.rule, now)));
  }

  const refresh: Handler = async (req, client) => {
    requireOriginForCookie(client);
    const body = client.mode === "native" ? await readJson(req) : null;
    const raw = await presentedRefresh(req, client, body);
    ipOf(req); // fail closed with no client IP
    if (!raw) {
      await limit("refresh", req, null);
      throw refreshFailure(client);
    }
    const now = r.now();
    const keys = await keysP;
    // Looked up first; only an unknown token is charged to the IP, so junk tokens from a shared
    // school address can never make the school's real refreshes fail.
    const row = await db
      .prepare(
        `SELECT s.id, s.family_id, s.user_id, s.auth_at, s.expires_at, s.rotated_at, s.revoked_at, u.trust,
                COALESCE(u.deleted_at, u.suspended_at) AS deleted_at
         FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.refresh_hash = ?1`,
      )
      .bind(await sha256(raw))
      .first<{
        id: string;
        family_id: string;
        user_id: string;
        auth_at: number;
        expires_at: number;
        rotated_at: number | null;
        revoked_at: number | null;
        trust: string;
        deleted_at: number | null;
      }>();
    if (!row) {
      await limit("refresh", req, null);
      throw refreshFailure(client);
    }
    if (row.revoked_at !== null || row.deleted_at !== null || row.expires_at <= now || row.auth_at + r.sessionMax * 1000 <= now)
      throw refreshFailure(client);

    // The successor is derived, never stored: HMAC(server key, predecessor).
    const successor = await hmac(keys.refresh, raw);
    const successorHash = await sha256(successor);
    const user = { id: row.user_id, trust: row.trust };

    if (row.rotated_at === null) {
      const expiresAt = Math.min(now + r.refreshTtl * 1000, row.auth_at + r.sessionMax * 1000);
      const [upd] = await db.batch<{ id: string }>([
        // The one atomic rotation: only one caller can flip rotated_at from NULL.
        db.prepare(`UPDATE sessions SET rotated_at = ?1 WHERE id = ?2 AND rotated_at IS NULL AND revoked_at IS NULL RETURNING id`).bind(now, row.id),
        // Inserted only if this batch did the rotation; OR IGNORE makes racing winners converge.
        db
          .prepare(
            `INSERT OR IGNORE INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, user_agent)
             SELECT ?1, family_id, user_id, ?2, ?3, auth_at, ?4, ?5 FROM sessions
             WHERE id = ?6 AND rotated_at = ?3 AND revoked_at IS NULL`,
          )
          .bind(crypto.randomUUID(), successorHash, now, expiresAt, ua(req), row.id),
      ]);
      if (upd!.results.length === 1) {
        emit({ type: "refresh_rotated", userId: user.id, sessionFamily: row.family_id });
        return tokenResponse(client, user, row.family_id, successor, expiresAt);
      }
    }

    // Already rotated (by us earlier, or by a concurrent request a moment ago).
    const cur = await db
      .prepare(`SELECT rotated_at, revoked_at FROM sessions WHERE id = ?1`)
      .bind(row.id)
      .first<{ rotated_at: number | null; revoked_at: number | null }>();
    if (!cur || cur.revoked_at !== null || cur.rotated_at === null) throw refreshFailure(client);
    if (now - cur.rotated_at <= r.graceMs) {
      // Inside the grace window: hand back the same successor (two tabs, or a retried request).
      // If that successor has itself been rotated meanwhile, follow the derived chain to the live
      // head, so the late caller is not handed a token that would trip reuse detection later.
      let tok = successor;
      let tokHash = successorHash;
      for (let step = 0; step < 8; step++) {
        const s = await db
          .prepare(`SELECT expires_at, revoked_at, rotated_at FROM sessions WHERE refresh_hash = ?1`)
          .bind(tokHash)
          .first<{ expires_at: number; revoked_at: number | null; rotated_at: number | null }>();
        if (!s || s.revoked_at !== null || s.expires_at <= now) break;
        if (s.rotated_at === null) {
          emit({ type: "refresh_grace_reuse", userId: user.id, sessionFamily: row.family_id });
          return tokenResponse(client, user, row.family_id, tok, s.expires_at);
        }
        tok = await hmac(keys.refresh, tok);
        tokHash = await sha256(tok);
      }
      throw refreshFailure(client);
    }
    // Outside the window, a rotated token showing up means it was copied: end the whole family.
    await db.prepare(`UPDATE sessions SET revoked_at = ?1 WHERE family_id = ?2 AND revoked_at IS NULL`).bind(now, row.family_id).run();
    reuse({ type: "refresh_reuse_revoked", userId: user.id, sessionFamily: row.family_id });
    throw refreshFailure(client);
  };

  const logout: Handler = async (req, client) => {
    requireOriginForCookie(client);
    let body: Record<string, unknown> | null = null;
    if (req.headers.get("Content-Type") !== null) body = await readJson(req);
    ipOf(req); // fail closed with no client IP
    const now = r.now();
    const everywhere = body?.scope === "global";
    // The device cookie survives a plain logout (the owner stays known on their own computer)
    // unless the user asks to forget this device, e.g. on a shared machine.
    const forgetDevice = body?.forget_device === true;
    const raw = await presentedRefresh(req, client, body);
    let userId: string | null = null;
    let family: string | null = null;
    let viaRefresh = false;
    if (raw) {
      const row = await db
        .prepare(`SELECT family_id, user_id FROM sessions WHERE refresh_hash = ?1 AND revoked_at IS NULL`)
        .bind(await sha256(raw))
        .first<{ family_id: string; user_id: string }>();
      if (row) {
        ({ family_id: family, user_id: userId } = row);
        viaRefresh = true;
      }
    }
    if (!family) {
      const p = await authenticate(req);
      if (p) ({ sessionFamily: family, userId } = p);
    }
    // Only a request that presented nothing valid is charged to the IP (junk from a shared address
    // must not make the school's real logouts fail).
    if (!family) await limit("refresh", req, null);
    if (everywhere && userId) {
      // Logging out everywhere also revokes every known device and re-keys the account's failure
      // counters, so it needs the refresh token, not just a (possibly stolen) 15-minute access token.
      if (!viaRefresh) throw new HttpError(401, "refresh_token_required");
      await db.batch([
        db.prepare(`UPDATE sessions SET revoked_at = ?1 WHERE user_id = ?2 AND revoked_at IS NULL`).bind(now, userId),
        db.prepare(`UPDATE users SET device_epoch = device_epoch + 1 WHERE id = ?1`).bind(userId),
      ]);
    } else if (family) {
      await db.prepare(`UPDATE sessions SET revoked_at = ?1 WHERE family_id = ?2 AND revoked_at IS NULL`).bind(now, family).run();
    }
    if (userId) emit({ type: "logout", userId, sessionFamily: family ?? undefined });
    // Always the same answer, so logout reveals nothing about the token.
    const res = empty(204, client.mode === "web" ? { "Set-Cookie": clearCookie(r.cookie) } : {});
    if (client.mode === "web" && (everywhere || forgetDevice)) res.headers.append("Set-Cookie", clearCookie(r.deviceCookie));
    return res;
  };

  /**
   * Redeems a mailed enrolment code for a device token. The code proves control of the mailbox and
   * the password proves identity, so a native app can become a known device even while the account
   * is locked down. Codes are single use, expire after 15 minutes, are bound to the account and its
   * device epoch, are leased before the hash (one hash per code at a time), and die after
   * ENROL_MAX_FAILURES wrong passwords. Only an unknown code is charged to the IP.
   */
  /**
   * A signed-in owner on a known device asks for a fresh enrolment code, to add another device
   * (a third laptop, a new phone) while the account is locked down. It needs a live access token
   * AND a valid device token for the account, is held to the mail gap and a daily cap, mails the
   * code to the account's address, and does NOT bump the epoch, so existing devices keep working.
   */
  const enrolmentCode: Handler = async (req, client, ctx) => {
    const p = await requirePrincipal(req);
    ipOf(req); // fail closed with no client IP
    const u = await db
      .prepare(`SELECT id, email, device_epoch FROM users WHERE id = ?1 AND deleted_at IS NULL`)
      .bind(p.userId)
      .first<{ id: string; email: string; device_epoch: number }>();
    if (!u) throw new HttpError(401, "unauthorized");
    if (!(await validDevice(req, client, u))) throw new HttpError(403, "known_device_required");
    if (!(await issueEnrolment(ctx, u.id, u.email, u.device_epoch, true)))
      throw new HttpError(429, "rate_limited", {}, { "Retry-After": String(Math.ceil(r.mailGapMs / 1000)) });
    return json(202, OK_BODY);
  };

  const enrol: Handler = async (req, client) => {
    const body = await readJson(req);
    const code = body.code;
    if (typeof code !== "string" || !TOKEN_RE.test(code)) throw new HttpError(400, "invalid_token");
    const password = readPassword(body);
    ipOf(req); // fail closed with no client IP
    const now = r.now();
    const ch = await sha256(code);
    const row = await db
      .prepare(
        `SELECT e.id, e.user_id, u.pw_hash, u.device_epoch FROM device_enrolments e JOIN users u ON u.id = e.user_id
         WHERE e.code_hash = ?1 AND e.used_at IS NULL AND e.expires_at > ?2 AND e.epoch = u.device_epoch
           AND e.failures < ?3 AND u.deleted_at IS NULL AND u.suspended_at IS NULL AND u.pw_hash IS NOT NULL`,
      )
      .bind(ch, now, ENROL_MAX_FAILURES)
      .first<{ id: string; user_id: string; pw_hash: string; device_epoch: number }>();
    if (!row) {
      await limit("enrol", req, null);
      throw new HttpError(400, "invalid_token");
    }
    // Every condition is re-checked inside the claim itself, so nothing that changed since the read
    // above (more wrong passwords, expiry, a reset bumping the epoch) can slip a hash through.
    const claimed = await db
      .prepare(
        `UPDATE device_enrolments SET claimed_until = ?3
         WHERE id = ?1 AND used_at IS NULL AND (claimed_until IS NULL OR claimed_until <= ?2)
           AND failures < ?4 AND expires_at > ?2
           AND epoch = (SELECT device_epoch FROM users WHERE id = device_enrolments.user_id AND deleted_at IS NULL)
         RETURNING id`,
      )
      .bind(row.id, now, now + LEASE_MS, ENROL_MAX_FAILURES)
      .first();
    if (!claimed) {
      const held = await db
        .prepare(`SELECT 1 FROM device_enrolments WHERE id = ?1 AND used_at IS NULL AND claimed_until > ?2`)
        .bind(row.id, now)
        .first();
      throw held ? new HttpError(409, "in_progress") : new HttpError(400, "invalid_token");
    }
    let ok = false;
    try {
      ok = await verifyPassword(config.hasher, row.pw_hash, password);
    } finally {
      if (!ok)
        await db.prepare(`UPDATE device_enrolments SET claimed_until = NULL, failures = failures + 1 WHERE id = ?1`).bind(row.id).run();
    }
    if (!ok) throw new HttpError(401, "invalid_credentials");
    const used = await db
      .prepare(
        `UPDATE device_enrolments SET used_at = ?2, claimed_until = NULL
         WHERE id = ?1 AND used_at IS NULL
           AND EXISTS (SELECT 1 FROM users WHERE id = ?3 AND pw_hash = ?4 AND device_epoch = ?5 AND deleted_at IS NULL)
         RETURNING id`,
      )
      .bind(row.id, now, row.user_id, row.pw_hash, row.device_epoch)
      .first();
    if (!used) throw new HttpError(400, "invalid_token");
    emit({ type: "device_enrolled", userId: row.user_id });
    return withDevice(json(200, OK_BODY), client, OK_BODY, row.user_id, row.device_epoch);
  };

  async function requirePrincipal(req: Request): Promise<Principal> {
    const p = await authenticate(req);
    if (!p) throw new HttpError(401, "unauthorized", {}, { "WWW-Authenticate": 'Bearer error="invalid_token"' });
    return p;
  }

  const me: Handler = async (req) => {
    const p = await requirePrincipal(req);
    const u = await db
      .prepare(
        `SELECT id, email, email_verified_at, display_name, profile, trust, created_at, suspended_at, suspension_reason
         FROM users WHERE id = ?1 AND deleted_at IS NULL`,
      )
      .bind(p.userId)
      .first<{
        id: string;
        email: string;
        email_verified_at: number | null;
        display_name: string | null;
        profile: string | null;
        trust: string;
        created_at: number;
        suspended_at: number | null;
        suspension_reason: string | null;
      }>();
    if (!u) throw new HttpError(401, "unauthorized");
    return json(200, {
      ...(admin ? { status: await admin.statusOf(u.id, u.trust, u.suspended_at, u.suspension_reason, r.now()) } : {}),
      id: u.id,
      email: u.email,
      email_verified: u.email_verified_at !== null,
      display_name: u.display_name,
      profile: u.profile === null ? null : JSON.parse(u.profile),
      trust: u.trust,
      created_at: u.created_at,
    });
  };

  const deleteAccount: Handler = async (req, client, ctx) => {
    const p = await requirePrincipal(req);
    const body = await readJson(req);
    const password = readPassword(body);
    // Re-authentication: a stolen 15-minute access token alone cannot delete the account.
    await limit("delete", req, `user:${p.userId}`);
    const u = await db
      .prepare(`SELECT id, email, pw_hash, trust, device_epoch, last_device FROM users WHERE id = ?1 AND deleted_at IS NULL`)
      .bind(p.userId)
      .first<{ id: string; email: string; pw_hash: string | null; trust: string; device_epoch: number; last_device: string | null }>();
    if (!u || !u.pw_hash) throw new HttpError(401, "unauthorized");
    // An admin-level account needs its known device and a sudo token too.
    if (admin) await admin.deletionGuard(req, client, p, u);
    // Password attempts here count only against the per-account delete limiter above (5 an hour, 10
    // a day), never the login lockdown: the owner can still delete during a lockdown. If they are
    // refused by the delete limiter, a password reset ends it (it revokes the stolen session too).
    if (!(await verifyPassword(config.hasher, u.pw_hash, password))) throw new HttpError(401, "invalid_credentials");
    const keys = await keysP;
    const now = r.now();
    const emailHmac = await hmac(keys.email, u.email);
    // The tombstone is written only if the password checked above is still the current one, so a
    // reset that committed in between makes this deletion fail instead of acting on stale proof.
    const tomb = `EXISTS (SELECT 1 FROM users WHERE id = ?1 AND deleted_at = ?2 AND deleted_email_hmac = ?3)`;
    const [upd] = await db.batch([
      db
        .prepare(
          `UPDATE users SET email = NULL, email_verified_at = NULL, pw_hash = NULL, pw_params = NULL, display_name = NULL,
             profile = NULL, suspension_reason = NULL, deleted_at = ?2, deleted_email_hmac = ?3, deleted_email_hmac_expires_at = ?4,
             device_epoch = device_epoch + 1
           WHERE id = ?1 AND deleted_at IS NULL AND pw_hash = ?5 ${admin ? `AND ${admin.notLastAdmin("?6")}` : ""}`,
        )
        .bind(p.userId, now, emailHmac, now + r.tombstoneTtl * 1000, u.pw_hash, ...(admin ? [admin.adminsJson] : [])),
      db.prepare(`DELETE FROM sessions WHERE user_id = ?1 AND ${tomb}`).bind(p.userId, now, emailHmac),
      db.prepare(`DELETE FROM email_tokens WHERE user_id = ?1 AND ${tomb}`).bind(p.userId, now, emailHmac),
      db.prepare(`DELETE FROM device_enrolments WHERE user_id = ?1 AND ${tomb}`).bind(p.userId, now, emailHmac),
      // Application records stay as anonymised tombstones (who decided what, when, on what kind of
      // evidence); the free text (notes, evidence notes, reasons) may hold PII and goes. An open
      // application is closed so it leaves the queue.
      db
        .prepare(
          `UPDATE approval_requests SET note = NULL, evidence_note = NULL, reason = NULL, display_name_snapshot = NULL,
             profile_snapshot = NULL, decision = COALESCE(decision, 'account_deleted'), decided_at = COALESCE(decided_at, ?2)
           WHERE user_id = ?1 AND ${tomb}`,
        )
        .bind(p.userId, now, emailHmac),
      // The audit trail survives; only its free text about this account is nulled (the one change the
      // append-only triggers allow, and only for a deleted account).
      db
        .prepare(
          `UPDATE auth_audit SET note = NULL, evidence_note = NULL, reason = NULL
           WHERE target_id = ?1 AND (note IS NOT NULL OR evidence_note IS NOT NULL OR reason IS NOT NULL) AND ${tomb}`,
        )
        .bind(p.userId, now, emailHmac),
      ...(admin
        ? [
            db.prepare(`UPDATE admin_proposals SET note = NULL, reason = NULL WHERE target_id = ?1 AND ${tomb}`).bind(p.userId, now, emailHmac),
            admin.auditDeletion(p.userId, u.trust, now, `EXISTS (SELECT 1 FROM users WHERE id = '${p.userId}' AND deleted_at = ${now})`),
          ]
        : []),
    ]);
    if ((upd!.meta.changes ?? 0) !== 1) {
      if (admin?.isAdminLevel(u.trust)) {
        const still = await db.prepare(`SELECT pw_hash FROM users WHERE id = ?1 AND deleted_at IS NULL`).bind(p.userId).first<{ pw_hash: string }>();
        if (still?.pw_hash === u.pw_hash) throw new HttpError(409, "last_admin");
      }
      throw new HttpError(401, "invalid_credentials");
    }
    if (admin?.isAdminLevel(u.trust)) admin.alert(ctx, { action: "admin_account_deleted", actorId: p.userId, targetId: p.userId, at: now });
    // Rate-limit counters keyed on this account go now, not when they would have expired.
    const purge = async (account: string) => {
      const pre = await acctPrefix(account);
      return db.prepare(`DELETE FROM rate_limits WHERE key >= ?1 AND key < ?2`).bind(pre, pre.slice(0, -1) + ";");
    };
    await db.batch([await purge(`email:${u.email}`), await purge(`user:${p.userId}`)]);
    // App content cleanup runs only once the account is certainly gone.
    if (config.onDelete) {
      const extra = config.onDelete(p.userId, now);
      if (extra.length > 0) await db.batch(extra);
    } else {
      console.warn("auth: account deleted but no onDelete hook is configured; app data referencing the user was not cleaned up");
    }
    emit({ type: "account_deleted", userId: p.userId });
    return empty(204, client.mode === "web" ? { "Set-Cookie": clearCookie(r.cookie) } : {});
  };

  const exportAccount: Handler = async (req) => {
    const p = await requirePrincipal(req);
    const u = await db
      .prepare(`SELECT id, email, email_verified_at, display_name, profile, trust, created_at FROM users WHERE id = ?1 AND deleted_at IS NULL`)
      .bind(p.userId)
      .first<Record<string, unknown> & { profile: string | null }>();
    if (!u) throw new HttpError(401, "unauthorized");
    const sessions = await db
      .prepare(
        `SELECT family_id, auth_at, created_at, expires_at, user_agent FROM sessions
         WHERE user_id = ?1 AND revoked_at IS NULL AND rotated_at IS NULL ORDER BY auth_at`,
      )
      .bind(p.userId)
      .all<{ family_id: string; auth_at: number; created_at: number; expires_at: number; user_agent: string | null }>();
    // The applicant's own history: decisions and the reasons given to them, never reviewer notes.
    const applications = await db
      .prepare(`SELECT requested_at, decided_at, decision, reason FROM approval_requests WHERE user_id = ?1 ORDER BY requested_at, id`)
      .bind(p.userId)
      .all();
    const approval = applications.results[applications.results.length - 1] ?? null;
    return json(200, {
      account: { ...u, profile: u.profile === null ? null : JSON.parse(u.profile) },
      sessions: sessions.results.map((s) => ({
        signed_in_at: s.auth_at,
        last_refreshed_at: s.created_at,
        expires_at: s.expires_at,
        user_agent: s.user_agent,
        current: s.family_id === p.sessionFamily,
      })),
      approval,
      applications: applications.results,
      ...(config.exportExtra ? await config.exportExtra(p.userId) : {}),
    });
  };

  /**
   * The sudo step: a signed-in user on a known device re-enters the password and gets a 5-minute
   * sudo token bound to this session family and device epoch. Admin writes need it (X-Sudo-Token),
   * so a stolen plain access token can never approve, reject, suspend, restore or revoke.
   */
  const reauth: Handler = async (req, client) => {
    const p = await requirePrincipal(req);
    const body = await readJson(req);
    const password = readPassword(body);
    // Only wrong passwords count (an active admin re-authenticates every few minutes).
    const refund = await limit("reauth", req, `user:${p.userId}`);
    const u = await db
      .prepare(`SELECT id, pw_hash, device_epoch, last_device FROM users WHERE id = ?1 AND deleted_at IS NULL AND suspended_at IS NULL`)
      .bind(p.userId)
      .first<{ id: string; pw_hash: string | null; device_epoch: number; last_device: string | null }>();
    if (!u || !u.pw_hash) throw new HttpError(401, "unauthorized");
    if (!(await validDevice(req, client, u))) {
      await refund();
      throw new HttpError(403, "known_device_required");
    }
    if (!(await verifyPassword(config.hasher, u.pw_hash, password))) throw new HttpError(401, "invalid_credentials");
    await refund();
    const { token, expiresIn } = await issueSudo(await keysP, p, u.device_epoch, r.now());
    emit({ type: "reauthenticated", userId: p.userId });
    return json(200, { sudo_token: token, expires_in: expiresIn });
  };

  /** A rejected or revoked applicant applies again, after the cooldown, with a fresh profile. */
  const reapply: Handler = async (req, _client, ctx) => {
    if (!admin) throw new HttpError(404, "not_found");
    const p = await requirePrincipal(req);
    const body = await readJson(req);
    const prof = readProfile(body);
    await limit("reapply", req, `user:${p.userId}`);
    void ctx;
    return admin.reapply(p, prof);
  };

  // ---- routing ---------------------------------------------------------------------------

  const admin: Admin | null = config.approval
    ? createAdmin({
        db,
        cfg: config.approval,
        pendingLevel: r.trust.afterVerify,
        keys: keysP,
        now: r.now,
        readJson,
        authenticate,
        limit,
        hasValidDevice: async (req, client, user) => (await validDevice(req, client, user)) !== null,
        mail,
        background,
        emit,
      })
    : null;

  const routes = new Map<string, Record<string, Handler>>();
  const add = (path: string, method: string, h: Handler) => {
    const m = routes.get(path) ?? {};
    m[method] = h;
    routes.set(path, m);
  };
  add(r.paths.signup, "POST", signup);
  add(r.paths.login, "POST", login);
  add(r.paths.refresh, "POST", refresh);
  add(r.paths.logout, "POST", logout);
  add(r.paths.verify, "POST", verify);
  add(r.paths.resend, "POST", resend);
  add(r.paths.forgot, "POST", forgot);
  add(r.paths.reset, "POST", reset);
  add(r.paths.me, "GET", me);
  add(r.paths.account, "DELETE", deleteAccount);
  add(r.paths.export, "GET", exportAccount);
  add(r.paths.enrol, "POST", enrol);
  add(r.paths.enrolmentCode, "POST", enrolmentCode);
  add(r.paths.reauth, "POST", reauth);
  if (admin) {
    const a = r.paths.admin;
    add(r.paths.reapply, "POST", reapply);
    add(`${a}/queue`, "GET", admin.queue);
    add(`${a}/approve`, "POST", admin.approve);
    add(`${a}/reject`, "POST", admin.reject);
    add(`${a}/reject-bulk`, "POST", admin.rejectBulk);
    add(`${a}/confirm`, "POST", admin.confirm);
    add(`${a}/cancel`, "POST", admin.cancel);
    add(`${a}/proposals`, "GET", admin.proposals);
    add(`${a}${r.paths.adminRevoke}`, "POST", admin.revoke);
    add(`${a}/suspend`, "POST", admin.suspend);
    add(`${a}/restore`, "POST", admin.restore);
    add(`${a}/audit`, "GET", admin.auditLog);
    for (const route of config.approval!.routes ?? []) add(route.path, route.method, admin.extra(route));
  }

  async function handle(req: Request, ctx: WaitUntil): Promise<Response | null> {
    const path = new URL(req.url).pathname;
    let methods = routes.get(path);
    const userPrefix = `${r.paths.admin}/users/`;
    if (!methods && admin && path.startsWith(userPrefix) && !path.slice(userPrefix.length).includes("/")) {
      const id = path.slice(userPrefix.length);
      methods = { GET: (rq, c) => admin.userDetail(rq, c, id) };
    }
    if (!methods) return null;
    const allow = Object.keys(methods).join(", ");
    if (req.method === "OPTIONS") return preflight(req, r.allowed, allow);
    const origin = req.headers.get("Origin");
    const cors = corsHeaders(origin, r.allowed);
    const h = methods[req.method];
    if (!h) return withHeaders(json(405, { error: "method_not_allowed" }, { Allow: allow }), cors);
    let res: Response;
    try {
      res = await h(req, clientInfo(req, r.allowed), ctx);
    } catch (e) {
      if (e instanceof HttpError) res = errorResponse(e);
      else {
        console.error(`auth: unhandled error on ${req.method} ${path}`);
        res = json(500, { error: "server_error" });
      }
    }
    return withHeaders(res, cors);
  }

  async function authenticate(req: Request): Promise<Principal | null> {
    const tok = bearer(req);
    if (!tok) return null;
    const keys = await keysP;
    const now = r.now();
    const c = await verifyAccessToken(keys.access, tok, { iss: r.issuer, aud: r.audience, nowSeconds: Math.floor(now / 1000) });
    if (!c) return null;
    const row = await db
      .prepare(
        `SELECT u.trust FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.family_id = ?1 AND s.user_id = ?2 AND s.revoked_at IS NULL AND s.rotated_at IS NULL
           AND s.expires_at > ?3 AND u.deleted_at IS NULL AND u.suspended_at IS NULL LIMIT 1`,
      )
      .bind(c.sid, c.sub, now)
      .first<{ trust: string }>();
    return row ? { userId: c.sub, sessionFamily: c.sid, trust: row.trust } : null;
  }

  async function purgeExpired(): Promise<PurgeReport> {
    const now = r.now();
    const stmts = [
      db.prepare(`DELETE FROM rate_limits WHERE expires_at <= ?1`).bind(now),
      // Rotated rows are kept until they expire: they are what detects a replayed token.
      db.prepare(`DELETE FROM sessions WHERE expires_at <= ?1 OR (revoked_at IS NOT NULL AND revoked_at <= ?2)`).bind(now, now - DAY * 1000),
      // Kept a day so the daily enrolment-code cap can still count them.
      db.prepare(`DELETE FROM device_enrolments WHERE created_at <= ?2 AND (expires_at <= ?1 OR used_at IS NOT NULL)`).bind(now, now - DAY * 1000),
      // Kept a day so the daily mail ceiling can still count them.
      db.prepare(`DELETE FROM email_tokens WHERE created_at <= ?2 AND (expires_at <= ?1 OR used_at IS NOT NULL)`).bind(now, now - DAY * 1000),
      db
        .prepare(
          `UPDATE users SET deleted_email_hmac = NULL, deleted_email_hmac_expires_at = NULL
           WHERE deleted_email_hmac IS NOT NULL AND deleted_email_hmac_expires_at <= ?1`,
        )
        .bind(now),
    ];
    const after = config.purgeUnverifiedAfterSeconds === undefined ? 7 * DAY : config.purgeUnverifiedAfterSeconds;
    let unverified = 0;
    if (after) {
      const cutoff = now - after * 1000;
      const stale = `SELECT id FROM users WHERE trust = ?2 AND deleted_at IS NULL AND created_at <= ?1`;
      stmts.push(
        db.prepare(`DELETE FROM sessions WHERE user_id IN (${stale})`).bind(cutoff, r.initialTrust),
        db.prepare(`DELETE FROM email_tokens WHERE user_id IN (${stale})`).bind(cutoff, r.initialTrust),
        db.prepare(`DELETE FROM device_enrolments WHERE user_id IN (${stale})`).bind(cutoff, r.initialTrust),
        db.prepare(`DELETE FROM approval_requests WHERE user_id IN (${stale})`).bind(cutoff, r.initialTrust),
        db.prepare(`DELETE FROM users WHERE id IN (${stale})`).bind(cutoff, r.initialTrust),
      );
    }
    const res = await db.batch(stmts);
    if (after) unverified = res[res.length - 1]!.meta.changes ?? 0;
    const changes = (i: number) => res[i]!.meta.changes ?? 0;
    return {
      rateLimits: changes(0),
      sessions: changes(1),
      deviceEnrolments: changes(2),
      emailTokens: changes(3),
      tombstoneHashes: changes(4),
      unverifiedAccounts: unverified,
    };
  }

  async function findTombstoneByEmail(raw: string): Promise<string | null> {
    const keys = await keysP;
    const email = normalizeEmail(raw);
    const row = await db
      .prepare(`SELECT id FROM users WHERE deleted_email_hmac = ?1 AND deleted_email_hmac_expires_at > ?2`)
      .bind(await hmac(keys.email, email), r.now())
      .first<{ id: string }>();
    return row?.id ?? null;
  }

  return {
    handle,
    authenticate,
    preflight: (req, methods) => (req.method === "OPTIONS" ? preflight(req, r.allowed, methods) : null),
    withCors: (req, res) => withHeaders(res, corsHeaders(req.headers.get("Origin"), r.allowed)),
    purgeExpired,
    findTombstoneByEmail,
    reviewReminders: async () => (admin ? admin.reviewReminders() : null),
  };
}
