// Regressions for the round-2 critics (critic4a e1-e8, critic4b s1-s10), each inverted to assert
// the fix. Run: npm test
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { deflateSync } from "node:zlib";
import { checkImage } from "../../src/pool/image.ts";
import { PUBLISH_CONTENT_TYPE } from "../../src/pool/shared.ts";
import { ACCOUNT_LOCK_MS } from "../../src/pool/publish.ts";
import { bankPayload, chunk, concat, enc, framed, genQuestion, png, publish, publishId, rng, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[], name = "copy") => JSON.stringify({ schema_version: 2, bank: { id: "x", name, questions: qs, created: 1, modified: 1 } });
const chainOf = (w: World, pid: string, pos = 0) => JSON.parse(w.q<{ chain_json: string }>(`SELECT chain_json FROM item_questions WHERE publish_id = ? AND pos = ?`, pid, pos)[0]!.chain_json);
const live = (w: World, u: User) => w.q<{ n: number | null }>(`SELECT SUM(question_count) AS n FROM items WHERE author_id = ? AND removed_at IS NULL`, u.id)[0]!.n ?? 0;
const tfBank = (seed: number) => bankPayload([0, 1, 2].map((i) => genQuestion(rng(seed), "Tf", i)));
/** The client's origin on each imported question (spec "Import"). */
const withOrigin = (r: any, itemId: string) =>
  r.body.payload.bank.questions.map((q: Q, i: number) => ({ ...q, origin: { item_id: itemId, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }));
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;

// ---- A6 / A7: caps -----------------------------------------------------------------------------------

test("E1/S6: undelete cannot take an account past the shared-question cap or the storage quota", async () => {
  const w = new World();
  const admin = w.user("admin");
  const a = w.user();
  await w.admin(admin, "PUT", "/admin/pool/limits", { user_id: a.id, metric: "shared_questions", value: 3 });
  const ids: string[] = [];
  for (let k = 0; k < 4; k++) {
    const p = await publish(w, a, tfBank(100 + k));
    assert.equal(p.status, 201, JSON.stringify(p.body));
    ids.push(p.body.item_id as string);
    await w.json("DELETE", `/items/${ids[k]}`, { user: a });
  }
  assert.equal((await w.json("POST", `/items/${ids[0]}/undelete`, { user: a })).status, 200);
  const over = await w.json("POST", `/items/${ids[1]}/undelete`, { user: a });
  assert.deepEqual([over.status, over.body.error, over.body.used, over.body.needed, over.body.limit], [422, "shared_questions_limit", 3, 3, 3]);
  assert.equal(live(w, a), 3);
  // Storage.
  const w2 = new World();
  const adm2 = w2.user("admin");
  const b = w2.user();
  const big = (s: number) => bankPayload(Array.from({ length: 30 }, (_, i) => genQuestion(rng(s), "Short", i)));
  const first = await publish(w2, b, big(5));
  const per = (await w2.json<{ storage: { used: number } }>("GET", "/items/mine", { user: b })).body.storage.used;
  await w2.admin(adm2, "PUT", "/admin/pool/limits", { user_id: b.id, metric: "storage_bytes", value: Math.floor(per * 1.5) });
  await w2.json("DELETE", `/items/${first.body.item_id}`, { user: b });
  const second = await publish(w2, b, big(6));
  assert.equal(second.status, 201);
  const q = await w2.json("POST", `/items/${first.body.item_id}/undelete`, { user: b });
  assert.equal(q.status, 413);
  assert.equal(q.body.error, "quota_exceeded");
  assert.ok((q.body.used as number) + (q.body.needed as number) > (q.body.limit as number));
  const mine = await w2.json<{ storage: { used: number; limit: number } }>("GET", "/items/mine", { user: b });
  assert.ok(mine.body.storage.used <= mine.body.storage.limit);
});

test("E3/E3b: concurrent new publishes cannot race past the shared-question cap", async () => {
  const w = new World();
  const admin = w.user("admin");
  const a = w.user();
  await w.admin(admin, "PUT", "/admin/pool/limits", { user_id: a.id, metric: "shared_questions", value: 3 });
  const res = await Promise.all([0, 1, 2, 3, 4].map((k) => publish(w, a, tfBank(300 + k))));
  assert.equal(res.filter((r) => r.status === 201).length, 1);
  assert.ok(res.filter((r) => r.status !== 201).every((r) => r.body.error === "shared_questions_limit" || r.status === 409));
  assert.equal(live(w, a), 3);
  // One landing in the middle of another's staging.
  const w2 = new World();
  const adm2 = w2.user("admin");
  const b = w2.user();
  await w2.admin(adm2, "PUT", "/admin/pool/limits", { user_id: b.id, metric: "shared_questions", value: 3 });
  let inner: { status: number } | null = null;
  w2.r2.onPut = async () => {
    w2.advance(ACCOUNT_LOCK_MS + 1); // past the per-account single flight: the cap must still hold
    inner = await publish(w2, b, tfBank(400));
  };
  const outer = await publish(w2, b, tfBank(401));
  assert.deepEqual([outer.status, inner!.status].sort(), [201, 422]);
  assert.equal(live(w2, b), 3);
});

// ---- A1: the modified flag never depends on a hidden key ---------------------------------------------

test("E2: a hidden key change does not show in the chain's modified flag (Eve cannot infer it)", async () => {
  for (const flip of [false, true]) {
    const w = new World();
    const A = w.user("teacher", "Ms. A");
    const B = w.user("teacher", "Mr. B");
    const Eve = w.user("teacher", "Eve");
    const q = { id: "q1", stem: "Mitochondria are found in prokaryotes.", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 };
    const pa = await publish(w, A, JSON.stringify({ schema_version: 1, bank: { id: "a", name: "A", questions: [q], created: 1, modified: 1 } }));
    const imp = await w.json<any>("POST", `/items/${pa.body.item_id}/import`, { user: B });
    const qs = imp.body.payload.bank.questions;
    if (flip) qs[0].kind.Tf.correct = false;
    const pb = await publish(w, B, JSON.stringify({ schema_version: 1, bank: { id: "b", name: "B", questions: qs, created: 1, modified: 1 } }), { hideKey: true });
    assert.equal(pb.status, 201);
    const j = await w.json<any>("GET", `/items/${pb.body.item_id}`, { user: Eve });
    assert.equal(j.body.questions[0].chain[0].modified, false, `flip=${flip}: decided on key-free facts only`);
  }
});

// ---- A2 / A3 / A4: provenance and licences ----------------------------------------------------------

test("S1: an edited derivative keeps every earlier author in its chain", async () => {
  const w = new World();
  const a = w.user("teacher", "Ms. A"), b = w.user("teacher", "Mr. B"), c = w.user("teacher", "Dr. C");
  const r = rng(5);
  const src = await publish(w, a, bankPayload([genQuestion(r, "Short", 0), genQuestion(r, "Tf", 1)]), { license: "CC-BY-4.0" });
  const ib = await w.json<any>("POST", `/items/${src.body.item_id}/import`, { user: b });
  const pb = await publish(w, b, bankOf(withOrigin(ib, src.body.item_id as string)), { license: "CC-BY-4.0" });
  const ic = await w.json<any>("POST", `/items/${pb.body.item_id}/import`, { user: c });
  const qs = withOrigin(ic, pb.body.item_id as string);
  qs[0].stem = qs[0].stem + " (fixed typo)";
  const pc = await publish(w, c, bankOf(qs), { license: "CC-BY-4.0" });
  assert.equal(pc.status, 201, JSON.stringify(pc.body));
  const ch = chainOf(w, pc.body.publish_id as string, 0);
  assert.deepEqual(ch.map((l: any) => [l.author_name, l.modified]), [["Ms. A", false], ["Mr. B", true]]);
  assert.deepEqual(chainOf(w, pc.body.publish_id as string, 1).map((l: any) => [l.author_name, l.modified]), [["Ms. A", false], ["Mr. B", false]]);
  // A declared origin naming a version the account never imported is ignored and flagged.
  const bogus = qs.map((x: Q) => ({ ...x, stem: x.stem + "?", origin: { ...x.origin, version: 99 } }));
  const pd = await publish(w, c, bankOf(bogus), { license: "CC-BY-4.0" });
  assert.equal(pd.status, 201);
  assert.ok(w.q(`SELECT * FROM admin_flags WHERE kind = 'origin_unverified' AND publish_id = ?`, pd.body.publish_id).length >= 1);
});

test("S2: a kept qid carries its origin licence; dropping origin cannot relicense NC as CC0", async () => {
  const w = new World();
  const a = w.user("teacher", "Ms. A"), b = w.user("teacher", "Mr. B");
  const src = await publish(w, a, bankPayload([genQuestion(rng(6), "Short", 0)]), { license: "CC-BY-NC-4.0" });
  const ib = await w.json<any>("POST", `/items/${src.body.item_id}/import`, { user: b });
  const pb = await publish(w, b, bankOf(withOrigin(ib, src.body.item_id as string)), { license: "CC-BY-NC-4.0" });
  const qs = ib.body.payload.bank.questions.map((q: Q, i: number) => ({ ...q, stem: q.stem + " Explain.", published: { item_id: pb.body.item_id, qid: (pb.body.qids as string[])[i], publish_id: pb.body.publish_id, version: 1 } }));
  const v2 = await publish(w, b, bankOf(qs), { license: "CC0-1.0", base: pb.body.publish_id as string }, pb.body.item_id as string);
  assert.deepEqual([v2.status, v2.body.error], [422, "license_incompatible"]);
  const ok = await publish(w, b, bankOf(qs), { license: "CC-BY-NC-SA-4.0", base: pb.body.publish_id as string }, pb.body.item_id as string);
  assert.equal(ok.status, 200);
  assert.deepEqual(chainOf(w, ok.body.publish_id as string).map((l: any) => [l.author_name, l.license, l.modified]), [["Ms. A", "CC-BY-NC-4.0", true]]);
});

test("S3/A4: may_republish_as and the publish check never disagree (union across versions, verbatim keeps its licence)", async () => {
  const w = new World();
  const a = w.user("teacher", "Ms. A"), b = w.user("teacher", "Mr. B");
  const pay = bankPayload([genQuestion(rng(7), "Short", 0)]);
  const v1 = await publish(w, a, pay, { license: "CC-BY-NC-4.0" });
  const id = v1.body.item_id as string;
  await w.json("POST", `/items/${id}/import`, { user: b });
  const p2 = JSON.parse(pay);
  p2.schema_version = 2;
  p2.bank.questions[0].published = { item_id: id, qid: (v1.body.qids as string[])[0], publish_id: v1.body.publish_id, version: 1 };
  await publish(w, a, JSON.stringify(p2), { license: "CC-BY-4.0", base: v1.body.publish_id as string }, id);
  const i2 = await w.json<any>("POST", `/items/${id}/import`, { user: b });
  // A verbatim copy keeps its licence: v2's CC-BY (combined with v1's NC grant from the same item).
  assert.deepEqual(i2.body.may_republish_as, ["CC-BY-4.0", "CC-BY-NC-4.0"]);
  for (const lic of ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"]) {
    const r = await publish(w, b, bankOf(i2.body.payload.bank.questions), { license: lic });
    assert.equal(r.status === 201, i2.body.may_republish_as.includes(lic), `${lic}: ${r.status} ${JSON.stringify(r.body)}`);
  }
});

test("A4 property: for random sources, licences and forms, import's may_republish_as is exactly what publish accepts for a verbatim copy", async () => {
  const L = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];
  for (let seed = 1; seed <= 12; seed++) {
    const r = rng(seed * 31);
    const w = new World();
    const a = w.user(), b = w.user(), c = w.user();
    const hide = r.bool();
    const qs = [genQuestion(r, r.pick(["Mc", "Tf", "Short", "Blank"]), 0), genQuestion(r, "Tf", 1)];
    const src = await publish(w, a, bankPayload(qs), { license: r.pick(L), hideKey: hide });
    // Sometimes through a middle author first.
    let item = src.body.item_id as string;
    if (r.bool()) {
      const ib = await w.json<any>("POST", `/items/${item}/import`, { user: b });
      const lic = r.pick(ib.body.may_republish_as as string[]);
      if (lic) {
        const mid = await publish(w, b, bankOf(ib.body.payload.bank.questions), { license: lic, hideKey: hide });
        assert.equal(mid.status, 201);
        item = mid.body.item_id as string;
      }
    }
    const ic = await w.json<any>("POST", `/items/${item}/import`, { user: c });
    for (const lic of L) {
      const res = await publish(w, c, bankOf(ic.body.payload.bank.questions), { license: lic, hideKey: hide });
      assert.equal(res.status === 201, ic.body.may_republish_as.includes(lic), `seed ${seed} ${lic}: ${res.status} ${JSON.stringify(res.body)}`);
    }
  }
});

// ---- A5: privacy purge reaches copies ---------------------------------------------------------------

test("E6/S4: after a student-data purge the image is served from no item, and copies using it are held", async () => {
  const w = new World();
  const admin = w.user("admin"), A = w.user(), B = w.user(), C = w.user();
  const img = png(8, 8, 7);
  const up = await upload(w, A, img);
  const pa = await publish(w, A, bankPayload([genQuestion(rng(3), "Short", 0, { sha: up.sha, ext: "png" })]));
  const imp = await w.json<any>("POST", `/items/${pa.body.item_id}/import`, { user: B });
  await upload(w, B, img);
  const pb = await publish(w, B, bankOf(imp.body.payload.bank.questions));
  assert.equal(pb.status, 201);
  await w.json("POST", `/items/${pa.body.item_id}/report`, { user: C, json: { reason: "student_data" } });
  assert.equal((await w.purge(admin, pa.body.item_id)).status, 200);
  assert.equal((await w.call("GET", `/items/${pb.body.item_id}/blobs/${up.sha}`, { user: C })).status, 404);
  assert.equal((await w.call("GET", `/items/${pb.body.item_id}/blobs/${up.sha}`, { user: B })).status, 404, "not even to the copy's author");
  assert.deepEqual(w.q(`SELECT reason, source FROM item_holds WHERE item_id = ?`, pb.body.item_id), [{ reason: "student_data", source: "privacy" }]);
  assert.equal((await w.json<{ items: unknown[] }>("GET", "/items", { user: C })).body.items.length, 0);
});

// ---- A8: claim first --------------------------------------------------------------------------------

async function takenDown() {
  const w = new World();
  const admin = w.user("admin"), a = w.user(), c = w.user();
  const p = await publish(w, a, bankPayload([genQuestion(rng(1), "Tf", 0)]), { hideKey: true });
  const id = p.body.item_id as string;
  const nid = await notice(w, admin);
  assert.equal((await w.takedown(admin, nid, id)).status, 200);
  return { w, admin, a, c, id, nid };
}
const consistent = (w: World, id: string) => {
  const keys = w.q<{ full_key: string; stripped_key: string | null }>(`SELECT full_key, stripped_key FROM item_publishes WHERE item_id = ?`, id).flatMap((r) => [r.full_key, r.stripped_key].filter(Boolean) as string[]);
  assert.ok(keys.every((k) => w.r2.objects.has(k)), "every recorded key has its body");
  assert.deepEqual([...w.r2.keys("items/"), ...w.r2.keys("quarantine/")].sort(), [...keys].sort(), "and no body is untracked");
};

test("E5/E5c: two concurrent put-backs: one wins, the item points at live bodies, nothing is left behind", async () => {
  const { w, admin, c, id, nid } = await takenDown();
  w.r2.onGet = async (key) => {
    if (key.startsWith("quarantine/")) await w.admin(admin, "POST", "/admin/dmca/withdraw", { notice_id: nid });
  };
  await w.admin(admin, "POST", "/admin/dmca/withdraw", { notice_id: nid });
  assert.equal((await w.call("GET", `/items/${id}`, { user: c })).status, 200);
  consistent(w, id);
  assert.equal(w.q(`SELECT * FROM dmca_actions WHERE restored_at IS NOT NULL`).length, 1);
});

test("E5b: a put-back racing the purge: either fully restored or fully purged, never half", async () => {
  const { w, admin, c, id, nid } = await takenDown();
  w.r2.onPutPrefix = "items/";
  w.r2.onPut = async () => {
    w.sql.prepare(`UPDATE items SET purge_after = 1 WHERE id = ?`).run(id);
    await w.pool.daily();
  };
  await w.admin(admin, "POST", "/admin/dmca/withdraw", { notice_id: nid });
  const row = w.q<{ purged_at: number | null; removed_reason: string | null }>(`SELECT purged_at, removed_reason FROM items WHERE id = ?`, id)[0]!;
  if (row.purged_at === null) {
    assert.equal(row.removed_reason, null);
    assert.equal((await w.call("GET", `/items/${id}`, { user: c })).status, 200);
  }
  consistent(w, id);
});

test("S8: a court action has a way out: resolve as restore (put back) or remove (purged, strike stands)", async () => {
  for (const outcome of ["restore", "remove"] as const) {
    const { w, admin, a, c, id } = await takenDown();
    const act = w.q<{ id: string }>(`SELECT id FROM dmca_actions`)[0]!.id;
    await w.json("POST", `/items/${id}/counter-notice`, { user: a, json: { signature: "A", material_desc: "m", name: "A", address: "addr", phone: "1", perjury_stmt: true, consent_jurisdiction: true, accept_service: true } });
    await w.admin(admin, "POST", "/admin/dmca/court-action", { action_id: act });
    w.advance(400 * DAY);
    assert.equal((await w.pool.daily() as { restored: number }).restored, 0);
    assert.equal((await w.admin(admin, "POST", "/admin/dmca/resolve", { action_id: act, outcome, note: "court order" })).status, 200);
    if (outcome === "restore") {
      assert.equal((await w.call("GET", `/items/${id}`, { user: c })).status, 200);
      assert.equal(w.q(`SELECT * FROM strikes WHERE voided_at IS NULL`).length, 0);
    } else {
      assert.equal(w.q<{ purged_at: number | null }>(`SELECT purged_at FROM items`)[0]!.purged_at !== null, true);
      assert.equal(w.r2.keys("quarantine/").length, 0);
      assert.equal(w.q(`SELECT * FROM strikes WHERE voided_at IS NULL`).length, 1);
    }
    assert.ok(w.q(`SELECT * FROM pool_audit WHERE action = 'dmca.resolve'`).length === 1);
  }
});

// ---- images (critic4a e4) ----------------------------------------------------------------------------

test("E4: hidden-data channels are refused (IDAT tail, palette on greyscale, padded tables, junk alpha)", async () => {
  const FX = new URL("./fixtures/", import.meta.url);
  const SECRET = enc("<html><script>alert(1)</script>STUDENT: Jane Doe, DOB 2011-03-04 %PDF-1.4 PK\x03\x04");
  const ihdr = new Uint8Array([0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]);
  const z = deflateSync(new Uint8Array([0, 0x80]));
  const sig = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const pngOf = (...ch: Uint8Array[]) => concat(sig, chunk("IHDR", ihdr), ...ch, chunk("IEND", new Uint8Array(0)));
  const tail = new Uint8Array(1_500_000).fill(0x41);
  tail.set(SECRET, 0);
  const r = async (b: Uint8Array) => ((await checkImage(b)) as { refused?: string }).refused ?? null;
  assert.equal(await r(pngOf(chunk("IDAT", z))), null, "the clean 1x1 passes");
  assert.equal(await r(pngOf(chunk("IDAT", concat(z, tail)))), "png_idat_size");
  assert.equal(await r(pngOf(chunk("IDAT", concat(z, SECRET)))), "png_idat_size", "more compressed bytes than the pixels can need");
  assert.equal(await r(pngOf(chunk("IDAT", concat(z, enc("PK\x03")))))!, "png_idat_data", "a short tail after the zlib stream");
  assert.equal(await r(pngOf(chunk("PLTE", enc("%PDF-1.4 aaa")), chunk("IDAT", z))), "png_plte_greyscale");
  const jpg = new Uint8Array(readFileSync(new URL("pil.jpg", FX)));
  const seg = (m: number, d: Uint8Array) => concat(new Uint8Array([0xff, m, (d.length + 2) >> 8, (d.length + 2) & 0xff]), d);
  const junk = new Uint8Array(60000).fill(0x42);
  junk.set(SECRET, 0);
  for (const [m, code] of [[0xdd, "jpeg_dri"], [0xdb, "jpeg_dqt"], [0xc4, "jpeg_dht"]] as const) assert.equal(await r(concat(jpg.subarray(0, 2), seg(m, junk), jpg.subarray(2))), code);
  const webp = new Uint8Array(readFileSync(new URL("pil_lossless.webp", FX)));
  const vp8l = webp.subarray(12);
  const bits = new DataView(webp.buffer, webp.byteOffset).getUint32(12 + 8 + 1, true);
  const ww = (bits & 0x3fff) + 1, hh = ((bits >>> 14) & 0x3fff) + 1;
  const alph = concat(enc("ALPH"), new Uint8Array([SECRET.length & 0xff, 0, 0, 0]), SECRET, SECRET.length & 1 ? new Uint8Array([0]) : new Uint8Array(0));
  const vp8x = concat(enc("VP8X"), new Uint8Array([10, 0, 0, 0, 0x10, 0, 0, 0, (ww - 1) & 255, ((ww - 1) >> 8) & 255, 0, (hh - 1) & 255, ((hh - 1) >> 8) & 255, 0]));
  const body = concat(enc("WEBP"), vp8x, alph, vp8l);
  const riff = concat(enc("RIFF"), new Uint8Array([body.length & 255, (body.length >> 8) & 255, (body.length >> 16) & 255, 0]), body);
  assert.notEqual(await r(riff), null);
  // End to end: the upload is refused and nothing is stored.
  const w = new World();
  const up = await upload(w, w.user(), pngOf(chunk("IDAT", concat(z, enc("PK\x03")))));
  assert.deepEqual([up.status, up.body.reason], [422, "png_idat_data"]);
  assert.equal(w.r2.keys().length, 0);
});

// ---- moderation abuse (critic4b s9) -------------------------------------------------------------------

test("S9: pending accounts get a clear code; released items cannot be re-hidden by the same reporters; instant holds capped per reporter", async () => {
  const w = new World();
  const p = w.user("pending");
  const r = await w.json("GET", "/items", { user: p });
  assert.equal(r.body.error, "account_not_approved");
  assert.match(r.body.message as string, /waiting for approval/);
  const a = w.user(), adm = w.user("admin");
  const it = await publish(w, a, bankPayload([genQuestion(rng(5), "Short", 0)]));
  const rs = [w.user(), w.user(), w.user()];
  for (const u of rs) await w.json("POST", `/items/${it.body.item_id}/report`, { user: u, json: { reason: "wrong_answer" } });
  assert.equal(w.q(`SELECT * FROM item_holds`).length, 1);
  await w.admin(adm, "POST", "/admin/pool/release", { item_id: it.body.item_id, reason: "reports" });
  for (const u of rs) await w.json("POST", `/items/${it.body.item_id}/report`, { user: u, json: { reason: "wrong_answer" } });
  assert.equal(w.q(`SELECT * FROM item_holds`).length, 0, "the same reporters cannot re-hide it for 30 days");
  const g = w.user();
  let hidden = 0;
  for (let i = 0; i < 8; i++) {
    const x = await publish(w, a, bankPayload([genQuestion(rng(100 + i), "Short", 0)]));
    assert.equal((await w.json("POST", `/items/${x.body.item_id}/report`, { user: g, json: { reason: "student_data" } })).status, 202);
    hidden += w.q(`SELECT * FROM item_holds WHERE item_id = ? AND reason = 'student_data'`, x.body.item_id).length;
  }
  assert.equal(hidden, 3, "at most 3 instant holds a day per reporter; the rest wait for the admin");
  assert.equal(w.q(`SELECT * FROM reports WHERE reporter_id = ? AND reason = 'student_data'`, g.id).length, 8, "every report is still recorded");
});

// ---- errors, allowances, lockout, retries -----------------------------------------------------------

test("S10: useful errors; failed validation does not use the publish allowance; your own deletes survive the miss lockout", async () => {
  const w = new World();
  const a = w.user();
  const it = await publish(w, a, bankPayload([genQuestion(rng(6), "Short", 0)]), { visibility: "unlisted" });
  await w.json("DELETE", `/items/${it.body.item_id}`, { user: a });
  w.advance(15 * DAY);
  await w.pool.daily();
  const rp = await publish(w, a, bankPayload([genQuestion(rng(6), "Short", 0)]), { base: it.body.publish_id as string }, it.body.item_id as string);
  assert.deepEqual([rp.status, rp.body.error, rp.body.purged], [409, "item_removed", true]);
  assert.match(rp.body.message as string, /new item/);
  // Invalid payloads do not count against the day's publishes.
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'publishes', 1, 0, 'x')`).run(a.id);
  for (let i = 0; i < 5; i++) assert.equal((await publish(w, a, `{"schema_version":1,"question":{}}`)).status, 422);
  const ok = await publish(w, a, bankPayload([genQuestion(rng(7), "Tf", 0)]));
  assert.equal(ok.status, 201);
  // 100 misses lock out id routes, but never deleting or undeleting your own item.
  for (let i = 0; i < 100; i++) await w.call("GET", `/items/${i.toString(16).padStart(32, "0")}`, { user: a });
  assert.equal((await w.call("GET", `/items/${ok.body.item_id}`, { user: a })).status, 429);
  assert.equal((await w.json("DELETE", `/items/${ok.body.item_id}`, { user: a })).status, 200);
  assert.equal((await w.json("POST", `/items/${ok.body.item_id}/undelete`, { user: a })).status, 200);
  // quota_exceeded says how far over.
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'storage_bytes', 10, 0, 'x')`).run(a.id);
  const q = await upload(w, a, png(3, 3, 3));
  assert.deepEqual([q.status, q.body.error, q.body.limit], [413, "quota_exceeded", 10]);
});

test("a retry after 60 s re-stages the publish; the slow original's late writes cannot attach to it", async () => {
  const w = new World();
  const a = w.user();
  const payload = bankPayload([0, 1, 2].map((i) => genQuestion(rng(9), "Tf", i)));
  const id = publishId();
  const meta = { publish_id: id, hide_key: false, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, labels: [], order_acks: [], image_ok: [], meta: { title: "Retry", subject: "Bio" }, visibility: "public" };
  const send = () => w.json("POST", "/items", { user: a, body: framed(meta, payload), contentType: PUBLISH_CONTENT_TYPE });
  let retry: { status: number; body: Record<string, unknown> } | null = null;
  w.r2.onPut = async () => {
    w.advance(61_000);
    retry = await send();
  };
  const original = await send();
  assert.equal(retry!.status, 201);
  assert.equal(original.status, 409);
  const pubs = w.q<{ publish_id: string; status: string; question_count: number }>(`SELECT publish_id, status, question_count FROM item_publishes`);
  assert.deepEqual(pubs.map((p) => p.status), ["current"]);
  assert.equal(w.q(`SELECT * FROM item_questions`).length, 3, "no duplicate or stray derived rows");
  assert.equal(w.r2.keys("items/").length, 1, "the original's body was cleaned up");
});

test("E8: the publish result never says whether something matched the blocklist", async () => {
  const w = new World();
  const admin = w.user("admin"), A = w.user(), E = w.user();
  const q = genQuestion(rng(12), "Mc", 0);
  const pa = await publish(w, A, bankPayload([q]));
  await w.takedown(admin, await notice(w, admin), pa.body.item_id);
  const probe = await publish(w, E, bankPayload([q]), { visibility: "unlisted" });
  const other = await publish(w, E, bankPayload([genQuestion(rng(13), "Mc", 0)]), { visibility: "unlisted" });
  assert.deepEqual(Object.keys(probe.body).sort(), Object.keys(other.body).sort());
  assert.equal("held" in probe.body, false);
});

test("the hourly reconciler marks blobs whose bytes vanished; the next publish asks for a re-upload; re-uploading clears it", async () => {
  const w = new World();
  const a = w.user();
  const up = await upload(w, a, png(4, 4, 8));
  await w.r2.delete(`blobs/${up.sha}`);
  const rep = (await w.pool.hourly()) as { missing: number };
  assert.equal(rep.missing, 1);
  assert.notEqual(w.q<{ missing_at: number | null }>(`SELECT missing_at FROM blobs`)[0]!.missing_at, null);
  const p = await publish(w, a, bankPayload([genQuestion(rng(1), "Short", 0, { sha: up.sha, ext: "png" })]));
  assert.deepEqual([p.status, p.body.error, p.body.reason, p.body.positions], [422, "blob_missing", "reupload", [0]]);
  await upload(w, a, png(4, 4, 8));
  assert.equal(w.q<{ missing_at: number | null }>(`SELECT missing_at FROM blobs`)[0]!.missing_at, null);
  assert.equal((await publish(w, a, bankPayload([genQuestion(rng(1), "Short", 0, { sha: up.sha, ext: "png" })]))).status, 201);
});

test("tags and standards are served with the item and the import", async () => {
  const w = new World();
  const a = w.user(), b = w.user();
  const q = genQuestion(rng(2), "Tf", 0);
  (q.json as Q).topics = ["Cell Biology"];
  const p = await publish(w, a, bankPayload([q]), { meta: { title: "T", subject: "Bio", tags: ["Unit 3"], standards: ["hs-ls1-2"] } });
  const g = await w.json<any>("GET", `/items/${p.body.item_id}`, { user: b });
  assert.deepEqual([g.body.item.tags, g.body.item.standards], [["cell biology", "unit 3"], ["HS-LS1-2"]]);
  const i = await w.json<any>("POST", `/items/${p.body.item_id}/import`, { user: b });
  assert.deepEqual(i.body.item.tags, ["cell biology", "unit 3"]);
});
