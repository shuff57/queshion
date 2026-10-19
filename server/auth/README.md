# d1-auth

Email and password auth for Cloudflare Workers and D1. One dependency-free TypeScript module:
sign-up, email verification, login, rotating refresh tokens with a reuse grace window, logout,
forgot/reset password, account deletion with a tombstone, and pre-hash rate limiting.

Argon2id is not bundled: you pass any implementation of one function (a wasm build, a native
binding, or a test stub).

## What it guarantees

- **No account enumeration.** Sign-up, resend and forgot return the same status, headers and body
  whatever the address. Login gives the same `401 invalid_credentials` for an unknown address and a
  wrong password, and both cost one full Argon2id hash (unknown addresses get a dummy hash with the
  current parameters). Sign-up always hashes. The database work is one batch in every case, and mail
  goes out from `waitUntil`, so response time does not depend on it. An address that already has an
  account gets a "someone tried to sign up" mail (`signup_existing`) instead of a verification mail.
  Rate limits apply per address whether or not it exists.
- **Confirming an address never activates anything chosen before confirmation.** Anyone can type
  someone else's address into sign-up. So an account is not *active* (it stays at the first trust
  level and cannot log in) until the mailbox owner has set its password and profile. Until then a
  login gets the same `401` as a wrong password. Confirming discards the sign-up password, display
  name and profile, and revokes every session and known-device token. The person who clicked the
  link either sends `password` and the profile fields with the token, which activates the account in
  one step (`{"ok":true,"password_set":true}`), or leaves them out and completes the account through
  forgot/reset, which then requires the profile fields. Activation opens the approval request and
  calls `onVerified` exactly once per account, on whichever path it happens. Accounts never activated
  are purged after 7 days by default.
- **Recovery cannot be blocked, even from a shared school address.**
  - `verify`, `reset`, `refresh`, `logout` and `device/enrol` look the token or code up first (by
    its SHA-256) and charge the IP limit only for an **unknown** one, so junk from a school NAT can
    never make a valid link, refresh or logout fail.
  - A valid one-time code is claimed with a 30-second **lease** before any password hash. Fifty
    concurrent requests with one link cost one hash. A duplicate arriving while the lease is held
    (a double click) gets `409 in_progress`; once the link is used, `400 invalid_token`. Only someone
    holding the link sees the difference. A failed request gives the lease back, and a stale lease
    expires on its own. Only the link's holder (the mailbox owner) can claim it, so the lease cannot
    keep the owner out.
  - `forgot` and `resend` have **no per-IP cap**: they do no hashing, and any per-IP cap can be
    filled from a shared address with random addresses. Their only rate limit is per (IP, address),
    5 an hour, which junk aimed at other addresses never touches.
  - Flood defence belongs in front of the Worker: Cloudflare WAF and rate-limiting rules for
    volumetric abuse.
  - Optionally, a global `mailBudget` (one counter, not keyed by IP) bounds the total verify/reset
    mail. It is taken **before** any link changes. Over it, nothing happens (no new link, and every
    link already mailed stays valid) and the reply stays the same. A request for an address with no
    account gives its unit back.
  - Enable `mailBudget` only with that per-address relief in mind: an owner whose request it
    suppresses is served by the links already in their mailbox. While it is exhausted, nobody new
    gets mail.
  - Per account there is never a `429`. A mail goes out unless one of that kind went to the account
    in the last 2 minutes (`mailGapSeconds`), or 10 went out within that kind's link lifetime
    (`mailCeiling`; reset links live 1 h, verify links 24 h). The newest 3 links
    (`outstandingLinks`) all stay valid.
  - Because the ceiling window equals the link lifetime, a suppressed mail always leaves recent,
    working links in the mailbox, and a new request mails again once the window passes. Every link
    goes to the mailbox, so someone who is not its owner can trigger mail but cannot leave the owner
    without a working link.
- **Rate limits run before any hash.**
  - The per-IP login limit (IPv6 /64) bounds CPU. A login refused by the failure check gives its IP
    hit back, so a shared address's budget counts only requests that were actually hashed.
  - **Every failed login**, for a real address or a made-up one, is charged like any other and then
    refunded in the background if it fits a separate per-IP allowance (`ipFailed`: 120 per
    5 minutes). So a flood of wrong passwords from a school NAT cannot starve real teachers there.
  - Because the refund is the same for every failure, the budget never behaves differently for
    addresses that exist (no existence oracle through rate limits). It happens after the response is
    written, so answers and timing are identical too.
  - For login, only **failures** count against an account. A failure slot is reserved atomically
    before the hash and given back on success, so concurrent guesses cannot overshoot and correct
    logins never count.
  - Per (account, network) pair, with IPv6 grouped by **/48** (one customer can rotate through
    65,536 /64s): 5 failures in 15 minutes block that pair.
  - Per account: past 30 failures in an hour, or 100 in 24 hours, from all networks, the account is
    **locked down**. Every login attempt is refused with `429` and `Retry-After`, without a hash,
    unless it carries a **known-device token** for that account. Refused attempts do not count, so
    the lockdown lifts once the failures age out (back-off, not a hard lock).
  - The D1 store is accurate. Each window is split into 12 sub-buckets and only the oldest, partly
    elapsed one is weighted, so hits bunched before a boundary do not buy a second allowance (the
    two-bucket estimate's flaw).
  - Measured worst case, with one guess a minute from rotating /48s:
    - At most **114 hashed guesses in any 24 h span** (100 to 114, depending on alignment).
    - **200 in any 24 h span if the owner resets every morning.** Each completed reset re-keys the
      counters and so starts a fresh budget. That is the price of letting a reset restore login at
      once.
  - The daily rule was preferred to lowering the hourly threshold to 10, because it locks out far
    fewer real users after a few typos and a handful of students' guesses.
  - **Every failure counter includes the account's `device_epoch`.** A reset, an activation by
    verify-with-password, or `logout` with `scope: "global"` bumps it in its own D1 batch, which
    re-keys all of the account's counters at once (every network, every device), with no separate
    clearing step and no race. So after a completed reset, a new device on the very school address
    where students failed logs straight in.
  - Refusals look the same for known and unknown addresses: the counters are keyed on the address
    either way, and unknown addresses use epoch 0.
  - Counter keys are HMACs, so the table holds no raw IP or email. Every counter that can identify an
    account shares an `a:<hmac>:` prefix, and deleting the account deletes those rows at once.
  - A request with no client IP (no `CF-Connecting-IP`, or your `clientIp` resolver returns
    nothing) is refused with `403 client_ip_required` rather than pooled.
- **Known devices.** Each successful login, reset, or activation by verify-with-password issues a
  token, `v1.<user>.<epoch>.<expiry>.<nonce>.<mac>` (the nonce makes each one distinct). It is per
  account, MACed with its own HKDF-derived key, and lasts 90 days (`knownDeviceTtlSeconds`).
  - *Web:* `__Host-device` cookie (`HttpOnly; Secure; SameSite=Strict; Path=/`). The `__Host-`
    prefix requires `Path=/` and forbids `Domain`, so a sibling subdomain cannot plant or overwrite
    it. That fixation protection is worth more than a narrower path.
  - *Native:* `device_token` in the login body; keep it in the keychain and send it as `X-Device-Token`.
  - A valid device token skips the per-IP login limit, the per-network failure limit and the account
    lockdown, so a teacher on a school NAT is never refused because students failed there.
  - Its own failures count per device (`perDevice`: 10 per 15 minutes and 20 a day). Devices other
    than the owner's **current** one (`users.last_device`: the device of the latest successful login,
    reset, activation or enrolment) also share `perAccountDevice` (40 a day). So several stolen
    tokens are bounded together but cannot use that shared cap to lock out the owner's current
    device. None of these counters feed the lockdown, so a stolen token can neither guess freely
    (measured: 20 hashed guesses a day) nor lock the owner out.
  - Bumping `users.device_epoch` revokes every token of an account at once. That happens on reset,
    confirmation, deletion, and `logout` with `scope: "global"`. Global logout needs the **refresh
    token** (cookie or body), not just an access token, so a stolen 15-minute access token cannot wipe
    the owner's known devices.
  - **Enrolling another device (the native app).** A completed reset or activation by
    verify-with-password mails a one-time **enrolment code**: `device_enrol`, valid 15 minutes,
    single use, bound to the account and its device epoch. The app redeems it at
    `POST /auth/device/enrol` with `{ code, password }` (native: `X-Client: native`) and gets a
    `device_token`. The code proves control of the mailbox and the password proves identity, so the
    app becomes a known device even while a student keeps the account locked down.
    - The code is leased before the hash.
    - It burns after 5 wrong passwords.
    - Only unknown codes are charged to the IP.
    - App flow: the mail's deep link opens the app with the code from the URL fragment, and the app
      asks for the password, redeems the code, keeps the device token in the keychain, and logs in
      with it.
    - **More devices during a lockdown:** a signed-in owner on a known device can ask for a fresh code
      with `POST /auth/device/enrolment-code`. It needs a live access token AND a valid device token
      for the account, is held to the mail gap and 5 codes a day (all sources), mails the code to the
      account's address, and does **not** bump the epoch. So a third, fourth or fifth device can join
      without logging the others out.
    - The redemption re-checks failures, expiry and the epoch inside its own claim, so nothing that
      changes between the lookup and the claim can let a hash through.
  - **Plain logout keeps the device cookie**, so the owner stays known on their own computer. Send
    `forget_device: true` with `/auth/logout` (a "forget this device" box) to clear it, for example
    on a shared machine. The residual: a cookie copied before logout stays valid until a reset or a
    logout everywhere. It is still held to 20 guesses a day and never locks the owner out.
  - Future work: a "remember this device" opt-in at login, off by default on shared machines.
- **Refresh tokens rotate.** A login starts a family; each refresh flips the row's `rotated_at` in one
  atomic `UPDATE ... WHERE rotated_at IS NULL RETURNING` and `INSERT OR IGNORE`s the successor, which
  is `HMAC(server key, predecessor)`. It can be re-derived without being stored, so concurrent
  refreshes converge on one successor. Presenting a rotated token within the grace window (default
  10 s, `reuseGraceSeconds`) returns that same successor (or, if the successor rotated too, the live
  head of the chain); that goes to `onEvent` only, since it is routine. After the window it revokes
  the whole family and calls `onRefreshReuse`, so you can alert on a likely stolen token. Only SHA-256 hashes of tokens are stored. Families also have a hard maximum age
  (default 90 days) that sliding refreshes cannot extend.
- **Web vs native delivery.**
  - *Web* (default): the refresh token goes only in an `__Host-refresh` cookie (`HttpOnly; Secure;
    SameSite=Strict; Path=/`, no `Domain`) and never in a body. `/auth/refresh` and `/auth/logout`
    read the cookie and need an allow-listed `Origin`. CORS echoes one exact origin from the
    allow-list with `Access-Control-Allow-Credentials: true`. Wildcards are refused when the config
    is built. A request from any other origin gets a 403 with no CORS headers.
  - *Native*: send `X-Client: native` and the refresh token comes back in the body and goes back in
    the body. `X-Client: native` together with an `Origin` header is refused, so a browser script can
    never get a token in a body. Preflights never allow the `X-Client` header.
- **Short access tokens.** HS256 JWTs that live 15 minutes. Verification only accepts our exact header
  (no `alg` confusion, no `none`). `authenticate(req)` always also checks in D1 (one indexed read)
  that the session is not revoked and the account still exists, and returns the trust level as it is
  now. There is no token-only mode, so nothing can act on a stale trust level.
- **Login cannot race a reset or deletion.** The session row is inserted only if the account still has
  the exact password hash that was checked, is not deleted and is confirmed. Otherwise the login gets
  the uniform `401`. Deletion likewise refuses if the password changed after it was checked.
- **Reset** tokens are single use, hashed, and expire after 1 hour. A bad
  token never costs a hash. A completed reset revokes every session, kills every outstanding mail
  link, and sends a `password_changed` notice. It also confirms the address, because the link proved
  control of it, and activates an account that was not yet active (the profile fields are then
  required). That is how a real owner takes back an address someone else registered.
- **Verification** tokens are single use, hashed, and expire after 24 hours. They are only consumed by
  `POST`, so mail scanners that prefetch links cannot burn them.
- **Deletion** needs a live session and the password. Its wrong passwords count only against a
  separate per-account delete limiter (5 an hour, 10 a day), never the login lockdown, so the owner
  can delete during a lockdown. If the delete limiter refuses them (someone with a stolen session is
  guessing), a password reset ends it and revokes that session. It nulls every PII column (a schema `CHECK`
  enforces this), deletes the account's sessions and mail tokens, and revokes its device tokens. The
  approval record stays as an anonymised tombstone: decision, decided_by and decided_at are kept, the
  note is nulled, and an open request is closed as `account_deleted`. The user row is kept as a tombstone
  row with `HMAC(server key, email)` for 14 days (`findTombstoneByEmail`) so content can be re-linked.
  Your own cleanup statements (`onDelete`) run in a second batch, only once the tombstone is written.
  Without the hook every deletion logs a warning, so nobody mistakes it for content cleanup. The
  address can sign up again at once.
- **Text people type for others to read** (display names, profile strings):
  - Refused if it contains control or format characters. That covers line breaks, BEL, bidi
    overrides and isolates (U+202A-202E, U+2066-2069), zero-width characters (U+200B-200F), and
    line/paragraph separators, so nobody can fake a label or hide text in an admin's view.
  - NFKC-normalized, whitespace collapsed, length-capped.
  - Checked whatever the `profile` hook returns, so the package is safe by default.
- **Email addresses** must be ASCII (IDN domains in punycode). That refuses lookalike `@` signs and
  every invisible character.
- **Passwords:** NFKC-normalized. At least 10 characters (counted as code points). Checked against a
  short list of common passwords, a common word padded with digits or symbols (including leetspeak),
  runs and repeats, and the user's own email. Add a breach check with `passwordCheck`. Hashes are PHC
  strings carrying their parameters, and they are re-hashed on login when you raise the parameters.
- **Raising the cost.** A login is verified with the account's *stored* parameters, and an unknown
  address is hashed with the *current* ones. Right after a raise, an account still on the old, cheaper
  hash answers a wrong password measurably faster than an unknown address does, which reveals that it
  exists, until its next successful login upgrades it. To avoid this, raise the cost together with a
  forced reset of every account; otherwise accept the leak for the transition.
- **Requests:** only `application/json`, at most 8 KB. Every response is `Cache-Control: no-store`.

## Use

```ts
import { createAuth } from "d1-auth";
import wasm from "./argon2_bg.wasm";
import { initSync, argon2id } from "./argon2.js";

initSync({ module: wasm });

export default {
  async fetch(req, env, ctx) {
    const auth = createAuth({
      db: env.DB,
      hasher: { argon2id: (pw, salt, p) => argon2id(pw, salt, p.m, p.t, p.p) },
      mailer: { send: async (m) => { /* m.kind, m.to, m.token: build the link (put the token in the fragment) */ } },
      secret: env.AUTH_SECRET,                 // 32+ chars; per-purpose keys are derived with HKDF
      allowedOrigins: ["https://app.example.org"],
      trust: { levels: ["unverified", "verified"], afterVerify: "verified", requestApproval: false },
    });
    const res = await auth.handle(req, ctx);   // null when the path is not an auth route
    if (res) return res;
    const who = await auth.authenticate(req); // checks revocation in D1 by default
    // ... your routes; wrap their responses with auth.withCors(req, res)
  },
  async scheduled(_c, env) {
    await createAuth({ /* same config */ }).purgeExpired();
  },
};
```

Apply `migrations/0001_auth.sql` with `wrangler d1 migrations apply`.

### Routes (paths are configurable)

| Method and path | Body | Result |
|---|---|---|
| `POST /auth/signup` | `email`, `password`, plus your profile fields | `202 {"ok":true}` always |
| `POST /auth/verify` | `token`; optional `password` plus the profile fields | `200 {"ok":true,"password_set":bool}`, `409 in_progress` (duplicate while the first is processed), or `400 invalid_token`, `weak_password` or `invalid_request` |
| `POST /auth/resend` | `email` | `202 {"ok":true}` always |
| `POST /auth/login` | `email`, `password`; native: optional `X-Device-Token` header | `200` tokens plus a device token, `401 invalid_credentials`, or `429 rate_limited` |
| `POST /auth/refresh` | native: `refresh_token` | `200` tokens, or `401 invalid_refresh_token` |
| `POST /auth/logout` | native: `refresh_token`; optional `scope: "global"` (needs the refresh token), `forget_device: true` | `204`, or `401 refresh_token_required` for a global logout with only an access token |
| `POST /auth/forgot` | `email` | `202 {"ok":true}` always |
| `POST /auth/reset` | `token`, `password`; the profile fields too if the account is not yet active | `200`, `409 in_progress`, or `400 invalid_token`, `weak_password` or `invalid_request` |
| `GET /me` | (bearer) | the account |
| `DELETE /account` | `password` (bearer) | `204`, `401`, or `429` from the delete limiter |
| `GET /account/export` | (bearer) | account, live sessions, approval state |
| `POST /auth/device/enrol` | `code` (from the `device_enrol` mail), `password` | `200` plus a device token, `409 in_progress`, `400 invalid_token` or `401 invalid_credentials` |
| `POST /auth/reauth` | `password` (bearer plus a known device) | `200 { sudo_token, expires_in: 300 }`, `401`, `403 known_device_required` or `429` |
| `POST /auth/device/enrolment-code` | (bearer plus a known-device cookie or `X-Device-Token`) | `202` (a code is mailed), `403 known_device_required`, or `429` (mail gap or daily cap) |

Errors are `{"error": code}` (plus `reason`/`field` where useful). A `429 rate_limited` carries
`Retry-After`.

### Trust levels

`trust.levels` lists every value the `trust` column may hold, lowest first. New accounts start at the
first level. Confirming the address moves the account to `afterVerify`, but only while it is still at
the first level, so an account is never demoted. With `requestApproval`, confirming also inserts an
`approvals` row for your admin queue. Everything above that (approve, reject, admin roles) is yours.

### Approval workflow (optional)

Pass `approval` in the `createAuth` config to get a review queue: every account that
becomes active lands in it, and nothing promotes an account automatically.

```ts
approval: {
  approvedLevel: "member",          // what an approval grants (never an admin level)
  adminLevels: ["admin"],           // who may review; admins are only ever made by a bootstrap script
  evidenceKinds: ["directory", "school_confirmed"],
  reapplyCooldownSeconds: 7 * 86400,
  escalateAfterSeconds: 48 * 3600,  // the reminder cron escalates past this
  digest: false,                    // or a daily digest even when nothing is overdue
  domainLabels: async (domains) => ({ /* domain: "label" */ }),  // hints only, never trust
  onQueueReminder: async (e) => { /* mail the admins (and a second contact on escalation) */ },
  onSuspend: (userId, now) => [/* statements that hide the user's content */],
  onRestore: (userId, now) => [],
  routes: [/* extra admin routes, served behind the same guard */],
}
```

**Data.**
- `approval_requests`: one row per application, at most one open per account. A re-application is a
  new row; the old decision stays as it was.
- `auth_audit`: append-only. Triggers refuse every UPDATE and DELETE, except nulling free-text notes
  once the target account is deleted, so the trail survives as an anonymised tombstone.
- `users.suspended_at` and `users.suspension_reason`.

**Every admin route** checks all of:
- a live session (revocation and the current trust level are read from D1 on every call);
- an admin trust level;
- a **known device** of that admin (its device cookie, or `X-Device-Token`);
- the per-IP and per-admin rate limits.

**Every admin write** additionally needs a **sudo token**:
- Get one at `POST /auth/reauth` (`{ password }`, with a live session and a known device) and send it
  as `X-Sudo-Token`.
- It is HMAC-signed with its own key, lasts 5 minutes, and is bound to that session family and the
  account's device epoch, so a token from another session, or one from before a reset or logout
  everywhere, is refused.
- Wrong passwords at `/auth/reauth` count against their own limiter (5 an hour, 10 a day); successful
  ones do not.
- So **a stolen admin access token alone can never approve, reject, revoke, suspend or restore.**
  Access tokens live 15 minutes, so the session is also at most that old.

| Route | Does |
|---|---|
| `GET /admin/queue?status=&domain=&min_age_seconds=&max_age_seconds=&limit=&cursor=` | Applications oldest first. Each item has `request_id`, `email`, `email_domain`, age, and two separate objects. `applicant` holds the display name and profile **as submitted with that application** (a snapshot; anything typed at sign-up was discarded at activation). `server_labels` holds the domain hint, the domain's earlier approvals and rejections (precomputed counters, one key lookup per domain), earlier rejections, `profile_changed` (whether the live profile differs from the snapshot), and `suspended`. Server text is never joined with applicant text. **Admin UIs must render every applicant field as plain text.** Also returns the number open and the age of the oldest. |
| `GET /admin/users/:id` | The account, all its applications (with reviewer notes) and its audit trail. |
| `POST /admin/approve` | `{ user_id, request_id, note, evidence, evidence_note }`: all required; `evidence` must be one of `evidenceKinds`. It decides **that** application; a stale page (the applicant has since re-applied) gets `409` and changes nothing. |
| `POST /admin/reject` | `{ user_id, request_id, note, reason }`: `reason` is shown to the applicant, `note` never is. |
| `POST /admin/reject-bulk` | `{ items: [{ user_id, request_id }], note, reason }`: at most `bulkRejectMax` (50). Each one is gated and audited on its own; returns a status per item. |
| `POST /admin/revoke` | `{ user_id, note, reason }`: approved back to pending; sessions and device tokens revoked. |
| `POST /admin/suspend` / `restore` | `{ user_id, note, reason }` / `{ user_id, note }`. Suspended: no login (a `403 account_suspended`, only after the right password), every session and device token revoked, `onSuspend` run. Against an **admin** it only proposes (see the two-person rule). |
| `POST /admin/confirm` / `cancel`, `GET /admin/proposals` | `{ proposal_id }`: confirm (a different admin) or cancel (any admin) a proposal; list proposals. |
| `GET /admin/audit?limit=&before=` | The audit log, newest first. |
| `POST /account/reapply` | The applicant, after a rejection or revocation and the cooldown, applies again with a fresh profile. |

**Two-person rule for admins** (`twoPersonAdminSuspension`, on by default).
- Suspending an admin-level account creates a proposal (`202`). A **different** admin, never the
  target, confirms it.
- If no other admin could confirm, the proposer may confirm alone after `adminSelfConfirmAfterSeconds`
  (24 h), so a sole possible confirmer is never stuck.
- Proposals expire after 72 h, and any admin can cancel one (including its target).
- Every action against an admin is sent, in the background, through `onAdminAlert`: a proposal, its
  cancellation, the suspension, an admin deleting their account. Send it to every admin **and** an
  escalation contact. A compromised admin therefore cannot quietly suspend the others one by one.

**Deleting an admin account** needs the admin's known device and a sudo token. It can never remove the
last active admin (the rule sits in the tombstone UPDATE itself), it alerts everyone, and every
deletion writes an `account_deleted` audit row in the same batch.

**Free text.**
- Reasons, notes and evidence notes live in their own audit columns.
- `detail` holds only structured, non-personal values (ids, a domain).
- After the target account is deleted, those free-text columns are nulled, along with the
  application's snapshots. That is the only change the append-only triggers allow.
- The audit log pages by time, then insertion order.

**Rules.**
- A decision is one atomic gate: `UPDATE approval_requests ... WHERE decided_at IS NULL AND <the
  account is pending, active, not suspended, not deleted> RETURNING`. So concurrent approve/reject
  requests produce exactly one decision and one audit row.
- Revocation is gated the same way on the account still being at the approved level.
- An admin can never decide on, revoke, suspend or restore their **own** account.
- The **last active admin** can never be suspended: the rule lives in the same UPDATE, so two admins
  suspending each other at once leave one active.

**Trust changes take effect at once.**
- Approval raises the level that live checks read, so even an access token minted while pending sees
  the new level.
- Revocation and suspension bump the device epoch and revoke every session, and a suspended account
  fails every live check, refresh and login even before its sessions are revoked.

**The applicant** sees `status` in `/me`: `pending`, `approved`, `rejected` or `revoked` (with the
reviewer's reason and when they may re-apply), or `suspended` (with the reason). Reviewer notes and
evidence are never shown. Decisions are mailed (`application_approved`, `application_rejected`,
`approval_revoked`, `account_suspended`, `account_restored`).

**Reminders.** Call `auth.reviewReminders()` from the cron. When the oldest open application is older
than `escalateAfterSeconds` it calls `onQueueReminder({ kind: "escalation", open, oldestAgeSeconds })`,
or sends a daily digest if `digest` is set. Send escalations to someone besides the admin, so one admin
going silent is visible.

**There is no route that creates an admin.** `makeAdminStatements({ email, adminLevel, fromLevels })`
returns the SQL for an operator to run with `wrangler d1 execute`:
- It promotes a confirmed, active account, or brings back a **suspended admin**, which is the way in
  again if every admin has ended up suspended.
- It closes the account's open application with `decided_by = 'bootstrap'`.
- It writes an `auth_audit` row with `actor_kind = 'bootstrap'`.

`demoteAdminStatements(...)` demotes an admin (sessions and device tokens revoked), never the last
active one. The tests run these exact statements. Promote only someone whose identity you have
verified out of band.

### Hooks

`profile(body)` validates extra sign-up fields (throw `InputError`). `passwordCheck` runs an async
password check (after the rate limit, before the hash). `onVerified` runs in the background.
`onDelete` returns cleanup statements, run once the tombstone is written. `exportExtra` adds data to the export. `onRefreshReuse` alerts on a revoking refresh replay. `onEvent` is
an audit stream that never receives passwords, tokens or emails. `clientIp` and `now` are for unusual
deployments and tests. `rateLimitStore` swaps the D1 counter, for example for the Workers rate-limit
binding.

## Tests

```sh
npm install
npm test        # vitest in workerd via @cloudflare/vitest-plugin, real D1, no Cloudflare account
npm run typecheck
```

How the suite stays fast and deterministic:
- Every test runs on an **injected clock** (`now`). Windows, expiries, grace periods and day-long
  attacks are simulated by advancing it; nothing waits on real time.
- The `Argon2idHasher` is a counting stub (except in `argon2-wasm.test.ts`), so tests can assert
  exactly how many hashes a flow costs.
- **Long simulations** (a day or more of attacker traffic) run against `MemoryRateLimitStore`. It
  uses the exact estimator of `D1RateLimitStore` (the same `decide` function) in memory, so they
  measure the real limiting logic without thousands of D1 round trips. Everything concurrency- or
  storage-related uses real D1.
- Each test makes its own account, client IP and server secret, so tests never share counters.
- No test sets its own timeout. The suite default (20 s, in `vitest.config.ts`) is headroom for a
  heavily loaded CI machine; on an idle machine no test needs more than about 2 s.

`test/argon2-wasm.test.ts` runs the flows against a real Argon2id wasm build and checks known-answer
vectors. It picks the build up from `../share-core/pkg` and is skipped when the build is absent.

## Not included (yet)

OAuth/OIDC, magic links, MFA (beyond the known-device + password sudo step), CAPTCHA, email change, user listing/search outside the queue, and asymmetric (JWKS) access tokens.
