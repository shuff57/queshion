// "New application" mails to the admins, with an hourly budget: past it, sign-ups are counted and
// the hourly cron sends one summary instead, so someone with many mailboxes cannot flood the admins.
import { sendTo, type MailEnv } from "./mail.ts";

export const NOTICES_PER_HOUR = 5;

export async function noticeNewApplication(env: MailEnv & { DB: D1Database }, userId: string, now = Date.now()): Promise<"sent" | "deferred"> {
  const hour = Math.floor(now / 3_600_000);
  const row = await env.DB.prepare(`INSERT INTO notice_budget (hour, sent) VALUES (?1, 1) ON CONFLICT (hour) DO UPDATE SET sent = sent + 1 RETURNING sent`)
    .bind(hour)
    .first<{ sent: number }>();
  if ((row?.sent ?? 1) <= NOTICES_PER_HOUR) {
    await sendTo(env, "admins", "verSHion: an account is awaiting approval", `Account ${userId} confirmed its address and is in the approval queue.`);
    return "sent";
  }
  await env.DB.prepare(`UPDATE notice_backlog SET pending = pending + 1 WHERE id = 1`).run();
  return "deferred";
}

/** Hourly: one summary mail for the sign-ups that were over the budget, and old budget rows dropped. */
export async function flushNoticeBacklog(env: MailEnv & { DB: D1Database }, now = Date.now()): Promise<number> {
  const row = await env.DB.prepare(`SELECT pending FROM notice_backlog WHERE id = 1`).first<{ pending: number }>();
  const n = row?.pending ?? 0;
  await env.DB.batch([
    env.DB.prepare(`UPDATE notice_backlog SET pending = pending - ?1 WHERE id = 1`).bind(n),
    env.DB.prepare(`DELETE FROM notice_budget WHERE hour < ?1`).bind(Math.floor(now / 3_600_000) - 24),
  ]);
  if (n > 0)
    await sendTo(env, "admins", `verSHion: ${n} more account(s) awaiting approval`, `${n} more account(s) joined the approval queue in the last hour (not mailed one by one). Bulk-reject from the queue if they are spam.`);
  return n;
}
