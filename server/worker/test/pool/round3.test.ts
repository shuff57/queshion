// Regressions for the round-3 critics (critic5a X1-X7, critic5b LIC/ATTR/PRIV/DMCA/IMG/PAGE/HOLD),
// each inverted to assert the fix, plus the round's other changes. Run: npm test
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { checkImage } from "../../src/pool/image.ts";
import { ACCOUNT_LOCK_MS } from "../../src/pool/publish.ts";
import { bankPayload, chunk, concat, genQuestion, png, publish, rng, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const tfBank = (seed: number, n: number) => bankPayload(Array.from({ length: n }, (_, i) => genQuestion(rng(seed), "Tf", i)));
const bankOf = (qs: Q[], name = "copy") => JSON.stringify({ schema_version: 2, bank: { id: "x", name, questions: qs, created: 1, modified: 1 } });
const live = (w: World, u: User) => w.q<{ n: number | null }>(`SELECT SUM(question_count) AS n FROM items WHERE author_id = ? AND removed_at IS NULL`, u.id)[0]!.n ?? 0;
const chainOf = (w: World, pid: string, pos = 0) => JSON.parse(w.q<{ chain_json: string }>(`SELECT chain_json FROM item_questions WHERE publish_id = ? AND pos = ?`, pid, pos)[0]!.chain_json);
const withOrigin = (r: any, itemId: string) =>
  r.body.payload.bank.questions.map((q: Q, i: number) => ({ ...q, origin: { item_id: itemId, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }));
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const seen = async (w: World, u: User, id: string) => (await w.call("GET", `/items/${id}`, { user: u })).status;
const republishOf = (seed: number, n: number, item: string, pub: Record<string, unknown>) => {
  const p = JSON.parse(tfBank(seed, n));
  p.schema_version = 2;
  p.bank.questions.forEach((q: Q, i: number) => {
    if (i < (pub.qids as string[]).length) q.published = { item_id: item, qid: (pub.qids as string[])[i], publish_id: pub.publish_id, version: pub.version };
  });
  return JSON.stringify(p);
};

// ---- C3: the question cap counts in-flight republishes ------------------------------------------------

test("X1/X1b: concurrent or nested republishes of live items cannot pass the question cap; nor can an undelete racing one", async () => {
  const w = new World();
  const admin = w.user("admin");
  const a = w.user();
  await w.admin(admin, "PUT", "/admin/pool/limits", { user_id: a.id, metric: "shared_questions", value: 12 });
  const made: Record<string, unknown>[] = [];
  for (let k = 0; k < 4; k++) made.push((await publish(w, a, tfBank(10 + k, 1))).body);
  await Promise.all(made.map((p, k) => publish(w, a, republishOf(50 + k, 9, p.item_id as string, p), { base: p.publish_id as string }, p.item_id as string)));
  assert.ok(live(w, a) <= 12, `live ${live(w, a)}`);
  // Nested: one republish lands inside another's staging window.
  const w2 = new World();
  const adm2 = w2.user("admin");
  const b = w2.user();
  await w2.admin(adm2, "PUT", "/admin/pool/limits", { user_id: b.id, metric: "shared_questions", value: 12 });
  const m2: Record<string, unknown>[] = [];
  for (let k = 0; k < 4; k++) m2.push((await publish(w2, b, tfBank(10 + k, 1))).body);
  let inner: { status: number; body: Record<string, unknown> } | null = null;
  w2.r2.onPut = async () => {
    w2.advance(ACCOUNT_LOCK_MS + 1); // past the per-account single flight: the cap must still hold
    inner = await publish(w2, b, republishOf(61, 9, m2[1]!.item_id as string, m2[1]!), { base: m2[1]!.publish_id as string }, m2[1]!.item_id as string);
  };
  const outer = await publish(w2, b, republishOf(60, 9, m2[0]!.item_id as string, m2[0]!), { base: m2[0]!.publish_id as string }, m2[0]!.item_id as string);
  assert.deepEqual([outer.status, inner!.status, inner!.body.error], [200, 422, "shared_questions_limit"]);
  assert.equal(live(w2, b), 12);
  // An undelete landing inside a republish's staging window.
  const w3 = new World();
  const adm3 = w3.user("admin");
  const c = w3.user();
  await w3.admin(adm3, "PUT", "/admin/pool/limits", { user_id: c.id, metric: "shared_questions", value: 10 });
  const x = (await publish(w3, c, tfBank(70, 1))).body;
  const y = (await publish(w3, c, tfBank(71, 1))).body;
  await w3.json("DELETE", `/items/${y.item_id}`, { user: c });
  let und: { status: number } | null = null;
  w3.r2.onPut = async () => {
    und = await w3.json("POST", `/items/${y.item_id}/undelete`, { user: c });
  };
  const rx = await publish(w3, c, republishOf(72, 10, x.item_id as string, x), { base: x.publish_id as string }, x.item_id as string);
  assert.equal(rx.status, 200);
  assert.equal(und!.status, 422, "the in-flight republish already uses the cap");
  assert.equal(live(w3, c), 10);
});

// ---- C1 / C2: privacy blocks ---------------------------------------------------------------------------

test("X2/X3: a privacy block on an image holds whatever DMCA rows exist for it, and no put-back lifts it", async () => {
  // X2: a lifted DMCA block on the same image first.
  const w = new World();
  const admin = w.user("admin"), A = w.user(), B = w.user(), C = w.user();
  const img = png(8, 8, 99);
  const up = await upload(w, A, img);
  const pa = await publish(w, A, bankPayload([genQuestion(rng(3), "Short", 0, { sha: up.sha, ext: "png" })]));
  const nid = await notice(w, admin);
  await w.takedown(admin, nid, pa.body.item_id);
  await w.admin(admin, "POST", "/admin/dmca/withdraw", { notice_id: nid });
  await w.json("POST", `/items/${pa.body.item_id}/report`, { user: C, json: { reason: "student_data" } });
  assert.equal((await w.purge(admin, pa.body.item_id)).status, 200);
  assert.ok(w.q(`SELECT * FROM privacy_blocks WHERE kind = 'blob' AND hash = ?`, up.sha).length === 1);
  // B cannot re-upload the blocked bytes (neutral refusal), so no new item can carry them.
  assert.equal((await upload(w, B, img)).body.reason, "not_accepted");
  assert.equal((await publish(w, B, bankPayload([genQuestion(rng(4), "Short", 0, { sha: up.sha, ext: "png" })]))).body.error, "blob_missing");
  // X3: a standing takedown on another item with the same image, put back after the privacy purge.
  const w2 = new World();
  const adm2 = w2.user("admin"), A2 = w2.user(), B2 = w2.user(), C2 = w2.user();
  const img2 = png(8, 8, 77);
  const u2 = await upload(w2, A2, img2);
  const pa2 = await publish(w2, A2, bankPayload([genQuestion(rng(5), "Short", 0, { sha: u2.sha, ext: "png" })]));
  await upload(w2, B2, img2);
  const pb2 = await publish(w2, B2, bankPayload([genQuestion(rng(6), "Short", 0, { sha: u2.sha, ext: "png" })]));
  const n2 = await notice(w2, adm2);
  await w2.takedown(adm2, n2, pb2.body.item_id);
  await w2.json("POST", `/items/${pa2.body.item_id}/report`, { user: C2, json: { reason: "student_data" } });
  await w2.purge(adm2, pa2.body.item_id);
  await w2.admin(adm2, "POST", "/admin/dmca/withdraw", { notice_id: n2 });
  assert.equal((await w2.call("GET", `/items/${pb2.body.item_id}/blobs/${u2.sha}`, { user: C2 })).status, 404);
  assert.equal(await seen(w2, C2, pb2.body.item_id as string), 404, "the taken-down copy was held too, and stays held after its put-back");
});

test("X4/PRIV-1: student data in question TEXT: copies are held, and any later publish of it (the author's too) is held", async () => {
  const w = new World();
  const admin = w.user("admin"), A = w.user(), B = w.user(), C = w.user(), E = w.user();
  const q = genQuestion(rng(5), "Short", 0);
  q.json.stem = "Jamie Smith (period 3, ID 448812) scored 12/20: what grade?";
  const pa = await publish(w, A, bankPayload([q]));
  const ib = await imp(w, B, pa.body.item_id as string);
  const pb = await publish(w, B, bankOf(withOrigin(ib, pa.body.item_id as string)));
  // A deleted copy is held too, so undoing its deletion does not bring the text back.
  const pc = await publish(w, B, bankOf(ib.body.payload.bank.questions), { visibility: "public" });
  await w.json("DELETE", `/items/${pc.body.item_id}`, { user: B });
  await w.json("POST", `/items/${pa.body.item_id}/report`, { user: C, json: { reason: "student_data" } });
  assert.equal((await w.purge(admin, pa.body.item_id)).status, 200);
  assert.equal(await seen(w, C, pb.body.item_id as string), 404);
  await w.json("POST", `/items/${pc.body.item_id}/undelete`, { user: B });
  assert.equal(await seen(w, C, pc.body.item_id as string), 404);
  for (const u of [E, A]) {
    const again = await publish(w, u, bankPayload([q]));
    assert.equal(again.status, 201, "a hold, never a refusal (common questions)");
    assert.deepEqual(w.q(`SELECT reason, source FROM item_holds WHERE item_id = ?`, again.body.item_id), [{ reason: "student_data", source: "privacy" }]);
  }
});

test("X5: a held item deleted and published again as new carries its hold", async () => {
  const w = new World();
  const A = w.user(), C = w.user();
  const pa = await publish(w, A, tfBank(8, 2));
  await w.json("POST", `/items/${pa.body.item_id}/report`, { user: C, json: { reason: "student_data" } });
  await w.json("DELETE", `/items/${pa.body.item_id}`, { user: A });
  const p2 = await publish(w, A, tfBank(8, 2));
  assert.equal(await seen(w, C, p2.body.item_id as string), 404);
  assert.deepEqual(w.q(`SELECT reason, source FROM item_holds WHERE item_id = ?`, p2.body.item_id), [{ reason: "student_data", source: `carried:${pa.body.item_id}` }]);
});

// ---- licences and attribution -------------------------------------------------------------------------

test("X6: a verbatim copy of something only READ (GET) is still checked: NC cannot become CC0", async () => {
  const w = new World();
  const A = w.user(), B = w.user();
  const pa = await publish(w, A, tfBank(20, 2), { license: "CC-BY-NC-4.0" });
  const g = await w.json<any>("GET", `/items/${pa.body.item_id}`, { user: B });
  const pb = await publish(w, B, JSON.stringify(g.body.payload), { license: "CC0-1.0" });
  assert.deepEqual([pb.status, pb.body.error, pb.body.allowed], [422, "license_incompatible", ["CC-BY-NC-4.0"]]);
  // A read is not an import: it does not count.
  const mine = await w.json<any>("GET", "/items/mine", { user: A });
  assert.equal(mine.body.items[0].import_count, 0);
});

test("LIC-1/LIC-1b: within one item, a verbatim copy uses only the grants of the version it copies (explanations included)", async () => {
  for (const [l1, l2, copyV2] of [["CC-BY-4.0", "CC-BY-NC-4.0", true], ["CC-BY-NC-4.0", "CC-BY-4.0", false]] as const) {
    const w = new World();
    const a = w.user(), b = w.user();
    const q = genQuestion(rng(1), "Short", 0);
    q.json.explanation = "v1 explanation";
    const v1 = await publish(w, a, bankPayload([q]), { license: l1 });
    const id = v1.body.item_id as string;
    const i1 = await imp(w, b, id);
    const p2 = JSON.parse(bankPayload([q]));
    p2.schema_version = 2;
    p2.bank.questions[0].explanation = "A LONG NEW WORKED SOLUTION written for v2 only";
    p2.bank.questions[0].published = { item_id: id, qid: (v1.body.qids as string[])[0], publish_id: v1.body.publish_id, version: 1 };
    await publish(w, a, JSON.stringify(p2), { license: l2, base: v1.body.publish_id as string }, id);
    const i2 = await imp(w, b, id);
    const copy = copyV2 ? i2 : i1;
    const only = copyV2 ? l2 : l1;
    if (copyV2) assert.deepEqual(i2.body.may_republish_as, [only]);
    assert.equal((await publish(w, b, bankOf(copy.body.payload.bank.questions), { license: "CC-BY-4.0" })).status === 201, only === "CC-BY-4.0");
    assert.equal((await publish(w, b, bankOf(copy.body.payload.bank.questions), { license: only })).status, 201);
  }
});

test("ATTR-1/ATTR-3: attribution follows identical content, never version numbers across items or import order", async () => {
  // ATTR-1: A (v5) -> B adds a solution -> C adds a topic; D imports A and C, copies C verbatim.
  const w = new World();
  const a = w.user("teacher", "Ms. A"), b = w.user("teacher", "Mr. B"), c = w.user("teacher", "Dr. C"), d = w.user("teacher", "Prof. D");
  const q = genQuestion(rng(3), "Short", 0);
  q.json.explanation = null;
  let pa = await publish(w, a, bankPayload([q]));
  const aid = pa.body.item_id as string;
  for (let k = 0; k < 4; k++) {
    const p = JSON.parse(bankPayload([q]));
    p.schema_version = 2;
    p.bank.questions[0].published = { item_id: aid, qid: (pa.body.qids as string[])[0], publish_id: pa.body.publish_id, version: pa.body.version };
    p.bank.questions[0].points = k + 1;
    pa = await publish(w, a, JSON.stringify(p), { base: pa.body.publish_id as string }, aid);
  }
  const ib = await imp(w, b, aid);
  const qb = withOrigin(ib, aid);
  qb[0].explanation = "B ADDED A WORKED SOLUTION";
  const pb = await publish(w, b, bankOf(qb));
  const ic = await imp(w, c, pb.body.item_id as string);
  const qc = withOrigin(ic, pb.body.item_id as string);
  qc[0].topics = ["c added topic"];
  const pc = await publish(w, c, bankOf(qc));
  await imp(w, d, aid);
  const id_ = await imp(w, d, pc.body.item_id as string);
  const pd = await publish(w, d, bankOf(withOrigin(id_, pc.body.item_id as string)));
  assert.deepEqual(chainOf(w, pd.body.publish_id as string).map((l: any) => [l.author_name, l.modified]), [["Ms. A", true], ["Mr. B", true], ["Dr. C", false]]);
  // ATTR-3: the same chain whichever import came first.
  const chains: string[] = [];
  for (const order of [0, 1]) {
    const w2 = new World();
    const a2 = w2.user("teacher", "Ms. A"), b2 = w2.user("teacher", "Mr. B"), d2 = w2.user("teacher", "Prof. D");
    const pa2 = await publish(w2, a2, bankPayload([genQuestion(rng(4), "Short", 0)]));
    const ib2 = await imp(w2, b2, pa2.body.item_id as string);
    const qb2 = withOrigin(ib2, pa2.body.item_id as string);
    qb2[0].explanation = "B ADDED";
    const pb2 = await publish(w2, b2, bankOf(qb2));
    let fromB: any;
    for (const which of order === 0 ? ["b", "a"] : ["a", "b"]) {
      w2.advance(1000);
      if (which === "b") fromB = await imp(w2, d2, pb2.body.item_id as string);
      else await imp(w2, d2, pa2.body.item_id as string);
    }
    const pd2 = await publish(w2, d2, bankOf(withOrigin(fromB, pb2.body.item_id as string)));
    chains.push(JSON.stringify(chainOf(w2, pd2.body.publish_id as string).map((l: any) => [l.author_name, l.modified])));
  }
  assert.equal(chains[0], chains[1]);
  assert.deepEqual(JSON.parse(chains[0]!), [["Ms. A", true], ["Mr. B", false]]);
});

test("X7: hidden key variants by two derivative authors look identical to a third party", async () => {
  const w = new World();
  const A = w.user(), B = w.user(), C = w.user(), D = w.user();
  const tf = (correct: boolean) => ({ id: "q", stem: "Is the sky green today?", kind: { Tf: { correct } }, points: 1, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 });
  const bank = (k: boolean) => JSON.stringify({ schema_version: 1, bank: { id: "b", name: "n", questions: [tf(k)], created: 1, modified: 1 } });
  const pa = await publish(w, A, bank(true), { hideKey: true });
  const outs: string[] = [];
  for (const [u, k] of [[B, true], [C, false]] as const) {
    await imp(w, u, pa.body.item_id as string);
    const p = await publish(w, u, bank(k), { hideKey: true });
    const g = await w.json<any>("GET", `/items/${p.body.item_id}`, { user: D });
    outs.push(JSON.stringify(g.body.questions.map((q: any) => q.chain)) + JSON.stringify(g.body.item.license));
  }
  assert.equal(outs[0], outs[1]);
});

test("latent leak: with the import record gone, a public v2 does not reveal whether a hidden v1's key differed", async () => {
  const views: string[] = [];
  for (const v1Key of [true, false]) {
    const w = new World();
    const A = w.user("teacher", "Ms. A"), B = w.user("teacher", "Mr. B"), Eve = w.user();
    const tf = (k: boolean) => ({ id: "q", stem: "Water is wet.", kind: { Tf: { correct: k } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 });
    const pa = await publish(w, A, JSON.stringify({ schema_version: 1, bank: { id: "a", name: "A", questions: [tf(true)], created: 1, modified: 1 } }));
    await imp(w, B, pa.body.item_id as string);
    const v1 = await publish(w, B, JSON.stringify({ schema_version: 1, bank: { id: "b", name: "B", questions: [tf(v1Key)], created: 1, modified: 1 } }), { hideKey: true });
    // The import record is gone (e.g. trimmed): only the previous publish's chain remains.
    w.sql.prepare(`DELETE FROM import_questions WHERE user_id = ?`).run(B.id);
    const v2q = { ...tf(true), published: { item_id: v1.body.item_id, qid: (v1.body.qids as string[])[0], publish_id: v1.body.publish_id, version: 1 } };
    const v2 = await publish(w, B, JSON.stringify({ schema_version: 2, bank: { id: "b", name: "B", questions: [v2q], created: 1, modified: 1 } }), { base: v1.body.publish_id as string }, v1.body.item_id as string);
    assert.equal(v2.status, 200);
    const g = await w.json<any>("GET", `/items/${v1.body.item_id}`, { user: Eve });
    views.push(JSON.stringify(g.body.questions[0].chain.map((l: any) => [l.author_name, l.version, l.license, l.modified])));
  }
  assert.equal(views[0], views[1]);
});

// ---- C6: admin removal is reversible -------------------------------------------------------------------

test("C6: an admin removal can be undone within 14 days (back to its prior state, within limits); the author sees why and can appeal", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user(), c = w.user();
  const it = (await publish(w, a, tfBank(1, 2))).body.item_id as string;
  await w.admin(admin, "POST", "/admin/pool/remove", { item_id: it, note: "policy" });
  const mine = await w.json<any>("GET", "/items/mine", { user: a });
  assert.deepEqual([mine.body.items[0].state, mine.body.items[0].removed_reason, mine.body.items[0].appeal], ["removed", "admin", "available"]);
  assert.equal((await w.json("POST", `/items/${it}/appeal`, { user: a, json: { note: "It was my own work." } })).status, 202);
  assert.equal((await w.json("POST", `/items/${it}/appeal`, { user: c, json: { note: "x" } })).status, 404);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'appeal' AND item_id = ?`, it).length, 1);
  assert.equal((await w.admin(admin, "POST", "/admin/pool/unremove", { item_id: it, note: "appeal upheld" })).status, 200);
  assert.equal(await seen(w, c, it), 200);
  // Prior state: an author-deleted item goes back to deleted, not live.
  await w.json("DELETE", `/items/${it}`, { user: a });
  await w.admin(admin, "POST", "/admin/pool/remove", { item_id: it });
  await w.admin(admin, "POST", "/admin/pool/unremove", { item_id: it });
  assert.equal(w.q<{ removed_reason: string }>(`SELECT removed_reason FROM items`)[0]!.removed_reason, "author_deleted");
  // Limits apply when it would go live; and only within the 14 days.
  await w.json("POST", `/items/${it}/undelete`, { user: a });
  await w.admin(admin, "POST", "/admin/pool/remove", { item_id: it });
  await w.admin(admin, "PUT", "/admin/pool/limits", { user_id: a.id, metric: "shared_questions", value: 1 });
  assert.equal((await w.admin(admin, "POST", "/admin/pool/unremove", { item_id: it })).status, 409);
  w.advance(15 * DAY);
  assert.equal((await w.admin(admin, "POST", "/admin/pool/unremove", { item_id: it })).status, 404);
  assert.ok(w.q(`SELECT * FROM pool_audit WHERE action = 'item.unremove'`).length === 2);
});

// ---- DMCA copies, counter-notice window ---------------------------------------------------------------

test("DMCA-1: a takedown lists the copies; one click holds them; after the put-back they are released together", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user(), b = w.user(), c = w.user();
  const pa = await publish(w, a, bankPayload([genQuestion(rng(6), "Short", 0)]));
  const ib = await imp(w, b, pa.body.item_id as string);
  const pb = await publish(w, b, bankOf(withOrigin(ib, pa.body.item_id as string)));
  const t = await w.takedown<any>(admin, await notice(w, admin), pa.body.item_id);
  assert.deepEqual(t.body.copies.map((x: any) => x.item_id), [pb.body.item_id]);
  const h = await w.admin<any>(admin, "POST", "/admin/dmca/hold-copies", { action_id: t.body.action_id });
  assert.deepEqual(h.body.held, [pb.body.item_id]);
  assert.equal(await seen(w, c, pb.body.item_id as string), 404);
  await w.json("POST", `/items/${pa.body.item_id}/counter-notice`, { user: a, json: { signature: "A", material_desc: "m", name: "A", address: "addr", phone: "1", perjury_stmt: true, consent_jurisdiction: true, accept_service: true } });
  w.advance(20 * DAY);
  await w.pool.daily();
  const copies = await w.admin<any>(admin, "GET", "/admin/dmca/copies", null, `?action_id=${t.body.action_id}`);
  assert.equal(copies.body.restored, true);
  assert.equal(copies.body.held.length, 1);
  assert.equal((await w.admin<any>(admin, "POST", "/admin/dmca/release-copies", { action_id: t.body.action_id })).body.released, 1);
  assert.equal(await seen(w, c, pb.body.item_id as string), 200);
  assert.equal(await seen(w, c, pa.body.item_id as string), 200);
});

test("a counter-notice after the 30-day quarantine window is refused", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user();
  const it = (await publish(w, a, tfBank(2, 1))).body.item_id as string;
  await w.takedown(admin, await notice(w, admin), it);
  w.advance(31 * DAY);
  const r = await w.json("POST", `/items/${it}/counter-notice`, { user: a, json: { signature: "A", material_desc: "m", name: "A", address: "addr", phone: "1", perjury_stmt: true, consent_jurisdiction: true, accept_service: true } });
  assert.equal(r.status, 404);
});

// ---- IMG-1, PAGE-1, HOLD-1 ----------------------------------------------------------------------------

test("IMG-1: an import holds the images it served for the importer, so a republish needs no re-upload, even after the source drops them", async () => {
  const w = new World();
  const a = w.user(), b = w.user();
  const up = await upload(w, a, png(8, 8, 3));
  const q = genQuestion(rng(1), "Short", 0, { sha: up.sha, ext: "png" });
  const pa = await publish(w, a, bankPayload([q]));
  const ib = await imp(w, b, pa.body.item_id as string);
  assert.equal(ib.body.images.held, 1);
  const p2 = JSON.parse(bankPayload([q]));
  p2.schema_version = 2;
  p2.bank.questions[0].image = null;
  p2.bank.questions[0].published = { item_id: pa.body.item_id, qid: (pa.body.qids as string[])[0], publish_id: pa.body.publish_id, version: 1 };
  await publish(w, a, JSON.stringify(p2), { base: pa.body.publish_id as string }, pa.body.item_id as string);
  const r1 = await publish(w, b, bankOf(ib.body.payload.bank.questions));
  assert.equal(r1.status, 201, JSON.stringify(r1.body));
  // The hold is charged to the importer's quota; over quota, nothing is held and the response says so.
  const c = w.user();
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'storage_bytes', 10, 0, 'x')`).run(c.id);
  const ic = await imp(w, c, r1.body.item_id as string);
  assert.deepEqual(ic.body.images, { held: 0, reason: "quota" });
});

test("PAGE-1: a republish during pagination never makes an item skip a page (browse is on creation time)", async () => {
  const w = new World();
  const a = w.user(), r = w.user();
  const made: Record<string, unknown>[] = [];
  for (let k = 0; k < 6; k++) {
    made.push((await publish(w, a, tfBank(50 + k, 1))).body);
    w.advance(1000);
  }
  const p1 = await w.json<any>("GET", "/items?limit=3", { user: r });
  const old = made[0]!;
  await publish(w, a, republishOf(50, 1, old.item_id as string, old), { base: old.publish_id as string }, old.item_id as string);
  const p2 = await w.json<any>("GET", `/items?limit=3&cursor=${encodeURIComponent(p1.body.next_cursor)}`, { user: r });
  const ids = new Set([...p1.body.items, ...p2.body.items].map((x: any) => x.id));
  assert.equal(ids.size, 6);
});

test("HOLD-1: a hidden republish that changes nothing served keeps the public version and the exact served bytes", async () => {
  const w = new World();
  const a = w.user(), e = w.user();
  const q = genQuestion(rng(80), "Tf", 0);
  q.json.explanation = null;
  q.json.figure = null;
  const v1 = await publish(w, a, bankPayload([q]), { hideKey: true });
  const id = v1.body.item_id as string;
  const g1 = await w.text("GET", `/items/${id}`, { user: e });
  await imp(w, e, id);
  const p2 = JSON.parse(bankPayload([q]));
  p2.schema_version = 2;
  p2.bank.questions[0].kind.Tf.correct = !q.json.kind.Tf.correct;
  p2.bank.questions[0].explanation = "A new hidden explanation";
  p2.bank.questions[0].published = { item_id: id, qid: (v1.body.qids as string[])[0], publish_id: v1.body.publish_id, version: 1 };
  w.advance(DAY);
  const v2 = await publish(w, a, JSON.stringify(p2), { hideKey: true, base: v1.body.publish_id as string }, id);
  assert.deepEqual([v2.status, v2.body.version, v2.body.public_version], [200, 2, 1]);
  const g2 = await w.text("GET", `/items/${id}`, { user: e });
  const payloadOf = (t: string) => t.slice(t.indexOf('"payload":'));
  assert.equal(payloadOf(g2.text), payloadOf(g1.text), "byte-identical served body");
  assert.deepEqual(JSON.parse(g2.text).item.version, 1);
  assert.equal(JSON.parse(g2.text).item.committed_at, JSON.parse(g1.text).item.committed_at);
  const v = await w.json<any>("POST", "/items/versions", { user: e, json: { ids: [id] } });
  assert.deepEqual(v.body.items[0], { id, status: "current", version: 1 });
  // The author sees the real version.
  assert.equal((await w.json<any>("GET", `/items/${id}`, { user: a })).body.item.version, 2);
  // A served change bumps it.
  p2.bank.questions[0].stem = "A changed stem?";
  p2.bank.questions[0].published.publish_id = v2.body.publish_id;
  const v3 = await publish(w, a, JSON.stringify(p2), { hideKey: true, base: v2.body.publish_id as string }, id);
  assert.equal(v3.body.public_version, 2);
});

// ---- filters, caps, refusals, images, config ----------------------------------------------------------

test("browse filters: subject is case-insensitive; tag, standard and title prefix; too many tags is an error, not a silent drop", async () => {
  const w = new World();
  const a = w.user(), r = w.user();
  const q = genQuestion(rng(2), "Tf", 0);
  (q.json as Q).topics = ["Cells"];
  await publish(w, a, bankPayload([q]), { meta: { title: "Mitosis 100%_fun", subject: "Biology", tags: ["unit 3"], standards: ["HS-LS1-4"] } });
  await publish(w, a, tfBank(3, 1), { meta: { title: "Optics", subject: "Physics", tags: [], standards: [] } });
  const n = async (qs: string) => (await w.json<any>("GET", `/items?${qs}`, { user: r })).body.items.length;
  assert.equal(await n("subject=biology"), 1);
  assert.equal(await n("tag=CELLS"), 1);
  assert.equal(await n("tag=unit%203"), 1);
  assert.equal(await n("standard=hs-ls1-4"), 1);
  assert.equal(await n("q=mito"), 1);
  assert.equal(await n("q=100%25"), 0, "% is literal");
  assert.equal(await n("q=Mitosis%20100%25_"), 1);
  const many = Array.from({ length: 210 }, (_, i) => genQuestion(rng(100 + i), "Tf", i));
  many.forEach((x, i) => ((x.json as Q).topics = [`topic ${i}`]));
  const r2 = await publish(w, a, bankPayload(many));
  assert.deepEqual([r2.status, r2.body.error, r2.body.limit], [422, "too_many_tags", 200]);
});

test("storage amplification: deleted items' bytes (held 14 days) are capped separately", async () => {
  const w = new World();
  const a = w.user();
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'removed_bytes', 2000, 0, 'x')`).run(a.id);
  let refused: Record<string, unknown> | null = null;
  for (let k = 0; k < 10 && !refused; k++) {
    const p = await publish(w, a, tfBank(200 + k, 5));
    if (p.status !== 201) refused = p.body;
    else await w.json("DELETE", `/items/${p.body.item_id}`, { user: a });
  }
  assert.equal(refused?.error, "removed_storage_exceeded");
  assert.ok((refused!.used as number) > 2000);
});

test("refused publishes are counted per day: past the allowance, 429 before any work", async () => {
  const w = new World();
  const a = w.user();
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'publish_refusals', 3, 0, 'x')`).run();
  for (let i = 0; i < 3; i++) assert.equal((await publish(w, a, `{"schema_version":1,"question":{}}`)).status, 422);
  const r = await publish(w, a, tfBank(1, 1));
  assert.deepEqual([r.status, r.body.metric], [429, "publish_refusals"]);
  w.advance(DAY);
  assert.equal((await publish(w, a, tfBank(1, 1))).status, 201);
});

test("PNG: a palette is refused on greyscale and truecolour images", async () => {
  const ihdr = (color: number) => new Uint8Array([0, 0, 0, 1, 0, 0, 0, 1, 8, color, 0, 0, 0]);
  const sig = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const { deflateSync } = await import("node:zlib");
  for (const color of [2, 6]) {
    const raw = new Uint8Array(1 + (color === 2 ? 3 : 4));
    const b = concat(sig, chunk("IHDR", ihdr(color)), chunk("PLTE", new Uint8Array(6)), chunk("IDAT", deflateSync(raw)), chunk("IEND", new Uint8Array(0)));
    assert.equal(((await checkImage(b)) as { refused?: string }).refused, "png_plte_truecolor");
  }
  assert.ok("contentType" in (await checkImage(png(3, 3, 1))));
});

test("production config: no workers.dev or preview URLs; the admin item view sends no-referrer and Vary", async () => {
  const toml = readFileSync(new URL("../../wrangler.toml", import.meta.url), "utf8");
  assert.match(toml, /^workers_dev = false$/m);
  assert.match(toml, /^preview_urls = false$/m);
  const w = new World();
  const admin = w.user("admin"), a = w.user();
  const it = (await publish(w, a, tfBank(1, 1))).body.item_id as string;
  const route = w.pool.adminRoutes.find((r) => r.path === "/admin/pool/item")!;
  const res = await route.handler(new Request(`https://api.example.org/admin/pool/item?id=${it}`), { admin: { userId: admin.id, sessionFamily: "f", trust: "admin", sudo: false }, body: null, now: w.clock, audit: () => w.db.prepare("SELECT 1") });
  assert.equal(res.headers.get("Referrer-Policy"), "no-referrer");
  assert.match(res.headers.get("Vary")!, /Authorization/);
});
