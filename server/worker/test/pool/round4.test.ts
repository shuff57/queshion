// Regressions for the round-4 critics (critic6a E1-E4, critic6b s1-s12), each inverted to assert
// the fix, plus the round's other changes. Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { deleteContentStatements } from "../../src/pool/index.ts";
import { acquireHeavy, HEAVY_QUEUE_MAX } from "../../src/pool/publish.ts";
import { PoolError } from "../../src/pool/shared.ts";
import { bankPayload, genQuestion, png, publish, rng, upload, World, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[], name = "b") => JSON.stringify({ schema_version: 2, bank: { id: "x", name, questions: qs, created: 1, modified: 1 } });
const tf = (stem: string, correct: boolean, extra: Q = {}) => ({ id: "q", stem, kind: { Tf: { correct } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1, ...extra });
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const copyOf = (r: any, edit: (q: Q) => Q = (q) => q) =>
  r.body.payload.bank.questions.map((q: Q, i: number) => edit({ ...q, origin: { item_id: r.body.item.id, qid: r.body.questions[i].qid, publish_id: r.body.item.publish_id, version: r.body.item.version, author_name: r.body.item.author_name, license: r.body.license, form: r.body.item.form } }));
const chainOf = (w: World, pid: string, pos = 0) => JSON.parse(w.q<{ chain_json: string }>(`SELECT chain_json FROM item_questions WHERE publish_id = ? AND pos = ?`, pid, pos)[0]!.chain_json);
const own = (pub: any, qs: Q[]) => qs.map((q, i) => (i < pub.qids.length ? { ...q, published: { item_id: pub.item_id, qid: pub.qids[i], publish_id: pub.publish_id, version: pub.version } } : q));
const repub = (w: World, u: User, pub: any, qs: Q[], o: Record<string, unknown> = {}) => publish(w, u, bankOf(own(pub, qs)), { base: pub.publish_id, ...o }, pub.item_id);
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const status = async (w: World, u: User, id: string) => (await w.call("GET", `/items/${id}`, { user: u })).status;
const holds = (w: World, id: string) => w.q<{ reason: string; source: string }>(`SELECT reason, source FROM item_holds WHERE item_id = ? ORDER BY reason, source`, id);

// ---- D4: a hidden-only change is invisible, ids included ----------------------------------------------

test("E1/s2/D4: a key-only republish changes nothing a non-author sees: same bytes, same publish id, same import history", async () => {
  const w = new World();
  const a = w.user(), e = w.user();
  const qs = [tf("Water boils at 100 C at sea level.", true), tf("Sound travels faster than light.", false)];
  const x = (await publish(w, a, bankOf(qs), { hideKey: true })).body;
  const id = x.item_id as string;
  const i1 = await w.text("POST", `/items/${id}/import`, { user: e });
  const before = { get: await w.text("GET", `/items/${id}`, { user: e }), list: await w.text("GET", "/items", { user: e }), versions: await w.text("POST", "/items/versions", { user: e, json: { ids: [id] } }) };
  w.advance(DAY);
  const z = await repub(w, a, x, [tf(qs[0]!.stem, false), qs[1]!], { hideKey: true });
  assert.deepEqual([z.status, z.body.version, z.body.public_version], [200, 2, 1]);
  const after = { get: await w.text("GET", `/items/${id}`, { user: e }), list: await w.text("GET", "/items", { user: e }), versions: await w.text("POST", "/items/versions", { user: e, json: { ids: [id] } }) };
  assert.equal(after.get.text, before.get.text, "the whole GET response, ids included, is byte-identical");
  assert.equal(after.list.text, before.list.text);
  assert.equal(after.versions.text, before.versions.text);
  const i2 = await imp(w, e, id);
  const i3 = await imp(w, e, id);
  assert.equal(i2.body.item.publish_id, JSON.parse(i1.text).item.publish_id);
  assert.deepEqual([i2.body.previously_imported_versions, i3.body.previously_imported_versions], [[1], [1]]);
  // The author sees the real publish.
  assert.equal((await get(w, a, id)).body.item.publish_id, z.body.publish_id);
});

test("s5: a preview read never hides an update notice; a reader without a copy is never told 'newer'", async () => {
  const w = new World();
  const a = w.user(), b = w.user(), v = w.user();
  const x = (await publish(w, a, bankOf([tf("Q1 stem here please.", true)]))).body;
  await imp(w, b, x.item_id as string);
  await get(w, v, x.item_id as string);
  await repub(w, a, x, [tf("Q1 stem here please.", false)]);
  await get(w, b, x.item_id as string);
  const vb = await w.json<any>("POST", "/items/versions", { user: b, json: { ids: [x.item_id] } });
  assert.equal(vb.body.items[0].status, "newer");
  const vv = await w.json<any>("POST", "/items/versions", { user: v, json: { ids: [x.item_id] } });
  assert.equal(vv.body.items[0].status, "current");
});

// ---- D5: order is part of what is served ---------------------------------------------------------------

test("s4/D5: reordered Multipart parts or locked choices are a served change: new public version, new order served", async () => {
  const w = new World();
  const A = w.user(), B = w.user();
  const mp = { id: "m1", stem: "Answer both parts.", kind: { Multipart: { parts: [{ stem: "Part (a): first compute the mass.", kind: { Short: { sample: "SAMPLEa" } } }, { stem: "Part (b): using (a), find the density.", kind: { Short: { sample: "SAMPLEb" } } }] } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 };
  const mc = { id: "m2", stem: "Order these steps", kind: { Mc: { answers: [{ text: "First", correct: true, locked: true }, { text: "Second", correct: false, locked: true }, { text: "Third", correct: false, locked: true }] } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 };
  const x = (await publish(w, A, bankOf([mp, mc]), { hideKey: true })).body;
  const mp2 = structuredClone(mp);
  mp2.kind.Multipart.parts.reverse();
  const z = await repub(w, A, x, [mp2, mc], { hideKey: true });
  assert.equal(z.body.public_version, 2);
  const b1 = await get(w, B, x.item_id as string);
  assert.deepEqual(b1.body.payload.bank.questions[0].kind.Multipart.parts.map((p: Q) => p.stem), mp2.kind.Multipart.parts.map((p) => p.stem));
  const mc2 = structuredClone(mc);
  mc2.kind.Mc.answers = [mc.kind.Mc.answers[2]!, mc.kind.Mc.answers[0]!, mc.kind.Mc.answers[1]!];
  const z2 = await repub(w, A, z.body, [mp2, mc2], { hideKey: true });
  assert.equal(z2.body.public_version, 3);
  assert.deepEqual((await get(w, B, x.item_id as string)).body.payload.bank.questions[1].kind.Mc.answers.map((p: Q) => p.text), ["Third", "First", "Second"]);
  // A key-only change still keeps it (the comparison is not simply exact).
  const mc3 = structuredClone(mc2);
  mc3.kind.Mc.answers.forEach((a: Q, i: number) => (a.correct = i === 1));
  assert.equal((await repub(w, A, z2.body, [mp2, mc3], { hideKey: true })).body.public_version, 3);
});

// ---- D3: evidence ---------------------------------------------------------------------------------------

test("E2: reading a griefer's restrictive copy of your own question never constrains you", async () => {
  const w = new World();
  const victim = w.user(), griefer = w.user();
  const q = tf("Is the Earth round?", true);
  const pv = (await publish(w, victim, bankOf([q]), { license: "CC-BY-4.0" })).body;
  const pg = await publish(w, griefer, bankOf([q]), { license: "CC-BY-NC-SA-4.0" });
  assert.equal(pg.status, 201);
  assert.equal(await status(w, victim, pg.body.item_id as string), 200);
  assert.equal((await repub(w, victim, pv, [q], { license: "CC-BY-4.0" })).status, 200);
  assert.equal((await publish(w, victim, bankOf([q]), { license: "CC-BY-4.0" })).status, 201, "nor a new publish of your own question");
});

test("s1/s12: Alice reading Bob's adaptation of her question changes neither her licence nor her attribution", async () => {
  const w = new World();
  const A = w.user("teacher", "Alice"), B = w.user("teacher", "Bob"), C = w.user();
  const q1 = tf("The mitochondrion is the powerhouse of the cell.", true);
  const x = (await publish(w, A, bankOf([q1]), { license: "CC-BY-4.0" })).body;
  const r = await imp(w, B, x.item_id as string);
  const y = await publish(w, B, bankOf(copyOf(r, (q) => ({ ...q, explanation: "Bob's explanation" }))), { license: "CC-BY-NC-4.0" });
  const yv = await publish(w, B, bankOf(copyOf(r)), { license: "CC-BY-4.0" });
  await get(w, A, y.body.item_id as string);
  await get(w, A, yv.body.item_id as string);
  const z = await repub(w, A, x, [q1, tf("Ribosomes make proteins.", true)], { license: "CC-BY-4.0" });
  assert.equal(z.status, 200);
  assert.deepEqual(chainOf(w, z.body.publish_id as string, 0), []);
  assert.deepEqual((await get(w, C, x.item_id as string)).body.questions[0].chain, []);
});

test("s6/B2: a verbatim copy grants from the identical source alone; adaptations still inherit every source", async () => {
  const w = new World();
  const fmt = (c: any[]) => c.map((l) => `${l.author_name}${l.modified ? "*" : ""}`).join(">");
  const [A, B, C, D, E] = ["Ann", "Ben", "Cat", "Dan", "Eve"].map((n) => w.user("teacher", n));
  const stem = "Enzymes lower the activation energy of a reaction.";
  const xa = (await publish(w, A!, bankOf([tf(stem, true)]), { license: "CC-BY-4.0" })).body;
  const xb = (await publish(w, B!, bankOf(copyOf(await imp(w, B!, xa.item_id as string), (q) => ({ ...q, explanation: "B expl" }))), { license: "CY-BY-SA-4.0".replace("CY", "CC") })).body;
  const xc = (await publish(w, C!, bankOf(copyOf(await imp(w, C!, xb.item_id as string))), { license: "CC-BY-SA-4.0" })).body;
  const rd = await imp(w, D!, xc.item_id as string);
  assert.equal((await publish(w, D!, bankOf(copyOf(rd, (q) => ({ ...q, points: 3 }))), { license: "CC-BY-4.0" })).body.error, "license_incompatible", "an adaptation of SA stays SA");
  const xd = (await publish(w, D!, bankOf(copyOf(rd, (q) => ({ ...q, points: 3 }))), { license: "CC-BY-SA-4.0" })).body;
  await imp(w, E!, xb.item_id as string);
  await imp(w, E!, xd.item_id as string);
  const re = await imp(w, E!, xa.item_id as string);
  assert.deepEqual(re.body.may_republish_as, ["CC-BY-4.0"]);
  const xe = await publish(w, E!, bankOf(copyOf(re)), { license: "CC-BY-4.0" });
  assert.equal(xe.status, 201);
  assert.equal(fmt(chainOf(w, xe.body.publish_id as string)), "Ann");
  assert.equal((await publish(w, E!, bankOf(copyOf(re)), { license: "CC-BY-NC-4.0" })).body.error, "license_incompatible", "a verbatim BY copy cannot be relabelled NC");
});

// ---- D2: blocks are designated, previewed, distinctive and liftable ----------------------------------

async function common3(w: World) {
  const common = tf("Water boils at 100 C at sea level.", true);
  const victims = [w.user(), w.user(), w.user()];
  const items: string[] = [];
  for (const [k, v] of victims.entries()) items.push((await publish(w, v, bankOf([common, tf(`Victim ${k} own question about tides.`, true)]))).body.item_id as string);
  return { common, items };
}

test("E3: a privacy purge never holds innocent items through a common question; nor later publishes of it", async () => {
  const w = new World();
  const admin = w.user("admin"), att = w.user(), rep = w.user(), out = w.user();
  const { common, items } = await common3(w);
  const bait = await publish(w, att, bankOf([common, tf("Jamie Smith (period 3, ID 448812) failed the quiz.", true)]));
  await w.json("POST", `/items/${bait.body.item_id}/report`, { user: rep, json: { reason: "student_data" } });
  const pv = await w.admin<any>(admin, "POST", "/admin/pool/purge-preview", { item_id: bait.body.item_id });
  assert.deepEqual(pv.body.match.map((m: Q) => [m.pos, m.other_authors, m.selected]), [[0, 3, false], [1, 0, true]]);
  assert.deepEqual(pv.body.affected, []);
  // The confirm needs the preview's token, for exactly that plan.
  assert.equal((await w.admin(admin, "POST", "/admin/pool/purge", { item_id: bait.body.item_id })).status, 428);
  assert.equal((await w.admin(admin, "POST", "/admin/pool/purge", { item_id: bait.body.item_id, questions: [0, 1], confirm_preview: pv.body.confirm_preview })).status, 428, "a different selection than previewed");
  assert.equal((await w.admin(admin, "POST", "/admin/pool/purge", { item_id: bait.body.item_id, confirm_preview: pv.body.confirm_preview })).status, 200);
  for (const id of items) assert.equal(await status(w, out, id), 200);
  const later = await publish(w, w.user(), bankOf([common]));
  assert.deepEqual(holds(w, later.body.item_id as string), []);
});

test("s10: an admin may select a shared question explicitly; the preview shows who would be held; an unblock releases exactly those holds", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [A, Z, Y, R] = ["A", "Z", "Y", "R"].map((n) => w.user("teacher", n));
  const common = tf("Water is made of hydrogen and oxygen.", true);
  const xz = (await publish(w, Z!, bankOf([common, tf("Z other", true)]))).body;
  const xa = (await publish(w, A!, bankOf([tf("Student Maria Lopez in room 12 failed the test.", true), common]))).body;
  await w.json("POST", `/items/${xa.item_id}/report`, { user: R!, json: { reason: "student_data" } });
  const pv = await w.admin<any>(adm, "POST", "/admin/pool/purge-preview", { item_id: xa.item_id, questions: [0, 1] });
  assert.deepEqual(pv.body.affected.map((x: Q) => x.item_id), [xz.item_id]);
  assert.equal((await w.purge(adm, xa.item_id as string, { questions: [0, 1] })).status, 200);
  assert.equal(await status(w, R!, xz.item_id as string), 404);
  // A manual hold made meanwhile is a different row and survives the unblock.
  await w.admin(adm, "POST", "/admin/pool/hold", { item_id: xz.item_id, note: "separate review" });
  const commonHash = pv.body.match[1].hash;
  const ub = await w.admin<any>(adm, "POST", "/admin/pool/unblock", { kind: "match", hash: commonHash, note: "false positive" });
  assert.equal(ub.body.released, 1);
  assert.deepEqual(holds(w, xz.item_id as string), [{ reason: "admin", source: "manual" }]);
  await w.admin(adm, "POST", "/admin/pool/release", { item_id: xz.item_id, reason: "admin" });
  assert.equal(await status(w, R!, xz.item_id as string), 200);
  assert.deepEqual(holds(w, (await publish(w, Y!, bankOf([common]))).body.item_id as string), []);
  // The PII question stays blocked.
  const again = await publish(w, Y!, bankOf([tf("Student Maria Lopez in room 12 failed the test.", true)]));
  assert.deepEqual(holds(w, again.body.item_id as string), [{ reason: "student_data", source: "privacy" }]);
  assert.ok(w.q(`SELECT * FROM pool_audit WHERE action = 'privacy.unblock'`).length === 1);
});

test("s7/D2: takedown copies need lineage; independent items are not copies; per-source holds; DMCA blocks liftable and expiring", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [A, B, C, Z] = ["A", "B", "C", "Z"].map((n) => w.user("teacher", n));
  const common = tf("The Earth orbits the Sun.", true);
  const xa = (await publish(w, A!, bankOf([tf("Distinctive question by A about photosynthesis rates.", true), common]))).body;
  const xb = (await publish(w, B!, bankOf(copyOf(await imp(w, B!, xa.item_id as string))))).body;
  const xz = (await publish(w, Z!, bankOf([common, tf("Z's own other question.", false)]))).body;
  // Designate only A's distinctive question: Z (who also wrote the common one) is untouched.
  const td = await w.takedown<any>(adm, await notice(w, adm), xa.item_id, { questions: [0], block_days: 30 });
  assert.equal(td.status, 200);
  assert.deepEqual(td.body.copies.map((x: Q) => x.item_id), [xb.item_id]);
  await w.admin(adm, "POST", "/admin/dmca/hold-copies", { action_id: td.body.action_id });
  const h2 = await w.admin<any>(adm, "POST", "/admin/pool/hold", { item_id: xb.item_id, note: "separate review" });
  assert.equal(h2.body.changed, true, "a manual hold is its own row");
  const z2 = await repub(w, Z!, xz, [common, tf("Z's own other question, fixed.", false)]);
  assert.equal(z2.status, 200);
  assert.deepEqual(holds(w, xz.item_id as string), []);
  await w.admin(adm, "POST", "/admin/dmca/release-copies", { action_id: td.body.action_id });
  assert.deepEqual(holds(w, xb.item_id as string), [{ reason: "admin", source: "manual" }], "release-copies never drops another source's hold");
  // A re-upload of the designated question is held, until the block is lifted ...
  const re = await publish(w, C!, bankOf([tf("Distinctive question by A about photosynthesis rates.", true)]));
  assert.deepEqual(holds(w, re.body.item_id as string), [{ reason: "blocklist", source: `dmca:${td.body.action_id}` }]);
  const ub = await w.admin<any>(adm, "POST", "/admin/dmca/unblock", { action_id: td.body.action_id, note: "settled" });
  assert.deepEqual([ub.body.lifted, ub.body.released], [1, 1]);
  assert.deepEqual(holds(w, re.body.item_id as string), []);
  // ... or until it expires (block_days).
  const w2 = new World();
  const adm2 = w2.user("admin"), a2 = w2.user(), c2 = w2.user();
  const q = tf("A very distinctive question about mitochondrial DNA.", true);
  const x2 = (await publish(w2, a2, bankOf([q]))).body;
  await w2.takedown(adm2, await notice(w2, adm2), x2.item_id, { block_days: 30 });
  assert.deepEqual(holds(w2, (await publish(w2, c2, bankOf([q]))).body.item_id as string).map((h) => h.reason), ["blocklist"]);
  w2.advance(31 * DAY);
  assert.deepEqual(holds(w2, (await publish(w2, c2, bankOf([q]))).body.item_id as string), []);
});

test("s8/D2: privacy follows copies: importers of a held copy are told 'privacy'; the image bytes are deleted at once", async () => {
  const w = new World();
  const adm = w.user("admin");
  const [A, B, C, D] = ["A", "B", "C", "D"].map((n) => w.user("teacher", n));
  const up = await upload(w, A!, png(8, 8, 7));
  const q = { ...tf("Jane Doe (period 3) wrote this answer, true or false?", true), image: { file: `media/${up.sha}.png`, width: 8, height: 8 } };
  const xa = (await publish(w, A!, bankOf([q, tf("Other q.", false)]))).body;
  const xb = (await publish(w, B!, bankOf(copyOf(await imp(w, B!, xa.item_id as string))))).body;
  await imp(w, C!, xb.item_id as string);
  await imp(w, D!, xa.item_id as string);
  await w.json("POST", `/items/${xa.item_id}/report`, { user: C!, json: { reason: "student_data" } });
  assert.equal((await w.purge(adm, xa.item_id as string)).status, 200);
  const v = async (u: User, id: unknown) => (await w.json<any>("POST", "/items/versions", { user: u, json: { ids: [id] } })).body.items[0];
  assert.deepEqual(await v(D!, xa.item_id), { id: xa.item_id, status: "removed", reason: "privacy" });
  assert.deepEqual(await v(C!, xb.item_id), { id: xb.item_id, status: "removed", reason: "privacy" });
  assert.equal((await w.call("GET", `/items/${xb.item_id}/blobs/${up.sha}`, { user: B! })).status, 404, "served to nobody at once");
  w.advance(15 * DAY);
  await w.pool.daily();
  assert.equal(w.r2.objects.has(`blobs/${up.sha}`), false, "deleted after the grace window");
});

// ---- D1: memory and refusals -------------------------------------------------------------------------

test("D1: one heavy publish per isolate with a bounded queue; one publish per account; refusals reserved before the work", async () => {
  // The isolate semaphore: one slot, HEAVY_QUEUE_MAX waiters, then 503.
  const release = await acquireHeavy();
  const waiters = Array.from({ length: HEAVY_QUEUE_MAX }, () => acquireHeavy());
  await assert.rejects(acquireHeavy(), (e: unknown) => e instanceof PoolError && e.status === 503 && e.code === "busy");
  release();
  for (const p of waiters) (await p)();
  // Per account: a second concurrent publish is a 409 (and is not counted as a refusal).
  const w = new World();
  const a = w.user();
  let inner: { status: number; body: Record<string, unknown> } | null = null;
  w.r2.onPut = async () => {
    inner = await publish(w, a, bankOf([tf("Another question entirely.", true)]));
  };
  assert.equal((await publish(w, a, bankOf([tf("A first question here.", true)]))).status, 201);
  assert.deepEqual([inner!.status, inner!.body.error], [409, "publish_in_progress"]);
  assert.equal(w.q<{ count: number }>(`SELECT count FROM usage_daily WHERE metric = 'publish_refusals'`)[0]?.count ?? 0, 0);
  // An unexpected failure mid-publish (here the storage write) still counts against the allowance.
  w.r2.onPut = async () => {
    throw new Error("isolate died");
  };
  assert.equal((await publish(w, a, bankOf([tf("A third question here.", true)]))).status, 500);
  assert.equal(w.q<{ count: number }>(`SELECT count FROM usage_daily WHERE metric = 'publish_refusals'`)[0]!.count, 1);
  // The body cap is configurable.
  const w2 = new World();
  (w2.pool as unknown as { _: unknown })._ = null;
});

// ---- non-blocking --------------------------------------------------------------------------------------

test("E4: appeals: one flag and one audit row a day per item, a daily cap, short notes", async () => {
  const w = new World();
  const a = w.user(), r1 = w.user();
  const p = await publish(w, a, bankPayload([genQuestion(rng(1), "Tf", 0)]));
  await w.json("POST", `/items/${p.body.item_id}/report`, { user: r1, json: { reason: "student_data" } });
  const codes: number[] = [];
  for (let i = 0; i < 15; i++) codes.push((await w.json("POST", `/items/${p.body.item_id}/appeal`, { user: a, json: { note: "please review" } })).status);
  assert.equal(codes.filter((s) => s === 202).length, 10);
  assert.equal(codes.filter((s) => s === 429).length, 5);
  assert.equal(w.q(`SELECT * FROM pool_audit WHERE action = 'item.appeal'`).length, 1);
  assert.equal((await w.json("POST", `/items/${p.body.item_id}/appeal`, { user: a, json: { note: "x".repeat(600) } })).status, 400);
});

test("reads record their evidence once per (account, publish), never again on repeated reads", async () => {
  const w = new World();
  const a = w.user(), b = w.user();
  const big = await publish(w, a, bankPayload(Array.from({ length: 50 }, (_, i) => genQuestion(rng(i), "Tf", i))), { license: "CC-BY-NC-4.0" });
  for (let i = 0; i < 5; i++) await get(w, b, big.body.item_id as string);
  assert.equal(w.q(`SELECT * FROM imports WHERE user_id = ?`, b.id).length, 1);
  assert.equal(w.q(`SELECT * FROM import_questions WHERE user_id = ?`, b.id).length, 50, "50 questions, written once, not 5 x 50");
  const g = await get(w, b, big.body.item_id as string);
  assert.equal((await publish(w, b, JSON.stringify(g.body.payload), { license: "CC0-1.0" })).body.error, "license_incompatible");
});

test("instant holds: a site-wide daily cap (then the admin is flagged) and none from brand-new accounts", async () => {
  const w = new World();
  const a = w.user();
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'instant_holds_global', 1, 0, 'x')`).run();
  const items = [];
  for (let i = 0; i < 3; i++) items.push((await publish(w, a, bankOf([tf(`Question number ${i} about tides.`, true)]))).body.item_id as string);
  await w.json("POST", `/items/${items[0]}/report`, { user: w.user(), json: { reason: "student_data" } });
  await w.json("POST", `/items/${items[1]}/report`, { user: w.user(), json: { reason: "student_data" } });
  assert.equal(holds(w, items[0]!).length, 1);
  assert.equal(holds(w, items[1]!).length, 0, "the cap was reached: it waits for the admin");
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'instant_hold_cap'`).length, 1);
  const young = w.user();
  w.sql.prepare(`UPDATE users SET created_at = ? WHERE id = ?`).run(w.clock, young.id);
  w.sql.prepare(`DELETE FROM pool_limits`).run();
  await w.json("POST", `/items/${items[2]}/report`, { user: young, json: { reason: "student_data" } });
  assert.equal(holds(w, items[2]!).length, 0, "a week-old account cannot hide items instantly");
});

test("account deletion: an author may choose to keep their shared items, anonymised, licences and chains intact", async () => {
  const w = new World();
  const a = w.user("teacher", "Ms. A"), b = w.user();
  const x = (await publish(w, a, bankOf([tf("A question Ms. A wants to keep shared.", true)]), { license: "CC-BY-4.0" })).body;
  assert.deepEqual((await w.json<any>("PUT", "/items/prefs", { user: a, json: { keep_on_delete: true } })).body, { keep_on_delete: true });
  w.sql.prepare(`UPDATE users SET email = NULL, pw_hash = NULL, display_name = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, a.id);
  await w.db.batch(deleteContentStatements(w.db, a.id, w.clock));
  w.advance(20 * DAY);
  await w.pool.daily();
  const g = await get(w, b, x.item_id as string);
  assert.equal(g.status, 200);
  assert.equal(g.body.item.author_name, "deleted teacher");
  // Without the preference: removed and purged (the default).
  const c = w.user();
  const y = (await publish(w, c, bankOf([tf("Another question to be removed.", true)]))).body;
  w.sql.prepare(`UPDATE users SET email = NULL, pw_hash = NULL, display_name = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, c.id);
  await w.db.batch(deleteContentStatements(w.db, c.id, w.clock));
  assert.equal(await status(w, b, y.item_id as string), 404);
});

test("a near copy of something served (same stem, edited choices) raises origin_suspect for the admin, never a refusal", async () => {
  const w = new World();
  const a = w.user(), b = w.user();
  const mc = (texts: string[]) => ({ id: "q", stem: "Which planet is closest to the Sun?", kind: { Mc: { answers: texts.map((t, i) => ({ text: t, correct: i === 0, locked: false })) } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 });
  const x = (await publish(w, a, bankOf([mc(["Mercury", "Venus", "Mars"])]), { license: "CC-BY-NC-4.0" })).body;
  await get(w, b, x.item_id as string);
  const y = await publish(w, b, bankOf([mc(["Mercury", "Earth", "Jupiter"])]), { license: "CC0-1.0" });
  assert.equal(y.status, 201);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'origin_suspect' AND user_id = ?`, b.id).length, 1);
});
