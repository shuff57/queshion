# Shared pool: accounts, circles and public question sharing

Status: phase 1 (accounts, approval queue, Rust account client) is implemented; see
`docs/phase1-progress.md`. Phases 2-6 are design only. This revision (2026-10-19, "phase 2 spec
revision") rewrote the sharing, stripped form, import, images, limits, data model, publish order,
blob rules, takedown and build order sections after a critic review; items still waiting on the
owner are under **Open questions** at the end.

## Goal

Teachers sign in, publish banks or single questions, and others find and copy them. Sharing has
three levels: **private** (the default, never leaves the device), **circles** (groups a teacher
chooses to collaborate with, joined by invite code) and **public** (any approved teacher).

verSHion stays local-first. Banks, exams and export work exactly as today with no account. Sign-in
is only needed to publish, browse or import from the pool.

## Non-goals (v1)

- Co-editing one bank live. Sharing is publish and copy.
- Student accounts or any student data. Shared content is questions, answers, explanations and figures only.
- Google sign-in and magic-link login (phase 6).
- District-wide admins. Circles are owned by the teacher who made them.
- Recalling copies already imported. A copy lives on the importer's device; the server can only
  tell the importer that the source was removed and why (see Import).

## Stack

| Piece | Choice | Why |
|---|---|---|
| API | Cloudflare Worker (TypeScript) | Nothing to patch or back up; TLS and DDoS protection included |
| Data | D1 (SQLite), main database `vershion-pool` | Relational data, Time Travel point-in-time restore |
| Search | A **second** D1 database holding only the FTS5 index (phase 4) | `wrangler d1 export` refuses any database that contains a virtual table, so FTS5 is kept out of the main database; everything in the search database is rebuildable |
| Images and bodies | R2 | Egress is free; images content-addressed by SHA-256 like the local `media/` store |
| Mail | Transactional sender (Resend or Cloudflare Email) | Verification, reset and admin alerts |
| Client | `src/share/` module in the Rust app, `ureq` natively and `fetch` on wasm | Same API on both builds |

Cloudflare's D1 import/export page states "Export is not supported for virtual tables, including
databases with virtual tables" and suggests dropping and recreating them around an export.
**Decided (owner, 2026-10-19): the FTS5 index lives in a separate D1 database**, so the main
database can always be exported and backed up by Time Travel. Search results are always re-checked
against the main database (see Discovery).

The wire format is one JSON schema (`docs/shared-pool.schema.json`, written in phase 2) that both
the Worker and the Rust types are tested against, so the two codebases cannot drift.

The personal VPS is not used. If the Worker is outgrown, the API is plain HTTP+JSON and can move.

**Hosting (assumption, confirm):** the web build and the API are served from one registrable
domain, e.g. `app.example.org` and `api.example.org`, or the Worker routed at `/api/*` on the
app's own host. Cookie auth below depends on this; if the web build stays on a different site
(e.g. `github.io`), browsers will not send the cookie and the design must change.

**One share-core crate.** `server/share-core` (today only Argon2id) grows the pool logic in phase 2:
`validate`, `canonicalize`, `strip`, `match_hash`, `served_hash` and the image header checks. The
app links it natively and the Worker runs it compiled to wasm, with fixture tests in both targets.
**Only the server strips**: it validates the uploaded full payload, applies the shuffles with
`crypto.getRandomValues`, and stores the result. The client never supplies a stripped form (a
server cannot verify a client's shuffle). The keyless form, index rows, previews and every
non-author read path come from that one function, so they cannot disagree about what counts as
part of the key. The client runs the **same** `validate` on every payload it receives (see Import),
because a build with `--features pool-custom-server` can point at any server.

## Accounts and auth

Implemented in phase 1; recorded here as the contract later phases build on.

- **Sign-up:** email, password, display name, and a statement that the person is an adult
  school employee (COPPA/SOPIPA). A verification mail is sent; until the address is verified and
  the account is approved (see Trust levels) it cannot publish, browse the public pool or join circles.
  An unverified account cannot log in at all. Confirming the address discards the sign-up password,
  display name and school, and revokes every session (anyone can type someone else's address into
  sign-up), so the person who clicks the link enters the password and profile at that step, or later
  through forgot/reset. Only then does the account become `pending` and enter the approval queue.
- **Password storage:** Argon2id compiled to wasm, per-user salt, parameters stored with the hash
  and re-hashed on login when raised. Needs the **paid Workers plan**. Minimum 10 characters,
  checked against a short common-password list. PBKDF2 is not used: workerd caps it at 100,000
  iterations, well under current guidance.
  **Measured (see `docs/phase1-argon2-spike.md`):** a hash is a synchronous wasm call, so requests
  in an isolate run one at a time and concurrent logins do not multiply memory. The 19 MiB
  parameters use about 21 MB of wasm memory and take about 25-35 ms warm locally. The cost that
  matters is CPU time, so the per-IP and per-account rate limit is checked **before** any hashing.
  Chosen parameters: m=19456 KiB, t=2, p=1, 16-byte salt, 32-byte tag, stored with each hash.
  Re-measure on the edge after the first deploy.
  Unknown-email logins do a **full** dummy hash, and sign-up always hashes, whether or not the
  email exists, so timing is the same. Mail is sent in the background (`waitUntil`).
- **Login** (verified accounts only): returns a short-lived access token (15 min) and a rotating
  refresh token (30 days). Refresh tokens are stored hashed in D1 so they can be revoked. **Reuse
  detection with a grace window:** presenting an already-rotated refresh token within 10 seconds of
  its rotation returns the same successor token (two tabs, or a native retry after a dropped
  response); a replay after the window revokes the whole session family and raises an alert.
- **Token delivery differs by client:**
  - *Web:* the refresh token is set only as an `HttpOnly; Secure; SameSite=Strict` cookie on the
    API host and is **never** returned in a response body. `/auth/refresh` reads the cookie. The
    client fetches with `credentials: 'include'`, and the API answers with
    `Access-Control-Allow-Credentials: true` and an exact origin from an allow-list (never `*`).
    The access token is held in memory only. The web build loads no third-party scripts.
  - *Native:* the refresh token is returned in the login response body, stored in the OS keychain,
    and sent in the body of `/auth/refresh`. A client identifies itself with an explicit
    `X-Client: native` header; the cookie path is used when it is absent. The server **rejects**
    `X-Client: native` on any request that carries an `Origin` header.
  - *Rotation (both):* the successor token is `HMAC(server_secret, predecessor)`. Rotation is one
    atomic statement (`UPDATE sessions SET rotated_at = now WHERE id = ? AND rotated_at IS NULL
    RETURNING ...`), and the successor row is `INSERT OR IGNORE`d, so concurrent refreshes converge.
- **Rate limiting:** per IP and per account on login, sign-up, reset, verification-resend and the
  pool routes (D1 `rate_limits` counters keyed by HMAC). Login counts only failures per account,
  and per account and network (IPv6 grouped by /48); past the account's limit (30 an hour or 100 a
  day), login is refused from every network except a known device (a revocable device token; a
  stolen one is held to 20 guesses a day and never locks the owner out). A completed reset re-keys
  every counter of the account and mails a one-time device enrolment code. Verify, reset, refresh
  and logout charge the IP only for an unknown token, and a valid code is leased before hashing.
  Forgot and resend are limited per (IP, address) and never refused per account: a mail goes out
  at most every 2 minutes and at most 10 per link lifetime, and the newest 3 links stay valid.
- **No account enumeration:** sign-up, forgot-password and resend return the same response and
  timing whether or not the email exists. Sign-up for an address that already has an account
  sends that address a "someone tried to sign up with your email" mail instead.
- **Reset:** single-use token, 1 hour expiry, mailed to the verified address. A reset revokes all
  refresh tokens and known devices.
- **Display text:** display names and profile strings are NFKC-normalized and **refused** (not
  stripped) if they contain control, format (bidi overrides, isolates, zero-width), private-use or
  unassigned characters (`server/auth/src/validate.ts`). The pool applies the same rule to titles,
  tags, topics, standards codes and every report or notice field (see Payload validation).
- **Account deletion** (the account stops working at once; content follows a 14-day grace):
  - Immediately: PII (email, name, profile, password hash) is nulled, all sessions are revoked,
    and a tombstone row keeps ids stable. Ratings and favorites are deleted. Reports the user
    filed are kept, anonymized, as moderation evidence. The tombstone keeps an HMAC of the email
    for 14 days and nothing else that identifies the person.
  - **Undo restores content only, never the old account.** Within the 14 days the person signs up
    again with the same address; the HMAC matches, and the admin re-links the held items and
    circles to the new account. After 14 days the HMAC is deleted.
  - Circles the user owns: see Circles (claim window).
  - Items the user authored: at once `removed_at` is set with `removed_reason = 'account_deleted'`
    and `purge_after = now + 14 days` (hidden everywhere, unimportable). The daily cron then purges
    them (see Purge). Items under an open DMCA action follow the same date: a deleted account
    cannot send a counter-notice.
  - Copies other teachers imported are untouched; they show "source removed". The author's
    display name stays in those copies and in their attribution chains (disclosed at publish).
  - Implemented by `onDeleteContent` in `server/worker/src/cleanup.ts`, which today only logs a
    `PHASE 2 TODO`.

## Trust levels

| Level | Who | Can |
|---|---|---|
| `unverified` | not yet active: email not confirmed, or the mailbox owner has not yet set the password and profile | nothing but verify (cannot log in) |
| `pending` | email confirmed, awaiting approval (or approval revoked) | see their own status (and a rejection reason); re-apply after a rejection; nothing else |
| `teacher` | approved by the admin | create and join circles, browse, publish, import, rate, favorite, report |
| `admin` | the site owner (only ever made by the bootstrap script, `server/worker/scripts/make-admin.sh`) | approval queue, suspension, reports, takedowns, limits |

`pending` accounts can also log out, export their account data and delete their account
(`/auth/logout`, `/account/export`, `DELETE /account`, `GET /me`); every other route needs `teacher`.
An `unverified` address can only have its verification mail resent, and an account never confirmed
is purged after 7 days.

**There is no automatic promotion.** Many districts issue students addresses on the same domain
as staff, so a domain match proves nothing. Every verified account lands in the admin queue and the
admin is emailed. The queue shows the email domain, any configured "staff domain" hint (a label
only, never a grant), and the declared school.

**Approval rule (assumption, confirm):** a name on a staff listing is never enough. The admin
approves only when (a) the verified email address itself appears on the school's staff directory,
or (b) the school's office confirms the person by a contact the admin initiates through the
school's published number or address. Anything else is rejected or left pending.

**How decisions are made and kept (implemented in phase 1):**
- An approval records its evidence (`directory` or `school_confirmed`), an evidence note and a note.
- A rejection records a reason, which the applicant sees, and a note, which they never see. The
  applicant may re-apply after 7 days, as a new application.
- An admin can revoke `teacher` (back to `pending`) or suspend any account: suspension blocks login
  and revokes every session and known device.
- Admin routes need a known device, and every admin change needs the password re-entered within 5
  minutes. An admin can never act on their own account, and the last active admin can never be
  suspended or deleted.
- **Two-person rule:** suspending an admin needs a second admin's confirmation (with only two admins,
  the proposer may confirm alone after 24 hours). Every action against an admin mails all admins and
  the escalation contact at once.
- Every decision goes to the append-only `auth_audit` log that survives account deletion with its
  notes removed.
- If an application waits more than 48 hours, the hourly cron mails the admins and a second contact.

**Content follows the author's standing (phase 2).** An item is visible to anyone but its author
only while its author has `trust IN ('teacher','admin')`, `suspended_at IS NULL` and
`deleted_at IS NULL`. Every non-author read path joins `users` on the author and applies that
condition, so suspension, revocation to `pending`, restore and re-approval hide and show the
author's items at once and reversibly, with no per-item writes and no hook that can be forgotten.
`onSuspendContent` therefore returns no statements but stops logging a TODO; it records a
`pool_audit` row. A suspended or revoked author's items are not deleted.

An invite code does **not** grant `teacher`; only `teacher` accounts can use a code (see Circles).
Only `teacher` reports count.

Reason for the gate: a public pool of answer keys is only worth having if students cannot read it.

## Circles

Phase 3. Unchanged by this revision except for table keys (see Data model).

- Any `teacher` creates a circle (name, optional description) and owns it.
- The owner mints **invite codes** with an expiry (default 7 days), a use limit (default 10) and
  optional note. Codes are random base32, 10+ characters, stored hashed, and can be revoked at any
  time. Revoking a code does not remove members who already joined.
- **Joining:** a valid code joins the circle directly, but only a `teacher` can redeem it.
  Redemption is one transaction: `INSERT OR IGNORE` the membership, and only if a row was inserted
  run `UPDATE invite_codes SET used = used + 1 WHERE id = ? AND used < max_uses AND revoked_at IS
  NULL AND expires_at > now RETURNING ...`, with the 200-member cap checked in the same statement.
  Joining is rate limited per account.
- The owner can remove a member. A member can leave. A teacher can be in many circles.
  When a member leaves or is removed, their `item_circles` rows for that circle are deleted, and
  any of their items left with no circles become `unlisted`.
- Ownership transfer: the owner can hand ownership to an existing member only.
- **Owner deletes their account without transferring:** the circle enters a 30-day claim window in
  which members may claim ownership. A claim is one atomic statement (`UPDATE circles SET owner_id
  = ? WHERE id = ? AND owner_id = <tombstone>`). If nobody claims it, the circle is deleted, its
  `item_circles` rows are removed, and every item left with no circles is set to `unlisted`.
- Publishing to a circle needs membership in that circle.

## Sharing model

An **item** is either a whole bank (a set, including its `BankDetails`) or a single question.
An item has a stable server id and a sequence of **publishes**; the item points at its current
publish. Each publish is immutable once committed.

Per **item** (can change without a new publish, via `PATCH /items/:id`):
- `visibility`: `unlisted` | `public` in phase 2, plus `circles` (with a list of circle ids) in
  phase 3. `unlisted` means published but visible only to its author. A local item that has never
  been published does not exist on the server at all. Making an item `public` is refused when the
  current publish's license is `circle-only` (checked in the same `UPDATE ... WHERE`).
- Moderation state: `removed_at`/`removed_reason` (permanent, see Purge) and `item_holds` rows
  (reversible hides, see Reports).

Per **publish** (fixed for that version forever; changing any of these needs a new publish):
- `hide_key`: if true, non-authors receive the **stripped form** (see below).
- `license`: one of `CC-BY-4.0`, `CC0-1.0`, `CC-BY-SA-4.0`, `CC-BY-NC-4.0`, `CC-BY-NC-SA-4.0`, or
  the non-SPDX value `circle-only` ("all rights reserved, copying only within the circles it is
  shared to"; never `public`). A CC grant made with a version is irrevocable for that version, so
  the license lives on the version: copies taken from v2 keep v2's license even if v3 changes it.
- **Rights attestation**: `attested_at` and `attest_text_rev` (which revision of the attestation
  wording the author saw and ticked). Required on every publish.
- Discovery metadata: `title`, `subject`, `course`, `grade`, `tags[]`, `standards[]`, plus per
  question `kind` and `difficulty`.
- `version`: integer, one more than the publish it replaced. `schema_version`: the payload format.
- The publish id (`publish_id`), generated by the client, which is also the idempotency key.

**Explicit license choice (decided).** The sharing panel lists the licenses with `CC-BY-4.0` first and
labelled "suggested", but **nothing is preselected**; Publish stays disabled until the author picks
one. A republish preselects the license of the item's current publish (the author already chose it),
and changing it shows what changes for future copies only.

**Global question identity.** Every question has a server-assigned `qid` that is stable across
versions of its item (the role Moodle's question bank entry plays for its versions). On first
publish the server assigns a fresh random `qid` to each question. On republish the client sends the
`qid` it recorded for each question (`published.qid`); the server accepts a sent `qid` only if it
already belongs to this item (`question_ids.item_id`) and appears once in the payload, and assigns a
fresh one otherwise. Local ids, `created` and `modified` never leave the device: the server replaces
`Question.id` and `QuestionBank.id` with server ids and sets `created`/`modified` to the publish's
commit time (UTC day), so authoring times are not disclosed.

Publishing needs `teacher` (public items and circle items alike). At publish time the author ticks
the rights attestation; it is recorded on the publish.

**Licenses inherit, per question.** The check runs over every question in a bank. The publish's
license must be allowed by the origin license of **every** question it contains; a bank mixing
incompatible origins (e.g. BY-SA and BY-NC) is rejected with the offending question positions listed.

| Origin license | May be republished as |
|---|---|
| `CC0-1.0` | any license |
| `CC-BY-4.0` | `CC-BY-4.0`, `CC-BY-SA-4.0`, `CC-BY-NC-4.0`, `CC-BY-NC-SA-4.0` |
| `CC-BY-SA-4.0` | `CC-BY-SA-4.0` only |
| `CC-BY-NC-4.0` | `CC-BY-NC-4.0`, `CC-BY-NC-SA-4.0` |
| `CC-BY-NC-SA-4.0` | `CC-BY-NC-SA-4.0` only |
| `circle-only` | cannot be re-shared by importers at all (the author can still republish their own) |

"Stricter" never means a different condition set: BY-SA cannot become BY-NC-SA.

**`match_hash`, the only hash compared across items.** `match_hash(q) = SHA-256(canonical(
strip_without_shuffle(q)))`, computed in share-core, where `strip_without_shuffle` applies the
stripped-form table below but, instead of shuffling, **sorts** every unordered list (Mc answer texts
with their letter labels removed, Matching left items, the merged right-hand word bank), and
`canonical` NFC-normalizes text, collapses whitespace and serializes with sorted keys. It covers the
stem, kind, and the key-free structure; it excludes images, figures, topics, points, difficulty and
ids. Two questions that differ only in their key (a Tf marked true vs false, the same Mc with a
different correct choice) have the **same** `match_hash`, so no check built on it can leak a key.
It is also identical for a full and a stripped publish of the same question, so the checks below
still work after a re-strip with a different random shuffle.

The full-form hash (`full_sha256`) exists only for integrity of the author's own body. It is never
compared across items and never sent to anyone but the author: for small answer spaces (a Tf has
two keys, a 4-choice Mc 4 single-answer or 15 multiple-answer keys) it would let anyone recover a
hidden key by hashing guesses.

Rules:
- **Hard check, against the user's own imports.** A question is "derived from an import" if its
  `match_hash` equals that of a question in any publish this user imported (`imports` joined to
  `item_questions`), or its declared `origin` names an item this user imported. Its origin license
  is that imported publish's license; the table applies and an incompatible publish is rejected,
  listing positions in the publisher's own payload. Because the hash ignores the key, every key
  variant of a question gets the same answer. The user's own items never match themselves.
- **Soft check, everything else.** A `match_hash` equal to a question in another visible publish the
  user did not import creates an `admin_flags` row and nothing else. The publish result is identical
  whether or not a flag was raised, and flags are never shown to the publisher. (Hash matches have
  false positives and invite griefing by pre-publishing common questions under a restrictive license.)
- **Re-upload blocklist.** A `match_hash` (or image hash) on `blocked_hashes` (see Takedown) does not
  fail the publish; the item gets an `item_holds` row `blocklist` and the author sees "held for
  review" in `/items/mine`.
- A user who edits their local JSON to drop `origin` still passes the hard check only if they never
  imported a matching item; the ToS puts the remaining legal responsibility on the author.

**Publish-time review.** Before upload the app shows everything that is about to leave the device:
every image (after re-encoding, see Images), the full stem text, the `topics` (merged into searchable
tags, and they could contain an answer), the bank's `BankDetails`, and, for a `hide_key` publish, a
preview of the stripped form exactly as importers will see it. It requires, per question where it
applies, the "image does not reveal the answer" tick and the "answer order preserved" acknowledgement
(see the stripped form), plus the license choice and the rights attestation. It states: "Your display
name is copied into every imported copy and its attribution, and stays there even if you later delete
your account. Your email and school are never shared."

Changing visibility later only affects the pool. Copies already imported stay.

### The stripped (keyless) form

`hide_key` cannot be a simple "remove the answer field", because the key is spread through the
`Kind` enum and through figure parameters. For each kind the stripped form is:

| Kind | Stripped form |
|---|---|
| `Mc` | every `Answer.correct` set to `false`; letter labels removed (below); the unlocked answers shuffled (`locked` answers keep their position) |
| `Tf` | `correct` set to `false` and the question marked keyless |
| `Matching` | `pairs[].right` set to `""`; **all** right-hand items, answers and `distractors` together, go into `distractors` in one random order (a word bank) |
| `Blank` | `accept` becomes empty lists, one per blank, so the blank count survives |
| `Short` | `sample` becomes `""` |
| `Multipart` | the rules above applied to each `Part` (`PartKind::Mc` relabelled and shuffled, `Tf`, `Short`) |
| any | `explanation` removed |
| any | `figure` **always** removed |
| any | `image` removed unless the author ticks "this image does not reveal the answer" for that question |

**Why `figure` always goes.** `FigureSpec.params` routinely hold the answer: the micropipette
figure's `volume` is the reading and `answer`/`answer_text` print it; the Lewis figure's `tweak`
encodes the bonds and lone pairs asked for; the pedigree's `genotypes` has an `Answers` mode; the
Punnett square has ratio and genotype switches. There are about 16 figures with an `answer` or
similar param, and new ones will be added, so no per-figure rule is safe. The tick keeps only the
rendered `image`, which the author has just looked at in the review.

**Letter labels.** Answer texts are normally stored unlettered (the importers in `formats.rs` strip
"A. " and the exporters add letters), but a hand-typed "A. Venus" would keep its original slot
letter after a shuffle and give the order away. Before shuffling, when at least two choices of a
question start with one ASCII letter, `.` or `)`, and whitespace (the same strict pattern as
`formats::letter_choice`), that prefix is removed from every choice that has it.

**Order preserved.** If fewer than two choices of an Mc (or Mc part) are unlocked, nothing moves and
the author's order survives. The review lists such questions with "the answer order is preserved and
may reveal the key" and requires a per-question acknowledgement; the server rejects a `hide_key`
publish (422, listing positions) whose payload lacks the acknowledgement for each such question.

Shuffles are random but performed **once at publish time** and stored, so every importer, preview
and index row sees the same order and repeated reads cannot be averaged to recover the original.

`topics`, `difficulty`, `points` and the stem are kept. The stem is the author's text and may itself
contain the answer; the publish-time review is where that is caught.

The stripped form is stored as **its own R2 object** next to the full one, and non-authors of a
`hide_key` publish are only ever served that object. **The author** gets the full version on every
read path. For a publish without `hide_key` everyone gets the full form. Index rows, previews and
result snippets are built from the form the reader is served (for search, always the stripped form
of a `hide_key` publish).

Imported keyless questions get a local `keyless: true` flag (a new `#[serde(default)]` field on
`Question`), so the app shows "no answer key; add yours" and exports warn instead of printing a key
of all-false answers. **Older builds drop unknown fields when they re-save `banks.json`** (serde's
default), which would silently lose `keyless`. Mitigation: an imported keyless question also gets
`explanation` set to a fixed sentinel ("No answer key: this question was imported without its key.
Add the correct answers before printing a key."), which every older build keeps and prints on the
answer key. The current build hides the sentinel while `keyless` is set and removes it when the
teacher adds a key.

A keyless copy is deliberately less useful; it suits an author who wants to share stems without
handing out the key.

### Import

Importing creates a **private copy** in the importer's own local store. Every question in the copy,
and the bank itself, carries provenance:

```
origin: {
  item_id, qid, publish_id, version,     // what was copied
  author_name, license,                   // of that publish
  form,                                   // full | stripped
  served_sha256,                          // SHA-256 of the canonical served question
  match_hash,
  chain: [ { author_name, item_id, qid, version, license, modified } ]  // earlier links, oldest first
}
```

- **Attribution chain.** When B publishes a question imported from A and C imports it from B, C's
  copy must still credit A, and CC 4.0 requires saying whether the material was modified. The server,
  not the client, builds the chain: on publish, for each question derived from an import (hard-check
  rule), it stores `chain = <that imported question's stored chain> + [{A's link}]` in
  `item_questions.chain_json`, with `modified = true` when the new question's canonical form differs
  from the canonical form that was served to this importer (so an added key, a changed explanation or
  a new image all count, not only a changed `match_hash`). A client-declared `origin` that matches no
  import is dropped and raises an `admin_flags` row. Chains are capped at 32 links (older links
  collapse to "and earlier contributors"). The served payload's `origin.chain` is copied from that row.
- `served_sha256` lets the client check the body it received. A non-author importer of a `hide_key`
  publish never receives `full_sha256`.
- Provenance is shown in the UI ("from Ms. X, CC-BY, modified from Mr. Y") and carried through
  exports where the license requires attribution. `BankDetails.author` and `license_name` are also
  filled from the bank's origin, because older builds keep those fields and drop `origin`.
- **Update notices (phase 2).** `POST /items/versions` takes up to 200 item ids and returns a status
  per id: `current`, `newer` (with the new version), `removed` (with `reason`: `deleted`, `takedown`
  or `privacy`), or `unavailable` (the caller can no longer see it: `unlisted`, held, a circle left,
  author suspended). For `newer` the importer can re-import (as a new copy) or ignore it; nothing
  changes in place, so exams that use the copy never shift. For `removed`/`privacy` (student data,
  see Reports) the app asks the teacher to delete their copy and offers to do it.
- **Atomic import.** The client (1) fetches the payload and every referenced image into memory (web)
  or a staging directory (native); (2) verifies all of it: `served_sha256`, share-core `validate`
  (the same limits the server enforces), each image's hash, sniffed type, header dimensions and a
  decode under explicit limits (`image::Limits`, 4096x4096, 64 MB allocation); (3) normalizes figures
  (below); (4) only then writes images with `put_image` (which writes before it measures, so it must
  never see unverified bytes) and inserts the questions with **one** `save_banks`. Any failure leaves
  the store unchanged.
- **Figures on received payloads.** A `FigureSpec` whose `id` is not in `figures::registry()` is
  dropped (the image stays). Otherwise params whose key is not in the figure's `ParamDef` list are
  dropped, `Params::normalize` clamps numbers and choices into range, and text params over 4 KB are
  refused (normalize does not cap lengths).
- **Duplicates.** Before importing, the client looks for local questions whose `origin.item_id`/`qid`
  match and warns "already imported (v3, into Bank X)"; the server also returns the versions this
  user imported before. Re-importing makes a new copy only if the teacher confirms.
- Images are downloaded through the Worker and written into the local `media/` store by hash, so
  imported content works offline. The client rebuilds `ImageRef.file` as `media/<sha256><ext>` using
  `store::sniff_ext` (after the WebP fix in Build order) and takes width and height from the image
  header, not from the payload.
- Importing records one row per teacher per publish. The import-count signal is
  `COUNT(DISTINCT user_id)` over those rows.
- **IDs:** `model::new_id()` is a per-process counter plus a millisecond clock, so ids are not
  globally unique. On import every bank and question gets a fresh local id; the server ids appear
  only in `origin` and `published`.
- **Where a single question lands:** importing one question asks which bank to put it in
  (default: a bank named for the item's subject, created if missing).
- **Schema versions.** v1 is today's model; v2 adds `keyless`, `origin` and `published`. Stripped
  payloads are always served as v2. A client refuses a payload whose `schema_version` is above the
  highest it knows ("update verSHion to import this"), so an old client never silently drops
  `keyless` from a fresh import. The server accepts publishes in its supported range and drops
  unknown fields during canonicalization.

Local model changes, all `#[serde(default)]` so old stores still load:
- `origin` on `Question` and `QuestionBank` (above).
- `keyless: bool` on `Question`.
- `published: Option<{ item_id, qid, publish_id, version }>` on `Question` (and `{ item_id,
  publish_id, version }` on `QuestionBank`). Republishing, `qid` reuse and update notices use it.
  An older build that re-saves drops it, and the next publish becomes a new item; the server
  answers a publish whose `match_hash` set equals one of the author's own items with
  `possible_duplicate_of`, and the app offers to republish that item instead.

## Images

Every image that leaves the device goes through one pipeline (ASVS 5.0 V5: 5.1.1, 5.2.1, 5.2.2,
5.2.4, 5.2.6, 5.4.1).

**Decided (owner, 2026-10-19): the pool accepts PNG, JPEG, GIF and WebP**, each as a single still
image of at most 4096x4096 and 2 MB. Anything else (SVG, TIFF, BMP, animated images, metadata) is
refused by the server even if a client sends it.

**Client (before upload), canonical re-encode.** The app already depends on `image` 0.25 with the
`png`, `jpeg`, `webp` and `gif` features, and that crate (its PNG, JPEG, GIF and WebP encoders) is
the only encoder used:
- Always decodes and **re-encodes**, never uploads the stored bytes. Decoding uses explicit limits
  (`image::Limits`: max 4096x4096 source, 64 MB allocation); EXIF orientation is applied first.
- The output keeps the source's format: PNG as PNG; JPEG as baseline JPEG, quality 85; GIF as a
  single-frame GIF (no loop or comment extensions; a graphic control extension only for
  transparency); WebP as lossless WebP (`VP8L`; `image` 0.25's WebP encoder is lossless-only, to
  verify at build time). Figure renders are PNG. Each is downscaled until it is within 4096 px per
  side and 2 MB. Re-encoding drops EXIF (GPS, camera, time), XMP, IPTC, ICC profiles and comments.
- **Animated GIF or WebP: only the first frame is shared, and the author is told.** The review shows
  the re-encoded still image with "This image is animated; only its first frame will be shared."
  Publishing is not blocked, because the review shows exactly what will be shared.
- The outgoing payload's `ImageRef` names the re-encoded hash; the local store is not changed.

**Server (`PUT /blobs/:sha256`, share-core image checks).** The server never trusts the client's
re-encode; it validates independently and refuses (422) anything outside these rules:
- Recomputes the SHA-256 and refuses a mismatch; enforces 2 MB; sniffs the format from the bytes
  (never from a name or header supplied by the client).
- **Every format:** width and height from the format's own header, each 1..4096 (pixel flood); no
  byte after the format's end marker; every length field consistent with the file.
- **PNG:** walks the chunks and allows only `IHDR`, `PLTE`, `tRNS`, `gAMA`, `cHRM`, `sRGB`,
  `pHYs`, `IDAT`, `IEND`; anything else (including `tEXt`, `zTXt`, `iTXt`, `eXIf`, `iCCP`, `tIME`,
  and `acTL`/`fcTL` for animated PNG) is refused. Nothing after `IEND`.
- **JPEG:** walks the segments and allows only `SOI`, `APP0` (JFIF), `APP14` (Adobe), `DQT`, `DHT`,
  `DRI`, `SOF0`-`SOF2`, `SOS`, `RSTn`, `EOI`; `APP1` (EXIF/XMP), `APP2` (ICC), `APP13` (IPTC), `COM`
  and every other `APPn` are refused. Nothing after `EOI`.
- **GIF:** `GIF87a` or `GIF89a`; logical screen size within the cap; walks every block and
  sub-block: **exactly one** image descriptor, whose frame lies inside the logical screen; the only
  extension allowed is one graphic control extension before it; comment (`0xFE`), application
  (`0xFF`, e.g. `NETSCAPE2.0` looping or XMP) and plain-text (`0x01`) extensions are refused; the
  trailer `0x3B` must be the last byte.
- **WebP (static only):** `RIFF` with `WEBP` at byte 8 and a RIFF size equal to the file length
  minus 8; walks the chunks, checking each size against the remaining bytes and the odd-size pad
  byte. Allowed layouts: one `VP8 ` chunk; one `VP8L` chunk; or `VP8X` followed by an optional
  `ALPH` and then one `VP8 `/`VP8L`, with the `VP8X` animation, EXIF, XMP and ICC flags all clear.
  `ANIM`, `ANMF`, `EXIF`, `XMP `, `ICCP` and any unknown chunk are refused (the client's re-encode
  never produces them). Dimensions come from the `VP8 ` frame header (after the `9d 01 2a` start
  code), the `VP8L` header (after the `0x2f` signature) or the `VP8X` canvas, and a `VP8X` canvas
  must equal the bitstream's size. Nothing after the RIFF end.
- `blobs.content_type` is set from the verified format (`image/png`, `image/jpeg`, `image/gif`,
  `image/webp`) and is the only type the blob is ever served with.
- On publish, overwrites every `ImageRef.width`/`height` in the canonical payload from the stored
  blob header, so a payload cannot claim different dimensions.
- share-core carries fixture tests: a valid and an invalid file for every rule above, and an
  `image`-crate round trip per format that must pass the server checks.
- Antivirus scanning (ASVS 5.4.3) is not done; accepted risk, documented here: only re-encoded
  raster images under a strict structural allow-list are stored, and they are served as attachments
  under a sandbox CSP (see Blob rules).

## Discovery

**Phase 2 (browse, no search).** `GET /items?subject=&kind=&scope=public|mine&cursor=&limit=` lists
the newest committed items first (`ORDER BY committed_at DESC, id DESC`), filtered by subject and
by question kind (a bank matches if any question has that kind). Cursor pagination: the cursor is an
opaque signed `(committed_at, id)`; `limit` defaults to 20 and is capped at 50. No totals are
returned. Visibility, holds, removal and the author-standing condition are in the same SQL statement
as the filter and the `LIMIT`. Rows show title, kind, subject, author display name, license, number
of questions, version, whether the key is hidden, and whether images are included.

**Phase 4 (search and filters).** Free-text search over title, tags, stem, MC answer text and
explanation (FTS5) in the separate search database. Index rows are built from the stripped object
whenever the publish has `hide_key`, so no answer text of any kind is indexed for those items.
Because D1 cannot join across databases, the search database is only a candidate source: each page
over-fetches candidate `(item_id, publish_id)` pairs and the main database keeps only those where
`publish_id = items.current_publish_id` and the caller may see the item, in one statement, before
the page is cut. No totals are shown, so counts never reveal hidden items. The search database is
rebuilt from the main database and R2 after a restore, and its rows for an item are deleted on every
takedown, removal and hold (and rebuilt when a hold is lifted).
- Filters: subject/course, grade, topic tags, standards, question type, difficulty, license,
  "has images", and scope (public, my circles, a given circle, mine). `Question.topics` are merged
  into the item's `tags[]` at publish time (lower-cased, de-duplicated).
- Sorting: relevance, newest, most imported, highest rated.
- Standards codes are normalized (case, whitespace) at publish time.

## Quality signals

Phase 5 except reports, which land in phase 2.

- **Import count:** `COUNT(DISTINCT user_id)` over `imports`; republished versions do not inflate it.
- **Favorites:** private bookmarks per teacher; never affect ranking.
- **Star ratings:** 1-5, one per `teacher` per item, changeable. Authors cannot rate their own items.
  The rating is only displayed once it has at least 3 ratings.
- **Reports (phase 2):** reason (`wrong_answer`, `copyright`, `student_data`, `inappropriate`,
  `other`) and a note; one open report per reporter per item and reason; reporting needs `teacher`
  and is capped per day (Limits).
  - `student_data`: **one** report hides the item at once (`item_holds` reason `student_data`) and
    mails the admin immediately, outside the notice budget. If the admin confirms, the item is purged
    immediately (every retained publish, bodies, derived rows, and blobs nothing else references; no
    quarantine), the image hashes go on `blocked_hashes`, and update checks return
    `removed`/`privacy` so importers are asked to delete their copies. If the admin rejects it, the
    hold is lifted; the reporter's history is shown on every report so abuse is visible.
  - `copyright` from a teacher is a report, not a DMCA notice; the admin may take the item down
    (see Takedown) or dismiss it.
  - Other reasons: 3 or more distinct `teacher` reporters hide the item pending review (`item_holds`
    reason `reports`; decided default 3, a setting).
  - The reporter is told only "thanks, the admin will review it".
- **Admin report review** (`GET /admin/reports`): open reports grouped by item, oldest first, with
  the item's current publish shown in **full** form to the admin, the reporters (display names) and
  their report counts, and the actions dismiss, hold, lift hold, take down, purge (student data), and
  open the author. Every action writes `pool_audit`.

## Takedown, DMCA and moderation

This section is written as engineering requirements derived from 17 U.S.C. 512. **It is not legal
advice; have counsel review the ToS, the repeat-infringer policy and this flow before public launch.**

| Requirement | Source | Implementation |
|---|---|---|
| Remove **or disable access** expeditiously on a valid notice | 512(c)(1)(C) | Takedown sets `removed_at`, `removed_reason='takedown'`, `quarantined_at`; bodies are copied to `quarantine/<item>/<publish>.json` and deleted from their normal keys; the keys in `item_publishes` are updated. Only admin routes read `quarantine/`. Search rows are deleted; titles and tags of the item stop appearing anywhere but the admin console. |
| Designated agent | 512(c)(2) | Registered with the US Copyright Office before public launch (an owner to-do, not a build blocker); name and address published on a "Copyright" page linked from the app and the ToS. |
| Notice elements | 512(c)(3)(A)(i)-(vi) | Notices arrive at the agent's address. The admin records each in `dmca_notices` with one field or tick per element: signature; the copyrighted work; the material (item ids or links); complainant contact; good-faith statement; accuracy and authority statement under penalty of perjury. A notice missing elements is stored as `deficient` and the admin contacts the sender (512(c)(3)(B)). |
| Notify the author | 512(g)(2)(A) | On takedown the author is mailed and sees the item as "taken down after a copyright notice" with a counter-notice form. |
| Counter-notice | 512(g)(3)(A)-(D) | The form collects: signature; identification of the material and where it was; a statement under penalty of perjury of good-faith belief it was removed by mistake or misidentification; name, address, phone, consent to federal district court jurisdiction, and acceptance of service from the complainant. Stored in `dmca_counter_notices`; incomplete ones are refused. |
| Forward and put back | 512(g)(2)(B)-(C) | A complete counter-notice is forwarded to the complainant at once with notice that the material will be restored in 10 business days. The cron restores it (bodies back from quarantine, derived rows rebuilt from the body, hold cleared) no sooner than 10 and no later than 14 business days after receipt (US federal holidays excluded), unless the admin records the complainant's notice of a court action, which keeps it disabled until resolved. |
| No counter-notice | engineering choice (decided) | Quarantine is kept for 30 days after the author was notified, then purged. |
| Repeat infringers | 512(i)(1)(A) | A published policy: each takedown that is not reversed by a put-back is an upheld strike (`strikes`); 3 upheld strikes within a rolling 12 months (decided) suspend the account, which hides all its items. The policy is in the ToS and shown to authors at their first strike. |
| Re-upload | engineering choice | Every question `match_hash` and image hash of an upheld takedown goes on `blocked_hashes`; a later publish matching one is held for review (Sharing model). A put-back lifts the entries. |

Other moderation:
- **Suspension and revocation** hide the author's items reversibly through the author-standing join
  (Trust levels); they never delete content.
- **Admin removal** for other policy reasons (`removed_reason='admin'`) follows Purge after 14 days.
- **Purge** (cron, daily) for items with `purge_after < now`: delete R2 bodies of every publish
  (including `quarantine/`), `item_publishes`, `item_questions`, `item_tags`, `item_standards`,
  `item_blobs` and search rows; keep the `items` tombstone (id, author, `removed_reason`), `imports`
  (for counts), `reports`, DMCA records and `pool_audit`. Blobs left with no reference are collected
  by the normal blob GC and the author's quota is released.
- **Author deletes an item** (`DELETE /items/:id`, decided 2026-10-19): `removed_reason='author_deleted'`,
  `purge_after = now + 14 days`. Within those 14 days the author can undo it
  (`POST /items/:id/undelete`, only while `removed_reason='author_deleted'`), which clears
  `removed_at` and `purge_after`; afterwards it is purged. Items removed for privacy (student data)
  or by a DMCA takedown are not author-deleted and follow their own rules above; deleting such an
  item does not start or shorten a 14-day window and cannot be undone by the author.

## Limits

| Limit | Default |
|---|---|
| Publish request body | 4 MB of JSON; `Content-Length` over it is refused and reading stops at 4 MB |
| Questions per bank publish | 1,000 |
| Stem, explanation | 8 KB UTF-8 each |
| Answer text, pair side, distractor, `Short.sample`, one `Blank` accept entry | 2 KB each |
| Mc answers | 26 (`A`-`Z`) |
| Matching pairs / distractors | 50 / 26 |
| Blanks per question, accept variants per blank | 50, 20 |
| Multipart parts | 50 |
| Topics per question, tags per item, standards per item | 20, 30, 30; each 64 chars |
| Title / `BankDetails` description / author | 200 chars / 2 KB / 200 chars |
| `figure.params` | 16 KB of JSON; text params 4 KB each |
| Single image | 2 MB, 4096x4096, still PNG, JPEG, GIF or WebP |
| Storage per teacher | 200 MB (decided), raisable by the admin |
| Shared questions per teacher | 5,000 across current publishes |
| Retained publishes per item | the current one plus the one before it |
| Circles owned per teacher / members per circle | 20 / 200 |
| Browse or search page | 50 rows |

**Storage charged to the author:** the distinct blobs referenced by their retained publishes (any
form), plus the full and stripped body bytes of those publishes, plus their pending blobs, plus 1 KB
per item. A second author referencing the same image pays for it too. Removal and purge release the
charge. Importing never uses the importer's quota because copies live on their device. A figure
question costs its rendered PNG; the figure spec is small.

**Daily caps per account (anti-automation, ASVS V2.4), decided 2026-10-19, each raisable by the admin:**

| Action | Per day |
|---|---|
| Publishes (new and republish) | 50 |
| Imports | 200 |
| Item reads (`GET /items/:id`, previews, import bodies) | 2,000 |
| Browse/search pages | 500 |
| Blob uploads | 500 and 200 MB |
| Blob reads that miss (404) | 100, then 429 for the rest of the day |
| Item ids that miss (404) | 100, then 429 for the rest of the day |
| Reports | 20 |

Crossing any cap, a burst of 404s or 403s, a refresh-token replay, or a hard-check rejection storm
raises an admin alert through the existing alert mail path and its notice budget (ASVS V16), and
writes `pool_audit`. Caps are also checked per IP for the read routes.

## Data model (D1)

### Phase 1 tables (exist; migrations `0001`-`0005` from `server/auth`, `0100`-`0101` from the Worker)

```
users(id, email UNIQUE, email_verified_at, pw_hash, pw_params, display_name, profile, trust,
      created_at, deleted_at, deleted_email_hmac, deleted_email_hmac_expires_at,
      device_epoch, last_device, suspended_at, suspension_reason)
                         -- profile: JSON (school, adult statement); deleted_at = tombstone, PII nulled
sessions(id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at, rotated_at,
         revoked_at, user_agent)
email_tokens(id, user_id, kind verify|reset, token_hash, created_at, expires_at, used_at,
             used_nonce, claimed_until)
device_enrolments(id, user_id, code_hash, epoch, created_at, expires_at, used_at, claimed_until, failures)
rate_limits(key, window_start, count, expires_at)           -- key is an HMAC
approval_requests(id, user_id, requested_at, decided_at, decided_by, decision, evidence,
                  evidence_note, note, reason, decision_nonce, email_domain,
                  display_name_snapshot, profile_snapshot)  -- one open per user
domain_decisions(domain, approved, rejected)
admin_proposals(id, action, target_id, proposed_by, proposed_at, expires_at, self_confirm_after,
                note, reason, decided_by, decided_at, outcome)
auth_audit(id, at, actor_id, actor_kind, action, target_id, prev_trust, new_trust, evidence,
           evidence_note, note, detail, reason)            -- append-only (triggers)
staff_domain_hint(domain, note, updated_at, updated_by)    -- a label, never grants trust
notice_budget(hour, sent)  notice_backlog(id=1, pending)
```

### Phase 2 tables (Worker migration `0102_pool.sql`)

```
items(id, author_id, kind bank|question, visibility unlisted|public,
      current_publish_id, prev_publish_id, version,
      title, subject, course, grade, kinds_mask, question_count, hide_key, license, has_images,
      committed_at, created_at, updated_at,
      removed_at, removed_reason author_deleted|account_deleted|takedown|privacy|admin,
      quarantined_at, purge_after)
      -- the listing columns are copies of the current publish's, written by the same CAS UPDATE
item_publishes(publish_id PK, item_id, author_id, base_publish_id, version,
               status staged|current|previous, schema_version, hide_key, license,
               attested_at, attest_text_rev, acks_json, request_sha256,
               full_key, stripped_key, full_bytes, stripped_bytes,
               title, subject, course, grade, kinds_mask, question_count, has_images,
               created_at, committed_at)
question_ids(qid PK, item_id, first_publish_id, created_at)     -- global question identity
item_questions(publish_id, qid, pos, qkind, difficulty, match_hash, full_sha256,
               stripped_sha256, chain_json, origin_item_id, origin_qid, modified)
               PK (publish_id, qid); INDEX (match_hash); INDEX (origin_item_id, origin_qid)
item_tags(publish_id, tag)  item_standards(publish_id, code)
blobs(sha256 PK, size, content_type, width, height, first_uploaded_at)
pending_blobs(sha256, uploader_id, until) PK (sha256, uploader_id)
item_blobs(publish_id, form full|stripped, sha256) PK (publish_id, form, sha256); INDEX (sha256)
imports(user_id, item_id, publish_id, version, form, at) UNIQUE (user_id, item_id, publish_id)
item_holds(item_id, reason reports|student_data|blocklist|admin, at, by) PK (item_id, reason)
reports(id, item_id, publish_id, reporter_id, reason, note, at, resolved_at, resolved_by, action)
        UNIQUE (item_id, reporter_id, reason) WHERE resolved_at IS NULL
admin_flags(id, kind license_soft|origin_unverified|blocklist|cap, item_id, publish_id, user_id,
            detail, at, resolved_at, resolved_by)
dmca_notices(id, received_at, complainant_name, complainant_contact, work_desc, material_desc,
             has_signature, has_good_faith, has_perjury_stmt, status open|deficient|actioned|withdrawn,
             recorded_by)
dmca_actions(id, notice_id, item_id, author_id, disabled_at, author_notified_at,
             counter_notice_id, restore_not_before, restore_by, court_action_at,
             restored_at, purged_at, outcome)
dmca_counter_notices(id, action_id, received_at, signature, material_desc, perjury_stmt,
                     name, address, phone, consent_jurisdiction, accept_service, forwarded_at)
strikes(id, user_id, action_id, at, upheld_at, voided_at)
blocked_hashes(kind match|blob, hash, action_id, at, lifted_at) PK (kind, hash)
usage_daily(user_id, day, metric, count) PK (user_id, day, metric)
quota(user_id PK, blob_bytes, body_bytes, pending_bytes, items, questions)
      -- maintained in the commit and purge batches, recomputed nightly
pool_audit(id, at, actor_id, actor_kind, action, item_id, target_user_id, detail, note)
      -- append-only, same trigger pattern as auth_audit; notes nulled when the target is deleted
```

Phase 3 (`0103_circles.sql`): `circles(id, name, description, owner_id, created_at, claim_until)`,
`circle_members(circle_id, user_id, joined_at)`, `invite_codes(id, circle_id, code_hash,
created_by, expires_at, max_uses, used, revoked_at, note)`, `item_circles(item_id, circle_id)`, and
`items.visibility` gains `circles`.
Phase 4 (search database, its own `migrations-search/0001_fts.sql`): `questions_src(publish_id, qid,
item_id, title, tags, stem_text, answer_text, explanation_text)` and the FTS5 table over it;
`answer_text` and `explanation_text` are empty for `hide_key` publishes.
Phase 5 (`0104_signals.sql`): `ratings(item_id, user_id, stars, at)`, `favorites(item_id, user_id, at)`.

Each publish body (the canonical bank or question JSON) is an **immutable** R2 object under a
**random key** (`items/<id>/<random>.json`), recorded in `item_publishes.full_key`; the stripped form
is another random key in `stripped_key`. Reading a body for an importer, preview or result always
goes through one function that takes the item row and the caller and returns the stripped object
when `hide_key` is set and the caller is not the author. Images live in R2 at `blobs/<sha256>`.

### Publish order

D1 limits a statement to 100 bound parameters and about 100 KB and a bank can hold 1,000 questions,
so a publish is staged and then committed by one compare-and-set.

1. The client generates `publish_id` (128 random bits) and keeps it until it gets a definite answer;
   every retry of the same publish reuses it. It uploads every image (`PUT /blobs`, full bytes).
2. It sends `POST /items` (new item) or `PUT /items/:id` (republish) with `publish_id`,
   `base_publish_id` (the publish it edited; `null` for a new item), the payload, `hide_key`,
   `license`, the attestation, the per-question acknowledgements and image ticks.
3. **Idempotency:** if `publish_id` already exists for this author with the same `request_sha256`,
   the server returns the first result (or resumes a staged publish); with a different body, 409.
4. Validate (Limits, schema, characters), canonicalize, assign `qid`s, compute hashes, run the
   license and blocklist checks, strip. Every referenced blob must have a `pending_blobs` row for
   this author or already be referenced by one of the author's publishes; validation extends those
   `pending_blobs.until` to `now + 24 h`.
5. Write the R2 bodies (full, and stripped if `hide_key`).
6. Insert the `item_publishes` row (`status='staged'`) and its derived rows, all keyed by
   `publish_id`, in chunks: `item_questions`, `item_tags`, `item_standards`, `item_blobs`, new
   `question_ids`. Nothing reads them yet, because every read joins on `items.current_publish_id`.
7. **Commit**, one D1 batch:
   `UPDATE items SET current_publish_id = :new, prev_publish_id = current_publish_id, version = :v,
   <listing columns> WHERE id = :item AND author_id = :me AND current_publish_id IS :base AND
   removed_at IS NULL`, then `UPDATE item_publishes SET status = 'current', committed_at = :now
   WHERE publish_id = :new AND EXISTS (SELECT 1 FROM items WHERE id = :item AND current_publish_id =
   :new)`, then the same guarded update setting the old current to `previous`. A new item instead
   `INSERT`s its `items` row in this batch with an id fixed at staging. If the CAS changed 0 rows the
   answer is **409 conflict** with the current version, the staged rows are deleted, and the app
   tells the author another device published first.
8. After commit: delete the publish that is now two behind (retention current+1): its derived rows,
   its R2 bodies and its search rows; clear `pending_blobs` rows for blobs the new publish references;
   update `quota`; write the search rows (phase 4, rebuildable).

Every query that reads derived rows joins `publish_id = items.current_publish_id`, so a search or
blob check never sees a staged or superseded publish's rows (an answer text from a v1 published with
its key cannot be found after a `hide_key` v2). Two devices republishing at once cannot interleave:
only one CAS wins, and the loser's rows were never reachable. An import reads the publish row and its
object key in one statement, and the previous publish is retained, so a swap during an import cannot
leave it without a body.

**Visibility change without a new publish:** `PATCH /items/:id {visibility}` by the author, one
`UPDATE` (refused for `public` when the current license is `circle-only`).

**GC (hourly cron):** delete `staged` publishes older than 1 hour (rows and bodies); delete blobs
with no `item_blobs` row and no `pending_blobs` row whose `until >= now - 1 h`; delete expired
`pending_blobs` rows. Ages, not states, guard in-flight work.

### Blob rules

- `PUT /blobs/:sha256` always takes the full bytes, runs the Images checks, charges the blob as
  pending to the uploader, and returns the **same** status and body (`201 {sha256, size, width,
  height}`) whether or not the blob already existed, so an upload is never an existence oracle.
- `GET /items/:id/blobs/:sha256` is the only read path; blobs are never served by a public or
  presigned R2 URL. A blob is readable **iff** the caller may see item `:id` and an `item_blobs` row
  exists with `publish_id = items.current_publish_id` and `form` equal to the caller's served form:
  `full` for the author or when the publish has no `hide_key`, otherwise `stripped`. A figure render
  dropped from a stripped form is listed only under `full`, so probing hashes of the ~1,000 possible
  pipette renders returns 404 for every one. A miss and a forbidden read are the same 404, counted
  against the daily miss cap.
- Blob responses carry `Content-Type` from `blobs.content_type` (the format verified from the bytes), `X-Content-Type-Options: nosniff`,
  `Content-Security-Policy: sandbox; default-src 'none'`, `Content-Disposition: attachment;
  filename="<sha256>.<ext>"` (the name built from server data only), `Cache-Control: no-store`.

### Caching (ASVS V14)

Every authenticated response, JSON and blob alike, sends `Cache-Control: no-store` and
`Vary: Authorization, Cookie, Origin`, so a shared school browser or proxy never caches an author's
full body for the next user. (Phase 1 already sends `no-store` on its JSON responses.)

## API (JSON over HTTPS, `Authorization: Bearer`)

```
-- phase 1 (implemented)
POST /auth/signup  /auth/login  /auth/refresh  /auth/logout  /auth/reauth
POST /auth/verify  /auth/resend  /auth/forgot  /auth/reset  /auth/device/enrol  /auth/device/enrolment-code
DELETE /account    GET /me    GET /account/export    POST /account/reapply
GET  /admin/queue  GET /admin/users/:id  GET /admin/audit  GET /admin/proposals
POST /admin/approve  /admin/reject  /admin/reject-bulk  /admin/revoke-teacher  /admin/suspend
     /admin/restore  /admin/confirm  /admin/cancel
GET|PUT|DELETE /admin/staff-domains

-- phase 2
POST /items                         (new item; body carries publish_id)
PUT  /items/:id                     (republish; publish_id + base_publish_id; 409 on a lost CAS)
PATCH /items/:id                    (visibility only)
DELETE /items/:id                   (author; 14-day undo)
POST /items/:id/undelete            (author, within the 14 days)
GET  /items?subject=&kind=&scope=&cursor=&limit=   (newest first, cursor pages, max 50)
GET  /items/mine                    (with holds and takedown state)
GET  /items/:id                     (served form: full for the author or without hide_key, else stripped)
POST /items/:id/import              (records the import; returns the served payload)
POST /items/versions                (update notices: current | newer | removed | unavailable)
PUT  /blobs/:sha256                 GET /items/:id/blobs/:sha256
POST /items/:id/report
POST /items/:id/counter-notice      (author of a taken-down item)
GET  /admin/reports  GET /admin/flags  GET /admin/items/:id (full form, any state)
POST /admin/items/:id/hold  /admin/items/:id/release  /admin/items/:id/purge
POST /admin/dmca/notices  /admin/dmca/takedown  /admin/dmca/court-action  /admin/dmca/withdraw
PUT  /admin/limits

-- phase 3: /circles ... (as in Circles)   phase 5: PUT /items/:id/rating  /items/:id/favorite
```

Every route checks trust level and, for item reads, visibility, holds, removal and the author's
standing. Ownership is checked per route as noted. Item ids, `qid`s and circle ids are random and
unguessable, and an item the caller cannot see returns 404, not 403, so existence is not revealed.

## App changes

- **Account screen** (exists): sign in, sign out, status, storage used.
- **Sharing panel** on a bank and on a question: visibility, hide-key toggle, license (no default),
  metadata, the publish-time review with its ticks and acknowledgements, the attestation, and the
  privacy notice. Publish and republish; a 409 says another device published first and offers to
  reload.
- **Pool browser**: phase 2 lists newest public items with subject and kind filters and "load more";
  item preview, Import, report. Search box and further filters in phase 4; rate and favorite in phase 5.
- **Circles screen** (phase 3): list, create, invite codes (create, copy, revoke), members.
- **Provenance chip** on imported questions and banks, with the chain, the update, source-removed
  and privacy-removed states.
- The `src/share/` client wraps all calls; the rest of the app never touches HTTP.
- Everything is hidden or disabled while signed out, and failures are non-blocking toasts.
- The **admin console** (in-app or web, open question) renders every pool field (titles, tags,
  stems, notes, display names, notice text) as plain text, never HTML or markup (ASVS V1).

## Safety and privacy

- No student data in any shared payload. The model has no student fields to strip, so the
  publish-time review is the control, backed by the one-report hide and immediate purge (Reports).
- Only the minimum personal data is kept: email, display name, school, trust level. DMCA records
  hold complainant and counter-notifier contact details because the statute requires them; they are
  admin-only.
- What is disclosed: the author's display name travels with every copy and attribution chain and
  survives account deletion; email and school are never shared. The privacy policy says so.
- **Legal before launch:** Terms of Service (including the repeat-infringer policy), a privacy
  policy, a teacher-only / adult statement at sign-up (COPPA, and California's SOPIPA), and a
  registered DMCA designated agent. Counsel reviews all of it.
- Passwords and tokens are never logged. Mail tokens and invite codes are stored hashed.
- Account export (`GET /account/export`) and deletion on request.

## Build order

1. **Done.** Worker skeleton (paid plan), D1 schema, sign-up/login/verify/reset, rate limiting, app
   account screen, and the admin approval queue.
2. **Prerequisite (first phase-2 task): fix the local image store in `src/store.rs`, with tests.**
   `imagesize` gains a WebP branch (`VP8 `, `VP8L` and `VP8X` headers); `sniff_ext` returns
   `.webp` only for `RIFF` with `WEBP` at byte offset 8 (any other `RIFF` is `.bin`); `put_image`
   sniffs the type and reads the dimensions **before** writing, and writes nothing for an
   unreadable or unsupported image (today it writes first and fails after). Unit tests cover a
   valid file of each of PNG, JPEG, GIF and WebP, a non-WebP `RIFF` (e.g. WAV), a truncated header,
   and that a refused image leaves no file in `media/`.

   Then: publish and **republish** (staged publish, CAS commit, idempotent `publish_id`, retention
   current+1), visibility change, browse (newest public items filtered by subject and kind, cursor
   pagination, no search), preview and import (atomic, revalidated, with the attribution chain and
   duplicate warning), **update notices**, the `share-core` pool functions (`validate`, `strip`,
   `match_hash`), the image pipeline and blob rules, limits, daily caps and alerts, the license
   table and hard/soft checks, the `origin`/`published`/`keyless` model fields and the keyless
   sentinel, reports (with the one-report student-data hide), takedown with quarantine,
   counter-notice and put-back, strikes and the blocklist, author-standing visibility, and the
   account-deletion content cleanup in `cleanup.ts`. Public publishing is live from this phase, so
   the DMCA path and the legal pages must exist before it ships publicly.
3. Circles, invite codes and circle visibility.
4. Search (separate D1), filters, tags, standards and sorting.
5. Ratings, favorites and import counts on display.
6. Google sign-in and magic link.

Each phase ships on its own and leaves the app fully usable offline.

## Open questions

Decided by the owner on 2026-10-19 (no longer open): separate search D1 database; PNG, JPEG, GIF
and WebP accepted as still images under the Images rules; 14-day undo for item deletion; 30-day
quarantine without a counter-notice; 3 upheld strikes in a rolling 12 months; the daily caps in
Limits; 200 MB quota; report threshold 3 for non-privacy reasons; licenses listed CC-BY-4.0 first,
nothing preselected.

**Owner to-dos before public launch (not build blockers):** register the DMCA designated agent;
have counsel review the ToS, privacy policy, repeat-infringer policy and the Takedown section (this
spec is not legal advice).

Still open. Each has a default the build uses if it is not answered; **none blocks writing code**.
Two block a public deploy, as noted.

| Question | Default if unanswered | Blocks |
|---|---|---|
| Transactional mail provider (Resend vs Cloudflare Email) | Resend (already wired in `server/worker/src/mail.ts`) | nothing |
| Standards pick-lists (CCSS math/ELA, NGSS, CA-specific) | free-text codes, normalized; CCSS math and NGSS pick-lists added in phase 4 | nothing |
| Should exams, not just banks, be shareable? | no; v1 shares banks and single questions | nothing |
| Admin console: separate web page or a screen in the app? | a screen in the app behind the admin trust level (the phase 1 admin routes are API-only already) | nothing |
| Approval rule (staff directory, or school confirms via a contact the admin initiates) | the rule as written in Trust levels | nothing (already implemented) |
| Hosting: web build and API on one registrable domain | assumed, as in Stack | public deploy of the web build |
| Production API URL (`VERSHION_POOL_URL`) | none; a release built without it has sharing switched off | any release with sharing on |
| Keyless sentinel wording printed by older builds | the text in the stripped form section | nothing |

To verify at build time: that `image` 0.25's PNG, JPEG, GIF and WebP encoders write only the parts
the server allows and that its WebP encoder is lossless-only (share-core round-trip fixtures cover
both), and D1's current statement and bound-parameter limits.

Resolved: Argon2id CPU time on Workers (measured in phase 1, `docs/phase1-argon2-spike.md`).
