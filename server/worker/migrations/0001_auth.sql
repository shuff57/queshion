-- d1-auth schema. Times are epoch milliseconds.

CREATE TABLE users (
  id                 TEXT PRIMARY KEY,
  -- Normalized (trimmed, lower-case). NULL once the account is deleted, so the address can
  -- register again; SQLite's UNIQUE allows any number of NULLs.
  email              TEXT UNIQUE,
  email_verified_at  INTEGER,
  -- PHC string: $argon2id$v=19$m=..,t=..,p=..$salt$tag (parameters travel with the hash).
  pw_hash            TEXT,
  -- The same parameters as JSON, for queries such as "who still has the old cost".
  pw_params          TEXT,
  display_name       TEXT,
  -- App-defined JSON (for example a school). Nulled on deletion.
  profile            TEXT,
  -- One of the configured trust levels. Not constrained here so the level names stay app-defined.
  trust              TEXT NOT NULL,
  created_at         INTEGER NOT NULL,
  -- Set = tombstone: every PII column above is NULL.
  deleted_at         INTEGER,
  -- Tombstone only: HMAC(server secret, email), kept for a limited time so content can be
  -- re-linked if the person signs up again. Nothing else identifying is kept.
  deleted_email_hmac TEXT,
  deleted_email_hmac_expires_at INTEGER,
  CHECK (deleted_at IS NULL OR (email IS NULL AND pw_hash IS NULL AND pw_params IS NULL
                               AND display_name IS NULL AND profile IS NULL))
);
CREATE INDEX users_deleted_email_hmac ON users (deleted_email_hmac) WHERE deleted_email_hmac IS NOT NULL;
CREATE INDEX users_unverified ON users (created_at) WHERE email_verified_at IS NULL AND deleted_at IS NULL;

-- One row per refresh token. A login starts a family; each rotation adds the successor row.
CREATE TABLE sessions (
  id           TEXT PRIMARY KEY,
  family_id    TEXT NOT NULL,
  user_id      TEXT NOT NULL REFERENCES users (id),
  -- SHA-256 of the refresh token. The token itself is never stored.
  refresh_hash TEXT NOT NULL UNIQUE,
  created_at   INTEGER NOT NULL,
  -- When the family's login happened; bounds the family's total life.
  auth_at      INTEGER NOT NULL,
  expires_at   INTEGER NOT NULL,
  rotated_at   INTEGER,
  revoked_at   INTEGER,
  user_agent   TEXT
);
CREATE INDEX sessions_family ON sessions (family_id);
CREATE INDEX sessions_user ON sessions (user_id);
CREATE INDEX sessions_expires ON sessions (expires_at);

CREATE TABLE email_tokens (
  id         TEXT PRIMARY KEY,
  user_id    TEXT NOT NULL REFERENCES users (id),
  kind       TEXT NOT NULL CHECK (kind IN ('verify', 'reset')),
  -- SHA-256 of the token.
  token_hash TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  used_at    INTEGER,
  -- Random per consuming request, so the rest of that request's batch can find exactly the
  -- row it consumed (a timestamp could collide with a row invalidated in the same millisecond).
  used_nonce TEXT UNIQUE
);
CREATE INDEX email_tokens_user ON email_tokens (user_id, kind);
CREATE INDEX email_tokens_expires ON email_tokens (expires_at);

-- One row per account awaiting or given a decision (written when the address is confirmed).
CREATE TABLE approvals (
  user_id      TEXT PRIMARY KEY REFERENCES users (id),
  requested_at INTEGER NOT NULL,
  decided_at   INTEGER,
  decided_by   TEXT REFERENCES users (id),
  decision     TEXT,
  note         TEXT
);
CREATE INDEX approvals_open ON approvals (requested_at) WHERE decided_at IS NULL;

-- Sliding-window counters. `key` is an HMAC; no raw IP or email is stored.
CREATE TABLE rate_limits (
  key          TEXT NOT NULL,
  window_start INTEGER NOT NULL,
  count        INTEGER NOT NULL,
  expires_at   INTEGER NOT NULL,
  PRIMARY KEY (key, window_start)
) WITHOUT ROWID;
CREATE INDEX rate_limits_expires ON rate_limits (expires_at);
