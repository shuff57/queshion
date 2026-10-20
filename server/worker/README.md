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

> **Warning.** An admin can approve teachers, and approved teachers can read every answer key an
> author chose to share (items published without "hide key"); hidden keys stay with their authors. Promote only someone you have verified yourself. Keep at least two admins: the last active
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

## The shared pool (phase 2, `src/pool/`)

`unlisted` means private: published (so it can be republished and kept in sync) but readable only by
its author, as in docs/shared-pool.md "Sharing model". Non-authors see a public version number that
does not move for changes they are not served (a hidden key correction).

R2 bucket binding `POOL` (`wrangler r2 bucket create vershion-pool`; private, never public or
presigned). Migration `0102_pool.sql`. Every route needs `teacher` or `admin`; anything an account may
not see is a 404. share-core is reached only through `src/pool/core.ts`.

| Route | Who | What |
|---|---|---|
| `POST /items` | teacher | new item. Body `Content-Type: application/vnd.vershion.publish`: `<meta JSON>\n<payload bytes>` (the payload goes to share-core unparsed). Meta: `publish_id` (client, >=128 bits, the idempotency key), `hide_key`, `license` (no default), `attestation {accepted, text_rev}`, `labels`, `order_acks`, `image_ok`, `visibility`, `meta {title, subject, course, grade, tags, standards}` |
| `PUT /items/:id` | author | republish; meta adds `base_publish_id`; 409 `conflict` on a lost CAS |
| `PATCH /items/:id` | author | `{visibility}`; `public` refused while the licence is `circle-only` |
| `DELETE /items/:id`, `POST /items/:id/undelete` | author | hidden at once, undo for 14 days, then purged |
| `GET /items?scope=&subject=&kind=&tag=&standard=&q=&cursor=&limit=` | teacher | newest created first (a republish never moves an item across a cursor), subject case-insensitive, `q` = title prefix; signed cursor, max 50, no totals |
| `GET /items/mine` | teacher | own items with state (`live`, `held_for_review`, `deleted`, `taken_down`, ...) and storage used |
| `GET /items/:id` | visible | served form + per-question provenance (`served_sha256`, `match_hash`, `chain`) |
| `POST /items/:id/import` | visible | same, recorded (author: own full copy, not recorded) |
| `POST /items/versions` | teacher | `{ids}` (<=200) -> `current` / `newer` / `removed` / `unavailable` |
| `PUT /blobs/:sha256` | teacher | image upload (PNG/JPEG/GIF/WebP still, <=2 MB, <=4096 px) |
| `GET /items/:id/blobs/:sha256` | visible | only blobs listed under the caller's served form of the current publish |
| `POST /items/:id/report` | teacher (not the author) | `{reason, note}` -> always "thanks" |
| `POST /items/:id/counter-notice` | author of a taken-down item | 512(g)(3) fields; refused after the 30-day quarantine window |
| `POST /items/:id/appeal` | author of an admin-removed or held item | `{note}`: raises an admin flag |

Admin routes (behind the admin guard; ids in the body or query, because the guard matches exact
paths): `GET /admin/pool/reports`, `GET /admin/pool/flags`, `GET /admin/pool/item?id=`,
`POST /admin/pool/hold|release|dismiss|remove|unremove|purge-preview|purge|unblock|resolve-flag`, `PUT /admin/pool/limits`,
`POST /admin/dmca/notices|takedown-preview|takedown|court-action|resolve|hold-copies|release-copies|unblock|withdraw`, `GET /admin/dmca/copies?action_id=` (`resolve {action_id, outcome:
"restore"|"remove"}` ends a court action).

Errors: `pending`/`unverified` accounts get `403 account_not_approved` with a message; `quota_exceeded`
and `shared_questions_limit` carry `used`, `needed`, `limit`; a refused payload does not use the day's
publish allowance; a purged item answers `409 item_removed {purged: true}` (publish it as new).

### Limits (defaults; an admin changes them with `PUT /admin/pool/limits`)

| Limit | Default |
|---|---|
| Publish body | 4 MB payload + 1 MB meta (`maxPublishBytes` in the pool deps). Measured: one 3.8 MB hidden-key bank peaks at about 49 MB of JS heap and buffers plus 25 MB of wasm; publishes are parsed one at a time per isolate (4 more may wait, then 503 `busy`) and one at a time per account (409 `publish_in_progress`) |
| Storage per account | 200 MB (`storage_bytes`); deleted-but-not-purged bytes have their own 200 MB (`removed_bytes`) |
| Shared questions per account | 5,000 (`shared_questions`), in-flight publishes included |
| Daily per account | 50 publishes, 300 refused publishes (reserved before the work), 200 imports, 2,000 item reads, 500 browse pages, 500 uploads and 200 MB, 100 id misses, 100 blob misses, 20 reports, 10 appeals |
| Per IP | 1,200 requests / 5 min; 50,000 reads / day |
| Reports | 3 distinct teachers hold an item; a student-data report holds at once, at most 3 a day per reporter, 50 a day site-wide (`instant_holds_global`), never from accounts younger than 7 days |
| Blocks | privacy and DMCA blocks cover only the questions and images the admin designated after a preview; a question or image used by 3+ other authors (`block_common_k`) is skipped unless selected; DMCA blocks can expire (`block_days`); both can be lifted (`/admin/pool/unblock`, `/admin/dmca/unblock`) |
| Tags | 200 distinct tags and topics per item (more is an error) |
| Images | PNG, JPEG, GIF, WebP; one still image, 1..4096 px a side, 2 MB; an import holds the images it served for the importer for 30 days |

Evidence for licences and attribution: a question the account imported or read counts only if it is not
the account's own work (nor derived from it) and was first published before the account first published
such a question; a verbatim copy takes its grants from the identical sources only. A mixed-licence bank
that cannot be published gets `per_question` in its 422 (split the bank, or pick a licence they all allow;
there is no per-question licence). An author may choose (`PUT /items/prefs {keep_on_delete}`) to keep
their shared items, shown as by "deleted teacher", when they delete their account; otherwise they are
removed and purged after 14 days.

The hourly cron runs the pool GC (abandoned staged publishes, expired pending uploads, unreferenced
blobs); the daily one runs DMCA put-backs, quarantine expiry and purges.

## Tests

```sh
npm test            # node --test: mail, alerts, notice budget, make/demote-admin on SQLite, domain validation, migrations
npm run typecheck
```

The auth flows themselves are tested in `../auth` (`npm test` there).
