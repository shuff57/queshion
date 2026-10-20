// Run: npm test  (no dependencies)
import assert from "node:assert/strict";
import { test } from "node:test";
import { onDeleteContent } from "../src/cleanup.ts";
import { assertMailModeSafe, makeMailer } from "../src/mail.ts";

test("MAIL_MODE=log refuses a non-local APP_URL", () => {
  for (const APP_URL of ["https://app.example.org", "https://localhost.evil.org", "http://127.0.0.1.nip.io", "", undefined]) {
    assert.throws(() => makeMailer({ MAIL_MODE: "log", APP_URL }), /only allowed/, String(APP_URL));
  }
});

test("MAIL_MODE=log runs on localhost", async () => {
  for (const APP_URL of ["http://localhost:5173", "http://127.0.0.1:8080", "http://[::1]:3000"]) assertMailModeSafe({ MAIL_MODE: "log", APP_URL });
  const logs: string[] = [];
  const orig = console.log;
  console.log = (m: string) => void logs.push(m);
  try {
    await makeMailer({ MAIL_MODE: "log", APP_URL: "http://localhost:5173" }).send({ kind: "verify", to: "a@b.org", token: "t" });
  } finally {
    console.log = orig;
  }
  assert.match(logs[0]!, /localhost:5173\/account\/verify#token=t/);
});

test("other modes do not need a local APP_URL", () => {
  assert.doesNotThrow(() => makeMailer({ APP_URL: "https://app.example.org" }));
});

test("onDelete returns the pool cleanup statements (no longer a logging stub)", () => {
  const prepared: string[] = [];
  const db = { prepare: (sql: string) => ({ bind: () => (prepared.push(sql), {}) }) } as unknown as D1Database;
  const errs: string[] = [];
  const orig = console.error;
  console.error = (m: string) => void errs.push(m);
  try {
    assert.ok(onDeleteContent(db, "u1", 0).length >= 5);
  } finally {
    console.error = orig;
  }
  assert.deepEqual(errs, []);
  assert.ok(prepared.some((s) => /removed_reason = 'account_deleted'/.test(s)));
  assert.ok(prepared.some((s) => /UPDATE reports SET reporter_id = NULL/.test(s)));
});

test("every mail kind renders, including device enrolment", async () => {
  const logs: string[] = [];
  const orig = console.log;
  console.log = (m: string) => void logs.push(m);
  try {
    const mailer = makeMailer({ MAIL_MODE: "log", APP_URL: "http://localhost:5173" });
    for (const kind of ["verify", "reset", "signup_existing", "password_changed", "device_enrol"] as const)
      await mailer.send({ kind, to: "a@b.org", token: "tok", accountVerified: true });
  } finally {
    console.log = orig;
  }
  assert.equal(logs.length, 5);
  assert.match(logs[4]!, /account\/enrol#token=tok/);
});
