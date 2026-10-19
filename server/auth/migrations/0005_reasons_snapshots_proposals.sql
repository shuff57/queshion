-- 0005: free-text reasons out of the immutable JSON detail; per-application snapshots of what was
-- submitted; precomputed per-domain decision counts; two-person proposals for actions on admins.

-- Reasons get their own audit column, which (like note and evidence_note) may be nulled only once
-- the target account is deleted. `detail` stays structured and free of free text.
DROP TRIGGER auth_audit_no_update;
ALTER TABLE auth_audit ADD COLUMN reason TEXT;
UPDATE auth_audit
  SET reason = json_extract(detail, '$.reason'),
      detail = CASE WHEN json_remove(detail, '$.reason') = '{}' THEN NULL ELSE json_remove(detail, '$.reason') END
  WHERE detail IS NOT NULL AND json_extract(detail, '$.reason') IS NOT NULL;
CREATE TRIGGER auth_audit_no_update BEFORE UPDATE ON auth_audit
WHEN NOT (
  NEW.id = OLD.id AND NEW.at = OLD.at AND NEW.actor_id IS OLD.actor_id AND NEW.actor_kind = OLD.actor_kind
  AND NEW.action = OLD.action AND NEW.target_id IS OLD.target_id AND NEW.prev_trust IS OLD.prev_trust
  AND NEW.new_trust IS OLD.new_trust AND NEW.evidence IS OLD.evidence AND NEW.detail IS OLD.detail
  AND NEW.note IS NULL AND NEW.evidence_note IS NULL AND NEW.reason IS NULL
  AND EXISTS (SELECT 1 FROM users WHERE id = OLD.target_id AND deleted_at IS NOT NULL)
)
BEGIN
  SELECT RAISE(ABORT, 'auth_audit is append-only');
END;

-- What the applicant submitted with THIS application (the live profile can change later), and the
-- email domain at that time (indexed, for the queue's domain filter).
ALTER TABLE approval_requests ADD COLUMN email_domain TEXT;
ALTER TABLE approval_requests ADD COLUMN display_name_snapshot TEXT;
ALTER TABLE approval_requests ADD COLUMN profile_snapshot TEXT;
UPDATE approval_requests SET
  email_domain = (SELECT substr(u.email, instr(u.email, '@') + 1) FROM users u WHERE u.id = approval_requests.user_id),
  display_name_snapshot = (SELECT u.display_name FROM users u WHERE u.id = approval_requests.user_id),
  profile_snapshot = (SELECT u.profile FROM users u WHERE u.id = approval_requests.user_id);
CREATE INDEX approval_requests_domain ON approval_requests (email_domain, requested_at, id);

-- Decisions per email domain, kept up to date in the deciding batch, so a queue page costs one
-- primary-key lookup per domain instead of a scan.
CREATE TABLE domain_decisions (
  domain   TEXT PRIMARY KEY,
  approved INTEGER NOT NULL DEFAULT 0,
  rejected INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;
INSERT INTO domain_decisions (domain, approved, rejected)
  SELECT email_domain, SUM(decision = 'approved'), SUM(decision IN ('rejected', 'revoked'))
  FROM approval_requests WHERE email_domain IS NOT NULL AND decided_at IS NOT NULL GROUP BY email_domain;

-- Two-person rule: an action against an admin-level account is proposed by one admin and confirmed
-- by a different one. When no other admin can confirm, the proposer may confirm alone only after
-- self_confirm_after (a delay during which every admin and the escalation contact are alerted).
CREATE TABLE admin_proposals (
  id                 TEXT PRIMARY KEY,
  action             TEXT NOT NULL CHECK (action IN ('suspend')),
  target_id          TEXT NOT NULL REFERENCES users (id),
  proposed_by        TEXT NOT NULL,
  proposed_at        INTEGER NOT NULL,
  expires_at         INTEGER NOT NULL,
  self_confirm_after INTEGER,
  note               TEXT,
  reason             TEXT,
  decided_by         TEXT,
  decided_at         INTEGER,
  -- confirmed | cancelled | expired
  outcome            TEXT
);
CREATE UNIQUE INDEX admin_proposals_one_open ON admin_proposals (action, target_id) WHERE decided_at IS NULL;
