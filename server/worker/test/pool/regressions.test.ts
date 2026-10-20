// Regressions for the findings of the security review (2026-10-20). Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { checkImage } from "../../src/pool/image.ts";
import { purgeItem } from "../../src/pool/moderation.ts";
import { makeCtx } from "../../src/pool/shared.ts";
import { bankPayload, chunk, concat, core, enc, genQuestion, png, publish, rng, SECRET, World } from "./harness.ts";

const DAY = 86_400_000;
const bank = (seed: number, n = 2) => bankPayload(Array.from({ length: n }, (_, i) => genQuestion(rng(seed), "Tf", i)));
function v2(payload: string, item: string, pid: string, qids: string[]) {
  const p = JSON.parse(payload);
  p.schema_version = 2;
  p.bank.questions.forEach((q: Record<string, unknown>, i: number) => (q.published = { item_id: item, qid: qids[i], publish_id: pid, version: 1 }));
  return JSON.stringify(p);
}

test("a committed publish_id stays reserved after retention: reusing it is a 409 and the author's qids survive", async () => {
  const w = new World();
  const victim = w.user();
  const attacker = w.user();
  const p0 = bank(1);
  const v1 = await publish(w, victim, p0);
  const item = v1.body.item_id as string;
  const qids = v1.body.qids as string[];
  const p1 = v2(p0, item, v1.body.publish_id as string, qids);
  const v2r = await publish(w, victim, p1, { base: v1.body.publish_id as string }, item);
  const v3 = await publish(w, victim, p1, { base: v2r.body.publish_id as string }, item);
  assert.equal(w.q(`SELECT * FROM item_publishes WHERE publish_id = ?`, v1.body.publish_id).length, 0, "v1 retired by retention");
  const reuse = await publish(w, attacker, bank(2), { publishId: v1.body.publish_id as string });
  assert.deepEqual([reuse.status, reuse.body], [409, { error: "publish_id_conflict" }]);
  w.advance(2 * 3600_000);
  await w.pool.hourly();
  const v4 = await publish(w, victim, p1, { base: v3.body.publish_id as string }, item);
  assert.deepEqual(v4.body.qids, qids);
});

test("a takedown takes over an author-deleted item: undelete refused, strike and blocklist recorded, a withdrawal returns it to deleted (critic E7/S5)", async () => {
  const w = new World();
  const a = w.user();
  const admin = w.user("admin");
  const it = (await publish(w, a, bank(3))).body.item_id as string;
  await w.json("DELETE", `/items/${it}`, { user: a });
  const n = await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "C", complainant_contact: "c@x.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true });
  assert.equal((await w.takedown(admin, n.body.notice_id, it)).status, 200);
  assert.equal((await w.json("POST", `/items/${it}/undelete`, { user: a })).status, 404, "undelete refused while the takedown stands");
  assert.equal(w.q(`SELECT * FROM strikes`).length, 1);
  assert.ok(w.q(`SELECT * FROM blocked_hashes`).length >= 1);
  await w.admin(admin, "POST", "/admin/dmca/withdraw", { notice_id: n.body.notice_id });
  assert.equal((await w.json("GET", `/items/${it}`, { user: w.user() })).status, 404, "never resurrected");
  assert.equal(w.q<{ removed_reason: string }>(`SELECT removed_reason FROM items`)[0]!.removed_reason, "author_deleted");
  assert.equal(w.r2.keys("quarantine/").length, 0);
  // Back in its 14-day window: the author may now undo their own deletion, or let it purge.
  assert.equal((await w.json("POST", `/items/${it}/undelete`, { user: a })).status, 200);
});

test("no payload rides in fixed-size PNG chunks or the JPEG APP0/APP14 segments", async () => {
  assert.equal((await checkImage(png(4, 4, 0, [chunk("gAMA", new Uint8Array(40_000))])) as { refused: string }).refused, "png_chunk_size");
  assert.equal((await checkImage(png(4, 4, 0, [chunk("PLTE", new Uint8Array(6))])) as { refused: string }).refused, "png_plte_greyscale");
  const seg = (m: number, d: Uint8Array) => concat(new Uint8Array([0xff, m, (d.length + 2) >> 8, (d.length + 2) & 255]), d);
  const body = concat(seg(0xc0, new Uint8Array([8, 0, 8, 0, 8, 1, 1, 0x11, 0])), seg(0xda, new Uint8Array([1, 1, 0, 0, 63, 0])), new Uint8Array([1, 2, 0xff, 0xd9]));
  const jfif = (extra: Uint8Array, thumb = [0, 0]) => concat(new Uint8Array([0xff, 0xd8]), seg(0xe0, concat(enc("JFIF\0"), new Uint8Array([1, 1, 0, 0, 1, 0, 1, ...thumb]), extra)), body);
  assert.ok("contentType" in await checkImage(jfif(new Uint8Array(0))));
  assert.equal((await checkImage(jfif(new Uint8Array(32_000))) as { refused: string }).refused, "jpeg_app0");
  assert.equal((await checkImage(jfif(new Uint8Array(3 * 4), [2, 2])) as { refused: string }).refused, "jpeg_app0", "an embedded thumbnail");
});

test("a reporter whose student_data report was dismissed cannot re-hide the item by reporting again", async () => {
  const w = new World();
  const a = w.user();
  const b = w.user();
  const admin = w.user("admin");
  const it = (await publish(w, a, bank(4))).body.item_id as string;
  await w.json("POST", `/items/${it}/report`, { user: b, json: { reason: "student_data" } });
  await w.admin(admin, "POST", "/admin/pool/release", { item_id: it, reason: "student_data" });
  const again = await w.json("POST", `/items/${it}/report`, { user: b, json: { reason: "student_data" } });
  assert.equal(again.status, 202, "still thanked");
  assert.equal((await w.json("GET", `/items/${it}`, { user: w.user() })).status, 200);
  assert.equal(w.notices.filter((n) => n.kind === "student_data_report").length, 1, "and no second mail");
});

test("purgeItem never touches a live item (a restore that raced the purge loop)", async () => {
  const w = new World();
  const a = w.user();
  const it = (await publish(w, a, bank(5))).body.item_id as string;
  const c = makeCtx({ db: w.db, r2: w.r2 as unknown as R2Bucket, core, secret: SECRET, allowedOrigins: [], authenticate: async () => null, now: () => w.clock, rateStore: { hit: async () => ({ allowed: true, retryAfter: 0 }) }, ipBucket: (x) => x }, w.r2 as unknown as R2Bucket);
  assert.equal(await purgeItem(c, it, { id: null, kind: "system" }, "test"), false);
  assert.equal((await w.json("GET", `/items/${it}`, { user: w.user() })).status, 200);
  assert.equal(w.r2.keys("items/").length, 1);
});

test("removal releases the quota charge; suspended reporters do not count; PUT obeys the miss guard; the cron catches up account deletions", async () => {
  const w = new World();
  const a = w.user();
  const it = (await publish(w, a, bank(6))).body.item_id as string;
  const used = async () => (await w.json<{ storage: { used: number } }>("GET", "/items/mine", { user: a })).body.storage.used;
  assert.ok((await used()) > 1024);
  await w.json("DELETE", `/items/${it}`, { user: a });
  assert.equal(await used(), 0);
  await w.json("POST", `/items/${it}/undelete`, { user: a });
  // Two teachers and a suspended one: no hold.
  const rs = [w.user(), w.user(), w.user()];
  for (const r of rs) await w.json("POST", `/items/${it}/report`, { user: r, json: { reason: "other" } });
  assert.equal(w.q(`SELECT * FROM item_holds`).length, 1);
  w.sql.prepare(`DELETE FROM item_holds`).run();
  w.sql.prepare(`UPDATE users SET suspended_at = 1 WHERE id = ?`).run(rs[2]!.id);
  await w.json("POST", `/items/${it}/report`, { user: w.user(), json: { reason: "wrong_answer" } });
  assert.equal(w.q(`SELECT * FROM item_holds`).length, 1, "3 teachers in good standing: held");
  w.sql.prepare(`DELETE FROM item_holds`).run();
  // Miss guard on PUT.
  const b = w.user();
  for (let i = 0; i < 100; i++) await w.call("GET", `/items/${i.toString(16).padStart(32, "0")}`, { user: b });
  assert.equal((await publish(w, b, bank(7), {}, it)).status, 429);
  // The account is deleted but its content hook never ran: the daily cron catches up.
  w.sql.prepare(`UPDATE users SET email = NULL, pw_hash = NULL, display_name = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, a.id);
  await w.pool.daily();
  assert.deepEqual(w.q(`SELECT removed_reason FROM items`), [{ removed_reason: "account_deleted" }]);
  w.advance(15 * DAY);
  assert.equal(((await w.pool.daily()) as { purged: number }).purged, 1);
});
