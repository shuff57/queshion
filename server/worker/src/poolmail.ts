// Mail for pool notices. Ids only: no item text goes into a mail.
import { sendPlain, sendTo, type MailEnv } from "./mail.ts";
import type { PoolNotice } from "./pool/shared.ts";

const EMAIL = /^[^\s@"(),:;<>[\]\\]{1,64}@[a-z0-9.-]{1,253}\.[a-z]{2,63}$/i;

export function poolNotifier(env: MailEnv & { DB: D1Database }) {
  return async (n: PoolNotice): Promise<void> => {
    switch (n.kind) {
      case "student_data_report":
        // Immediately and outside every budget: the item is already hidden.
        return sendTo(env, "admins", "verSHion: student data reported (item hidden)", `Item ${n.itemId} was reported as containing student data and is hidden until you review it (report ${n.reportId}). Confirm and purge, or release the hold, from the pool reports.`);
      case "takedown_author": {
        const u = await env.DB.prepare(`SELECT email FROM users WHERE id = ?1 AND deleted_at IS NULL`).bind(n.authorId).first<{ email: string | null }>();
        if (!u?.email) return;
        return sendPlain(
          env,
          u.email,
          "verSHion: an item was taken down after a copyright notice",
          `One of your shared items (${n.itemId}) was taken down after a copyright notice. If you believe it was removed by mistake or misidentification, you can send a counter-notice from "My shared items" in the app. Each takedown that is not reversed counts as a strike under the repeat-infringer policy in the Terms.`,
        );
      }
      case "counter_notice": {
        const when = new Date(n.restoreNotBefore).toISOString().slice(0, 10);
        const text = `A counter-notice was received for DMCA action ${n.actionId}. The material will be restored no sooner than ${when} (10 business days) unless you notify us that you have filed a court action.`;
        if (EMAIL.test(n.complainantContact.trim())) return sendPlain(env, n.complainantContact.trim(), "Counter-notice received", text);
        return sendTo(env, "admins", "verSHion: forward this counter-notice by hand", `${text}\n\nThe complainant's contact is not an email address; forward it to them yourself.`);
      }
      case "cap":
        return sendTo(env, "admins", `verSHion: an account reached its daily ${n.metric} limit`, `Account ${n.userId} reached its daily ${n.metric} limit. See the pool flags.`);
      case "repeat_infringer":
        return sendTo(env, "everyone", "verSHion: repeat-infringer threshold reached", `Account ${n.userId} has ${n.strikes} upheld copyright strikes in 12 months. Under the repeat-infringer policy, suspend it with /admin/suspend.`);
    }
  };
}
