-- Known-device tokens are stateless (HMAC over user id, epoch and expiry). Bumping this epoch
-- revokes every one of an account's device tokens at once: on reset, logout everywhere,
-- confirmation and deletion.
ALTER TABLE users ADD COLUMN device_epoch INTEGER NOT NULL DEFAULT 0;
