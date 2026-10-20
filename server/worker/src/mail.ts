// verSHion mail. The provider is still an open question in docs/shared-pool.md (Resend vs
// Cloudflare Email); Resend is wired because it is a single HTTPS call.
import type { MailMessage, MailSender } from "../../auth/src/index.js";

export interface MailEnv {
  /** "resend" (default) or "log". "log" is for local development only and prints links. */
  MAIL_MODE?: string;
  RESEND_API_KEY?: string;
  MAIL_FROM?: string;
  /** Base URL of the app page that handles the links (it reads the token from the fragment). */
  APP_URL?: string;
  /** Comma-separated: every admin who should hear about new applications and reminders. */
  ADMIN_EMAIL?: string;
  /** Comma-separated: a second contact (not an admin) told when the queue is overdue, so one admin going silent is visible. */
  ESCALATION_EMAIL?: string;
}

function link(env: MailEnv, path: string, token: string): string {
  // Fragment, not query: it never reaches a server log or a Referer header.
  return `${env.APP_URL ?? ""}${path}#token=${token}`;
}

function render(env: MailEnv, m: MailMessage): { subject: string; text: string } {
  switch (m.kind) {
    case "verify":
      return {
        subject: "Confirm your verSHion address",
        text: `Confirm this address to finish signing up:\n\n${link(env, "/account/verify", m.token!)}\n\nThe link works once and expires in 24 hours. If you did not sign up, ignore this mail.`,
      };
    case "reset":
      return {
        subject: "Reset your verSHion password",
        text: `Set a new password:\n\n${link(env, "/account/reset", m.token!)}\n\nThe link works once and expires in 1 hour. Using it signs you out everywhere. If you did not ask for this, ignore this mail.`,
      };
    case "signup_existing":
      return {
        subject: "Someone tried to sign up with your address",
        text: m.accountVerified
          ? "Someone tried to create a verSHion account with this address, which already has one. If it was you, sign in or reset your password. Nothing has changed."
          : "Someone tried to create a verSHion account with this address. An account for it already exists but was never confirmed. If you did not create it, reset the password to take it over; that confirms the address as yours.",
      };
    case "password_changed":
      return {
        subject: "Your verSHion password was changed",
        text: "Your password was just reset and every session was signed out. If this was not you, reset it again now and contact the site owner.",
      };
    case "application_approved":
      return {
        subject: "Your verSHion account was approved",
        text: "Your account has been approved. You can now publish to and browse the shared pool, and join circles.",
      };
    case "application_rejected":
      return {
        subject: "Your verSHion application",
        text: `We could not approve your account.\n\n${m.reason ?? ""}\n\nYou can update your details and apply again from the account screen after the waiting period.`,
      };
    case "approval_revoked":
      return {
        subject: "Your verSHion approval was withdrawn",
        text: `Your account's approval was withdrawn and you were signed out.\n\n${m.reason ?? ""}\n\nYou can apply again from the account screen after the waiting period.`,
      };
    case "account_suspended":
      return {
        subject: "Your verSHion account is suspended",
        text: `Your account has been suspended and signed out everywhere.\n\n${m.reason ?? ""}\n\nReply to this mail to contact the site owner.`,
      };
    case "account_restored":
      return {
        subject: "Your verSHion account is restored",
        text: "Your account's suspension has ended. You can sign in again.",
      };
    case "device_enrol":
      return {
        subject: "Add the verSHion app on your other devices",
        text: `To sign in to the verSHion app on another device (even while sign-ins are being blocked because of failed attempts), open this link on that device within 15 minutes and enter your password:\n\n${link(env, "/account/enrol", m.token!)}\n\nThe link works once. If you did not just reset your password or confirm your address, ignore this mail.`,
      };
  }
}

async function sendRaw(env: MailEnv, to: string, subject: string, text: string): Promise<void> {
  if (env.MAIL_MODE === "log") {
    assertMailModeSafe(env);
    console.log(`[dev mail] to=${to} subject=${subject}\n${text}`);
    return;
  }
  if (!env.RESEND_API_KEY || !env.MAIL_FROM) throw new Error("mail not configured");
  const res = await fetch("https://api.resend.com/emails", {
    method: "POST",
    headers: { Authorization: `Bearer ${env.RESEND_API_KEY}`, "Content-Type": "application/json" },
    body: JSON.stringify({ from: env.MAIL_FROM, to: [to], subject, text }),
  });
  if (!res.ok) throw new Error(`mail provider answered ${res.status}`);
}

/** One plain-text mail to one address (pool notices: takedowns, counter-notice forwarding). */
export function sendPlain(env: MailEnv, to: string, subject: string, text: string): Promise<void> {
  return sendRaw(env, to, subject, text);
}

const LOCAL_HOSTS = new Set(["localhost", "127.0.0.1", "[::1]"]);

/** Log mode prints live tokens, so it refuses to run unless the app is plainly a local dev build. */
export function assertMailModeSafe(env: MailEnv): void {
  if (env.MAIL_MODE !== "log") return;
  let host = "";
  try {
    host = new URL(env.APP_URL ?? "").hostname;
  } catch {
    /* falls through to the refusal */
  }
  if (!LOCAL_HOSTS.has(host)) throw new Error("MAIL_MODE=log is only allowed when APP_URL is on localhost or 127.0.0.1");
}

export function makeMailer(env: MailEnv): MailSender {
  assertMailModeSafe(env);
  return {
    send: (m) => {
      const { subject, text } = render(env, m);
      return sendRaw(env, m.to, subject, text);
    },
  };
}

const list = (v: string | undefined) => (v ?? "").split(",").map((s) => s.trim()).filter(Boolean);

/** "admins": every admin; "everyone": every admin plus the escalation contact. */
export async function sendTo(env: MailEnv, who: "admins" | "everyone", subject: string, text: string): Promise<void> {
  const to = who === "admins" ? list(env.ADMIN_EMAIL) : [...list(env.ADMIN_EMAIL), ...list(env.ESCALATION_EMAIL)];
  for (const addr of to) await sendRaw(env, addr, subject, text);
}

/**
 * Any action against an admin-level account, at once, to every admin AND the escalation contact:
 * a compromised admin cannot quietly suspend or remove the others.
 */
export async function adminAlert(env: MailEnv, e: { action: string; actorId: string | null; targetId: string; at: number; proposalId?: string }): Promise<void> {
  const what: Record<string, string> = {
    admin_suspension_proposed: "proposed suspending the admin account",
    admin_suspension_cancelled: "cancelled a proposal to suspend the admin account",
    admin_suspended: "suspended the admin account",
    admin_account_deleted: "deleted their own admin account",
  };
  await sendTo(
    env,
    "everyone",
    `verSHion ALERT: admin action (${e.action})`,
    `Admin ${e.actorId ?? "(unknown)"} ${what[e.action] ?? e.action} ${e.targetId} at ${new Date(e.at).toISOString()}.` +
      (e.proposalId ? ` Proposal ${e.proposalId}: another admin must confirm it, or it can be cancelled from the admin screen.` : "") +
      " If this was not expected, check the audit log now; scripts/make-admin.sh can restore a suspended admin.",
  );
}

/**
 * The cron's queue reminder. An overdue queue goes to every admin AND the escalation contact, so a
 * single admin going silent cannot leave applicants waiting unnoticed.
 */
export async function queueReminder(
  env: MailEnv,
  e: { open: number; oldestAgeSeconds: number; kind: "digest" | "escalation"; lastDay?: { approved: number; rejected: number; byAdmin: { admin_id: string; approved: number; rejected: number }[] } },
): Promise<void> {
  const hours = Math.floor(e.oldestAgeSeconds / 3600);
  // The digest also goes to the escalation contact: the last day's decisions per admin are what
  // makes one admin approving a wave of accounts visible to someone else.
  const to = [...list(env.ADMIN_EMAIL), ...list(env.ESCALATION_EMAIL)];
  const subject =
    e.kind === "escalation"
      ? `verSHion: ${e.open} application(s) waiting, the oldest for ${hours} h`
      : `verSHion: daily approval digest (${e.open} waiting)`;
  const text =
    e.kind === "escalation"
      ? `The approval queue has ${e.open} open application(s); the oldest has waited ${hours} hours. If the admin is unavailable, a second admin can be promoted with server/worker/scripts/make-admin.sh.`
      : `The approval queue has ${e.open} open application(s); the oldest has waited ${hours} hours.` +
        (e.lastDay
          ? `\nLast 24 h: ${e.lastDay.approved} approved, ${e.lastDay.rejected} rejected.` +
            e.lastDay.byAdmin.map((x) => `\n  admin ${x.admin_id}: ${x.approved} approved, ${x.rejected} rejected`).join("")
          : "");
  for (const addr of to) await sendRaw(env, addr, subject, text);
}
