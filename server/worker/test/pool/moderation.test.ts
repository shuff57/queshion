// Delete with 14-day undo, reports and holds, student data, DMCA takedown / counter-notice /
// put-back / expiry, account deletion, and the cron's GC. Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { addBusinessDays, federalHolidays, isBusinessDay } from "../../src/pool/bizdays.ts";
import { onDeleteContent, onSuspendContent } from "../../src/cleanup.ts";
import { bankPayload, genQuestion, png, publish, rng, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;

async function setup() {
  const w = new World();
  const a = w.user("teacher", "Ms. A");
  const b = w.user("teacher", "Mr. B");
  const admin = w.user("admin", "Admin");
  const img = await upload(w, a, png(6, 6, 4));
  const r = rng(21);
  const res = await publish(w, a, bankPayload([genQuestion(r, "Mc", 0, { sha: img.sha, ext: "png" }), genQuestion(r, "Short", 1)]), { hideKey: true, imageOk: [0] });
  assert.equal(res.status, 201);
  return { w, a, b, admin, img, id: res.body.item_id as string };
}
const seen = async (w: World, u: User, id: string) => (await w.json("GET", `/items/${id}`, { user: u })).status === 200;

test("author delete: hidden at once, undo within 14 days, then purged (bodies, rows, blobs; tombstone kept)", async () => {
  const { w, a, b, img, id } = await setup();
  await w.json("POST", `/items/${id}/import`, { user: b });
  const del = await w.json<{ undo_until: number }>("DELETE", `/items/${id}`, { user: a });
  assert.equal(del.body.undo_until, w.clock + 14 * DAY);
  assert.equal(await seen(w, b, id), false);
  assert.equal(await seen(w, a, id), false);
  const again = await w.json<{ undo_until: number }>("DELETE", `/items/${id}`, { user: a });
  assert.equal(again.body.undo_until, del.body.undo_until, "a second delete never extends the window");
  const mine = await w.json<{ items: { state: string; undo_until: number }[] }>("GET", "/items/mine", { user: a });
  assert.deepEqual([mine.body.items[0]!.state, mine.body.items[0]!.undo_until], ["deleted", del.body.undo_until]);
  const v = await w.json<{ items: { status: string; reason: string }[] }>("POST", "/items/versions", { user: b, json: { ids: [id] } });
  assert.deepEqual(v.body.items[0], { id, status: "removed", reason: "deleted" });
  // Someone else cannot undo it.
  assert.equal((await w.json("POST", `/items/${id}/undelete`, { user: b })).status, 404);
  assert.equal((await w.json("POST", `/items/${id}/undelete`, { user: a })).status, 200);
  assert.equal(await seen(w, b, id), true);
  // Delete again and let the window pass: undo is refused and the daily cron purges.
  await w.json("DELETE", `/items/${id}`, { user: a });
  w.advance(14 * DAY + 1);
  assert.equal((await w.json("POST", `/items/${id}/undelete`, { user: a })).status, 404);
  const report = await w.pool.daily();
  assert.deepEqual(report, { purged: 1, restored: 0, quarantineExpired: 0 });
  assert.equal(w.r2.keys("items/").length, 0);
  // The importer's copy-on-import hold keeps the image for them (30 days from the import).
  assert.equal(w.r2.keys("blobs/").length, 1);
  for (const t of ["item_publishes", "item_questions", "item_blobs", "item_tags", "question_ids"]) assert.equal(w.q(`SELECT * FROM ${t}`).length, 0, t);
  w.advance(17 * DAY);
  await w.pool.hourly();
  assert.equal(w.r2.keys("blobs/").length, 0, "collected once nothing references or holds it");
  assert.deepEqual(w.q(`SELECT id, removed_reason, purged_at IS NOT NULL AS purged FROM items`), [{ id, removed_reason: "author_deleted", purged: 1 }]);
  assert.equal(w.q(`SELECT * FROM imports`).length, 1, "imports kept for counts");
  assert.equal((await w.json("GET", `/items/${id}/blobs/${img.sha}`, { user: a })).status, 404);
});

test("reports: one open per reporter and reason, 3 distinct teachers hold, student data holds at once and mails the admin, the reporter only hears thanks", async () => {
  const { w, a, b, admin, id } = await setup();
  assert.equal((await w.json("POST", `/items/${id}/report`, { user: a, json: { reason: "other" } })).body.error, "own_item");
  assert.equal((await w.json("POST", `/items/${id}/report`, { user: b, json: { reason: "spam" } })).status, 400);
  assert.equal((await w.json("POST", `/items/${id}/report`, { user: b, json: { reason: "other", note: "bad‮text" } })).body.field, "note");
  const thanks = await w.json("POST", `/items/${id}/report`, { user: b, json: { reason: "wrong_answer", note: "Q2 is wrong" } });
  assert.deepEqual([thanks.status, thanks.body], [202, { ok: true, message: "Thanks, the admin will review it." }]);
  // The same reporter again: no second report, no progress towards the threshold.
  for (let i = 0; i < 3; i++) await w.json("POST", `/items/${id}/report`, { user: b, json: { reason: "wrong_answer" } });
  assert.equal(w.q(`SELECT * FROM reports`).length, 1);
  const c = w.user();
  await w.json("POST", `/items/${id}/report`, { user: c, json: { reason: "inappropriate" } });
  assert.equal(await seen(w, b, id), true, "2 reporters: still visible");
  const d = w.user();
  await w.json("POST", `/items/${id}/report`, { user: d, json: { reason: "other" } });
  assert.equal(await seen(w, b, id), false, "3 distinct reporters: held");
  assert.equal(await seen(w, a, id), true, "the author still sees it");
  // The admin's view: grouped by item, reporters with their history.
  const q = await w.admin<{ items: { item_id: string; reports: { reporter_reports: number }[] }[] }>(admin, "GET", "/admin/pool/reports");
  assert.equal(q.body.items[0]!.item_id, id);
  assert.equal(q.body.items[0]!.reports.length, 3);
  // Lifting the hold dismisses those reports; the item shows again.
  assert.equal((await w.admin(admin, "POST", "/admin/pool/release", { item_id: id, reason: "reports" })).status, 200);
  assert.equal(await seen(w, b, id), true);
  assert.equal(w.q(`SELECT * FROM reports WHERE resolved_at IS NULL`).length, 0);
  // Student data: ONE report hides it and mails the admin immediately.
  const e = w.user();
  await w.json("POST", `/items/${id}/report`, { user: e, json: { reason: "student_data" } });
  assert.equal(await seen(w, b, id), false);
  assert.equal(w.notices.filter((n) => n.kind === "student_data_report").length, 1);
  // A report needs a visible item: reporting a held item is the same 404.
  assert.equal((await w.json("POST", `/items/${id}/report`, { user: w.user(), json: { reason: "other" } })).status, 404);
  // Reports are capped per day.
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'reports', 1, 0, 'x')`).run();
  const f = w.user();
  const other = await publish(w, a, bankPayload([genQuestion(rng(1), "Tf", 0)]));
  const other2 = await publish(w, a, bankPayload([genQuestion(rng(2), "Tf", 0)]));
  assert.equal((await w.json("POST", `/items/${other.body.item_id as string}/report`, { user: f, json: { reason: "other" } })).status, 202);
  assert.equal((await w.json("POST", `/items/${other2.body.item_id as string}/report`, { user: f, json: { reason: "other" } })).status, 429);
});

test("student data confirmed: purged immediately, image hashes blocklisted, importers told 'privacy', a re-upload is held", async () => {
  const { w, a, b, admin, img, id } = await setup();
  await w.json("POST", `/items/${id}/import`, { user: b });
  await w.json("POST", `/items/${id}/report`, { user: b, json: { reason: "student_data" } });
  assert.equal((await w.purge(admin, id)).status, 200);
  assert.equal(w.r2.keys("items/").length, 0);
  assert.equal(w.r2.keys("blobs/").length, 0, "no quarantine: the bytes are gone now");
  assert.deepEqual(w.q(`SELECT kind, hash FROM privacy_blocks WHERE kind = 'blob'`), [{ kind: "blob", hash: img.sha }]);
  assert.equal(w.q(`SELECT * FROM privacy_blocks WHERE kind = 'match'`).length, 2, "the question text is blocked too");
  assert.equal(w.q(`SELECT * FROM blocked_hashes`).length, 0, "kept apart from DMCA blocks");
  const v = await w.json<{ items: unknown[] }>("POST", "/items/versions", { user: b, json: { ids: [id] } });
  assert.deepEqual(v.body.items[0], { id, status: "removed", reason: "privacy" });
  // A re-upload of the same bytes is refused, with the neutral answer any refused image gets.
  const again = await upload(w, a, png(6, 6, 4));
  assert.deepEqual([again.status, again.body.error, again.body.reason], [422, "image_refused", "not_accepted"]);
  // The purged item's own question text published again: accepted, but held for review.
  const r21 = rng(21);
  genQuestion(r21, "Mc", 0, { sha: img.sha, ext: "png" });
  const re = await publish(w, a, bankPayload([genQuestion(r21, "Short", 1)]));
  assert.equal(re.status, 201);
  assert.equal("held" in re.body, false, "the publish result says nothing (no blocklist oracle)");
  assert.equal(await seen(w, b, re.body.item_id as string), false);
  const mine = await w.json<{ items: { id: string; state: string }[] }>("GET", "/items/mine", { user: a });
  assert.equal(mine.body.items.find((x) => x.id === re.body.item_id)!.state, "held_for_review");
});

test("DMCA: takedown disables at once and quarantines bodies; counter-notice; put-back after 10 business days; strikes and blocklist lifted", async () => {
  const { w, a, b, admin, id } = await setup();
  await w.json("POST", `/items/${id}/import`, { user: b });
  const deficient = await w.admin<{ notice_id: string; status: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "Pub Co", complainant_contact: "legal@pub.example", work_desc: "Textbook ch. 3", material_desc: id, has_signature: true, has_good_faith: true, has_perjury_stmt: false });
  assert.equal(deficient.body.status, "deficient");
  assert.equal((await w.takedown(admin, deficient.body.notice_id, id)).status, 409);
  const n = await w.admin<{ notice_id: string; status: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "Pub Co", complainant_contact: "legal@pub.example", work_desc: "Textbook ch. 3", material_desc: id, has_signature: true, has_good_faith: true, has_perjury_stmt: true });
  assert.equal(n.body.status, "open");
  const t = await w.takedown<{ action_id: string }>(admin, n.body.notice_id, id);
  assert.equal(t.status, 200);
  assert.equal(await seen(w, b, id), false);
  assert.equal(await seen(w, a, id), false);
  assert.equal(w.r2.keys("items/").length, 0, "nothing left under the normal keys");
  assert.equal(w.r2.keys("quarantine/").length, 2, "full and stripped bodies quarantined");
  assert.ok(w.q<{ full_key: string }>(`SELECT full_key FROM item_publishes`).every((x) => x.full_key.startsWith("quarantine/")));
  assert.equal((await w.json("DELETE", `/items/${id}`, { user: a })).body.error, "under_moderation");
  assert.equal(w.notices.filter((x) => x.kind === "takedown_author").length, 1);
  const mine = await w.json<{ items: { state: string; counter_notice: string }[] }>("GET", "/items/mine", { user: a });
  assert.deepEqual([mine.body.items[0]!.state, mine.body.items[0]!.counter_notice], ["taken_down", "available"]);
  const v = await w.json<{ items: unknown[] }>("POST", "/items/versions", { user: b, json: { ids: [id] } });
  assert.deepEqual(v.body.items[0], { id, status: "removed", reason: "takedown" });
  assert.equal(w.q(`SELECT * FROM strikes WHERE voided_at IS NULL`).length, 1);
  assert.ok(w.q(`SELECT * FROM blocked_hashes WHERE lifted_at IS NULL`).length >= 2);
  // The admin can still read the full form from quarantine.
  const adm = await w.admin<{ payload_full: { bank: unknown } }>(admin, "GET", "/admin/pool/item", null, `?id=${id}`);
  assert.ok(adm.body.payload_full.bank);
  // Counter-notice: only the author; incomplete is refused with the missing fields.
  assert.equal((await w.json("POST", `/items/${id}/counter-notice`, { user: b, json: {} })).status, 404);
  const inc = await w.json<{ missing: string[] }>("POST", `/items/${id}/counter-notice`, { user: a, json: { signature: "Ms. A" } });
  assert.equal(inc.status, 400);
  assert.deepEqual(inc.body.missing.sort(), ["accept_service", "address", "consent_jurisdiction", "material_desc", "name", "perjury_stmt", "phone"]);
  const full = { signature: "Ms. A", material_desc: `Item ${id}`, perjury_stmt: true, name: "Ann A", address: "1 School Rd", phone: "555-0100", consent_jurisdiction: true, accept_service: true };
  const cn = await w.json<{ restore_not_before: number; restore_by: number }>("POST", `/items/${id}/counter-notice`, { user: a, json: full });
  assert.equal(cn.status, 202);
  assert.equal((await w.json("POST", `/items/${id}/counter-notice`, { user: a, json: full })).status, 404, "one counter-notice per action");
  assert.equal(w.notices.filter((x) => x.kind === "counter_notice").length, 1, "forwarded to the complainant at once");
  // The put-back window: 10 to 14 business days.
  let bd = 0;
  for (let d = Math.floor(w.clock / DAY) * DAY + DAY; d < cn.body.restore_not_before; d += DAY) if (isBusinessDay(d)) bd++;
  assert.equal(bd, 9, "the restore day is the 10th business day");
  assert.ok(cn.body.restore_by > cn.body.restore_not_before);
  w.advance(cn.body.restore_not_before - w.clock - 1000);
  assert.equal((await w.pool.daily()).restored, 0, "not before 10 business days");
  w.advance(2000);
  assert.equal((await w.pool.daily()).restored, 1);
  assert.equal(await seen(w, b, id), true);
  assert.equal(w.r2.keys("quarantine/").length, 0);
  assert.equal(w.r2.keys("items/").length, 2);
  assert.equal(w.q(`SELECT * FROM strikes WHERE voided_at IS NULL`).length, 0, "a put-back is not an upheld strike");
  assert.equal(w.q(`SELECT * FROM blocked_hashes WHERE lifted_at IS NULL`).length, 0);
  const imp = await w.json("POST", `/items/${id}/import`, { user: b });
  assert.equal(imp.status, 200, "importable again");
});

test("DMCA: without a counter-notice the quarantine is purged after 30 days; a court action keeps it disabled; 3 strikes flag the account", async () => {
  const { w, a, admin, id } = await setup();
  const notice = async () =>
    (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "P", complainant_contact: "Postal: 1 Main St", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
  await w.takedown(admin, await notice(), id);
  w.advance(29 * DAY);
  assert.equal((await w.pool.daily()).purged, 0);
  w.advance(2 * DAY);
  const rep = await w.pool.daily();
  assert.equal(rep.quarantineExpired, 1);
  assert.equal(rep.purged, 1);
  assert.equal(w.r2.keys("quarantine/").length, 0);
  // Court action: counter-notice received, but the complainant sued; nothing is restored.
  const r = rng(4);
  const two = await publish(w, a, bankPayload([genQuestion(r, "Tf", 0)]));
  const t2 = await w.takedown<{ action_id: string }>(admin, await notice(), two.body.item_id as string);
  await w.json("POST", `/items/${two.body.item_id as string}/counter-notice`, { user: a, json: { signature: "A", material_desc: "m", perjury_stmt: true, name: "A", address: "x", phone: "1", consent_jurisdiction: true, accept_service: true } });
  assert.equal(w.notices.filter((n) => n.kind === "counter_notice").length, 1, "a postal contact still produces a notice (the mailer routes it to the admin)");
  assert.equal((await w.admin(admin, "POST", "/admin/dmca/court-action", { action_id: t2.body.action_id })).status, 200);
  w.advance(60 * DAY);
  const rep2 = await w.pool.daily();
  assert.equal(rep2.restored, 0);
  assert.equal(rep2.purged, 0, "kept disabled until resolved");
  // A third upheld strike in 12 months flags the account for suspension.
  const three = await publish(w, a, bankPayload([genQuestion(r, "Short", 0)]));
  await w.takedown(admin, await notice(), three.body.item_id as string);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'repeat_infringer' AND user_id = ?`, a.id).length, 1);
  assert.equal(w.notices.filter((n) => n.kind === "repeat_infringer").length, 1);
  // Withdrawal puts everything under the notice back.
  const withdrawn = await w.admin<{ restored: number }>(admin, "POST", "/admin/dmca/withdraw", { notice_id: w.q<{ notice_id: string }>(`SELECT notice_id FROM dmca_actions WHERE id = ?`, t2.body.action_id)[0]!.notice_id });
  assert.equal(withdrawn.body.restored, 1);
  assert.equal(await seen(w, w.user(), two.body.item_id as string), true);
});

test("admin removal (14 days) and holds; limits are admin-settable per user or site-wide; every write is audited", async () => {
  const { w, b, admin, id } = await setup();
  assert.equal((await w.admin(admin, "POST", "/admin/pool/hold", { item_id: id, note: "checking" })).status, 200);
  assert.equal(await seen(w, b, id), false);
  assert.equal((await w.admin(admin, "POST", "/admin/pool/release", { item_id: id, reason: "admin" })).status, 200);
  assert.equal(await seen(w, b, id), true);
  assert.equal((await w.admin(admin, "PUT", "/admin/pool/limits", { user_id: b.id, metric: "imports", value: 0 })).status, 200);
  assert.equal((await w.json("POST", `/items/${id}/import`, { user: b })).status, 429);
  assert.equal((await w.admin(admin, "PUT", "/admin/pool/limits", { user_id: "*", metric: "nope", value: 1 })).status, 400);
  assert.equal((await w.admin(admin, "POST", "/admin/pool/remove", { item_id: id, note: "policy" })).status, 200);
  assert.equal(await seen(w, b, id), false);
  assert.equal(w.q<{ purge_after: number }>(`SELECT purge_after FROM items`)[0]!.purge_after, w.clock + 14 * DAY);
  assert.equal((await w.admin(admin, "POST", "/admin/pool/hold", { item_id: "not-an-id" })).status, 400);
  const actions = w.q<{ action: string }>(`SELECT action FROM pool_audit WHERE actor_kind = 'admin' ORDER BY at, rowid`).map((x) => x.action);
  assert.deepEqual(actions, ["item.hold", "item.release", "limits.set", "item.remove"]);
  assert.ok(w.adminAudits.length >= 4, "and the auth package's audit log");
  assert.throws(() => w.sql.exec(`DELETE FROM pool_audit`), /append-only/);
  assert.throws(() => w.sql.exec(`UPDATE pool_audit SET note = NULL`), /append-only/, "notes are nulled only for a deleted account");
});

test("account deletion: items removed and purged in 14 days, reports anonymised, notes nulled; suspension writes only an audit row", async () => {
  const { w, a, b, id } = await setup();
  await w.json("POST", `/items/${id}/report`, { user: b, json: { reason: "other", note: "hmm" } });
  w.sql.prepare(`INSERT INTO pool_audit (id, at, actor_kind, action, target_user_id, note) VALUES ('n1', 1, 'admin', 'x', ?, 'about b')`).run(b.id);
  assert.equal(onSuspendContent(w.db, a.id, w.clock).length, 1);
  // The auth package tombstones the account, then runs the hook's statements.
  w.sql.prepare(`UPDATE users SET email = NULL, pw_hash = NULL, display_name = NULL, profile = NULL, deleted_at = ? WHERE id IN (?, ?)`).run(w.clock, a.id, b.id);
  await w.db.batch(onDeleteContent(w.db, a.id, w.clock));
  await w.db.batch(onDeleteContent(w.db, b.id, w.clock));
  assert.deepEqual(w.q(`SELECT removed_reason, purge_after FROM items`), [{ removed_reason: "account_deleted", purge_after: w.clock + 14 * DAY }]);
  assert.deepEqual(w.q(`SELECT reporter_id, note FROM reports`), [{ reporter_id: null, note: "hmm" }]);
  assert.equal(w.q<{ note: string | null }>(`SELECT note FROM pool_audit WHERE id = 'n1'`)[0]!.note, null);
  assert.equal(w.q(`SELECT * FROM pending_blobs`).length, 0);
  w.advance(14 * DAY + 1);
  assert.equal((await w.pool.daily()).purged, 1);
});

test("cron GC: abandoned staged publishes after an hour, expired pending uploads, unreferenced blobs; a live upload survives", async () => {
  const w = new World();
  const a = w.user();
  const img = await upload(w, a, png(3, 3, 1));
  // A publish whose request died after staging (the R2 write threw).
  w.r2.onPut = async () => {
    throw new Error("network");
  };
  const r = await publish(w, a, bankPayload([genQuestion(rng(1), "Tf", 0)]));
  assert.equal(r.status, 500);
  assert.equal(w.q(`SELECT * FROM item_publishes`).length, 0, "a failed write cleans up after itself");
  // Simulate a crash mid-staging: a staged row with no live request.
  w.sql.prepare(`INSERT INTO item_publishes (publish_id, item_id, author_id, version, status, schema_version, hide_key, license, attested_at, attest_text_rev, acks_json, request_sha256, stage_token, full_key, full_bytes, title, subject, course, grade, kinds_mask, question_count, has_images, created_at) VALUES ('stale-publish-id-0000000', 'i', ?, 1, 'staged', 1, 0, 'CC0-1.0', 1, 'r', '{}', 'h', 't', 'items/i/x.json', 10, '', '', '', '', 0, 0, 0, ?)`).run(a.id, w.clock);
  await w.r2.put("items/i/x.json", "{}");
  w.advance(30 * 60_000);
  assert.equal(((await w.pool.hourly()) as { staged: number }).staged, 0, "younger than an hour: kept");
  w.advance(31 * 60_000);
  assert.equal(((await w.pool.hourly()) as { staged: number }).staged, 1);
  assert.equal(w.r2.objects.has("items/i/x.json"), false);
  // The pending upload: kept while live (and the hour after), then collected.
  assert.equal(w.r2.objects.has(`blobs/${img.sha}`), true);
  w.advance(23 * 3600_000); // now = upload + 24 h 01 min: expired 1 min ago, inside the grace hour
  await w.pool.hourly();
  assert.equal(w.r2.objects.has(`blobs/${img.sha}`), true, "within the grace hour");
  w.advance(2 * 3600_000);
  await w.pool.hourly();
  assert.equal(w.r2.objects.has(`blobs/${img.sha}`), false);
  // A re-upload after GC works (content-addressed, repaired).
  assert.equal((await upload(w, a, png(3, 3, 1))).status, 201);
  assert.equal(w.r2.objects.has(`blobs/${img.sha}`), true);
});

test("business days skip weekends and observed federal holidays", () => {
  const h2026 = federalHolidays(2026);
  for (const d of ["2026-01-01", "2026-01-19", "2026-02-16", "2026-05-25", "2026-06-19", "2026-07-03", "2026-09-07", "2026-10-12", "2026-11-11", "2026-11-26", "2026-12-25"]) assert.ok(h2026.has(d), d);
  assert.ok(federalHolidays(2027).has("2027-12-31"), "New Year 2028 is a Saturday: observed Fri Dec 31, 2027");
  // A counter-notice on Fri 2026-11-20: Thanksgiving is skipped, the 10th business day is Mon Dec 7.
  const end = addBusinessDays(Date.UTC(2026, 10, 20, 15), 10);
  assert.equal(new Date(end).toISOString().slice(0, 10), "2026-12-07");
});
