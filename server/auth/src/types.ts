// Public configuration and plug-in interfaces.

export interface Argon2Params {
  /** Memory in KiB. */
  m: number;
  /** Iterations. */
  t: number;
  /** Parallelism. */
  p: number;
}

/**
 * Raw Argon2id. Implementations return the 32-byte tag for (password, salt, params).
 * The package never sees the implementation, so a wasm build, a native binding or a
 * test stub all fit. Synchronous or asynchronous results are both accepted.
 */
export interface Argon2idHasher {
  argon2id(password: Uint8Array, salt: Uint8Array, params: Argon2Params): Uint8Array | Promise<Uint8Array>;
}

export type MailKind =
  /** Confirm a newly registered address. */
  | "verify"
  /** Reset a password. */
  | "reset"
  /** Someone tried to register an address that already has an account. */
  | "signup_existing"
  /** The password on the account was changed through a reset. */
  | "password_changed"
  /** A one-time code (15 minutes) a native app redeems at /auth/device/enrol, with the password, for a device token. */
  | "device_enrol"
  /** The application was approved. */
  | "application_approved"
  /** The application was rejected (`reason` is the reviewer's message to the applicant). */
  | "application_rejected"
  /** The approved level was revoked (`reason` as above). */
  | "approval_revoked"
  /** The account was suspended (`reason` as above). */
  | "account_suspended"
  /** The account was restored after a suspension. */
  | "account_restored";

export interface MailMessage {
  kind: MailKind;
  to: string;
  /** Present for "verify", "reset" and "device_enrol": the one-time token to put in the link (prefer a URL fragment). */
  token?: string;
  /** Epoch ms after which the token stops working. */
  expiresAt?: number;
  /** For "signup_existing": whether the existing account has confirmed its address. */
  accountVerified?: boolean;
  /** For a rejection, revocation or suspension: the reviewer's message to the applicant. */
  reason?: string;
}

/** Sends mail. Called from `waitUntil`, so it never adds to response time. */
export interface MailSender {
  send(msg: MailMessage): Promise<void>;
}

export interface RateLimitRule {
  limit: number;
  windowSeconds: number;
}

export interface RateLimitResult {
  allowed: boolean;
  /** Seconds until a retry can succeed, when not allowed. */
  retryAfter: number;
}

/** Counts hits per key. `key` is already an opaque HMAC; never a raw IP or email. */
export interface RateLimitStore {
  hit(key: string, rule: RateLimitRule, now: number): Promise<RateLimitResult>;
  /**
   * Take back a hit: used when the request was refused (so refused attempts do not extend a
   * lockout) and when a login succeeds (so only failures count). Without it every login attempt
   * counts as a failure.
   */
  undo?(key: string, rule: RateLimitRule, now: number): Promise<void>;
}

/**
 * Login lockout that counts only FAILED attempts (a slot is reserved atomically before the hash
 * and given back on success), so nobody can lock an account by logging in correctly.
 */
export interface FailureLimits {
  /** Failures allowed per (account, IP) pair, IPv6 grouped by /48. Exceeding it blocks only that pair. */
  perAccountIp: RateLimitRule;
  /**
   * Failures per account from all networks (every rule must hold). Past any of them the account is
   * locked down: every attempt is refused without a hash unless it carries a valid known-device
   * token for that account. A completed reset clears these counters and issues a device token, so
   * the owner can always get back in.
   */
  perAccount: RateLimitRule | RateLimitRule[];
  /**
   * Failures per known-device token (default 10 per 15 minutes and 20 a day). A device token skips
   * the per-IP, per-network and per-account limits, so it is never refused for other people's
   * failures; its own failures still count here, so a stolen token cannot guess freely.
   */
  perDevice?: RateLimitRule | RateLimitRule[];
  /**
   * Failures across all of an account's device tokens (default 40 a day). Bounds several stolen
   * tokens together, and is never part of the lockdown that refuses logins without a device.
   */
  perAccountDevice?: RateLimitRule[];
}

export type Action = "signup" | "login" | "refresh" | "verify" | "resend" | "forgot" | "reset" | "delete" | "enrol" | "reauth" | "admin" | "reapply";

export interface ActionLimits {
  /** Per client IP (IPv6 grouped by /64). */
  ip?: RateLimitRule[];
  /** Per account, every attempt: keyed on the normalized email (or user id once signed in), whether or not it exists. */
  account?: RateLimitRule[];
  /** Per (IP, account) pair: junk aimed at other addresses from a shared IP never uses up this allowance. */
  ipAccount?: RateLimitRule[];
  /**
   * Login only: per-IP allowance within which every failed login (known address or not) is refunded
   * from `ip`, so the budget effect never depends on whether the address has an account.
   */
  ipFailed?: RateLimitRule[];
  /** Login only: limits on failed attempts (see FailureLimits). */
  failures?: FailureLimits;
}

export interface Paths {
  signup: string;
  login: string;
  refresh: string;
  logout: string;
  verify: string;
  resend: string;
  forgot: string;
  reset: string;
  me: string;
  account: string;
  export: string;
  enrol: string;
  enrolmentCode: string;
  /** Re-authentication for a short-lived sudo token. */
  reauth: string;
  /** An applicant whose application was rejected or revoked applies again (after the cooldown). */
  reapply: string;
  /** Prefix of the admin routes (queue, users/:id, approve, reject, suspend, restore, revoke, audit). */
  admin: string;
  /** Sub-path of the revoke route under `admin` (default "/revoke"). */
  adminRevoke: string;
}

export interface QueueStats {
  open: number;
  /** Age of the oldest open application, in seconds (0 when the queue is empty). */
  oldestAgeSeconds: number;
}

export interface DecisionSummary {
  approved: number;
  rejected: number;
  byAdmin: { admin_id: string; approved: number; rejected: number }[];
}

export interface AdminAlert {
  action: "admin_suspension_proposed" | "admin_suspension_cancelled" | "admin_suspended" | "admin_account_deleted";
  /** The admin who acted (null for none). */
  actorId: string | null;
  targetId: string;
  at: number;
  proposalId?: string;
}

export interface AdminPrincipal extends Principal {
  /** Present on write routes: the request carried a valid sudo token. */
  sudo: boolean;
}

export interface AdminAuditEntry {
  action: string;
  targetId?: string | null;
  note?: string | null;
  /** Structured, non-personal extras, stored as JSON. */
  detail?: Record<string, unknown> | null;
}

/** An extra admin route supplied by the app; it runs behind the same guard as the built-in ones. */
export interface AdminRoute {
  method: "GET" | "POST" | "PUT" | "DELETE";
  /** Full path, e.g. "/admin/staff-domains". */
  path: string;
  /** Writes need a sudo token (re-entered password within 5 minutes) and are audited by the caller. */
  write: boolean;
  handler: (
    req: Request,
    ctx: {
      admin: AdminPrincipal;
      body: Record<string, unknown> | null;
      now: number;
      audit: (e: AdminAuditEntry) => D1PreparedStatement;
    },
  ) => Promise<Response>;
}

export interface ApprovalConfig {
  /** Trust level an approval grants (it is never an admin level). */
  approvedLevel: string;
  /** Trust levels that may review. Admins are only ever created by the bootstrap script. */
  adminLevels: readonly string[];
  /** Accepted evidence kinds for an approval (default ["directory", "school_confirmed"]). */
  evidenceKinds?: readonly string[];
  /** Wait after a rejection or revocation before the applicant may re-apply (default 7 days). */
  reapplyCooldownSeconds?: number;
  /** The reminder cron escalates when the oldest open application is older than this (default 48 h). */
  escalateAfterSeconds?: number;
  /** Send a daily digest from the reminder cron even when nothing is overdue (default false). */
  digest?: boolean;
  /** Labels for email domains shown in the queue (e.g. staff-domain hints). Labels only; never trust. */
  domainLabels?: (domains: string[]) => Promise<Record<string, string>>;
  /** Called from `reviewReminders()` with the queue state and the last 24 h of decisions (totals and per admin). */
  onQueueReminder?: (e: QueueStats & { kind: "digest" | "escalation"; lastDay: DecisionSummary }) => Promise<void>;
  /**
   * Called (in the background) on every action against an admin-level account: a suspension
   * proposal, its cancellation, a suspension, an admin deleting their account. Send it to every
   * admin AND an escalation contact, so a compromised admin cannot act unnoticed.
   */
  onAdminAlert?: (e: AdminAlert) => Promise<void>;
  /**
   * Two-person rule (default on): suspending an admin-level account is a proposal that a DIFFERENT
   * admin confirms (never the target). If no other admin could confirm, the proposer may confirm
   * alone after `adminSelfConfirmAfterSeconds`, so a sole remaining admin is never stuck.
   */
  twoPersonAdminSuspension?: boolean;
  /** Proposals expire after this (default 72 h). */
  adminProposalTtlSeconds?: number;
  /** Delay before a proposer may confirm alone when nobody else can (default 24 h). */
  adminSelfConfirmAfterSeconds?: number;
  /** Most applications one bulk-reject call may name (default 50). */
  bulkRejectMax?: number;
  /** Extra statements run with a suspension / restore (e.g. hide or show the user's content). */
  onSuspend?: (userId: string, now: number) => D1PreparedStatement[];
  onRestore?: (userId: string, now: number) => D1PreparedStatement[];
  /** Extra admin routes (app-specific), served behind the same guard. */
  routes?: AdminRoute[];
}

export interface TrustConfig {
  /** Every value the trust column may hold. The first is the level a new account starts at. */
  levels: readonly string[];
  /** Level given when the address is confirmed (only if the account is still at the initial level). */
  afterVerify: string;
  /** Insert an `approvals` row when an address is confirmed. */
  requestApproval: boolean;
}

export interface ProfileInput {
  displayName: string | null;
  /** App-defined fields stored as JSON (for example a school name). Nulled on deletion. */
  profile: Record<string, unknown> | null;
}

export class InputError extends Error {
  constructor(public code: string, public field?: string) {
    super(code);
  }
}

export interface AuthEvent {
  type:
    | "signup"
    | "login_succeeded"
    | "login_failed"
    | "rate_limited"
    | "verified"
    /** The account became usable: confirmed, with a password and profile set by the mailbox owner. */
    | "activated"
    | "refresh_rotated"
    /** A rotated refresh token came back inside the grace window. Usually two tabs; alert on bursts. */
    | "refresh_grace_reuse"
    | "refresh_reuse_revoked"
    | "logout"
    | "reset_requested"
    | "password_reset"
    | "account_deleted"
    | "device_enrolled"
    | "reauthenticated"
    | "admin_action";
  userId?: string;
  sessionFamily?: string;
  action?: Action;
}

export interface AuthConfig {
  db: D1Database;
  hasher: Argon2idHasher;
  mailer: MailSender;
  /** At least 32 characters of high-entropy secret. Sub-keys are derived with HKDF. */
  secret: string;
  /** Exact origins allowed to call the API with credentials. Never a wildcard. */
  allowedOrigins: readonly string[];

  /** Token issuer and audience written into access tokens. */
  issuer?: string;
  audience?: string;
  argon2?: Argon2Params;
  trust?: Partial<TrustConfig>;
  paths?: Partial<Paths>;
  rateLimits?: Partial<Record<Action, ActionLimits>>;
  rateLimitStore?: RateLimitStore;
  /** Name of the refresh cookie. Default `__Host-refresh` (host-only, Path=/, Secure). */
  cookieName?: string;
  accessTokenTtlSeconds?: number;
  refreshTokenTtlSeconds?: number;
  /** Hard cap on a session family's life, regardless of refreshes. */
  sessionMaxAgeSeconds?: number;
  /** The approval workflow (queue, decisions, suspension, audit). Off when absent. */
  approval?: ApprovalConfig;
  /** Default 90 days. Known-device tokens, issued at each successful login, expire after this. */
  knownDeviceTtlSeconds?: number;
  /** Name of the web known-device cookie. Default `__Host-device`. Native clients use the `X-Device-Token` header. */
  deviceCookieName?: string;
  /** Minimum gap between verify/reset mails to one account (default 120 s). Requests inside it get the uniform reply and no new mail. */
  mailGapSeconds?: number;
  /**
   * Most verify/reset mails of one kind to one account within that kind's link lifetime (default
   * 10; reset links live 1 h, verify links 24 h). Because the window equals the lifetime, whenever
   * the ceiling suppresses a mail the newest links already sent are still valid.
   */
  mailCeiling?: number;
  /**
   * Optional global budget on verify/reset mails (one counter, NOT keyed by IP), counted before any
   * link changes: over it, nothing happens (no new link, existing links untouched) and the reply
   * stays the same. Enable it only together with that per-address relief in mind: an owner whose
   * request is suppressed must still have the links already mailed. Volumetric floods belong to
   * Cloudflare WAF / rate-limiting rules in front of the Worker.
   */
  mailBudget?: RateLimitRule;
  /** Lifetime of a device enrolment code (default 15 minutes). */
  enrolCodeTtlSeconds?: number;
  /** How many of an account's newest links of one kind stay valid at once (default 3). */
  outstandingLinks?: number;
  /** Default 10. A rotated token presented within this many seconds returns the same successor. */
  reuseGraceSeconds?: number;
  /**
   * Called when a rotated refresh token is replayed after the grace window and the family is
   * revoked (`refresh_reuse_revoked`): a likely stolen token. Wire it to alerting. Must not throw.
   * Grace-window repeats are routine (two tabs) and go only to `onEvent`.
   */
  onRefreshReuse?: (e: AuthEvent) => void;
  verifyTokenTtlSeconds?: number;
  resetTokenTtlSeconds?: number;
  /** How long a deleted account's salted email hash is kept. */
  tombstoneEmailTtlSeconds?: number;
  /**
   * `purgeExpired` hard-deletes accounts still at the initial trust level (never activated) after
   * this time (default 7 days), freeing an address someone squatted. Pass null to keep them.
   */
  purgeUnverifiedAfterSeconds?: number | null;
  minPasswordLength?: number;
  /** Extra words to reject, in addition to the built-in list. */
  extraCommonPasswords?: readonly string[];
  /** Async extra check (for example a breached-password lookup). Runs after rate limiting. */
  passwordCheck?: (password: string, email: string) => Promise<string | null>;
  /** Validates and extracts profile fields from the sign-up body. Throw `InputError` to reject. */
  profile?: (body: Record<string, unknown>) => ProfileInput;
  /**
   * App content cleanup on account deletion. The statements run in a second batch, only after the
   * tombstone has been written. Without this hook a warning is logged on every deletion, because the
   * package alone cannot know what else references the user.
   */
  onDelete?: (userId: string, now: number) => D1PreparedStatement[];
  /**
   * Called once per account, from waitUntil, when it becomes active: the address is confirmed and
   * the mailbox owner has set the password and profile (verify with password, or a reset).
   */
  onVerified?: (userId: string) => Promise<void>;
  /** Extra data merged into the account export. */
  exportExtra?: (userId: string) => Promise<Record<string, unknown>>;
  /** Audit hook; must not throw. Never receives passwords, tokens or emails. */
  onEvent?: (e: AuthEvent) => void;
  /**
   * Client IP. Default: the CF-Connecting-IP header. When it yields nothing, every rate-limited
   * route refuses the request (403 client_ip_required) instead of pooling such requests together.
   */
  clientIp?: (req: Request) => string | null;
  /** Clock in epoch ms (tests). */
  now?: () => number;
}

export interface Principal {
  userId: string;
  sessionFamily: string;
  trust: string;
}

export interface WaitUntil {
  waitUntil(p: Promise<unknown>): void;
}
