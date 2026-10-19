# vershion-pool Worker

The verSHion shared-pool API (docs/shared-pool.md). Accounts, sessions and the approval workflow come
from the generic auth package in `../auth`. This Worker adds the verSHion parts: the share-core
Argon2id wasm, the trust levels `unverified | pending | teacher | admin`, the sign-up profile, mail
text, staff-domain hints and the cron.

## Setup

```sh
wrangler d1 create vershion-pool                 # put the id in wrangler.toml
wrangler secret put AUTH_SECRET                  # 32+ random characters
wrangler secret put RESEND_API_KEY
# vars: ALLOWED_ORIGINS, APP_URL, MAIL_FROM, ADMIN_EMAIL (comma list),
#       ESCALATION_EMAIL (comma list; someone who is NOT the admin), APPROVAL_DIGEST=1 (optional)
scripts/sync-migrations.sh                       # copy the auth package's migrations here
wrangler d1 migrations apply vershion-pool --remote
```

The auth package owns migrations `0001`-`0099` and the Worker owns `0100+` (staff-domain hints).
wrangler reads one directory, so the package's files are copied into `migrations/`. `npm test` fails
if a copy drifts from the package.

## The first admin (and any later one)

**There is no API route that creates an admin.** To promote an account:

1. The person signs up, confirms the address and sets a password (so the account is `pending` or
   `teacher`).
2. You verify who they are **out of band**: in person, or through the school's published phone
   number. Never through a contact they supplied.
3. Run:

```sh
scripts/make-admin.sh person@school.example.org --remote
```

The script refuses anything that is not a plain email address, and generates its SQL from the auth
package's `makeAdminStatements` (the same statements the tests run). It:
- promotes only a confirmed, active, unsuspended account, **or brings back a suspended admin** (the
  way in again if every admin has ended up suspended);
- closes the account's open application with `decided_by = 'bootstrap'`;
- writes an `auth_audit` row with actor `bootstrap`.

Running it twice changes nothing.

`scripts/demote-admin.sh EMAIL --remote` demotes an admin to teacher (sessions and device tokens
revoked, audited as `bootstrap`). It never demotes the last active admin.

> **Warning.** An admin can approve teachers, and approved teachers can read every answer key in the
> pool. Promote only someone you have verified yourself. Keep at least two admins: the last active
> admin can never be suspended, and the cron mails `ESCALATION_EMAIL` when applications wait more than
> 48 hours, so one admin going silent is noticed.

## Using the admin routes

Every admin route needs the admin's live session (bearer token) from a **known device**: the device
cookie set at login, or `X-Device-Token` for a native client.

Every change (approve, reject, revoke-teacher, suspend, restore, staff-domain edits) also needs a
**sudo token**:
1. `POST /auth/reauth` with `{ "password": ... }` returns `{ sudo_token, expires_in: 300 }`.
2. Send it as `X-Sudo-Token` for the next 5 minutes.

It only works for the same session and device epoch, so a stolen access token cannot approve anyone.

| Route | Body |
|---|---|
| `GET /admin/queue?status=open` | Applications, oldest first, with the email domain, the staff-domain hint, the domain's history, and the queue's age. |
| `POST /admin/approve` | `user_id`, `request_id` (from the queue), `evidence` (`directory` or `school_confirmed`), `evidence_note`, `note` |
| `POST /admin/reject` | `user_id`, `request_id`, `reason` (shown to the applicant), `note` (never shown) |
| `POST /admin/reject-bulk` | `items: [{ user_id, request_id }]` (at most 50), `reason`, `note`: for a wave of spam sign-ups |
| `POST /admin/revoke-teacher` | `user_id`, `reason`, `note` |
| `POST /admin/suspend` / `restore` | `user_id`, `reason` (for suspend), `note`. Suspending an **admin** only proposes it. |
| `POST /admin/confirm` / `cancel`, `GET /admin/proposals` | `proposal_id`. Another admin confirms; anyone can cancel. With only two admins, the proposer may confirm alone after 24 h. |
| `GET /admin/users/:id`, `GET /admin/audit` | Account detail with its audit trail; the audit log. |
| `GET`/`PUT`/`DELETE /admin/staff-domains` | `domain`, `note`: a label in the queue only. It **never** grants trust, because students often get addresses on the staff domain. |

Every action against an admin (a suspension proposal, its cancellation, the suspension, an admin
deleting their account) mails **every admin and `ESCALATION_EMAIL` at once**. With `APPROVAL_DIGEST=1`
the daily digest goes to both as well, with the last day's approvals and rejections per admin.

"New application" mails to the admins are limited to 5 an hour. The hourly cron rolls the rest into
one summary, so many sign-ups cannot flood the admins' inboxes. Use bulk reject to clear a spam wave.

The queue keeps what the applicant typed (`applicant`) apart from what the server adds
(`server_labels`: the staff-domain hint, the domain history, `profile_changed`). Render applicant
fields as plain text.

**Approval rule** (docs/shared-pool.md): approve only when the **verified address itself** is on the
school's staff directory (`directory`), or the school office confirmed the person through a contact
**you** initiated (`school_confirmed`). A name on a staff list is never enough.

## Tests

```sh
npm test            # node --test: mail, alerts, notice budget, make/demote-admin on SQLite, domain validation, migrations
npm run typecheck
```

The auth flows themselves are tested in `../auth` (`npm test` there).
