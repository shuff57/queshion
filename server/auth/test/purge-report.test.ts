// D5: every purge report field counts the table it is named after.
import { expect, it } from "vitest";
import { env, Harness } from "./helpers.js";

it("purgeExpired reports each kind of row under its own name", async () => {
  const t = new Harness();
  t.clock = Date.UTC(2031, 0, 1);
  await t.auth.purgeExpired(); // clear anything stale left by other tests in this file's storage
  const now = t.clock;
  const old = now - 3 * 86400_000;
  const uid = crypto.randomUUID();
  await env.DB.prepare("INSERT INTO users (id, email, trust, created_at, email_verified_at) VALUES (?, ?, 'pending', ?, ?)")
    .bind(uid, `p-${uid}@school.example.org`, old, old)
    .run();
  const rows: [string, number][] = [];
  const add = async (sql: string, n: number, bind: (i: number) => unknown[]) => {
    for (let i = 0; i < n; i++) await env.DB.prepare(sql).bind(...bind(i)).run();
  };
  await add("INSERT INTO rate_limits (key, window_start, count, expires_at) VALUES (?, ?, 1, ?)", 1, (i) => [`purge-test-${uid}-${i}`, old, old]);
  await add(
    "INSERT INTO sessions (id, family_id, user_id, refresh_hash, created_at, auth_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    2,
    (i) => [crypto.randomUUID(), crypto.randomUUID(), uid, `rh-${uid}-${i}`, old, old, old],
  );
  await add(
    "INSERT INTO device_enrolments (id, user_id, code_hash, epoch, created_at, expires_at, used_at) VALUES (?, ?, ?, 0, ?, ?, ?)",
    3,
    (i) => [crypto.randomUUID(), uid, `ch-${uid}-${i}`, old, old, old],
  );
  await add(
    "INSERT INTO email_tokens (id, user_id, kind, token_hash, created_at, expires_at, used_at) VALUES (?, ?, 'reset', ?, ?, ?, ?)",
    4,
    (i) => [crypto.randomUUID(), uid, `th-${uid}-${i}`, old, old, old],
  );
  await add(
    "INSERT INTO users (id, trust, created_at, deleted_at, deleted_email_hmac, deleted_email_hmac_expires_at) VALUES (?, 'pending', ?, ?, ?, ?)",
    5,
    (i) => [crypto.randomUUID(), old, old, `hmac-${uid}-${i}`, old],
  );
  await add("INSERT INTO users (id, email, trust, created_at) VALUES (?, ?, 'unverified', ?)", 6, (i) => [
    crypto.randomUUID(),
    `u-${uid}-${i}@school.example.org`,
    now - 8 * 86400_000,
  ]);
  void rows;
  expect(await t.auth.purgeExpired()).toEqual({
    rateLimits: 1,
    sessions: 2,
    deviceEnrolments: 3,
    emailTokens: 4,
    tombstoneHashes: 5,
    unverifiedAccounts: 6,
  });
});
