-- A short lease on a mail token, claimed atomically before the password hash, so many concurrent
-- requests with one valid token cost one hash, not one each.
ALTER TABLE email_tokens ADD COLUMN claimed_until INTEGER;

-- Nonce of the device token issued at the account's most recent successful login (or reset /
-- activation). That device is exempt from the all-devices failure cap, so stolen tokens cannot use
-- the shared cap to lock the owner's current device out.
ALTER TABLE users ADD COLUMN last_device TEXT;

-- One-time device enrolment codes, mailed when a reset or activation completes. A native app redeems
-- one together with the password to obtain a device token: the code proves control of the mailbox,
-- the password proves identity.
CREATE TABLE device_enrolments (
  id            TEXT PRIMARY KEY,
  user_id       TEXT NOT NULL REFERENCES users (id),
  -- SHA-256 of the code.
  code_hash     TEXT NOT NULL UNIQUE,
  -- users.device_epoch when issued; a later reset (or logout everywhere) voids the code.
  epoch         INTEGER NOT NULL,
  created_at    INTEGER NOT NULL,
  expires_at    INTEGER NOT NULL,
  used_at       INTEGER,
  claimed_until INTEGER,
  -- Wrong passwords tried with this code; it is burned after a few.
  failures      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX device_enrolments_user ON device_enrolments (user_id);
CREATE INDEX device_enrolments_expires ON device_enrolments (expires_at);
