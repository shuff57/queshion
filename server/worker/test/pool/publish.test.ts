// Publish, republish, idempotency, CAS races, retention, qids, licences, blobs, quota, daily caps.
// Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { PUBLISH_CONTENT_TYPE } from "../../src/pool/shared.ts";
import { ACCOUNT_LOCK_MS } from "../../src/pool/publish.ts";
import { bankPayload, chunk, CountingStore, framed, genQuestion, png, publish, publishId, questionPayload, rng, sha256, upload, World, type User } from "./harness.ts";

const r0 = () => rng(42);
const MB = 1024 * 1024;

function plainBank(n = 3, seed = 1) {
  const r = rng(seed);
  return bankPayload(Array.from({ length: n }, (_, i) => genQuestion(r, ["Mc", "Tf", "Short"][i % 3]!, i)));
}

/** Republish payload: the same questions with the qids the server assigned (schema v2 `published`). */
function withQids(payload: string, itemId: string, publish_id: string, qids: string[]): string {
  const p = JSON.parse(payload) as { schema_version: number; bank: { questions: Record<string, unknown>[] } };
  p.schema_version = 2;
  p.bank.questions.forEach((q, i) => (q.published = { item_id: itemId, qid: qids[i], publish_id, version: 1 }));
  return JSON.stringify(p);
}

test("meta is checked: licence chosen, attestation ticked, public never circle-only, framing and media type", async () => {
  const w = new World();
  const a = w.user();
  const payload = plainBank();
  const base = { publish_id: publishId(), hide_key: false, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, visibility: "public", meta: {} };
  const send = (meta: Record<string, unknown>, ct = PUBLISH_CONTENT_TYPE) => w.json("POST", "/items", { user: a, body: framed(meta, payload), contentType: ct });
  assert.equal((await send({ ...base, license: undefined })).body.field, "license");
  assert.equal((await send({ ...base, license: "GPL-3.0" })).body.field, "license");
  assert.equal((await send({ ...base, attestation: { accepted: false, text_rev: "2026-10-19" } })).body.field, "attestation");
  assert.equal((await send({ ...base, attestation: { accepted: true, text_rev: "1999-01-01" } })).body.field, "attestation");
  assert.equal((await send({ ...base, publish_id: "short" })).body.field, "publish_id");
  assert.equal((await send({ ...base, visibility: undefined })).body.field, "visibility");
  assert.deepEqual((await send({ ...base, license: "circle-only" })).body, { error: "license_not_public" });
  assert.equal((await send(base, "application/json")).status, 415);
  const noNl = await w.json("POST", "/items", { user: a, body: JSON.stringify(base), contentType: PUBLISH_CONTENT_TYPE });
  assert.equal(noNl.status, 400);
  // Nothing was staged by any of these.
  assert.equal(w.q(`SELECT * FROM item_publishes`).length, 0);
  // Unlisted circle-only is fine (it is never public).
  assert.equal((await publish(w, a, payload, { license: "circle-only", visibility: "unlisted" })).status, 201);
});

test("share-core decides validity: errors carry codes and paths only; keyless needs hide_key; labels and order acks are required", async () => {
  const w = new World();
  const a = w.user();
  const bad = await publish(w, a, `{"schema_version":1,"question":{"id":"x","stem":"S ‮","kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"topics":[],"image":null,"explanation":null,"figure":null,"created":1,"modified":1}}`);
  assert.equal(bad.status, 422);
  assert.equal(bad.body.error, "invalid_payload");
  assert.ok(!JSON.stringify(bad.body).includes("S ‮"), "no input text in errors");
  // Duplicate keys are a share-core refusal (the payload is never JSON.parsed by the Worker first).
  const dup = await publish(w, a, `{"schema_version":1,"schema_version":1,"question":{}}`);
  assert.equal(dup.status, 422);
  // A keyless question cannot be published with its placeholder "key" visible.
  const kq = JSON.parse(questionPayload(genQuestion(r0(), "Tf", 0)));
  kq.schema_version = 2;
  kq.question.keyless = true;
  kq.question.explanation = null;
  kq.question.figure = null;
  assert.match(JSON.stringify((await publish(w, a, JSON.stringify(kq))).body), /keyless_needs_hide_key/);
  // Labelled choices: hide_key needs a decision per finding; the 422 carries the author's preview.
  const mc = { schema_version: 1, question: { id: "q", stem: "Pick", kind: { Mc: { answers: [{ text: "A. one", correct: true, locked: false }, { text: "B. two", correct: false, locked: false }, { text: "C. three", correct: false, locked: false }] } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 } };
  const noLabels = await publish(w, a, JSON.stringify(mc), { hideKey: true, labels: [], auto: false });
  assert.equal(noLabels.status, 422);
  assert.equal(noLabels.body.error, "labels_need_decision");
  assert.ok(Array.isArray(noLabels.body.label_findings));
  // Keeping order-revealing labels preserves the order, which must be acknowledged.
  const keep = await publish(w, a, JSON.stringify(mc), { hideKey: true, labels: [{ question: 0, choice: "keep" }], auto: false });
  assert.equal(keep.status, 422);
  assert.equal(keep.body.error, "order_ack_required");
  assert.deepEqual(keep.body.needs_order_ack, [{ question: 0 }]);
  const acked = await publish(w, a, JSON.stringify(mc), { hideKey: true, labels: [{ question: 0, choice: "keep" }], orderAcks: [{ question: 0 }] });
  assert.equal(acked.status, 201, JSON.stringify(acked.body));
  assert.equal(w.q(`SELECT * FROM item_publishes WHERE status <> 'current'`).length, 0, "refusals stage nothing");
});

test("idempotent publish_id: a retry replays, a different body or another author is 409", async () => {
  const w = new World();
  const a = w.user();
  const b = w.user();
  const payload = plainBank();
  const id = publishId();
  const first = await publish(w, a, payload, { publishId: id });
  assert.equal(first.status, 201);
  // The app retries with the same id and the same bytes (the meta is identical: same order of keys).
  const meta = { publish_id: id, hide_key: false, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, labels: [], order_acks: [], image_ok: [], meta: { title: "Unit test", subject: "Biology", course: "", grade: "", tags: [], standards: [] }, visibility: "public" };
  const again = await w.json("POST", "/items", { user: a, body: framed(meta, payload), contentType: PUBLISH_CONTENT_TYPE });
  assert.equal(again.status, 200);
  assert.equal(again.body.replayed, true);
  assert.equal(again.body.item_id, first.body.item_id);
  assert.equal(w.q(`SELECT * FROM items`).length, 1, "no second item");
  assert.equal((await publish(w, a, plainBank(3, 9), { publishId: id })).status, 409);
  const stolen = await w.json("POST", "/items", { user: b, body: framed(meta, payload), contentType: PUBLISH_CONTENT_TYPE });
  assert.deepEqual([stolen.status, stolen.body], [409, { error: "publish_id_conflict" }], "another author's publish_id reveals nothing");
});

test("republish: CAS on base_publish_id, qids kept, versions immutable, retention current + previous", async () => {
  const w = new World();
  const a = w.user();
  const p0 = plainBank(3);
  const v1 = await publish(w, a, p0);
  const item = v1.body.item_id as string;
  const qids = v1.body.qids as string[];
  const p1 = withQids(p0, item, v1.body.publish_id as string, qids);
  // A stale base is a 409 with the current version.
  const stale = await publish(w, a, p1, { base: publishId() }, item);
  assert.equal(stale.status, 409);
  assert.equal(stale.body.current_version, 1);
  const v2 = await publish(w, a, p1, { base: v1.body.publish_id as string }, item);
  assert.equal(v2.status, 200);
  assert.equal(v2.body.version, 2);
  assert.deepEqual(v2.body.qids, qids, "qids survive a republish");
  const v1Body = w.r2.objects.get(w.q<{ full_key: string }>(`SELECT full_key FROM item_publishes WHERE publish_id = ?`, v1.body.publish_id)[0]!.full_key)!.bytes;
  const v3 = await publish(w, a, p1, { base: v2.body.publish_id as string }, item);
  assert.equal(v3.body.version, 3);
  const rows = w.q<{ publish_id: string; status: string }>(`SELECT publish_id, status FROM item_publishes ORDER BY version`);
  assert.deepEqual(rows.map((x) => x.status), ["previous", "current"], "v1 deleted, v2 previous, v3 current");
  assert.equal(w.r2.keys("items/").length, 2, "v1's body deleted from R2");
  assert.equal(w.q(`SELECT * FROM item_questions WHERE publish_id = ?`, v1.body.publish_id).length, 0, "and its derived rows");
  assert.ok(v1Body.length > 0);
  // A qid that belongs to another item is not accepted; a qid sent twice is not accepted.
  const other = await publish(w, a, plainBank(3, 2));
  const foreign = JSON.parse(p1);
  foreign.bank.questions[0].published.qid = (other.body.qids as string[])[0];
  foreign.bank.questions[1].published.qid = qids[2];
  const v4 = await publish(w, a, JSON.stringify(foreign), { base: v3.body.publish_id as string }, item);
  const q4 = v4.body.qids as string[];
  assert.notEqual(q4[0], (other.body.qids as string[])[0]);
  assert.notEqual(q4[1], qids[2]);
  assert.notEqual(q4[2], qids[2]);
  // A bank item cannot become a single question.
  const asQ = await publish(w, a, questionPayload(genQuestion(r0(), "Tf", 0)).replace('"schema_version":1', '"schema_version":2'), { base: v4.body.publish_id as string }, item);
  assert.equal(asQ.body.error, "kind_mismatch");
  // Ids and authoring times never leave the device: server ids and the commit day.
  const stored = JSON.parse(new TextDecoder().decode(w.r2.objects.get(w.q<{ full_key: string }>(`SELECT full_key FROM item_publishes WHERE status = 'current' AND item_id = ?`, item)[0]!.full_key)!.bytes));
  assert.equal(stored.bank.id, item);
  assert.ok(stored.bank.questions.every((q: { id: string; created: number }) => /^[0-9a-f]{32}$/.test(q.id) && q.created % 86_400_000 === 0));
  assert.ok(!JSON.stringify(stored).includes("q0local"));
});

test("race: two devices republish from the same base; exactly one wins and the loser's rows never existed", async () => {
  const w = new World();
  const a = w.user();
  const p0 = plainBank(3);
  const v1 = await publish(w, a, p0);
  const item = v1.body.item_id as string;
  const p1 = withQids(p0, item, v1.body.publish_id as string, v1.body.qids as string[]);
  let inner: Awaited<ReturnType<typeof publish>> | null = null;
  // While device 1 is writing its body, device 2 publishes and commits.
  w.r2.onPut = async () => {
    w.advance(ACCOUNT_LOCK_MS + 1); // device 2 runs in another isolate after device 1's lock timed out
    inner = await publish(w, a, p1, { base: v1.body.publish_id as string }, item);
  };
  const outer = await publish(w, a, p1, { base: v1.body.publish_id as string }, item);
  assert.equal(inner!.status, 200);
  assert.equal(outer.status, 409);
  assert.equal(outer.body.error, "conflict");
  assert.equal(outer.body.current_version, 2);
  assert.deepEqual(w.q(`SELECT version, status FROM item_publishes ORDER BY version`), [{ version: 1, status: "previous" }, { version: 2, status: "current" }]);
  assert.equal(w.r2.keys("items/").length, 2, "the loser's body was removed");
});

test("race: a publish during a delete loses; the item stays deleted and nothing is staged", async () => {
  const w = new World();
  const a = w.user();
  const p0 = plainBank(3);
  const v1 = await publish(w, a, p0);
  const item = v1.body.item_id as string;
  w.r2.onPut = async () => {
    assert.equal((await w.json("DELETE", `/items/${item}`, { user: a })).status, 200);
  };
  const res = await publish(w, a, withQids(p0, item, v1.body.publish_id as string, v1.body.qids as string[]), { base: v1.body.publish_id as string }, item);
  assert.equal(res.status, 409);
  assert.equal(w.q<{ removed_reason: string }>(`SELECT removed_reason FROM items`)[0]!.removed_reason, "author_deleted");
  assert.equal(w.q(`SELECT * FROM item_publishes WHERE status IN ('staging', 'staged')`).length, 0);
  // And once deleted, republishing is refused up front.
  assert.equal((await publish(w, a, p0, { base: v1.body.publish_id as string }, item)).body.error, "item_removed");
});

test("blob rules: hash recomputed, format sniffed, same answer whether or not it existed, only your own blobs, dims from the header", async () => {
  const w = new World();
  const a = w.user();
  const b = w.user();
  const img = png(7, 5, 3);
  const sha = await sha256(img);
  assert.equal((await w.json("PUT", `/blobs/${"0".repeat(64)}`, { user: a, body: img })).body.error, "hash_mismatch");
  assert.equal((await w.json("PUT", `/blobs/NOTHEX`, { user: a, body: img })).status, 400);
  const svg = new TextEncoder().encode(`<svg xmlns="http://www.w3.org/2000/svg"/>`);
  assert.deepEqual((await upload(w, a, svg)).body, { error: "image_refused", reason: "format" });
  const withText = png(4, 4, 1, [chunk("tEXt", new TextEncoder().encode("Comment\0secret"))]);
  assert.equal((await upload(w, a, withText)).body.reason, "png_chunk");
  const big = new Uint8Array(2 * MB + 1);
  assert.equal((await w.json("PUT", `/blobs/${"1".repeat(64)}`, { user: a, body: big, headers: { "Content-Length": String(big.length) } })).status, 413);
  const first = await w.text("PUT", `/blobs/${sha}`, { user: a, body: img });
  const second = await w.text("PUT", `/blobs/${sha}`, { user: b, body: img });
  const third = await w.text("PUT", `/blobs/${sha}`, { user: a, body: img });
  assert.equal(first.status, 201);
  assert.equal(first.text, second.text, "no existence oracle");
  assert.equal(first.text, third.text);
  assert.equal(w.r2.objects.get(`blobs/${sha}`)!.contentType, "image/png");
  // A blob someone else uploaded cannot be referenced until you upload it yourself.
  const c = w.user();
  const q = genQuestion(r0(), "Short", 0, { sha, ext: "jpg" });
  const refused = await publish(w, c, questionPayload(q));
  assert.deepEqual(refused.body, { error: "blob_missing", positions: [0] });
  // The payload's dimensions and extension are replaced by the verified ones.
  const ok = await publish(w, a, questionPayload(q));
  assert.equal(ok.status, 201);
  const key = w.q<{ full_key: string }>(`SELECT full_key FROM item_publishes WHERE item_id = ?`, ok.body.item_id)[0]!.full_key;
  const stored = JSON.parse(new TextDecoder().decode(w.r2.objects.get(key)!.bytes));
  assert.deepEqual(stored.question.image, { file: `media/${sha}.png`, width: 7, height: 5 });
  assert.equal(w.q(`SELECT * FROM pending_blobs WHERE uploader_id = ?`, a.id).length, 0, "pending cleared once referenced");
});

test("quota arithmetic: distinct blobs + bodies + 1 KB per item, charged once per author, refused atomically, released by purge", async () => {
  const w = new World();
  const a = w.user();
  const b = w.user();
  const usage = async (u: User) => ((await w.json<{ storage: { used: number } }>("GET", "/items/mine", { user: u })).body.storage.used);
  assert.equal(await usage(a), 0);
  const img1 = png(40, 40, 1);
  const img2 = png(40, 40, 2);
  await upload(w, a, img1);
  assert.equal(await usage(a), img1.length);
  await upload(w, a, img1); // the same blob again: not charged twice
  assert.equal(await usage(a), img1.length);
  await upload(w, b, img1); // another author pays for their own copy
  assert.equal(await usage(b), img1.length);
  await upload(w, a, img2);
  const r = rng(3);
  const qa = genQuestion(r, "Short", 0, { sha: await sha256(img1), ext: "png" });
  const qb = genQuestion(r, "Short", 1, { sha: await sha256(img1), ext: "png" });
  const pub = await publish(w, a, bankPayload([qa, qb]));
  const bodies = w.q<{ n: number }>(`SELECT SUM(full_bytes + stripped_bytes) AS n FROM item_publishes WHERE author_id = ?`, a.id)[0]!.n;
  assert.equal(await usage(a), img1.length + img2.length + bodies + 1024, "img1 referenced twice is counted once; img2 still pending");
  // Pending uploads expire and stop counting.
  w.advance(25 * 3600_000);
  assert.equal(await usage(a), img1.length + bodies + 1024);
  // A limit the next upload would cross refuses it, and nothing is charged.
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'storage_bytes', ?, 0, 'x')`).run(a.id, (await usage(a)) + 100);
  const img3 = png(30, 30, 9);
  const over = await upload(w, a, img3);
  assert.deepEqual([over.status, over.body.error], [413, "quota_exceeded"]);
  assert.equal(w.q(`SELECT * FROM pending_blobs WHERE sha256 = ?`, over.sha).length, 0);
  assert.equal(w.r2.objects.has(`blobs/${over.sha}`), false);
  // A publish over the quota is refused before anything is written.
  const before = w.r2.keys().length;
  const tooBig = await publish(w, a, plainBank(20, 5));
  assert.deepEqual([tooBig.status, tooBig.body.error], [413, "quota_exceeded"]);
  assert.equal(w.r2.keys().length, before);
  // Delete -> purge releases everything (the 1 KB tombstone charge too); unreferenced blobs are collected.
  await w.json("DELETE", `/items/${pub.body.item_id as string}`, { user: a });
  w.advance(15 * 86_400_000);
  await w.pool.daily();
  assert.equal(await usage(a), 0);
  // b's pending copy of img1 expired too, so the purge collected the bytes nothing references.
  await w.pool.hourly();
  assert.equal(w.r2.objects.has(`blobs/${await sha256(img1)}`), false);
  assert.equal(w.q(`SELECT * FROM blobs`).length, 0);
  assert.equal(await usage(b), 0);
});

test("daily caps: publishes, uploads, reads, browse, misses; crossing one flags the admin once", async () => {
  const w = new World();
  const a = w.user();
  const b = w.user();
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'publishes', 2, 0, 'x')`).run();
  assert.equal((await publish(w, a, plainBank(1, 1))).status, 201);
  const second = await publish(w, a, plainBank(1, 2));
  assert.equal(second.status, 201);
  const third = await publish(w, a, plainBank(1, 3));
  assert.deepEqual([third.status, third.body.error, third.body.metric], [429, "daily_limit", "publishes"]);
  assert.equal((await publish(w, a, plainBank(1, 4))).status, 429);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'cap'`).length, 1, "one flag per user, day and metric");
  assert.equal(w.notices.filter((n) => n.kind === "cap").length, 1);
  // A new day resets it.
  w.advance(86_400_000);
  assert.equal((await publish(w, a, plainBank(1, 5))).status, 201);
  // Item-id misses: past 100 in a day, every id route is 429 (even for a real id).
  const real = second.body.item_id as string;
  for (let i = 0; i < 100; i++) assert.equal((await w.call("GET", `/items/${i.toString(16).padStart(32, "0")}`, { user: b })).status, 404);
  assert.equal((await w.call("GET", `/items/${real}`, { user: b })).status, 429);
  assert.equal((await w.call("GET", `/items/${real}`, { user: a })).status, 200, "per account");
  // Blob misses likewise.
  for (let i = 0; i < 100; i++) await w.call("GET", `/items/${real}/blobs/${i.toString(16).padStart(64, "0")}`, { user: a });
  assert.equal((await w.call("GET", `/items/${real}/blobs/${"f".repeat(64)}`, { user: a })).status, 429);
  // Browse pages.
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'browse', 1, 0, 'x')`).run(b.id);
  assert.equal((await w.call("GET", `/items`, { user: b })).status, 200);
  assert.equal((await w.call("GET", `/items`, { user: b })).status, 429);
});

test("burst rate limits: per account and per IP, before any work; refused hits are given back", async () => {
  const store = new CountingStore();
  const w = new World({ rateStore: store });
  const a = w.user();
  const b = w.user();
  for (let i = 0; i < 300; i++) assert.equal((await w.call("GET", "/items/mine", { user: a, ip: `10.1.${i >> 8}.${i & 255}` })).status, 200);
  const r = await w.call("GET", "/items/mine", { user: a, ip: "10.9.9.9" });
  assert.equal(r.status, 429);
  assert.ok(Number(r.headers.get("Retry-After")) > 0);
  assert.equal((await w.call("GET", "/items/mine", { user: b, ip: "10.9.9.9" })).status, 200, "another account from the same IP is fine");
  for (let i = 0; i < 1200; i++) assert.equal((await w.call("GET", "/items/mine", { user: w.user(), ip: "10.7.7.7" })).status, 200);
  assert.equal((await w.call("GET", "/items/mine", { user: w.user(), ip: "10.7.7.7" })).status, 429, "per IP");
  assert.equal((await w.call("GET", "/items/mine", { user: a, ip: null })).status, 403, "no client IP: refused, never pooled");
  // Keys are HMACs in the auth package's scheme (purged with the account), never raw ids or IPs.
  assert.ok([...store.counts.keys()].every((k) => /^[ai]:[A-Za-z0-9_-]{43}:pool:(read:)?\d+$/.test(k) && !k.includes(a.id) && !k.includes("10.")));
});

test("licences: the hard check against your own imports, chains with 'modified', the soft check is silent", async () => {
  const w = new World();
  const a = w.user("teacher", "Ms. A");
  const b = w.user("teacher", "Mr. B");
  const c = w.user("teacher", "Dr. C");
  const r = rng(11);
  const q1 = genQuestion(r, "Mc", 0);
  const q2 = genQuestion(r, "Tf", 1);
  const src = await publish(w, a, bankPayload([q1, q2]), { license: "CC-BY-SA-4.0" });
  const srcId = src.body.item_id as string;
  // B imports the full form (no hide_key) and republishes it.
  const imp = await w.json<{ payload: { bank: { questions: Record<string, unknown>[] } }; may_republish_as: string[] }>("POST", `/items/${srcId}/import`, { user: b });
  assert.deepEqual(imp.body.may_republish_as, ["CC-BY-SA-4.0"]);
  const copy = JSON.stringify({ schema_version: 1, bank: { id: "x", name: "B's copy", questions: imp.body.payload.bank.questions, created: 1, modified: 1 } });
  const asBy = await publish(w, b, copy, { license: "CC-BY-4.0" });
  assert.deepEqual(asBy.body, {
    error: "license_incompatible",
    positions: [0, 1],
    allowed: ["CC-BY-SA-4.0"],
    per_question: [
      { pos: 0, allowed: ["CC-BY-SA-4.0"] },
      { pos: 1, allowed: ["CC-BY-SA-4.0"] },
    ],
  });
  // Changing the key does not escape the check (match_hash ignores the key).
  const edited = JSON.parse(copy);
  edited.bank.questions[1].kind.Tf.correct = !edited.bank.questions[1].kind.Tf.correct;
  assert.equal((await publish(w, b, JSON.stringify(edited), { license: "CC0-1.0" })).body.error, "license_incompatible");
  const ok = await publish(w, b, JSON.stringify(edited), { license: "CC-BY-SA-4.0" });
  assert.equal(ok.status, 201);
  const chains = w.q<{ pos: number; chain_json: string; modified: number }>(`SELECT pos, chain_json, modified FROM item_questions WHERE publish_id = ? ORDER BY pos`, ok.body.publish_id);
  const link0 = JSON.parse(chains[0]!.chain_json);
  assert.deepEqual(link0.map((l: { author_name: string; item_id: string; license: string; modified: boolean }) => [l.author_name, l.item_id, l.license, l.modified]), [["Ms. A", srcId, "CC-BY-SA-4.0", false]]);
  assert.equal(chains[1]!.modified, 1, "a changed key counts as modified");
  // C imports B's copy: the chain carries A, then B, oldest first.
  const impC = await w.json<{ questions: { chain: { author_name: string }[] }[] }>("POST", `/items/${ok.body.item_id as string}/import`, { user: c });
  assert.deepEqual(impC.body.questions[0]!.chain.map((l) => l.author_name), ["Ms. A"]);
  const cc = await w.json<{ payload: { bank: { questions: unknown[] } } }>("GET", `/items/${ok.body.item_id as string}`, { user: c });
  const cPub = await publish(w, c, JSON.stringify({ schema_version: 1, bank: { id: "y", name: "C", questions: cc.body.payload.bank.questions, created: 1, modified: 1 } }), { license: "CC-BY-SA-4.0" });
  const cChain = JSON.parse(w.q<{ chain_json: string }>(`SELECT chain_json FROM item_questions WHERE publish_id = ? AND pos = 0`, cPub.body.publish_id)[0]!.chain_json);
  assert.deepEqual(cChain.map((l: { author_name: string }) => l.author_name), ["Ms. A", "Mr. B"]);
  // The soft check: D publishes the same question without importing it. Flagged for the admin, and
  // the answer is the same as for a question nobody has.
  const d = w.user();
  const dup = await publish(w, d, bankPayload([q1]), { license: "CC0-1.0" });
  const fresh = await publish(w, d, bankPayload([genQuestion(rng(999), "Mc", 0)]), { license: "CC0-1.0" });
  const shape = (x: Record<string, unknown>) => Object.keys(x).sort().join(",") + `|${x.possible_duplicate_of}`;
  assert.equal(shape(dup.body), shape(fresh.body));
  assert.ok(w.q(`SELECT * FROM admin_flags WHERE kind = 'license_soft' AND user_id = ?`, d.id).length >= 1);
  // A declared origin that matches no import is dropped and flagged.
  const fake = JSON.parse(bankPayload([genQuestion(rng(77), "Short", 0)]));
  fake.schema_version = 2;
  fake.bank.questions[0].origin = { item_id: "e".repeat(32), qid: "f".repeat(32), publish_id: "p".repeat(22), version: 1, author_name: "Famous Author", license: "CC0-1.0", form: "full" };
  const fp = await publish(w, d, JSON.stringify(fake), { license: "CC0-1.0" });
  assert.equal(fp.status, 201);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'origin_unverified' AND user_id = ?`, d.id).length, 1);
  const stored = new TextDecoder().decode(w.r2.objects.get(w.q<{ full_key: string }>(`SELECT full_key FROM item_publishes WHERE publish_id = ?`, fp.body.publish_id)[0]!.full_key)!.bytes);
  assert.ok(!stored.includes("Famous Author"), "client-declared provenance is never stored");
  // The author's own items never count as imports.
  const own = await publish(w, a, bankPayload([q1, q2]), { license: "CC-BY-4.0" });
  assert.equal(own.status, 201);
  assert.equal(own.body.possible_duplicate_of, srcId, "the same questions as one of their items");
});

test("the licence table", async () => {
  const { mayRepublish, allowedByAll } = await import("../../src/pool/license.ts");
  const L = ["CC0-1.0", "CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0", "circle-only"];
  const table: Record<string, string[]> = {
    "CC0-1.0": L,
    "CC-BY-4.0": ["CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"],
    "CC-BY-SA-4.0": ["CC-BY-SA-4.0"],
    "CC-BY-NC-4.0": ["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"],
    "CC-BY-NC-SA-4.0": ["CC-BY-NC-SA-4.0"],
    "circle-only": [],
  };
  for (const o of L) for (const t of L) assert.equal(mayRepublish(o, t), table[o]!.includes(t), `${o} -> ${t}`);
  assert.equal(mayRepublish("CC-BY-SA-4.0", "CC-BY-NC-SA-4.0"), false, "BY-SA never becomes BY-NC-SA");
  assert.deepEqual(allowedByAll(["CC-BY-SA-4.0", "CC-BY-NC-4.0"]), [], "a bank mixing BY-SA and BY-NC origins has no licence");
  assert.equal(mayRepublish("MIT", "CC0-1.0"), false);
});
