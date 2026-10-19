# Shared pool: accounts, circles and public question sharing

Status: design spec, nothing implemented. Written before any code so the decisions can be argued with.

## Goal

Teachers sign in, publish banks or single questions, and others find and copy them. Sharing has
three levels: **private** (the default, never leaves the device), **circles** (groups a teacher
chooses to collaborate with, joined by invite code) and **public** (any verified teacher).

verSHion stays local-first. Banks, exams and export work exactly as today with no account. Sign-in
is only needed to publish, browse or import from the pool.

## Non-goals (v1)

- Co-editing one bank live. Sharing is publish and copy.
- Student accounts or any student data. Shared content is questions, answers, explanations and figures only.
- Google sign-in and magic-link login (phase 6).
- District-wide admins. Circles are owned by the teacher who made them.

## Stack

| Piece | Choice | Why |
|---|---|---|
| API | Cloudflare Worker (TypeScript) | Nothing to patch or back up; TLS and DDoS protection included |
| Data | D1 (SQLite) with FTS5 | Relational data plus full-text search |
| Images | R2 | Egress is free; content-addressed by SHA-256 like the local `media/` store |
| Mail | Transactional sender (Resend or Cloudflare Email) | Verification and reset mail |
| Client | `src/share/` module in the Rust app, `ureq` natively and `fetch` on wasm | Same API on both builds |

The wire format is one JSON schema (`docs/shared-pool.schema.json`, to be written in phase 1) that
both the Worker and the Rust types are tested against, so the two codebases cannot drift.

The personal VPS is not used. If the Worker is outgrown, the API is plain HTTP+JSON and can move.

**Hosting (assumption, confirm):** the web build and the API are served from one registrable
domain, e.g. `app.example.org` and `api.example.org`, or the Worker routed at `/api/*` on the
app's own host. Cookie auth below depends on this; if the web build stays on a different site
(e.g. `github.io`), browsers will not send the cookie and the design must change.

**One share-core crate.** `strip(question)`, the canonical content hash, and payload validation
live in one Rust crate (`share-core`) that the app links natively and the Worker runs compiled to
wasm. **Only the server strips**: it validates the uploaded full payload, applies the shuffles
with `crypto.getRandomValues`, and stores the result. The client never supplies a stripped form
(a server cannot verify a client's shuffle). The keyless form, the search index rows, previews and
every non-author read path all come from that one function, so they cannot disagree about what
counts as part of the key. The crate has fixture tests that run in both targets.

**Payload validation (server, on every publish).** `ImageRef.file` must match
`^media/[0-9a-f]{64}\.(png|jpg|gif)$` and name a blob the uploader has uploaded; `FigureSpec.params`
is size-capped; unknown fields are dropped; question and part counts are bounded. On the client,
imported images are fully validated (`imagesize` succeeds, hash matches) **before** `put_image`
writes them, because `put_image` currently writes the file first.

## Accounts and auth

- **Sign-up:** email, password, display name, and a statement that the person is an adult
  school employee (COPPA/SOPIPA). A verification mail is sent; until the address is verified and
  the account is approved (see Trust levels) it cannot publish, browse the public pool or join circles.
  An unverified account cannot log in at all. Confirming the address discards the sign-up password,
  display name and school, and revokes every session (anyone can type someone else's address into
  sign-up), so the person who clicks the link enters the password and profile at that step, or later
  through forgot/reset. Only then does the account become `pending` and enter the approval queue.
- **Password storage:** Argon2id compiled to wasm, per-user salt, parameters stored with the hash
  and re-hashed on login when raised. Parameters: `m` between 19 and 46 MiB, `t` = 2, `p` = 1.
  (The wasm module itself is only tens of KB; the memory figure is the per-hash working set.)
  Needs the **paid Workers plan**; the free plan's 10 ms CPU limit cannot run it. Minimum 10
  characters, checked against a short common-password list. PBKDF2 is not used: workerd caps it
  at 100,000 iterations, well under current guidance.
  **Measured (see `docs/phase1-argon2-spike.md`):** a hash is a synchronous wasm call, so requests
  in an isolate run one at a time and concurrent logins do not multiply memory. The 19 MiB
  parameters use about 21 MB of wasm memory and take about 25-35 ms warm locally. The cost that
  matters is CPU time, so the per-IP and per-account rate limit is checked **before** any hashing.
  Chosen parameters: m=19456 KiB, t=2, p=1, 16-byte salt, 32-byte tag, stored with each hash.
  Re-measure on the edge after the first deploy.
  Unknown-email logins do a **full** dummy hash, and sign-up always hashes, whether or not the
  email exists, so timing is the same. A cheaper dummy verify would itself reveal which emails
  exist. The pre-hash rate limit is what bounds the CPU cost. Mail is sent in the background
  (`waitUntil`) so response time does not depend on it.
- **Login** (verified accounts only): returns a short-lived access token (15 min) and a rotating refresh token (30 days).
  Refresh tokens are stored hashed in D1 so they can be revoked. **Reuse detection with a grace
  window:** presenting an already-rotated refresh token within 10 seconds of its rotation (configurable; a replay after the window raises an alert) returns
  the same successor token (two tabs, or a native retry after a dropped response). After the
  window, presenting it revokes the whole session family.
- **Token delivery differs by client:**
  - *Web:* the refresh token is set only as an `HttpOnly; Secure; SameSite=Strict` cookie on the
    API host and is **never** returned in a response body. `/auth/refresh` reads the cookie. The
    app and the API share one registrable domain (see Stack), the client fetches with
    `credentials: 'include'`, and the API answers with `Access-Control-Allow-Credentials: true` and
    an exact origin from an allow-list (never `*`). The access token is held in memory only.
    The web build loads no third-party scripts.
  - *Native:* the refresh token is returned in the login response body, stored in the OS keychain,
    and sent in the body of `/auth/refresh`. A client identifies itself with an explicit
    `X-Client: native` header; the cookie path is used when it is absent. The server **rejects**
    `X-Client: native` on any request that carries an `Origin` header, so a browser script can
    never obtain a refresh token in a response body.
  - *Rotation (both):* the successor token is `HMAC(server_secret, predecessor)`, so it can be
    re-derived without storing it. Rotation is one atomic statement
    (`UPDATE sessions SET rotated_at = now WHERE id = ? AND rotated_at IS NULL RETURNING ...`),
    and the successor row is `INSERT OR IGNORE`d, so concurrent refreshes converge on the same
    successor. Within the 10-second grace window a repeat presentation returns that same derived
    successor (re-sent with `Set-Cookie` on the web path). After the window it revokes the family.
- **Rate limiting:** per IP and per account on login, sign-up, reset, verification-resend and
  blob upload (Workers rate-limit binding or a D1 counter). Locked accounts back off rather than hard-lock.
  Login counts only failures per account, and per account and network (IPv6 grouped by /48); past the
  account's limit (30 an hour or 100 a day), login is refused from every network except a device
  that logged in successfully before (a revocable known-device token, which also skips the per-network
  limits; a stolen one is held to 20 guesses a day and never locks the owner out). A completed reset
  re-keys every counter of the account, ending the lockdown on every network, and makes that device
  known; it also mails a one-time device enrolment code that the native app redeems with the password
  to become a known device (a signed-in owner on a known device can request more codes without
  logging the other devices out). Verify, reset, refresh and logout charge the IP only for an unknown token,
  and a valid code is leased before hashing. Forgot and resend have no per-IP cap (flood defence is
  Cloudflare WAF and an optional global mail budget); they are limited per (IP, address) and never
  refused per account: a mail goes out at most every 2 minutes and at most 10 per link lifetime, and
  the newest 3 links stay valid, so nobody can block the owner's recovery.
- **No account enumeration:** sign-up, forgot-password and resend return the same response and
  timing whether or not the email exists. Sign-up for an address that already has an account
  sends that address a "someone tried to sign up with your email" mail instead of a verification mail.
- **Reset:** single-use token, 1 hour expiry, mailed to the verified address. A reset revokes all refresh tokens.
- **Account deletion** (the account stops working at once; content follows a 14-day grace):
  - Immediately: PII (email, name, school, password hash) is nulled, all sessions are revoked,
    and a tombstone row keeps ids stable. Ratings and favorites are deleted. Reports the user
    filed are kept, anonymized, as moderation evidence. The tombstone keeps a **salted hash of the
    email** (HMAC with a server secret) for 14 days and nothing else that identifies the person.
  - **Undo restores content only, never the old account.** Within the 14 days the person signs up
    again with the same address; the salted hash matches, and the admin re-links the held items and
    circles to the new account. After 14 days the hash is deleted. Because the old row's email is
    nulled, a fresh sign-up with that address does not collide with the unique-email constraint.
  - Circles the user owns: see Circles (claim window).
  - Items the user authored: at once they are set `removed_at` (hidden from every listing and
    unimportable). After the 14 days their R2 bodies (full and stripped, every version) and
    `item_blobs` rows are deleted, the blobs are garbage-collected if nothing else references
    them, and the quota is released.
  - Copies other teachers imported are untouched; they show "source removed".

## Trust levels

| Level | Who | Can |
|---|---|---|
| `unverified` | not yet active: email not confirmed, or the mailbox owner has not yet set the password and profile | nothing but verify (cannot log in) |
| `pending` | email confirmed, awaiting approval | see their own status (and a rejection reason); re-apply after a rejection; nothing else |
| `teacher` | approved by the admin | create and join circles, browse, publish, import, rate, favorite, report |
| `admin` | the site owner (only ever made by the bootstrap script, `server/worker/scripts/make-admin.sh`) | approval queue, suspension, reports, takedowns, limits |

`pending` accounts can also log out, export their account data and delete their account
(`/auth/logout`, `/account/export`, `DELETE /account`, `GET /me`); every other route needs `teacher`.
An `unverified` address can only have its verification mail resent (`/auth/resend` takes just the
email), and an account never confirmed is purged after 7 days.

**There is no automatic promotion.** Many districts issue students addresses on the same domain
as staff, so a domain match proves nothing. Every verified account lands in the admin queue and the
admin is emailed. The queue shows the email domain, any configured "staff domain" hint
(a label only, never a grant), and the declared school.

**Approval rule (assumption, confirm):** a name on a staff listing is never enough, because a
student can sign up with a personal address under a real teacher's name. The admin approves only
when (a) the verified email address itself appears on the school's staff directory, or (b) the
school's office confirms the person by a contact the admin initiates through the school's published
number or address (not one the applicant supplies). Anything else is rejected or left pending.
One admin is a bottleneck by design for v1; a second admin role can be added later.

**How decisions are made and kept (implemented in phase 1):**
- An approval records its evidence (`directory` or `school_confirmed`), an evidence note and a note.
- A rejection records a reason, which the applicant sees, and a note, which they never see. The
  applicant may re-apply with fresh details after 7 days, as a new application.
- An admin can revoke `teacher` (back to `pending`) or suspend any account: suspension blocks login
  and revokes every session and known device.
- Admin routes need a known device, and every admin change needs the password re-entered within 5
  minutes. An admin can never act on their own account, and the last active admin can never be
  suspended or deleted.
- **Two-person rule:** suspending an admin needs a second admin's confirmation (with only two admins,
  the proposer may confirm alone after 24 hours). Every action against an admin mails all admins and
  the escalation contact at once.
- Every decision goes to an append-only audit log that survives account deletion with its notes
  removed.
- If an application waits more than 48 hours, the daily cron mails the admins and a second contact,
  so a silent admin is noticed.

An invite code does **not** grant `teacher`; only `teacher` accounts can use a code (see Circles).
Only `teacher` reports count toward auto-hide.

Reason for the gate: a public pool of answer keys is only worth having if students cannot read it.

## Circles

- Any `teacher` creates a circle (name, optional description) and owns it.
- The owner mints **invite codes** with an expiry (default 7 days), a use limit (default 10) and
  optional note. Codes are random base32, 10+ characters, stored hashed, and can be revoked at any
  time. Revoking a code does not remove members who already joined.
- **Joining:** a valid code joins the circle directly, but only a `teacher` can redeem it.
  A `pending` account cannot, so a student who sees a code on a projector gets nothing.
  Redemption is atomic and does one transaction: `INSERT OR IGNORE` the membership, and only if a
  row was actually inserted run `UPDATE invite_codes SET used = used + 1 WHERE id = ? AND used <
  max_uses AND revoked_at IS NULL AND expires_at > now RETURNING ...`, with the 200-member cap
  checked in the same statement. So concurrent joins cannot exceed the use limit or the member cap,
  and someone who is already a member does not burn a use. Joining is rate limited per account.
- The owner can remove a member. A member can leave. A teacher can be in many circles.
  When a member leaves or is removed, their `item_circles` rows for that circle are deleted, and
  any of their items left with no circles become `unlisted`.
- Ownership transfer: the owner can hand ownership to an existing member only.
- **Owner deletes their account without transferring:** the circle enters a 30-day claim window in
  which members may claim ownership; items stay in the circle during that window. A claim is one
  atomic statement (`UPDATE circles SET owner_id = ? WHERE id = ? AND owner_id = <tombstone>`), so
  two simultaneous claims cannot both win. If nobody claims it, the circle is deleted, its
  `item_circles` rows are removed, and every item left with no circles is set to `unlisted`
  (visible only to its author, who is told). Deleting a circle by hand does the same.
- Publishing to a circle needs membership in that circle.

## Sharing model

An **item** is either a whole bank (a set, including its `BankDetails`) or a single question.

Each item has:
- `visibility`: `unlisted` | `circles` | `public`; for `circles`, a list of circle ids. `unlisted`
  means published but visible only to its author (a state items fall into when their circles go
  away). It is a server state. A local item that has never been published is *private* in the
  sense of the Goal and does not exist on the server at all.
- `hide_key`: **a property of each version**, stored on `item_versions`. If true, importers
  receive a **stripped form** with the key removed (see below). Chosen per version at publish time.
  Only the current version is ever importable, so an older version published without a stripped
  object can never be served by mistake.
- `license`: one of a fixed set of SPDX ids: `CC-BY-4.0` (default), `CC0-1.0`, `CC-BY-SA-4.0`,
  `CC-BY-NC-4.0`, `CC-BY-NC-SA-4.0`, or the non-SPDX value `circle-only` ("all rights reserved,
  copying only within the circles it is shared to"; not allowed on `public`).
- Discovery metadata: `subject`, `course`, `grade`, `tags[]`, `standards[]` (e.g. CCSS codes),
  plus `kind` and `difficulty` taken from the question itself.
- `version`: integer, incremented on each republish. Each version is its own immutable object.
- `schema_version`: the payload format version, so an older app can refuse or degrade for a
  payload it does not understand (e.g. an unknown figure registry id). Stripped payloads carry a
  higher `schema_version` than any build that predates the `keyless` flag, so an old client
  refuses them instead of silently dropping the flag and exporting an all-false key.
- Storage counted against the **author**: the distinct blobs their live items reference, plus a
  small per-item overhead.

Publishing needs `teacher` (public items and circle items alike). At publish time the author
confirms they own the content or have the right to share it; the confirmation is recorded.
Publishing to a circle needs membership of it.

**Licenses inherit, per question.** `origin` is recorded per question (and per bank), so the check
runs over every question in a bank, not the bank's own `origin`. The item's license must be allowed
by the origin license of **every** question it contains; a bank mixing incompatible origins
(e.g. BY-SA and BY-NC) is rejected with the offending questions listed.

Allowed target licenses for material that came from an origin license:

| Origin license | May be republished as |
|---|---|
| `CC0-1.0` | any license |
| `CC-BY-4.0` | `CC-BY-4.0`, `CC-BY-SA-4.0`, `CC-BY-NC-4.0`, `CC-BY-NC-SA-4.0` |
| `CC-BY-SA-4.0` | `CC-BY-SA-4.0` only |
| `CC-BY-NC-4.0` | `CC-BY-NC-4.0`, `CC-BY-NC-SA-4.0` |
| `CC-BY-NC-SA-4.0` | `CC-BY-NC-SA-4.0` only |
| `circle-only` | cannot be re-shared by importers at all (the author can still republish their own) |

"Stricter" never means a different condition set: BY-SA cannot become BY-NC-SA. The server
enforces the table, but it can only check what it can see. Rules:
- **Hard check, against the user's own imports.** The server knows which items this user imported
  (`imports`). A publish whose questions carry (or whose hashes match) an item that user imported is
  checked against the table and rejected if incompatible. Their own earlier publishes never match
  themselves.
- **Soft check, everything else.** A question whose content hash matches some other published item
  the user did not import is **flagged for the admin**, not rejected. Hash matches have false
  positives (trivial questions written independently) and invite griefing (pre-publishing common
  questions under a restrictive licence to block others), and "more restrictive" is not a total
  order. Matching uses the stripped-form hash for `hide_key` versions, never the full-form hash.
- A user who edits their local JSON to drop `origin` still passes the hard check only if they never
  imported the item; the ToS puts the remaining legal responsibility on the author.
- Changing an item's visibility to `public` re-checks that its license is not `circle-only`.

**Publish-time review.** Before upload the app shows every image, the full stem text and the
`topics` (which are merged into searchable tags and could contain an answer) of what is about to
leave the device, because real personal data would be in screenshots and pasted stems (a
`Question` has no student fields to strip).

Changing visibility later only affects the pool. Copies already imported stay.

### The stripped (keyless) form

`hide_key` cannot be a simple "remove the answer field", because the key is spread through the
`Kind` enum. For each kind the stripped form is:

| Kind | Stripped form |
|---|---|
| `Mc` | every `Answer.correct` set to `false`, **and the unlocked answers shuffled** (authors often put the right answer first; `locked` answers keep their position) |
| `Tf` | `correct` is a plain `bool`, so it is set to `false` and the question is marked keyless |
| `Matching` | `pairs[].right` set to `""`; **all** right-hand items, answers and `distractors` together, go into `distractors` in one random order (a word bank). Keeping a separate distractor list, or showing only wrong pairings, would narrow down the key; with 2-3 pairs it would give it away |
| `Blank` | `accept` becomes empty lists, one per blank, so the blank count survives |
| `Short` | `sample` becomes `""` |
| `Multipart` | the rules above applied to each `Part` (`PartKind::Mc` shuffled, `Tf`, `Short`) |
| any | `explanation` removed |
| any | `figure` and `image` removed unless the author ticks "this figure does not reveal the answer" for that question |

Shuffles are random but performed **once at publish time** and stored, so every importer, preview
and index row sees the same order and repeated reads cannot be averaged to recover the original.

`topics`, `difficulty`, `points` and the stem are kept. The stem is the author's text and may itself
contain the answer; the publish-time review (above) is where that is caught.

The stripped form is stored as **its own R2 object** next to the full version, and importers are
only ever served that object. The full payload never reaches them. Search-index rows, previews and
result snippets for a `hide_key` version are built from the stripped object, never the full one, so
no answer text (`Short.sample`, `Blank.accept`, `Matching` rights, MC answers, explanation) can
be matched or shown. **The author** gets the full version on every read path (an author
re-importing on a new device gets their own full copy); everyone else gets the stripped one.
Imported keyless questions get a local `keyless: true` flag (a new `#[serde(default)]` field on
`Question`) so the app shows "no answer key; add yours" and exports warn instead of printing a key
of all-false answers.

A keyless copy is deliberately less useful; it suits an author who wants to share stems without
handing out the key.

### Import

Importing creates a **private copy** in the importer's own local store. The copy keeps provenance:

```
origin: { item_id, qid, author_id, author_name, version, license, hash, form }
```

`qid` is the question's id inside the shared version (so a bank update can match questions up).
`hash` is the content hash of the form **actually served to this importer**, and `form` is
`full` or `stripped`. A non-author importer of a `hide_key` version only ever receives the
stripped-form hash. The full-form hash is never sent to anyone but the author: for small answer
spaces (a Tf has two, a 4-choice Mc about a hundred orderings) it would let anyone recover the
hidden key offline by hashing guesses. On a bank, `origin` describes the bank; each question
inside it carries its own `origin` as well.

- Provenance is shown in the UI ("from Ms. X, CC-BY") and carried through exports where the
  license requires attribution.
- If the author republishes, the importer sees "update available" and can re-import (as a new
  copy) or ignore it. Nothing changes in place, so exams that use the copy never shift silently.
- If the author deletes the original or an admin removes it, copies stay and show "source removed".
  The "update available" check (`POST /items/versions`) returns a distinct status per id
  (`current`, `newer`, `removed`, `unavailable`), so an item the caller can no longer see (for
  example an author who made it `unlisted`, or a circle the caller left) shows "no longer
  available", not "source removed".
- Images are downloaded through the Worker and written into the local `media/` store by hash, so
  imported content works offline. The server stores `blobs/<sha256>` with no extension; the client
  rebuilds `ImageRef.file` as `media/<sha256><ext>` using `store::sniff_ext`, and re-checks
  the hash and the dimensions on download.
  **WebP fix needed in `store.rs`:** `imagesize` has no WebP branch (so `put_image` fails with
  "unreadable image" on a WebP the server accepted) and `sniff_ext` calls any `RIFF` file WebP.
  Phase 2 adds a WebP branch to `imagesize` and checks `WEBP` at byte offset 8 in `sniff_ext`.
  Until then the server rejects WebP.
- Importing records one row per teacher per item version. The import-count signal is
  `COUNT(DISTINCT user_id)` over those rows.
- **IDs:** `model::new_id()` is a per-process counter plus a millisecond clock, so ids are not
  globally unique. The server assigns the item id. On import every bank and question gets a fresh
  local id from `new_id()`; the shared id only appears in `origin`.
- **Where a single question lands:** importing one question asks which bank to put it in
  (default: a bank named for the item's subject, created if missing).
- **Licence fields:** `BankDetails.license_name` is free text and the item's `license` is a fixed
  id. On import the item's license id is written to `origin` and, as display text, to
  `license_name`; on publish, `license_name` is ignored in favour of the item's `license`.

Local model changes, all `#[serde(default)]` so old stores still load:
- `origin` on `Question` and `QuestionBank` (above).
- `keyless: bool` on `Question`.
- `published: Option<{ item_id, version }>` on `QuestionBank` and `Question`, recording which server
  item a local bank or question was published as. Republishing and "update available" use it; without
  it the app could not tell a republish from a new publish.

## Discovery

- Free-text search over title, tags, stem, MC answer text and explanation (FTS5). Index rows
  are built from the **stripped object** whenever the version has `hide_key`, so no answer text
  of any kind is indexed for those items. **Visibility is applied in the same SQL statement as
  `MATCH`, before `LIMIT` and before any count**, so totals never reveal items the caller cannot
  see. D1 exports do not include FTS5 tables, so the index is rebuilt from `questions_idx` after
  a restore rather than backed up.
- Filters: subject/course, grade, topic tags, standards, question type, difficulty, license,
  "has images/figures", and scope (public, my circles, a given circle, mine). On a bank,
  `kind` and `difficulty` match if **any** question in it matches. `Question.topics` are merged into
  the item's `tags[]` at publish time (lower-cased, de-duplicated).
- Sorting: relevance, newest, most imported, highest rated.
- Results show the author, import count, rating, license and a preview with the key hidden or
  visible as the author set it.
- Standards codes are normalized (case, whitespace) at publish time.

## Quality signals

- **Import count:** `COUNT(DISTINCT user_id)` over `imports`; republished versions do not inflate it.
- **Favorites:** private bookmarks per teacher; never affect ranking.
- **Star ratings:** 1-5, one per `teacher` per item, changeable. Authors cannot rate their
  own items. The shown rating is only displayed once it has at least 3 ratings. Ratings are
  weakest against sock-puppet accounts; the admin approval of every account is the real defence.
- **Reports:** reason (wrong answer, copyrighted, student data, inappropriate, other) and a
  note. Reports go to the admin queue. An item with **3 or more** distinct `teacher` reporters is
  hidden from search pending review (the threshold is a setting). Reporting needs `teacher`.

## Limits

| Limit | Default |
|---|---|
| Image bytes per teacher | 200 MB, raisable by the admin |
| Single image | 2 MB; the client downsizes before upload |
| Shared questions per teacher | 5,000 |
| Circles owned per teacher | 20 |
| Members per circle | 200 |
| Items per bank publish | 1,000 questions |

Rough capacity at 200 MB: about 1,000-2,000 compressed question images, or 200-400 large PNG
screenshots. A figure question stores both its small JSON spec (1-5 KB) **and** the rendered PNG in
`Question.image`, so figure questions cost an image each, not nothing; the figure spec only lets
the importer re-open and edit the figure.
Images are deduplicated by hash. Each author is charged for the distinct blobs their **live**
items reference, so a second author uploading the same image pays for it too, and deleting an item
or account releases the charge. **Blobs uploaded but not yet referenced by a published version are
charged as "pending" against the uploader**, and `PUT /blobs` is rate limited, so repeated 2 MB
uploads that are garbage-collected after 24 hours cannot be used to get around the cap. Importing
never uses the importer's quota because imported copies live on their device.

## Data model (D1)

```
users(id, email, email_verified_at, pw_hash, pw_params, display_name, school, trust, created_at, deleted_at)
                                                                       -- trust: unverified|pending|teacher|admin
                                                                       -- deleted_at set = tombstone; PII columns nulled
sessions(id, family_id, user_id, refresh_hash, created_at, expires_at, rotated_at, revoked_at, user_agent)
email_tokens(id, user_id, kind, token_hash, expires_at, used_at)      -- verify | reset
staff_domain_hint(domain, note)                                        -- label shown in the queue; never grants trust
approvals(user_id, requested_at, decided_at, decided_by, decision, note)
circles(id, name, description, owner_id, created_at)
circle_members(circle_id, user_id, joined_at)
invite_codes(id, circle_id, code_hash, created_by, expires_at, max_uses, used, revoked_at, note)
items(id, author_id, kind, title, visibility, license, subject, course,
      grade, current_version, created_at, updated_at, removed_at, removed_reason)
                                                                       -- visibility: unlisted|circles|public
item_versions(item_id, version, schema_version, hide_key, full_key, stripped_key, created_at)
                                                                       -- immutable; stripped_key set iff hide_key
item_circles(item_id, circle_id)
item_tags(item_id, tag)  item_standards(item_id, code)
question_hashes(item_id, version, qid, form, hash, license)            -- form: full|stripped; the full hash
                                                                       -- is never returned to non-authors
questions_idx(item_id, version, qid, qkind, difficulty, title, tags, stem_text, answer_text,
              explanation_text)   -- FTS5 source; answer/explanation empty for hide_key versions
blobs(sha256, size, content_type, first_uploaded_at)
pending_blobs(sha256, uploader_id, until)          -- per uploader, so a second uploader of the same
                                                   -- hash is charged too; cleared when a published
                                                   -- version references it, or after 24 h
item_blobs(item_id, version, form, sha256)       -- form: full|stripped. A figure dropped from the
                                                  -- stripped form is still listed under `full`,
                                                  -- so GC and quota count every row; access checks
                                                  -- use `stripped` rows for non-authors and all rows
                                                  -- for the author
retained_versions: at most 10 per item (settable); older versions' R2 objects and item_blobs rows
                   are deleted and their charge released
imports(item_id, version, user_id, at)                                 -- unique (item_id, user_id, version)
ratings(item_id, user_id, stars, at)  favorites(item_id, user_id, at)
reports(id, item_id, reporter_id, reason, note, at, resolved_at, resolved_by, action)
audit(id, at, actor_id, action, target, detail)
```

Each version's body (the serialized bank or question JSON) is an **immutable** R2 object under a
**random key** (`items/<id>/<random>.json`), recorded in `item_versions.full_key`; the keyless form
is another random key in `stripped_key`, set when that version's `hide_key` is set. Random keys
matter: two devices republishing at once would otherwise write the same `v<n>` key, and the D1 row
that wins might point at the other device's body. Reading a body for an importer, preview or result always goes through
one function that picks the stripped object when `hide_key` is set and the caller is not the author.
D1 holds only what search and access control need. Images live in R2 at
`blobs/<sha256>`.

**Publish order (a bank can hold 1,000 questions, and D1 limits statements to 100 bound
parameters and about 100 KB, so one giant batch is not possible):**
1. Write the R2 objects (full, and stripped if `hide_key`).
2. Write the derived rows for the **new version number** in chunks: `questions_idx`,
   `question_hashes`, `item_tags`, `item_standards`, `item_blobs`. All carry `version`, so they
   are invisible until step 3.
3. In one small D1 batch, insert the `item_versions` row and set `items.current_version`.
4. Delete the previous version's derived rows (or at the retention limit, its objects).

Every query that reads derived rows joins on `version = items.current_version`, so a search never
matches an older version's index rows (an answer text from a v1 published with its key can't be
found after a `hide_key` v2). A failure leaves only unreferenced rows and R2 objects; a scheduled
Worker deletes those older than 1 hour (an age threshold, so an in-flight publish is never
collected). An import reads the version and the object key from the same row, so it can never
receive a body tagged with a different version.

**Blob rules:**
- `PUT /blobs/:sha256` recomputes the hash from the bytes, enforces the 2 MB limit server-side,
  sniffs the type itself, and accepts only PNG, JPEG and GIF for now (no SVG; WebP is added once
  `store.rs` can read it, see Import). Each upload is charged as pending to the uploader.
- Responses carry `X-Content-Type-Options: nosniff` and `Cache-Control: private, no-store`.
- Blobs are served only through the Worker with the bearer token, never by a public or presigned
  R2 URL, so the access check cannot be bypassed.
- A blob is readable if any non-removed item that references it is visible to the caller.
  Unreferenced blobs are garbage-collected after 24 hours.

## API (JSON over HTTPS, `Authorization: Bearer`)

```
POST /auth/signup  /auth/login  /auth/refresh  /auth/logout
POST /auth/verify  /auth/resend  /auth/forgot  /auth/reset  /auth/device/enrol  /auth/device/enrolment-code
DELETE /account                     GET /me

GET  /account/export

POST /circles                       GET /circles          DELETE /circles/:id   (owner)
GET  /circles/:id/members           (members)
POST /circles/:id/invites           GET /circles/:id/invites   (owner)
DELETE /invites/:id                 (owner of the invite's circle)
POST /circles/join                  (teacher; atomic redemption)
DELETE /circles/:id/members/:uid    (owner, or the member themselves)
POST /circles/:id/transfer          (owner; target must already be a member)

POST /items                         PUT /items/:id (author publishes a new version of their own item)
DELETE /items/:id                   (author)
GET  /items/:id                     GET /items?q=&subject=&grade=&tag=&std=&kind=&diff=&scope=&sort=
                                    (both return the stripped form to everyone but the author when
                                    the current version has hide_key)
GET  /items/mine                    POST /items/versions   (ids -> current versions, for "update available")
POST /items/:id/import              (records the import; returns the payload, keyless if hide_key)
PUT  /blobs/:sha256                 GET  /blobs/:sha256   (Worker-served only; rules under Data model)
PUT  /items/:id/rating  /items/:id/favorite   POST /items/:id/report

GET  /admin/queue  GET /admin/reports  POST /admin/approve  /admin/reject  /admin/takedown
GET  /admin/users/:id  GET /admin/audit  POST /admin/revoke-teacher  /admin/suspend  /admin/restore
POST /admin/reject-bulk  /admin/confirm  /admin/cancel  GET /admin/proposals
GET|PUT|DELETE /admin/staff-domains  PUT /admin/limits
POST /auth/reauth (sudo token for admin changes)  POST /account/reapply
```

Every route checks trust level and, for item reads, visibility against the caller's circles.
Ownership is checked per route as noted. Item ids and circle ids are random and unguessable,
and an item the caller cannot see returns 404, not 403, so existence is not revealed.

## App changes

- **Account screen** (Settings): sign in, sign out, status (verified, awaiting approval), storage used.
- **Sharing panel** on a bank and on a question: visibility, circles, hide-key toggle, license,
  metadata, and the ownership confirmation. Publish and republish.
- **Pool browser**: search box, filters, result list, item preview, Import, rate, favorite, report.
- **Circles screen**: list, create, invite codes (create, copy, revoke), members.
- **Provenance chip** on imported questions and banks, with the update and source-removed states.
- The `src/share/` client wraps all calls; the rest of the app never touches HTTP.
- Everything is hidden or disabled while signed out, and failures are non-blocking toasts.

## Safety and privacy

- No student data in any shared payload. The model has no student fields to strip, so the
  publish-time image and stem review (see Sharing model) is the control.
- Only the minimum personal data is kept: email, display name, school, trust level.
- **Legal before launch:** Terms of Service, a privacy policy, a teacher-only / adult statement at
  sign-up (COPPA, and California's SOPIPA), and a registered DMCA designated agent so the takedown
  process below qualifies for safe harbor.
- Takedown path for copyrighted material (publisher test banks): any teacher can report, the
  admin removes, the audit table records it. A contact address for outside rights holders is shown.
  A takedown sets `removed_at` **and purges** the R2 bodies (every version, full and stripped) and
  `item_blobs` rows, since a DMCA notice requires the content to be gone, not just hidden.
- Passwords and tokens are never logged. Mail tokens and invite codes are stored hashed.
- Account export (`GET /account/export`) and deletion on request.

## Build order

1. Worker skeleton (paid plan), D1 schema, sign-up/login/verify/reset, rate limiting, app sign-in
   screen, and the admin approval queue, since every account needs approval before it can do anything.
2. Publish, browse and import for public and unlisted items, with versioned storage, blob rules,
   the `share-core` crate (`strip`, hash, validation) and keyless form, the publish-time review,
   the license table, the `origin`/`published`/`keyless` model fields and the WebP fix in
   `store.rs`. **Reports, takedown and R2 purge also land here**, since public publishing is live
   from this phase and a DMCA path must exist from day one.
3. Circles, invite codes and circle visibility.
4. Search, filters, tags, standards and sorting.
5. Ratings, favorites, import counts, republish and update notices (reports and takedowns are
   already in phase 2).
6. Google sign-in and magic link.

Each phase ships on its own and leaves the app fully usable offline.

## Open questions

- Which transactional mail provider (Resend vs Cloudflare Email)?
- Which standards lists to offer as pick-lists (CCSS math/ELA, NGSS, CA-specific)?
- Does an exam, as opposed to a bank, need to be shareable? v1 shares banks and questions only.
- Should the admin approval be a separate small web page or a screen inside the app?
- **Confirm the approval rule.** The spec assumes "verified email appears on the school directory,
  or the school confirms through a contact the admin initiates". With one admin this is the main
  scaling limit; a second admin role may be needed early.
- **Confirm hosting.** The spec assumes the web build and the API share one registrable domain.
- Argon2id in wasm on Workers needs a spike to confirm CPU time per hash on the paid plan before
  phase 1 commits to it.
