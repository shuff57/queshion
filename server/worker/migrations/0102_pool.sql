-- verSHion phase 2: the shared pool (docs/shared-pool.md, "Data model", "Publish order", "Blob rules",
-- "Reports", "Takedown"). Times are epoch milliseconds. Every non-author read joins
-- items.current_publish_id, so a staged or superseded publish's derived rows are never reachable.

-- One row per published item (a bank or a single question). The listing columns are copies of the
-- current publish's, written by the same compare-and-set UPDATE that moves current_publish_id.
CREATE TABLE items (
  id                 TEXT PRIMARY KEY,
  author_id          TEXT NOT NULL,
  kind               TEXT NOT NULL CHECK (kind IN ('bank', 'question')),
  visibility         TEXT NOT NULL CHECK (visibility IN ('unlisted', 'public')),
  current_publish_id TEXT,
  prev_publish_id    TEXT,
  version            INTEGER NOT NULL DEFAULT 0,
  title              TEXT NOT NULL DEFAULT '',
  subject            TEXT NOT NULL DEFAULT '',
  course             TEXT NOT NULL DEFAULT '',
  grade              TEXT NOT NULL DEFAULT '',
  kinds_mask         INTEGER NOT NULL DEFAULT 0,
  question_count     INTEGER NOT NULL DEFAULT 0,
  hide_key           INTEGER NOT NULL DEFAULT 0,
  license            TEXT NOT NULL,
  has_images         INTEGER NOT NULL DEFAULT 0,
  committed_at       INTEGER,
  created_at         INTEGER NOT NULL,
  updated_at         INTEGER NOT NULL,
  removed_at         INTEGER,
  removed_reason     TEXT CHECK (removed_reason IN ('author_deleted', 'account_deleted', 'takedown', 'privacy', 'admin')),
  quarantined_at     INTEGER,
  purge_after        INTEGER,
  -- What an admin removal replaced (an author deletion), so /admin/pool/unremove can put it back.
  prior_removed_reason TEXT,
  prior_purge_after  INTEGER,
  -- The version non-authors see: not bumped by a republish that changes nothing they are served
  -- (a hidden key correction), so "newer" cannot signal a hidden change.
  public_version     INTEGER NOT NULL DEFAULT 0,
  -- When the public version was first committed (what non-authors see as committed_at), and the
  -- publish whose stripped body non-authors are served: the only publish id they ever see.
  public_at          INTEGER,
  public_publish_id  TEXT,
  -- The author deleted their account but chose to keep their shared items (shown as "deleted teacher").
  anonymized         INTEGER NOT NULL DEFAULT 0,
  -- Set once the purge ran: the row stays as a tombstone (id, author, removed_reason) for counts,
  -- reports, DMCA records and update notices.
  purged_at          INTEGER,
  CHECK (visibility = 'unlisted' OR license <> 'circle-only'),
  CHECK ((removed_at IS NULL) = (removed_reason IS NULL))
);
-- Browse pages on the immutable creation time, so a republish never moves an item across a cursor.
CREATE INDEX items_browse ON items (created_at DESC, id DESC) WHERE removed_at IS NULL AND visibility = 'public';
CREATE INDEX items_author ON items (author_id, committed_at DESC, id DESC);
CREATE INDEX items_purge ON items (purge_after) WHERE purge_after IS NOT NULL AND purged_at IS NULL;

-- One row per publish (a version). Immutable once committed, except status and the R2 keys
-- (moved to quarantine/ by a takedown).
CREATE TABLE item_publishes (
  publish_id      TEXT PRIMARY KEY,
  item_id         TEXT NOT NULL,
  author_id       TEXT NOT NULL,
  base_publish_id TEXT,
  version         INTEGER NOT NULL,
  public_version  INTEGER NOT NULL DEFAULT 0,
  -- staging: rows being written; staged: complete, waiting for the CAS; current; previous.
  status          TEXT NOT NULL CHECK (status IN ('staging', 'staged', 'current', 'previous')),
  schema_version  INTEGER NOT NULL,
  hide_key        INTEGER NOT NULL,
  license         TEXT NOT NULL,
  attested_at     INTEGER NOT NULL,
  attest_text_rev TEXT NOT NULL,
  -- The author's label decisions, order acknowledgements and image ticks, as share-core returned them.
  acks_json       TEXT NOT NULL,
  request_sha256  TEXT NOT NULL,
  -- Random per staging attempt: a slow request's late writes can never attach to a retry's row.
  stage_token     TEXT NOT NULL,
  -- The answer the first request got, replayed for a retry with the same publish_id and body.
  result_json     TEXT,
  full_key        TEXT NOT NULL,
  stripped_key    TEXT,
  full_bytes      INTEGER NOT NULL,
  stripped_bytes  INTEGER NOT NULL DEFAULT 0,
  title           TEXT NOT NULL,
  subject         TEXT NOT NULL,
  course          TEXT NOT NULL,
  grade           TEXT NOT NULL,
  kinds_mask      INTEGER NOT NULL,
  question_count  INTEGER NOT NULL,
  has_images      INTEGER NOT NULL,
  created_at      INTEGER NOT NULL,
  committed_at    INTEGER
);
CREATE INDEX item_publishes_item ON item_publishes (item_id, status);
CREATE INDEX item_publishes_author ON item_publishes (author_id);
CREATE INDEX item_publishes_staged ON item_publishes (created_at) WHERE status IN ('staging', 'staged');

-- Every publish_id that was ever committed stays reserved after retention or purge deletes its
-- row: a client-chosen id can never be reused (by anyone) to collide with another author's history.
CREATE TABLE retired_publish_ids (
  publish_id TEXT PRIMARY KEY,
  author_id  TEXT NOT NULL
);

-- Global question identity, stable across versions of one item.
CREATE TABLE question_ids (
  qid              TEXT PRIMARY KEY,
  item_id          TEXT NOT NULL,
  first_publish_id TEXT NOT NULL,
  created_at       INTEGER NOT NULL,
  -- Strictly increasing across publishes (all questions of one publish share it): orders "who
  -- published it first" without depending on clock resolution.
  seq              INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX question_ids_item ON question_ids (item_id);

-- full_sha256 and full_content are key oracles for small answer spaces: never sent to a non-author,
-- never compared across items. match_hash is key-free (share-core) and is the only cross-item hash.
CREATE TABLE item_questions (
  publish_id       TEXT NOT NULL,
  qid              TEXT NOT NULL,
  pos              INTEGER NOT NULL,
  qkind            TEXT NOT NULL,
  difficulty       TEXT,
  match_hash       TEXT NOT NULL,
  full_sha256      TEXT NOT NULL,
  stripped_sha256  TEXT,
  -- Hash of the question with id/created/modified/origin/published neutralised, per form: what
  -- "modified" in an attribution link is decided against.
  full_content     TEXT NOT NULL,
  stripped_content TEXT,
  -- Hash of key-free facts only (match_hash, points, difficulty, topics, the image as served): what
  -- "modified" is decided on when the publish hides its key, so the flag can never reveal a key.
  keyfree_content  TEXT NOT NULL,
  -- A loose hash of the normalised stem only (a soft "origin_suspect" signal, never a refusal).
  stem_hash        TEXT,
  chain_json       TEXT NOT NULL DEFAULT '[]',
  origin_item_id   TEXT,
  origin_qid       TEXT,
  modified         INTEGER,
  PRIMARY KEY (publish_id, qid)
);
CREATE INDEX item_questions_match ON item_questions (match_hash);
CREATE INDEX item_questions_origin ON item_questions (origin_item_id, origin_qid);

CREATE TABLE item_tags (publish_id TEXT NOT NULL, tag TEXT NOT NULL, PRIMARY KEY (publish_id, tag));
CREATE TABLE item_standards (publish_id TEXT NOT NULL, code TEXT NOT NULL, PRIMARY KEY (publish_id, code));

-- Verified images (content-addressed). content_type comes from the bytes, never the client.
CREATE TABLE blobs (
  sha256            TEXT PRIMARY KEY,
  size              INTEGER NOT NULL,
  content_type      TEXT NOT NULL CHECK (content_type IN ('image/png', 'image/jpeg', 'image/gif', 'image/webp')),
  width             INTEGER NOT NULL,
  height            INTEGER NOT NULL,
  first_uploaded_at INTEGER NOT NULL,
  -- The hourly reconciler: when the bytes were last confirmed in R2, and since when they are missing
  -- (the author is asked to upload again; a publish names the positions).
  checked_at        INTEGER,
  missing_at        INTEGER
);
CREATE INDEX blobs_checked ON blobs (checked_at);
-- Uploaded but not yet referenced by a publish; charged to the uploader's quota until it expires.
CREATE TABLE pending_blobs (
  sha256      TEXT NOT NULL,
  uploader_id TEXT NOT NULL,
  size        INTEGER NOT NULL,
  until       INTEGER NOT NULL,
  PRIMARY KEY (sha256, uploader_id)
);
CREATE INDEX pending_blobs_until ON pending_blobs (until);
CREATE INDEX pending_blobs_sha ON pending_blobs (sha256);
-- A figure render dropped from a stripped form is listed only under 'full'.
CREATE TABLE item_blobs (
  publish_id TEXT NOT NULL,
  form       TEXT NOT NULL CHECK (form IN ('full', 'stripped')),
  sha256     TEXT NOT NULL,
  PRIMARY KEY (publish_id, form, sha256)
);
CREATE INDEX item_blobs_sha ON item_blobs (sha256);

-- One row per teacher per publish imported. Import count = COUNT(DISTINCT user_id).
CREATE TABLE imports (
  user_id    TEXT NOT NULL,
  item_id    TEXT NOT NULL,
  publish_id TEXT NOT NULL,
  version    INTEGER NOT NULL,
  form       TEXT NOT NULL CHECK (form IN ('full', 'stripped')),
  license    TEXT NOT NULL,
  author_id  TEXT NOT NULL,
  at         INTEGER NOT NULL,
  -- 'view': the body was read (GET) but not imported. It counts as licence evidence (a verbatim copy
  -- of something read is still a copy) but not towards import counts.
  kind       TEXT NOT NULL DEFAULT 'import' CHECK (kind IN ('import', 'view')),
  -- publish_id is the public publish id (what the account was shown); this is the real one whose
  -- item_questions a 'view' derives its evidence from at publish time.
  real_publish_id TEXT,
  -- Every licence this account was shown for this publish (a hidden-only republish can change the
  -- licence without changing what is served): each one is a grant it holds (CC grants are irrevocable).
  licenses_json TEXT,
  UNIQUE (user_id, item_id, publish_id)
);
CREATE INDEX imports_item ON imports (item_id);
-- What an importer was served, per question: the hard license check and the attribution chain must
-- still work after the source publish is superseded or purged.
CREATE TABLE import_questions (
  user_id      TEXT NOT NULL,
  publish_id   TEXT NOT NULL,
  qid          TEXT NOT NULL,
  item_id      TEXT NOT NULL,
  match_hash   TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  keyfree_hash TEXT NOT NULL,
  -- The source question's own chain (oldest first) plus nothing: the link to the source is built
  -- from imports at publish time.
  chain_json   TEXT NOT NULL,
  author_name  TEXT NOT NULL,
  -- question_ids.seq of the source question: evidence only if it precedes the publisher's own.
  source_first_seq INTEGER,
  stem_hash    TEXT,
  -- Set when the source was found infringing (an upheld takedown) or by an admin: the row no longer
  -- counts as licence or attribution evidence. Audited; reversible (a put-back clears it).
  void_at      INTEGER,
  PRIMARY KEY (user_id, publish_id, qid)
);
CREATE INDEX import_questions_match ON import_questions (user_id, match_hash);

CREATE TABLE item_holds (
  item_id TEXT NOT NULL,
  reason  TEXT NOT NULL CHECK (reason IN ('reports', 'student_data', 'blocklist', 'admin')),
  at      INTEGER NOT NULL,
  by      TEXT,
  -- Why the hold exists beyond its reason: '' (reports), 'manual' (an admin), 'privacy',
  -- 'dmca:<action id>' (a copy of taken-down material), 'carried:<item id>' (an evaded hold).
  -- Part of the key: releasing one source's holds can never drop another's.
  source  TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (item_id, reason, source)
);

CREATE TABLE reports (
  id          TEXT PRIMARY KEY,
  item_id     TEXT NOT NULL,
  publish_id  TEXT,
  -- NULL once the reporter deleted their account (kept, anonymised, as moderation evidence).
  reporter_id TEXT,
  reason      TEXT NOT NULL CHECK (reason IN ('wrong_answer', 'copyright', 'student_data', 'inappropriate', 'other')),
  note        TEXT,
  at          INTEGER NOT NULL,
  resolved_at INTEGER,
  resolved_by TEXT,
  action      TEXT
);
CREATE UNIQUE INDEX reports_open ON reports (item_id, reporter_id, reason) WHERE resolved_at IS NULL;
CREATE INDEX reports_reporter ON reports (reporter_id, at);

CREATE TABLE admin_flags (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('license_soft', 'origin_unverified', 'origin_modified', 'origin_suspect', 'blocklist', 'cap', 'repeat_infringer', 'appeal', 'dmca_copy', 'instant_hold_cap', 'needs_reupload')),
  item_id     TEXT,
  publish_id  TEXT,
  user_id     TEXT,
  detail      TEXT,
  -- Dedup key for flags that must be raised at most once (e.g. one cap alert per user, day and metric).
  dedup       TEXT UNIQUE,
  at          INTEGER NOT NULL,
  resolved_at INTEGER,
  resolved_by TEXT
);

CREATE TABLE dmca_notices (
  id                  TEXT PRIMARY KEY,
  received_at         INTEGER NOT NULL,
  complainant_name    TEXT NOT NULL,
  complainant_contact TEXT NOT NULL,
  work_desc           TEXT NOT NULL,
  material_desc       TEXT NOT NULL,
  has_signature       INTEGER NOT NULL,
  has_good_faith      INTEGER NOT NULL,
  has_perjury_stmt    INTEGER NOT NULL,
  status              TEXT NOT NULL CHECK (status IN ('open', 'deficient', 'actioned', 'withdrawn')),
  recorded_by         TEXT NOT NULL
);
CREATE TABLE dmca_actions (
  id                 TEXT PRIMARY KEY,
  notice_id          TEXT NOT NULL,
  item_id            TEXT NOT NULL,
  author_id          TEXT NOT NULL,
  disabled_at        INTEGER NOT NULL,
  author_notified_at INTEGER,
  counter_notice_id  TEXT,
  restore_not_before INTEGER,
  restore_by         INTEGER,
  court_action_at    INTEGER,
  restored_at        INTEGER,
  purged_at          INTEGER,
  outcome            TEXT,
  -- What the item was before the takedown (an author deletion is resumed by a put-back, never undone).
  prev_removed_reason TEXT,
  prev_purge_after    INTEGER
);
CREATE INDEX dmca_actions_item ON dmca_actions (item_id);
CREATE TABLE dmca_counter_notices (
  id                   TEXT PRIMARY KEY,
  action_id            TEXT NOT NULL UNIQUE,
  received_at          INTEGER NOT NULL,
  signature            TEXT NOT NULL,
  material_desc        TEXT NOT NULL,
  perjury_stmt         INTEGER NOT NULL,
  name                 TEXT NOT NULL,
  address              TEXT NOT NULL,
  phone                TEXT NOT NULL,
  consent_jurisdiction INTEGER NOT NULL,
  accept_service       INTEGER NOT NULL,
  forwarded_at         INTEGER
);
CREATE TABLE strikes (
  id        TEXT PRIMARY KEY,
  user_id   TEXT NOT NULL,
  action_id TEXT NOT NULL UNIQUE,
  at        INTEGER NOT NULL,
  upheld_at INTEGER,
  voided_at INTEGER
);
CREATE INDEX strikes_user ON strikes (user_id, at);
CREATE TABLE blocked_hashes (
  kind      TEXT NOT NULL CHECK (kind IN ('match', 'blob')),
  hash      TEXT NOT NULL,
  action_id TEXT,
  at        INTEGER NOT NULL,
  lifted_at INTEGER,
  -- An admin-chosen end (an upheld takedown's block need not last forever); NULL = until lifted.
  expires_at INTEGER,
  PRIMARY KEY (kind, hash)
);

-- Student-data blocks, kept apart from DMCA blocks (blocked_hashes): a put-back never lifts one,
-- and an existing DMCA row can never swallow one. Images are served from no item; question text
-- (match) and images hold every item that uses them, and any later publish that does.
CREATE TABLE privacy_blocks (
  kind    TEXT NOT NULL CHECK (kind IN ('match', 'blob')),
  hash    TEXT NOT NULL,
  item_id TEXT NOT NULL,
  at      INTEGER NOT NULL,
  by      TEXT,
  -- Images: served to nobody at once, bytes deleted from R2 only after this (a grace window in which
  -- an unblock restores everything), then bytes_purged_at is set.
  bytes_purge_after INTEGER,
  bytes_purged_at   INTEGER,
  PRIMARY KEY (kind, hash)
);

-- Per-account single flight for publishes (across isolates): a second concurrent publish is a 409.
CREATE TABLE pool_locks (
  user_id TEXT PRIMARY KEY,
  token   TEXT NOT NULL,
  until   INTEGER NOT NULL
);

-- What happens to an author's shared items when they delete their account (default: removed and
-- purged in 14 days; 'keep': kept, anonymised as "deleted teacher", licences and chains intact).
CREATE TABLE pool_prefs (
  user_id        TEXT PRIMARY KEY,
  keep_on_delete INTEGER NOT NULL DEFAULT 0,
  updated_at     INTEGER NOT NULL
);

-- Daily per-account counters (anti-automation caps). day = floor(ms / 86400000).
CREATE TABLE usage_daily (
  user_id TEXT NOT NULL,
  day     INTEGER NOT NULL,
  metric  TEXT NOT NULL,
  count   INTEGER NOT NULL,
  PRIMARY KEY (user_id, day, metric)
);
-- Admin-set limits: user_id '*' is the site-wide default for that metric, a user id overrides it.
CREATE TABLE pool_limits (
  user_id    TEXT NOT NULL,
  metric     TEXT NOT NULL,
  value      INTEGER NOT NULL CHECK (value >= 0),
  updated_at INTEGER NOT NULL,
  updated_by TEXT NOT NULL,
  PRIMARY KEY (user_id, metric)
);
-- Admin alert mails from the pool, at most a few an hour (the rest are only in admin_flags).
CREATE TABLE pool_alert_budget (hour INTEGER PRIMARY KEY, sent INTEGER NOT NULL);

-- Append-only, same pattern as auth_audit: only `note` may be nulled, and only once the target
-- account is deleted.
CREATE TABLE pool_audit (
  id             TEXT PRIMARY KEY,
  at             INTEGER NOT NULL,
  actor_id       TEXT,
  actor_kind     TEXT NOT NULL CHECK (actor_kind IN ('user', 'admin', 'system')),
  action         TEXT NOT NULL,
  item_id        TEXT,
  target_user_id TEXT,
  detail         TEXT,
  note           TEXT
);
CREATE INDEX pool_audit_item ON pool_audit (item_id, at);
CREATE INDEX pool_audit_target ON pool_audit (target_user_id);
CREATE TRIGGER pool_audit_no_delete BEFORE DELETE ON pool_audit
BEGIN
  SELECT RAISE(ABORT, 'pool_audit is append-only');
END;
CREATE TRIGGER pool_audit_no_update BEFORE UPDATE ON pool_audit
WHEN NOT (
  NEW.id = OLD.id AND NEW.at = OLD.at AND NEW.actor_id IS OLD.actor_id AND NEW.actor_kind = OLD.actor_kind
  AND NEW.action = OLD.action AND NEW.item_id IS OLD.item_id AND NEW.target_user_id IS OLD.target_user_id
  AND NEW.detail IS OLD.detail AND NEW.note IS NULL
  AND EXISTS (SELECT 1 FROM users WHERE id = OLD.target_user_id AND deleted_at IS NOT NULL)
)
BEGIN
  SELECT RAISE(ABORT, 'pool_audit is append-only');
END;
