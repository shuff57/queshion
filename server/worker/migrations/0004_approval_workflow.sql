-- Approval workflow: one row per application (a re-application is a new row), suspension, and an
-- append-only audit log.

CREATE TABLE approval_requests (
  id             TEXT PRIMARY KEY,
  user_id        TEXT NOT NULL REFERENCES users (id),
  requested_at   INTEGER NOT NULL,
  decided_at     INTEGER,
  -- The reviewer (a user id; stays as the tombstone id if that account is deleted).
  decided_by     TEXT,
  -- approved | rejected | revoked | withdrawn | account_deleted | bootstrap
  decision       TEXT,
  -- How the reviewer satisfied the approval rule (e.g. directory | school_confirmed).
  evidence       TEXT,
  evidence_note  TEXT,
  -- Reviewer-only note. Never shown to the applicant.
  note           TEXT,
  -- Shown to the applicant on rejection, revocation or suspension.
  reason         TEXT,
  -- Random per deciding request, so the rest of that request's batch acts only if it won the race.
  decision_nonce TEXT UNIQUE
);
-- At most one open application per account.
CREATE UNIQUE INDEX approval_requests_one_open ON approval_requests (user_id) WHERE decided_at IS NULL;
CREATE INDEX approval_requests_queue ON approval_requests (requested_at, id) WHERE decided_at IS NULL;
CREATE INDEX approval_requests_user ON approval_requests (user_id, requested_at);

INSERT INTO approval_requests (id, user_id, requested_at, decided_at, decided_by, decision, note)
  SELECT lower(hex(randomblob(16))), user_id, requested_at, decided_at, decided_by, decision, note FROM approvals;
DROP TABLE approvals;

-- Set = suspended: cannot log in, every session and device token revoked. Trust is kept, so a
-- restore returns the account to where it was.
ALTER TABLE users ADD COLUMN suspended_at INTEGER;
-- The reviewer's message to the account holder about the suspension (shown in /me). Nulled on deletion.
ALTER TABLE users ADD COLUMN suspension_reason TEXT;

-- Append-only. Survives account deletion: the target stays as the tombstone id and free-text notes
-- are nulled (the only change the triggers allow, and only once the target account is deleted).
CREATE TABLE auth_audit (
  id            TEXT PRIMARY KEY,
  at            INTEGER NOT NULL,
  actor_id      TEXT,
  actor_kind    TEXT NOT NULL CHECK (actor_kind IN ('admin', 'bootstrap', 'system', 'user')),
  action        TEXT NOT NULL,
  target_id     TEXT,
  prev_trust    TEXT,
  new_trust     TEXT,
  evidence      TEXT,
  evidence_note TEXT,
  note          TEXT,
  -- Structured, non-personal extras (JSON), e.g. a staff-domain change.
  detail        TEXT
);
CREATE INDEX auth_audit_target ON auth_audit (target_id, at);
CREATE INDEX auth_audit_at ON auth_audit (at, id);

CREATE TRIGGER auth_audit_no_delete BEFORE DELETE ON auth_audit
BEGIN
  SELECT RAISE(ABORT, 'auth_audit is append-only');
END;

CREATE TRIGGER auth_audit_no_update BEFORE UPDATE ON auth_audit
WHEN NOT (
  NEW.id = OLD.id AND NEW.at = OLD.at AND NEW.actor_id IS OLD.actor_id AND NEW.actor_kind = OLD.actor_kind
  AND NEW.action = OLD.action AND NEW.target_id IS OLD.target_id AND NEW.prev_trust IS OLD.prev_trust
  AND NEW.new_trust IS OLD.new_trust AND NEW.evidence IS OLD.evidence AND NEW.detail IS OLD.detail
  AND NEW.note IS NULL AND NEW.evidence_note IS NULL
  AND EXISTS (SELECT 1 FROM users WHERE id = OLD.target_id AND deleted_at IS NOT NULL)
)
BEGIN
  SELECT RAISE(ABORT, 'auth_audit is append-only');
END;
