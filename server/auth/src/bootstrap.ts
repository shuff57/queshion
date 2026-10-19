// Operator-only SQL for creating and demoting admins. There is no API route for either. The worker's
// scripts run these through `wrangler d1 execute`, and the tests run the very same statements, so the
// eligibility rules are tested where they are used. No imports: scripts load this file directly.

const EMAIL = /^[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}$/;
const LEVEL = /^[a-z0-9_-]{1,32}$/;

function lit(s: string, re: RegExp, what: string): string {
  if (!re.test(s)) throw new Error(`refusing ${what}: ${JSON.stringify(s)}`);
  return `'${s}'`; // the pattern admits no quote, backslash or whitespace
}

const nowExpr = (now?: number) => (now === undefined ? `CAST(strftime('%s', 'now') AS INTEGER) * 1000` : String(Math.floor(now)));

/**
 * Promote one account to `adminLevel`, or recover a SUSPENDED admin (when every admin is suspended,
 * this is the way back in). Eligible: confirmed address, a password set by the mailbox owner, not
 * deleted, and either at one of `fromLevels` and not suspended, or an admin who is suspended. Audited
 * with actor 'bootstrap'. Running it twice changes nothing.
 */
export function makeAdminStatements(o: { email: string; adminLevel: string; fromLevels: string[]; now?: number }): string[] {
  const e = lit(o.email.toLowerCase(), EMAIL, "email");
  const admin = lit(o.adminLevel, LEVEL, "level");
  const from = o.fromLevels.map((l) => lit(l, LEVEL, "level")).join(", ");
  const now = nowExpr(o.now);
  const eligible = `email = ${e} AND email_verified_at IS NOT NULL AND pw_hash IS NOT NULL AND deleted_at IS NULL
    AND ((trust IN (${from}) AND suspended_at IS NULL) OR (trust = ${admin} AND suspended_at IS NOT NULL))`;
  return [
    // The audit row first, while the previous level is still on the row.
    `INSERT INTO auth_audit (id, at, actor_id, actor_kind, action, target_id, prev_trust, new_trust, note)
     SELECT lower(hex(randomblob(16))), ${now}, 'bootstrap', 'bootstrap',
            CASE WHEN suspended_at IS NULL THEN 'make_admin' ELSE 'restore_admin' END, id, trust, ${admin}, 'operator bootstrap script'
     FROM users WHERE ${eligible}`,
    `UPDATE approval_requests SET decided_at = ${now}, decided_by = 'bootstrap', decision = 'bootstrap'
     WHERE decided_at IS NULL AND user_id IN (SELECT id FROM users WHERE ${eligible})`,
    `UPDATE users SET trust = ${admin}, suspended_at = NULL, suspension_reason = NULL WHERE ${eligible}`,
    `SELECT email, trust, suspended_at FROM users WHERE email = ${e}`,
  ];
}

/**
 * Demote one admin to `toLevel`, revoking their sessions and device tokens. Never the last active
 * admin. Audited with actor 'bootstrap'.
 */
export function demoteAdminStatements(o: { email: string; adminLevels: string[]; toLevel: string; now?: number }): string[] {
  const e = lit(o.email.toLowerCase(), EMAIL, "email");
  const admins = o.adminLevels.map((l) => lit(l, LEVEL, "level")).join(", ");
  const to = lit(o.toLevel, LEVEL, "level");
  const now = nowExpr(o.now);
  const eligible = `email = ${e} AND trust IN (${admins}) AND deleted_at IS NULL
    AND (SELECT COUNT(*) FROM users o WHERE o.trust IN (${admins}) AND o.id != users.id
           AND o.deleted_at IS NULL AND o.suspended_at IS NULL) >= 1`;
  return [
    `INSERT INTO auth_audit (id, at, actor_id, actor_kind, action, target_id, prev_trust, new_trust, note)
     SELECT lower(hex(randomblob(16))), ${now}, 'bootstrap', 'bootstrap', 'demote_admin', id, trust, ${to}, 'operator bootstrap script'
     FROM users WHERE ${eligible}`,
    `UPDATE sessions SET revoked_at = ${now} WHERE revoked_at IS NULL AND user_id IN (SELECT id FROM users WHERE ${eligible})`,
    `UPDATE users SET trust = ${to}, device_epoch = device_epoch + 1 WHERE ${eligible}`,
    `SELECT email, trust FROM users WHERE email = ${e}`,
  ];
}
