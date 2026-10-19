# Phase 1 progress

## Piece 1: auth core (sign-up, verify, login, refresh, logout, reset, deletion, rate limits)

2026-10-19, builder pass 1. Nothing committed.

**Built**
- `server/auth/`: a self-contained TypeScript package (`d1-auth`, MIT, no verSHion naming) with its
  D1 migration, README, and 96 tests (`npm test`, vitest in workerd with real D1, no Cloudflare account).
- `server/worker/src/index.ts`, `mail.ts`: the verSHion wiring. It uses the share-core wasm as the
  hasher, the trust levels `unverified|pending|teacher|admin`, and a sign-up profile hook (display name,
  school, adult school employee statement). Mail goes through Resend, or `MAIL_MODE=log` for local
  dev. The admin is notified on verification, and a daily cron runs `purgeExpired`. `wrangler.toml`
  now points `main` at `src/index.ts` (the spike file is kept), binds D1, and uses
  `migrations_dir = "../auth/migrations"`.
- Verified: package tests 96/96 and typecheck; worker typecheck; `wrangler deploy --dry-run` bundles
  (97 KiB including the wasm); a local `wrangler dev` smoke run on port 8811 with the real wasm
  (sign-up, verify, web login cookie, refresh, /me, uniform 401s, native+Origin refusal). Wasm output
  was cross-checked against Bun's independent Argon2id.

**Spec deviations and decisions**
- `users`: school and the attestation live in a `profile` JSON column (generic), not a `school` column.
  The tombstone email hash is stored in `deleted_email_hmac` and `deleted_email_hmac_expires_at` on
  the user row.
- `sessions` gains `auth_at` (family start), so a family has a hard maximum age (default 90 days) on
  top of the sliding 30-day refresh.
- `email_tokens` gains `created_at` and `used_nonce`. The nonce identifies the row this request
  consumed. An earlier version keyed on `used_at = now`, which let an invalidated token verify an
  account when both happened in the same millisecond. That bug was caught by a test.
- New table `rate_limits` (the D1 counter store).
- `trust` has no CHECK constraint. The allowed values are configured in code.
- Forgot/reset also works for an unconfirmed account, and completing the reset confirms the address.
  The spec says reset mail goes "to the verified address". This is how a real owner takes back an
  address someone else registered first.
- Sign-up does not log the user in (returning tokens would break the uniform response). (Round 2:
  login is now refused while unverified; see below.)
- `DELETE /account` needs the password as well as a live session. Deletion removes the account's
  `approvals` row.
- Grace-window repeat (default now 10 s, see round 2): when the successor has itself been rotated inside the window, the live head of
  the derived chain is returned instead of a dead link.
- Web `/auth/refresh` and `/auth/logout` require an allow-listed `Origin` (CSRF defence in depth).
- `GET /account/export` is implemented in its basic form (account, live sessions, approval).

**Open for later pieces:** admin approval queue and endpoints, the app sign-in screen, the
`shared-pool.schema.json` wire schema, the choice of mail provider, a worker-owned migrations
directory once pool tables exist, and measuring Argon2 CPU time on the edge.

### Round 2 (critic findings), 2026-10-19

1. **Pre-hijack (blocker).** Chosen design: confirming an address never activates a password chosen
   before confirmation.
   - Login is refused while `email_verified_at IS NULL`. The response is the same uniform `401` at
     the same full hash cost.
   - `/auth/verify` discards the sign-up password hash and revokes every session of the user. It
     accepts an optional `password`: if present it is policy-checked and hashed (after the rate
     limit, and only once a cheap token lookup succeeds) and becomes the password. Without it the
     account has no password until forgot/reset.
   - Never-confirmed accounts are purged after 7 days by default (the worker sets it explicitly).
     Resends are capped at 3 an hour and 5 a day per address.
   - `docs/shared-pool.md` edits, kept to the wording this changes: the Sign-up bullet (no login
     before confirmation; confirming discards the password and revokes sessions), the Login bullet
     ("verified accounts only"), the `unverified` row of the trust table ("cannot log in"), and the
     paragraph after the table (unverified can only resend; purged after 7 days).
   - Consequence: sign-up still takes a password (spec, and equal cost), but it never becomes the
     working password; the confirmation page asks for it.
2. **Lockout griefing.**
   - Login's per-account limit now counts only failures, keyed per (account, IP): 5 per 15 min.
   - An account-wide failure count (30 per hour) switches on "under attack" mode, in which a pair
     with any failure waits while a clean IP still logs in.
   - Successes never count.
3. **Login racing reset or deletion.**
   - The session insert is `INSERT ... SELECT ... FROM users WHERE id = ? AND pw_hash = <checked hash>
     AND deleted_at IS NULL AND email_verified_at IS NOT NULL`. If nothing is inserted, the login
     gets the uniform `401`. The re-hash update and the old-cookie revocation are guarded on the new
     session existing.
   - Deletion now writes the tombstone only `WHERE pw_hash = <checked hash>`. Its session, token and
     approval deletes are guarded on the tombstone, and `onDelete` runs in a second batch afterwards.
4. **`authenticate()`** now checks the session in D1 by default; `{ live: false }` is an explicit,
   documented opt-out.
5. **Refresh grace window** defaults to 10 s (configurable). The new `onRefreshReuse` hook fires on
   both the grace repeat and the revoking replay (the worker logs an alert line).
6. **`onDelete`.** The package warns on every deletion when no hook is set. The worker passes
   `onDeleteContent` (`server/worker/src/cleanup.ts`), which logs a `PHASE 2 TODO` error on every
   deletion.
7. **`MAIL_MODE=log`** throws unless the `APP_URL` host is `localhost`, `127.0.0.1` or `[::1]`.
8. **No client IP.** A missing `CF-Connecting-IP` (or a custom resolver returning nothing) gives
   `403 client_ip_required` on every rate-limited route. Local `wrangler dev` sets the header.

**Tests.**
- Package: 115 passing (96 earlier tests, 5 of them updated for the new behaviour, plus 19 new in
  `test/hardening.test.ts`).
- Each fix was mutation-checked: reverting it alone makes its new tests fail.
- Worker: 4 node tests (`server/worker`, `npm test`, no dependencies) and a passing typecheck.

**Spec deviation:** the refresh grace window is 10 s against the spec's 45 s. I left that sentence of
`shared-pool.md` alone because the round-2 instruction limited spec edits to the trust, verify and
login wording.

### Round 3 (second critic), 2026-10-19

1. **Online guessing (blocker).**
   - Login failure slots are now reserved atomically before the hash (hit, then give back on
     success), so concurrent guesses can no longer overshoot. Only failures count.
   - The (account, network) pair groups IPv6 by /48.
   - Past 30 failures per hour the account is locked down: every attempt gets `429` and
     `Retry-After`, without a hash, unless it carries a known-device token for that account. The
     clean-IP exception from round 2 is gone.
   - Known-device tokens are `v1.<user>.<epoch>.<expiry>.<mac>`. They have their own HKDF key, are
     per account, last 90 days, go in the `__Host-device` cookie (web) or the `X-Device-Token` header
     (native), and are issued at every successful login.
   - They are revoked by bumping the new `users.device_epoch` (migration `0002_known_devices.sql`) on
     reset, logout-all, confirmation and deletion.
2. **Recovery cannot be blocked.**
   - There are no per-account 429s on forgot or resend. A mail goes out unless one went to that
     account in the last 2 minutes, or 30 in 24 hours, and the newest 3 links of each kind stay valid.
   - The reply is always the uniform `202`.
   - Mail tokens are now kept a day after use so the ceiling can count them.
3. **Profile.**
   - Confirmation clears `display_name` and `profile` together with the password.
   - "Active" now means at least `afterVerify` trust. It requires the confirmed address plus a
     password and profile set by the mailbox owner.
   - Verify with a password requires the profile fields. A reset on an account that is not yet active
     requires them too.
   - The approval row is only created at activation, so the queue never shows student-typed data.
4. **Activation and onVerified.** One `activate()` step (an UPDATE ... RETURNING) handles both paths
   (verify with password, and reset). `onVerified` and the approval row fire exactly once per account.
5. **The two tests.**
   - `ratelimit.test.ts` now uses the shipped limits.
   - The hardening distributed-attack test also asserts that a fresh network with a wrong password is
     refused.
6. **Dummy verify.** The unknown-address dummy uses the current parameters, and known accounts use
   their stored parameters. A test documents the residual, and the README ("Raising the cost")
   recommends raising the cost together with a forced reset, or accepting the leak.
7. **`onRefreshReuse`** fires only for the revoking replay. Grace repeats go to `onEvent` only.
8. **`authenticate({ live: false })` removed.** `authenticate(req)` always does the indexed D1 check.
9. **Deletion approvals.** Deletion keeps the approval row as an anonymised tombstone: the note is
   nulled, and an open request is closed as `account_deleted`.

**`docs/shared-pool.md` edits:**
- The Sign-up bullet: confirmation also clears the name and school, and the account enters the queue
  only once the owner sets them.
- The grace-window aside: an alert fires only after the window.
- The Rate limiting bullet: failure-only per account and per /48, the lockdown with known devices,
  and the recovery gap with 3 valid links.
- The `unverified` trust row: "not yet active".

**Tests.** Package: 130 passing (round 2's 115, a few updated for the new behaviour, plus 14 new in
`test/round3.test.ts`, which include the reviewer's probes P1-P4 as assertions). Each round-3 fix was
mutation-checked: reverting it alone fails at least one test. Worker: 4 node tests and a passing
typecheck.

### Round 4 (third critic), 2026-10-19

1. **C1, a completed reset restores login.**
   - The reset's user update now returns the bumped `device_epoch`, and the reset response issues a
     fresh device token (web `__Host-device` cookie, native `device_token` body field).
   - The reset clears the account's lockdown counters (both per-account rules). Per-network pair
     history is untouched.
   - Verify-with-password activation does the same, so a lockdown planted before activation does not
     survive it.
2. **C2, a valid device token is never refused for others' failures.** It skips the per-IP login
   limit, the per-network pair limit and the account lockdown. Its own failures count against a new
   per-device rule (10 per 15 min), so a stolen token cannot guess freely.
3. **C3, the mail ceiling is counted over each link's own lifetime** (reset 1 h, verify 24 h) and
   defaults to 10. `mailDailyCeiling` is renamed to `mailCeiling`. Whenever a mail is suppressed, the
   recent links already in the mailbox are still valid; after the lifetime a new request mails again.
   The 2-minute gap is unchanged.
4. **The round-3 flood test is rewritten.** It sends 60 forgot requests in 30 minutes, far past the
   ceiling, checks a burst link works, then advances past the link lifetime and checks a new link
   arrives and works.
5. **Tightened failure rate.** I added a second per-account rule, 100 failures a day, and kept the
   hourly 30. I chose it over lowering the hourly threshold to 10 because it locks out fewer real
   users. (Corrected in round 5: with that round-4 store the worst 24 h span was 176, and 205 a day
   with a daily reset. Round 5 replaced the store; see the round-5 numbers below.)
6. **Minor fixes.**
   - Confirmation now retires every outstanding link, including reset links requested before
     confirmation (C4).
   - Wrong passwords at `DELETE /account` count against the same failure slots as login.
   - Plain web logout keeps the device cookie by design. The README documents the shared-lab-computer
     residual (the per-device limit still applies; "log out everywhere" clears it) and suggests a
     "remember this device" opt-in as future work.

**`docs/shared-pool.md`:** in the Rate limiting bullet, I added the 30/h and 100/day numbers, that a
device token also skips the per-network limits, that a completed reset ends the lockdown, and the
per-lifetime ceiling of 10.

**Tests.** Package: 142 passing (130 earlier plus 12 new in `test/round4.test.ts`, which include
C1, C1b, C2, C3 and C4 as assertions). The round-3 flood test was replaced in place. Each round-4 fix
was revert-checked: undoing it alone fails at least one test. Worker: 4 node tests and a passing
typecheck.

### Round 5 (fourth critic), 2026-10-19

Threat model held to: students share the school's IPv4 with teachers, rotate IPv6, run many IPs,
and must never keep a real teacher out for more than minutes.

1. **Recovery from the shared address (P2).**
   - `verify` and `reset` look the token up first and charge the IP limit only for an invalid token.
   - `forgot` and `resend` are limited per (IP, address), 5 an hour, through a new `ipAccount` rule
     type. The per-IP cap is sized for a NAT (300 per 10 minutes). The residual is documented: a
     flood above that blocks `forgot` from that one address, and the owner can request from another
     network, since the link then works from the NAT too.
2. **A reset restores login everywhere (P1).**
   - `device_epoch` is part of every login-failure key (pair, account, device-account). The epoch
     bump in the reset, verify-with-password and logout-everywhere batches therefore re-keys all of
     the account's counters atomically.
   - I removed `clearAccountFailures` and the store's `clear` method, which was the separate step
     that raced the batch.
3. **Refused logins refund the IP (P3).** `limit()` now returns a refund function; a login refused by
   the failure check gives back its per-IP hits.
4. **Device tokens (P4, P5).**
   - The per-device limit is now 10 per 15 min plus 20 a day. A new `perAccountDevice` rule caps all
     of an account's devices together at 40 a day; it never feeds the lockdown.
   - Web logout clears the device cookie only with `forget_device: true`.
   - `__Host-device` keeps the `__Host-` prefix (`Path=/` is required; I chose fixation protection
     over a narrower path).
   - **Also fixed:** tokens issued in the same second for one account were identical, so two devices
     could share a token and its counter. Tokens now carry a random nonce:
     `v1.<user>.<epoch>.<expiry>.<nonce>.<mac>`.
5. **Measured bounds, honest numbers (P6, P7).**
   - I replaced the two-bucket sliding estimate in `D1RateLimitStore` with 12 sub-buckets per window
     (only the oldest, partly elapsed one is weighted). Hits bunched before a boundary no longer buy
     a second allowance.
   - Worst any-24 h span: **100 to 114 at every alignment, was up to 176**.
   - With a reset every morning: **worst any-24 h span 200** (was 205 on average). Each reset starts a
     fresh budget, by design of fix 2.
   - A copied device token gets **20 a day, was about 865**.
6. **The wrong tests.**
   - round4's native-reset test now logs in from a network that took part in the lockdown.
   - The two round4 attacker tests now check the worst 24 h span.
   - The shared-IP, new-device and reset case is in round5 (P1).
   - Verify-token IP charging has its own test.
7. **Deletion purges the account's counters at once (P8).** Every counter that can identify an
   account shares an `a:<hmac(account)>:` key prefix, and deletion range-deletes them for the email
   and user-id forms.

**`docs/shared-pool.md`:** in the Rate limiting bullet I added that verify/reset charge only invalid
tokens, that forgot is limited per (IP, address), that a stolen device token is capped at 20 a day
and never locks the owner out, and that a completed reset re-keys every counter.

**Tests.** Package: 155 passing (142 earlier, a few round-4 assertions corrected, store unit tests
rewritten, plus 12 new in `test/round5.test.ts`, which include P1-P5, P7 and P8 as assertions; the P6
bound is in round4). Each round-5 fix was revert-checked, including the per-account device cap after
the nonce fix. Worker: 4 node tests and a passing typecheck.

### Round 6 (fifth critic), 2026-10-19

Threat model unchanged: students share the school NAT with the teacher, rotate IPv6, run many IPs;
the teacher must never be kept out of recovery or login for more than minutes.

1. **Forgot/resend: no per-IP cap (P-A).**
   - Removed. The only rate limit is per (IP, address), 5 an hour.
   - Flood defence is documented as Cloudflare WAF and rate-limiting rules in front of the Worker.
   - New optional `mailBudget`: one global counter, not keyed by IP. Over it, the link is withdrawn
     unsent and the reply is unchanged.
   - round5 P2 now sends 1200 junk forgots, and P-A sends two floods of 400 (above the old 300 cap).
2. **Refresh and logout** look the token up first and charge the IP only for unknown tokens (or,
   for logout, when nothing valid was presented).
3. **New devices during a lockdown.**
   - **(a) Device enrolment (P-C).** New `device_enrolments` table (migration 0003). A completed
     reset or verify-with-password activation mails a `device_enrol` code: 15 minutes, single use,
     bound to the user and epoch, burned after 5 wrong passwords, leased before the hash.
   - The native app redeems it with the password at `POST /auth/device/enrol` for a device token.
     Only unknown codes are charged to the IP. The app flow is in the README.
   - **(b) Unknown-address login floods (P-B).** A login for an address with no account is refunded
     from the per-IP login budget, in the background, if it fits a separate per-IP allowance
     (`ipUnknown`, 120 per 5 minutes). Responses and timing are unchanged.
4. **Lease before hash (P-D).** `email_tokens.claimed_until` holds a 30-second lease claimed
   atomically before the hash. It is released on any failure and consumed on success. 50 concurrent
   verifies or resets now cost 1 hash.
5. **Global logout needs the refresh token (P-E).** An access token alone gets
   `401 refresh_token_required`.
6. **All-devices cap.** `users.last_device` (migration 0003) records the device of the latest
   successful login, reset, activation or enrolment. That device is exempt from `perAccountDevice`,
   so stolen tokens cannot use the shared cap to lock the owner's current device out.
7. **Retry-After.** One pure `decide()` function now serves both stores. Retry-After is found by
   bisection on the same weighted estimate, so a retry at exactly Retry-After succeeds and one second
   earlier does not (tested on D1 and memory).
8. **Test speed and stability.**
   - New `MemoryRateLimitStore` (same estimator) for long simulations.
   - Two-minute attacker steps and 36-hour spans where a worst-24-hour window is measured.
   - The worst-span test is split per alignment.
   - No per-test timeouts; a suite default of 20 s as headroom, documented in the README.
   - Full suite was about 73 s; now three runs at load averages 6-12 took 9.9 s, 8.8 s and 6.7 s
     (vitest Duration), all passing.

**`docs/shared-pool.md`:**
- The Rate limiting bullet now covers: enrolment code on reset; verify/reset/refresh/logout charging
  only unknown tokens; leases; no per-IP cap on forgot/resend; WAF and global mail budget.
- `/auth/device/enrol` is added to the API list.

**Tests.** Package: 171 passing (155 earlier, a few updated for the new behaviour, plus 15 new in
`test/round6.test.ts`, and the worst-span test split in two). Each round-6 fix was revert-checked.
Worker: 5 node tests (one new for the enrolment mail) and a passing typecheck.

### Round 7 (sixth critic, final), 2026-10-19

The critic found no blocker. Fixed:
- **D1, no existence oracle through rate limits.** The per-IP refund now applies to every failed
  login, real address or not, under one `ipFailed` allowance (renamed from `ipUnknown`). The round6
  test compares a whole sequence; the E1 probe is a test.
- **D2, more devices during a lockdown.** New `POST /auth/device/enrolment-code`. It needs a live
  access token plus a valid device token, is held to the mail gap and 5 codes a day, and does not
  bump the epoch.
- **D3, enrolment claim.** The claim re-checks failures, expiry and the epoch inside the same
  statement. Tested with a D1 wrapper that injects the change between the lookup and the claim.
- **D4, mail budget.** `mailBudget` is taken before the issue/retire batch. Over it, nothing changes
  and links already mailed stay valid (M1 probe). Unknown addresses give the unit back.
- **D5, purge report.** Fields are correctly labelled, and a `deviceEnrolments` count is added.
- **D6, double clicks.** A duplicate while a lease is held gets `409 in_progress`
  (verify/reset/enrol).
- **D7, deletion.** It uses only its own per-account limiter (5/h, 10/day), never the login lockdown.
  The reset escape is documented.

Tests: auth 183 (2 runs: 10.3 s, 11.3 s at load average ~12), worker 5. Both typecheck. Every
round-7 fix was revert-checked.

## Final status (piece 1, auth core)

**What exists**
- `server/auth/`: the `d1-auth` package (MIT, no verSHion naming). Routes:
  - signup, verify, resend, login, refresh (rotation with a 10 s grace), logout (with
    `scope: "global"` and `forget_device`), forgot, reset;
  - `/me`, `DELETE /account`, `/account/export`;
  - `/auth/device/enrol`, `/auth/device/enrolment-code`.
- Also in the package:
  - D1 migrations 0001-0003.
  - Pluggable Argon2id adapter, mailer and rate-limit store (`D1RateLimitStore`, plus
    `MemoryRateLimitStore` for tests).
  - `authenticate()` (always checks the session is live) and a CORS helper.
  - Cron `purgeExpired()` and `findTombstoneByEmail()`.
- `server/worker/`: the verSHion wiring:
  - share-core wasm hasher;
  - trust levels `unverified|pending|teacher|admin`;
  - profile hook (name, school, adult statement);
  - Resend mailer, plus a dev log mode that runs on localhost only;
  - admin notice on activation, daily cron, `requireTrust`;
  - an `onDelete` hook that logs loudly until phase 2.

**How to run**
- `cd server/auth && npm install && npm test` (vitest in workerd with real D1, no Cloudflare
  account).
- `npm run typecheck` in `server/auth`.
- `cd server/worker && npm test` (node, no dependencies) and `npm run typecheck`.

**Spec deviations** (also in `docs/shared-pool.md` where the wording changed)

*Schema*
- School and the attestation live in a `profile` JSON column.
- The tombstone email HMAC lives on the user row.
- New columns: `sessions.auth_at` (90-day family cap); `email_tokens.created_at`, `used_nonce`,
  `claimed_until`; `users.device_epoch`, `last_device`.
- New tables: `rate_limits`, `device_enrolments`.
- `trust` has no CHECK constraint; the levels are configured in code.

*Accounts*
- An account cannot log in until it is active: the address is confirmed AND the mailbox owner has
  set the password and profile. Confirmation discards the sign-up password, name, profile, sessions
  and device tokens. The account enters the approval queue only when it becomes active.
- Reset works on an account that is not yet active, and completes it.
- Sign-up never logs in.
- Never-activated accounts are purged after 7 days.

*Sessions and deletion*
- Deletion needs the password. The approval row is kept as an anonymised tombstone.
- The grace window is 10 s.
- Web refresh and logout need an allow-listed `Origin`.
- Global logout needs the refresh token.

*Rate limiting* (replaces the spec's "per IP and per account")
- Login counts failures only, per account and per network (/48), with a lockdown past 30 an hour or
  100 a day.
- Known-device tokens are exempt from the lockdown and per-IP limits.
- Device enrolment codes let new devices in during a lockdown.
- Forgot and resend have no per-IP cap; they are limited per (IP, address), with a mail gap and a
  ceiling per link lifetime.
- Verify, reset, refresh, logout and enrol charge the IP only for unknown tokens.
- One-time codes are leased before hashing.

**Documented residuals**

*Guessing*
- A persistent attacker gets up to 114 hashed guesses per account in any 24 h span, or 200 if the
  owner resets every day (each completed reset starts a fresh budget).
- A copied device token gets 20 guesses a day, until a reset or a global logout. Plain logout keeps
  the device cookie unless `forget_device` is set.
- On a shared lab computer, the next user inherits the known-device status.
- One device cookie per browser.
- Past 120 failed logins per 5 minutes from one IP, refunds stop and the shared budget fills.
- With a stolen session, deletion allows 10 password guesses a day.

*Mail and recovery*
- In-Worker forgot/resend floods cost D1 writes; the defence is Cloudflare WAF.
- While `mailBudget` is exhausted, nobody gets new mail; owners rely on links already sent.
- Each enrolment code allows 5 wrong passwords, and a hasher error counts as one.

*Other*
- After an Argon2 cost raise, accounts still on the old hash answer faster (an existence leak) until
  their next login.
- The dev `MAIL_MODE=log` prints tokens, and is refused off localhost.
- Argon2 CPU time on the edge is not yet measured.
- `wrangler.toml` still has a placeholder `database_id`.

**Not built yet**
- Approval queue and admin routes (approve/reject, staff-domain hints, limits).
- The Rust app's sign-in, account and approval-status screens (`src/share/`).
- Content cleanup on deletion (`onDelete` is a loud TODO) and every pool table (items, circles,
  blobs, imports, ratings, reports, audit).
- `docs/shared-pool.schema.json`.
- The mail-provider decision (Resend is wired), a worker-owned migrations directory, and a
  Workers-rate-limit-binding store.

## Piece 2: approval queue and admin routes, 2026-10-19

**Built**

*Package (`server/auth`, generic, enabled by the `approval` config)*
- Migration 0004:
  - `approval_requests` (one row per application, at most one open; it replaces `approvals`, whose
    rows are copied over);
  - `users.suspended_at` and `suspension_reason`;
  - `auth_audit`, append-only through triggers (the only change allowed is nulling free-text notes
    once the target account is deleted).
- Routes:
  - `GET /admin/queue` (keyset-paginated, oldest first, status filter; shows queue age and the open
    count);
  - `GET /admin/users/:id`, `GET /admin/audit`;
  - `POST /admin/approve`, `reject`, `revoke` (path configurable), `suspend`, `restore`;
  - `POST /auth/reauth` (sudo) and `POST /account/reapply`.
- Also:
  - `/me` now has a `status`.
  - `reviewReminders()` for the cron.
  - Pluggable extra admin routes, run behind the same guard.

*Worker (`server/worker`)*
- A worker-owned `migrations/` folder: the package's copies plus `0100_staff_domains.sql`, with a
  drift test and `scripts/sync-migrations.sh`.
- Staff-domain hint routes (`GET`/`PUT`/`DELETE /admin/staff-domains`), with registrable-domain
  validation and labels worded "hint (not proof)".
- `/admin/revoke-teacher`, and mail text for every decision.
- The cron's queue reminder goes to `ADMIN_EMAIL` and, on escalation, also `ESCALATION_EMAIL`.
- A loud `onSuspend` TODO for phase 2.
- `scripts/make-admin.sh` with `scripts/make-admin.sql`, tested on node:sqlite against the real
  migrations, and `README.md` with the bootstrap warning.

**Decisions the coordinator should know**

*Sudo token*
- Format `s1.<user>.<session family>.<device epoch>.<exp>.<mac>`: stateless, its own HKDF key, 5
  minutes.
- It is obtained from `POST /auth/reauth`, which needs a live session, a known device and the
  password, and is sent as `X-Sudo-Token`. It is now in the CORS allow-list.
- Bound to the session family, so it survives refresh rotation but not another session; bound to the
  device epoch, so a reset or logout everywhere voids it.
- Required for **every** admin write, reject included. Reads need a known device but no sudo.
- `reauth` counts only wrong passwords (5 an hour, 10 a day). An admin who reauths every 5 minutes is
  never locked out.

*Approvals*
- Evidence kinds are `directory` and `school_confirmed` (configurable). `note` and `evidence_note`
  are always required.
- A rejection needs a `reason` (shown to the applicant) and a `note` (never shown).

*Cooldowns and reminders*
- The re-apply cooldown is 7 days (the worker sets it). Revocation counts like a rejection for
  re-applying.
- The queue escalates after 48 h. The daily digest is off by default (`APPROVAL_DIGEST=1`).

*Suspension*
- Suspension is a column, not a trust level, so restore returns the account to its previous level.
- Login with the right password then gets `403 account_suspended` (wrong passwords still get the
  uniform 401).

*Admin protections*
- An admin can never act on their own account.
- The last active admin can never be suspended. The rule sits inside the suspending UPDATE, so mutual
  suspension leaves one admin.
- Approval cannot create admins; only the bootstrap script can.

*Audit*
- Writes are audited; reads (the queue, user detail, the audit log) are not.

**Spec deviations**
- `approvals` became `approval_requests` (many rows per user), and `audit` is `auth_audit`, with
  structured columns plus a JSON `detail`. The spec's data-model lines were left alone.
- New routes: `/admin/users/:id`, `/admin/audit`, `/admin/revoke-teacher`, `/admin/suspend`,
  `/admin/restore`, `GET`/`DELETE /admin/staff-domains`, `/auth/reauth`, `/account/reapply`.

**`docs/shared-pool.md` edits:**
- The `pending` and `admin` rows of the Trust levels table.
- A short "How decisions are made and kept" list under the approval rule.
- The admin and auth lines of the API list.

**Tests**
- Auth: 202 passing (183 earlier, 6 updated for the table rename, plus 19 new in
  `test/admin.test.ts`). Two runs: 8.9 s and 6.4 s at load averages 9-12.
- Worker: 11 (6 new: migration drift, make-admin on SQLite, domain validation, hint wording, decision
  mails, reminder recipients).
- Revert checks:
  - Each of these, removed alone, fails a test: the sudo check, its session binding, the known-device
    check, the admin-level check, the self-action check, the atomic decision gate, the pending/active
    condition, the last-admin guard, the revoke `changes()` gate, the re-apply cooldown, the
    suspended-login check, and the live suspended check (a test was added for the last one).

**Gaps**
- No admin UI. The Rust app's sign-in, status and admin screens are not built.
- An admin deleting their own account is not blocked by the last-admin rule. The bootstrap script is
  the recovery path.
- Domain validation uses a short public-suffix list, not the full PSL. Hints are labels, so a miss
  only mislabels.
- Content hiding on suspension (`onSuspend`) and on deletion (`onDelete`) are loud phase-2 TODOs.

### Piece 2 fix round (seventh critic), 2026-10-19

No blocker. Fixed:
1. **Last admin and deletion (P1).**
   - Deleting an admin-level account needs the known device and a sudo token.
   - The last-admin rule is inside the tombstone UPDATE (`409 last_admin`), so "A suspends B, then
     deletes self" is refused too.
   - Every deletion writes an `account_deleted` audit row (actor_kind `user`) in the same batch, and
     an admin's deletion alerts everyone.
   - Recovery: `makeAdminStatements` (`scripts/make-admin.sh`) also restores a SUSPENDED admin
     (`restore_admin`, bootstrap-audited).
   - New `demoteAdminStatements` (`scripts/demote-admin.sh`): demotes with sessions and device tokens
     revoked, never the last active admin.
2. **Free text survived deletion (P2).**
   - Migration 0005 moves reasons from `auth_audit.detail` into a `reason` column. Existing rows are
     migrated with the trigger dropped and recreated.
   - The trigger now allows nulling `reason`, `note` and `evidence_note` only once the target is
     deleted. Deletion nulls them, plus the application snapshots and proposal notes.
   - `detail` holds only structured, non-personal values.
3. **Control and format characters (P3).**
   - New `validate.ts`: `cleanText` and `profileIsSafe` refuse Cc, Cf, Cs, Co, Cn, Zl and Zp, then
     NFKC-normalize, collapse whitespace and cap length.
   - Applied in the package to the display name and to every string in the profile JSON, whatever
     the hook returns. The worker's profile and staff-domain notes use it too.
   - Emails must be ASCII, which refuses fullwidth/lookalike `@` and invisible characters.
   - The queue separates `applicant` (typed text) from `server_labels` (never concatenated). The
     README says to render applicant fields as plain text.
4. **Two-person rule.**
   - Suspending an admin-level account is a proposal (`admin_proposals`); a different admin (never
     the target) confirms it.
   - With no third admin, the proposer may confirm alone after 24 h, so a sole possible confirmer is
     not stuck. Proposals expire after 72 h; any admin may cancel; one open proposal per target.
   - `onAdminAlert` fires on every admin-targeted action. The worker mails all admins plus
     `ESCALATION_EMAIL`.
   - With `APPROVAL_DIGEST=1` (default off), the digest goes to the admins and the escalation
     contact, with the last 24 h of decisions per admin.
5. **Named applications.** approve/reject require `request_id`, and the gate is `id = ? AND user_id
   = ?`. A stale page after a re-apply gets 409 and changes nothing (the critic's sequence is tested).
6. **Race-free audit fields.**
   - suspend/restore/proposal audit rows read `prev_trust`/`new_trust` from the row inside the
     statement.
   - All audit `at` values are in ms (bootstrap included). The log pages by `at` then rowid.
   - A bootstrap closing an application sets `decided_by = 'bootstrap'`.
7. **Admin mail flood and queue cost.**
   - The worker sends at most 5 "new application" mails an hour; the rest go into `notice_backlog`,
     and the now-hourly cron mails one summary (migration 0101). Purge and reminders still run once a
     day (04 UTC).
   - Queue filters: `domain`, `status`, `min_age_seconds`, `max_age_seconds`.
   - `POST /admin/reject-bulk`: at most 50, each one gated and audited.
   - Domain history comes from a `domain_decisions` counter table updated in the deciding batch (one
     key lookup per domain on a page). `approval_requests.email_domain` is indexed.
8. **Tests share the real bootstrap.**
   - `admin.test.ts` creates admins only through `makeAdminStatements`.
   - The worker tests run `scripts/admin-sql.ts` (the scripts' generator) on node:sqlite against the
     real migrations.
   - Added: the 0005 data-migration test, last-admin deletion, audit text surviving deletion, and
     control characters in every class.
9. **Snapshots.** Each application stores `display_name_snapshot`, `profile_snapshot` and
   `email_domain` as submitted. The queue and user detail show the snapshot; `profile_changed` flags
   when the live profile differs.

**`docs/shared-pool.md`:** the last admin can be neither suspended nor deleted; the two-person rule
bullet; `reject-bulk`, `confirm`, `cancel` and `proposals` added to the API list.

**Tests.**
- Auth 219 (36 in `admin.test.ts`). Two runs: 5.7 s and 5.3 s.
- Worker 15 (0.6 s). Both typecheck.
- The bundle dry-run builds at 168 KiB, and 0005 and 0101 applied to the existing local D1.
- Revert-checked: last-admin deletion, delete device/sudo, the delete audit, the suspended-admin
  recovery, demote's last-admin rule, reason nulling, `cleanText` on the profile, the profile-JSON
  check, ASCII email, the two-person proposal, confirm not by the target, confirm not by the
  proposer, alerts, the `request_id` gate, the bulk maximum, domain counters, the snapshot display.

**Remaining gaps:**
- The two-person rule covers suspension only (revoke-teacher never targets admins, and admins are
  demoted only by the bootstrap script).
- A target can cancel a proposal against themselves. That is deliberate, so an attacker cannot
  silently propose against a real admin, but a compromised admin can also cancel a legitimate
  proposal. The alerts make that visible.
- The notice budget is a fixed hourly window.
- The public-suffix list is still short.

## Piece 3: Rust client for accounts (`src/share/`), 2026-10-19

Builder pass 1. Nothing committed.

**Built**
- `src/share/`: `transport` (ureq natively with `X-Client: native` and never an `Origin`; `fetch` in
  the browser with `credentials: "include"` and no native headers), `client` (every auth route,
  single-flight refresh, the lost-reply retry inside the 10 s grace), `vault` (OS keychain via
  `keyring`; memory only when there is none, never a file), `api` (errors and their text, `/me`
  status, link and server-address parsing), `model` (Account screen state machine and polling, no
  egui), `ui` (the egui Account window), `tests` (mock server, 63 tests).
- `src/app.rs`: an `account` field, `self.account.show(&ctx)` after `draw_screen` (returns at once
  while closed), `VERSHION_SCREEN=account` as the QA entry, and the web deep link
  (`/account/verify|reset|enrol#token=...`) opening the right page and scrubbing the token from the
  address bar.
- `tests/share_e2e.rs`: an ignored end-to-end run against a local Worker (instructions inside) and
  an ignored real-keychain round trip.
- `Cargo.toml`: `zeroize`, `url` (both targets), `keyring` (native only; Linux store is the pure-Rust
  zbus Secret Service client, so no libdbus headers are needed to build), web-sys
  `RequestCredentials` and `History`.

### Round 2 (critic), 2026-10-19

1. **Server address (blocker).** A build with `VERSHION_POOL_URL` uses only that server: `share.json`
   is ignored for the address, there is no Change anywhere, and it is trusted without asking.
   Without one, every page that sends a password (sign in, sign up, confirm, reset, enrol, delete)
   shows the server large (`api::server_label`: scheme, punycode host, port, with the Unicode form
   beside an IDN) and stays disabled behind a "Send your password to <server>?" modal until that
   exact address is confirmed; the model refuses the submit too. The confirmed address lives in
   the keychain (`Slot::TrustedServer`), not in `share.json`, because a fingerprint stored next to
   the address in the same file can be rewritten by whoever rewrites the address. Any change of
   address (typed, or `share.json` edited behind the app's back) resets trust; an edited file
   only retires the old client, it does not wipe the real server's saved session. Links never
   change the server; a link whose host is not the server's own site (equal, parent/child, or a
   sibling under one parent, with common two-label public suffixes refused) is not used.
2. **Sign-out vs. keychain.** A failed delete returns `ApiError::LocalCleanup`; the screen shows an
   error, never "Signed out". The account goes on a purge list written to `share.json`; the next
   time the screen opens the saved token is deleted (or, if the keychain still refuses, dropped
   from memory and never used), and while the screen is open the delete is retried every 2 s.
   Sign-out also clears the index's current account, a second barrier. A rotated refresh token
   that cannot be saved makes the client delete the old saved one (it would replay and revoke
   the family next run); if that fails too the account goes on the purge list. A later
   successful save lifts the purge (otherwise the retry would delete the live token).
3. **Accounts.** Vault entries are per server and account id (`refresh:<id>@<server>`,
   `device:<id>@<server>`, plus an `index@<server>` mapping addresses to ids). The id comes from
   `user.id` (login/refresh) or from the device token itself (`v1.<id>.…`: verify, reset, enrol
   return no id). A device token is only stored under the account it names. A verify/reset link
   for another account leaves this session and device token alone and says so; signing in as a
   second account ends the first one's local session.
4. **Processes.** Refreshes take a cross-process lock (`std::fs::File::lock` on
   `<store>/share-refresh.lock`; `navigator.locks` in the browser, compile-checked only) and
   re-read the saved token under it, so a token another process rotated is never replayed.
5. **Refresh timeout.** `/auth/refresh` gets its own 4 s limit (a second ureq agent; an
   `AbortSignal` on the web); a lost reply is retried once only if the first try started under
   5 s ago, so the retry reaches the server inside its 10 s grace. Compile-time asserts tie
   `REFRESH_TIMEOUT`, `REFRESH_RETRY_WINDOW` and `SERVER_REUSE_GRACE` together.
6. **TLS.** ureq's `platform-verifier` feature: the share transport uses the OS trust store (a
   school's inspection root is honoured). TLS failures are `NetKind::Tls` / `ApiError::Tls`, with
   a network/certificate message and never a password hint. `src/ai.rs` is unchanged.
7. **Late results.** `save_server` refuses while a request runs; jobs carry the client generation
   and results of a replaced client are dropped; a retired client writes nothing more.
8. **Panics.** A drop guard resets the single-flight flag and wakes waiters with an error.
9. **Bodies.** `client::json_body` serialises into an exactly sized buffer and overwrites every
   string of the JSON value; the body is serialised once per call and each try's copy is a
   `Secret`. Not reachable: serde_json's and ureq's internal buffers, kernel socket buffers, and the
   JS string `fetch` gets.
10. **Paste box.** Masked like a password with a "Show what I pasted" box (egui disables copy
    out of a password field; AccessKit reports it as a password), cleared when parsed, re-masked
    on every page change.

**Disagreements.**
- Trust is kept in the keychain instead of a fingerprint in `share.json` (reason above). With no
  keychain, or in a browser without a built-in address, the confirmation lasts for that run or
  page only.
- "Show which account a link belongs to before using it" is not possible: the server has no
  lookup for a token's account (one would also be an oracle). The pages say which account is
  signed in and that the link may be for another; afterwards the client tells and protects the
  current session.
- "Two processes refresh once": each process needs its own access token (they are never stored),
  so the guarantee is that no refresh token is ever presented twice. Tested with two clients over
  one vault and a real file lock.
- Background retries of a failed delete run only while the Account screen is open, plus at the
  next open: the app does no I/O for sharing while the screen is closed.

### Round 3 (verification critic), 2026-10-19

1. **Sign-out deletes locally first (D2).** `Client::begin_sign_out` takes the session out of memory
   on the UI thread (no I/O) and bumps a session epoch; the job then deletes the saved refresh
   token (and the device token if asked) from the keychain, reports that at once
   (`Outcome::SignedOutLocal`), and only then tells the server with the tokens it took, which
   exist only in that call. Killing the app at any instant leaves "signed in" (before the click
   took effect) or "signed out", never a saved token waiting on a sign-out. A login or refresh
   answered after the sign-out keeps nothing (the epoch check runs before, during and after the
   keychain write). "Sign out everywhere" works the same way; if the server call fails, this
   device is still signed out and the person signs in again to retry. **Deletion is the
   opposite on purpose:** nothing local is wiped until the server confirmed the deletion, since
   a failed deletion must leave a working account; an app killed mid-deletion keeps a token that
   the next refresh finds dead (401, signed out).
2. **Server mode is a build decision (D1).** `model::server_mode` (pure; tests cover every case):
   a built-in `VERSHION_POOL_URL` is the only server and needs no confirmation; typed addresses
   (and `http://localhost`) exist only in debug builds or with the off-by-default
   `pool-custom-server` feature, with a "DEVELOPMENT BUILD: custom server" banner on every
   password page and the per-address confirmation; a release without a built-in address shows
   "Sharing is not set up in this build" and no password page. `build.rs` fails the build on an
   invalid `VERSHION_POOL_URL` (an invalid one could otherwise never reach run time; if it did,
   the mode is "not set up", never "typed").
3. **The lock gives up (D3).** `FileLock` uses `try_lock` in a loop for 10 s, then fails with the
   retryable `ApiError::LockBusy` (session kept). Sign-out never waits for a running request: it
   abandons it (its results are dropped and, by the epoch, cannot save anything).
4. **No keychain on the UI thread (D4).** Opening the account (factory, keychain open, loading
   tokens, pending purges, trust check) runs as a job; the screen shows "Opening". Saving trust
   and purge retries run on a second lane. The UI thread only reads cached state. A test records
   the thread of every vault call during open, sign-in, sign-out and purge retry and asserts none
   is the UI thread. The 500-frame closed-window test still passes (nothing runs while closed).
5. **Purge entries per server (D6).** `share.json` `purge` is a list of `{server, account}`; only the
   current server's entries are applied, the others are kept for later.
6. **Shared hosting (D7).** Hosts under `pages.dev`, `workers.dev`, `github.io`, `netlify.app`,
   `vercel.app` and 27 more shared-hosting suffixes only match exactly.
7. **Redirects on the web (D8).** `fetch` uses `redirect: "error"` (compile-checked only).
8. **Token text in parsed replies (D9).** Tokens are moved out of the parsed JSON (no copy), and
   the rest of the value is overwritten. Residual: serde_json's parse buffers and the response
   bytes held by ureq or the browser.
9. **Tests (D10).** Model-level tests for the trust gate, two app copies over one keychain and a
   real file lock, and sign-out while a request hangs (the vault is checked while it hangs).
   `tests/share_e2e.rs` lists what the opt-in run does not cover.
10. **Neutral link messages (D5).** After a confirm or reset link: "The address from this link is
    confirmed… If you did not request this on this device, ignore it."

**Server API gaps (for the server owner; the server was not changed).**
- `POST /auth/verify`, `/auth/reset` and `/auth/device/enrol` should return the account's masked
  address (e.g. `t***@school.org`), so the client can say whose account a link changed (and show
  it before an enrolment). Today the client can only tell "this account or another" by the
  account id inside a native device token, and the browser cannot tell at all.
- A reset completed in a browser clears that browser's refresh cookie even when the link was for
  another account.
- The 429 on login does not say whether it is the account lockdown or the per-network limit.

### Pending hooks for the owner

`src/screens.rs` has uncommitted changes, so it was not touched. Until this hook is in, the Account
window opens only through `VERSHION_SCREEN=account` or a web deep link. (Unchanged by rounds 2
and 3.)

1. **Settings card** (`src/screens.rs`, `draw_settings`): insert between the "Where your work is
   stored" card's trailing `ui.add_space(12.0);` and the `// --- AI conversion ---` comment:

```rust
                // --- Sharing account (optional; src/share) ---
                kit::card().inner_margin(Margin::same(18)).show(ui, |ui| {
                    ui.set_min_width(ui.available_width().max(0.0));
                    ui.label(theme::serif("Sharing account").size(18.0));
                    ui.add_space(4.0);
                    ui.label(theme::muted(
                        "Optional. Sign in to share questions with other teachers. Nothing else in \
                         verSHion needs an account.",
                    ));
                    ui.add_space(10.0);
                    if kit::ghost_button(ui, "Open account…").clicked() {
                        app.account.open();
                    }
                });
                ui.add_space(12.0);
```

2. **Web hosting (deployment, not code):** mailed links point at `APP_URL/account/verify` (and
   `reset`, `enrol`). The static host must serve `index.html` for `/account/*`, and the web build
   must use an absolute public URL (`trunk build --release --public-url /`), because `Trunk.toml`'s
   `public_url = "./"` resolves the wasm and JS relative to `/account/`.
3. **Decide the production API URL and build every release with it** (none exists yet):
   `VERSHION_POOL_URL=https://api.example.org cargo build --release`, and for the web
   `VERSHION_POOL_URL=https://api.example.org trunk build --release --public-url /`. `build.rs`
   fails the build on an invalid address. A release built without it has sharing switched off
   ("Sharing is not set up in this build", no password page). Never ship a build with
   `--features pool-custom-server`.
4. **Point the Worker's `APP_URL` and `ALLOWED_ORIGINS` at the web app's origin**, on the same
   registrable domain as the API, and not on a shared-hosting suffix (`*.pages.dev`,
   `*.github.io`, ...): mailed links from a host the client doesn't consider the API's own site
   are refused.
