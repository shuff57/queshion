// Round 5: the structural rule (visible derivations see key-free content only for a hidden publish)
// as a property, and the critics' repros (critic7a x1-x5, critic7b s1-s7) inverted. Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { bytesReserved, PUBLISH_CONTENT_TYPE, reserveBytes, ISOLATE_BYTES_BUDGET } from "../../src/pool/shared.ts";
import { framed, genQuestion, png, publish, publishId, rng, upload, World, type Rng, type User } from "./harness.ts";

const DAY = 86_400_000;
type Q = Record<string, any>;
const bankOf = (qs: Q[], sv = 2) => JSON.stringify({ schema_version: sv, bank: { id: "x", name: "b", questions: qs, created: 1, modified: 1 } });
const strip = (x: Q) => {
  const y = { ...x };
  delete y.published;
  return y;
};
const imp = (w: World, u: User, id: string) => w.json<any>("POST", `/items/${id}/import`, { user: u });
const get = (w: World, u: User, id: string) => w.json<any>("GET", `/items/${id}`, { user: u });
const notice = async (w: World, admin: User) =>
  (await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "X", complainant_contact: "x@example.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true })).body.notice_id;
const LIC = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];

/** A different key for the same key-free question (Tf flip, another Mc answer, a swapped pairing, another sample). */
function otherKey(q: Q, r: Rng): Q {
  const x = structuredClone(q);
  const k = x.kind;
  if (k.Tf) k.Tf.correct = !k.Tf.correct;
  else if (k.Mc) {
    const n = k.Mc.answers.length;
    const was = k.Mc.answers.findIndex((a: Q) => a.correct);
    const to = (was + 1 + r.int(n - 1)) % n;
    k.Mc.answers.forEach((a: Q, j: number) => (a.correct = j === to));
  } else if (k.Matching) [k.Matching.pairs[0].right, k.Matching.pairs[1].right] = [k.Matching.pairs[1].right, k.Matching.pairs[0].right];
  else if (k.Short) k.Short.sample = `${k.Short.sample} (other)`;
  return x;
}

// ---- THE STRUCTURAL RULE, as a property -----------------------------------------------------------------

test("structural rule: for key variants of a hidden publish x public sources with differing keys and licences, everything a non-author can observe is identical", async () => {
  let pairs = 0;
  for (let seed = 1; seed <= 30; seed++) {
    const views: string[] = [];
    for (const variant of [0, 1]) {
      const r = rng(seed * 7717); // the same choices in both worlds; only Y's hidden key differs
      const w = new World();
      const X = w.user("teacher", "Xena"), Z = w.user("teacher", "Zeke"), Y = w.user("teacher", "Yuri"), Eve = w.user("teacher", "Eve"), Imp = w.user("teacher", "Ian");
      const kind = r.pick(["Tf", "Mc", "Matching", "Short"]);
      const base = genQuestion(r, kind, 0).json as Q;
      base.explanation = null;
      base.figure = null;
      const kx = base;
      const kz = otherKey(base, r);
      const lx = r.pick(LIC);
      const lz = r.pick(LIC);
      const pxr = await publish(w, X, bankOf([kx], 1), { license: lx });
      const px = pxr.body;
      const pz = (await publish(w, Z, bankOf([kz], 1), { license: lz })).body;
      // Y reads or imports the sources (the evidence), in a random way that is the same in both worlds.
      for (const [p, how] of [[px, r.pick(["get", "import", "none"])], [pz, r.pick(["get", "import", "none"])]] as const)
        if (how === "get") await get(w, Y, p.item_id);
        else if (how === "import") await imp(w, Y, p.item_id);
      const yl = r.pick(LIC);
      // The only difference between the worlds: Y's hidden key equals X's or Z's.
      const res = await publish(w, Y, bankOf([variant === 0 ? kx : kz], 1), { hideKey: true, license: yl });
      assert.ok(px.item_id && pz.item_id, `seed ${seed}: the sources must publish`);
      const label = (s: string) => s.replace(new RegExp(px.item_id, "g"), "<X>").replace(new RegExp(pz.item_id, "g"), "<Z>").replace(/"(?:[0-9a-f]{32}|[A-Za-z0-9_-]{22})"/g, '"<id>"').replace(/"[0-9a-f]{64}"/g, '"<hash>"');
      const seen: unknown[] = [res.status, res.body.error ?? null, res.body.allowed ?? null];
      if (res.status === 201) {
        const id = res.body.item_id as string;
        const g = await get(w, Eve, id);
        const i = await imp(w, Imp, id);
        const v = await w.json("POST", "/items/versions", { user: Imp, json: { ids: [id] } });
        const l = await w.json<any>("GET", "/items?limit=50", { user: Eve });
        seen.push(g.body.item.license, g.body.questions.map((q: Q) => q.chain), i.body.may_republish_as, i.body.questions.map((q: Q) => q.chain), v.body, l.body.items.filter((x: Q) => x.id === id).map((x: Q) => ({ ...x, id: "<id>" })));
        seen.push(w.q(`SELECT reason, source FROM item_holds WHERE item_id = ?`, id), w.q(`SELECT kind FROM admin_flags WHERE item_id = ? ORDER BY kind`, id));
      }
      views.push(label(JSON.stringify(seen)));
    }
    assert.equal(views[0], views[1], `seed ${seed}: something visible depends on Y's hidden key`);
    pairs++;
  }
  assert.equal(pairs, 30);
});

// ---- E1 / E2 --------------------------------------------------------------------------------------------

test("x1/E1: a hidden copy credits every key-free-identical source, whichever key it has", async () => {
  for (const key of [true, false]) {
    const w = new World();
    const X = w.user("teacher", "X"), Z = w.user("teacher", "Z"), Y = w.user(), S = w.user();
    const tf = (k: boolean) => bankOf([{ id: "q", stem: "The mitochondrion is the powerhouse of the cell.", kind: { Tf: { correct: k } }, points: 1, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 }], 1);
    const a = (await publish(w, X, tf(true))).body;
    const b = (await publish(w, Z, tf(false))).body;
    await get(w, Y, a.item_id);
    await get(w, Y, b.item_id);
    const y = (await publish(w, Y, tf(key), { hideKey: true })).body;
    const chain = (await get(w, S, y.item_id)).body.questions[0].chain;
    assert.deepEqual(chain.map((l: Q) => [l.author_name, l.modified]), [["X", false], ["Z", false]]);
  }
});

test("x2/E2: the licence outcome of a hidden copy is the same for every key", async () => {
  const outs: string[] = [];
  for (const key of [true, false]) {
    const w = new World();
    const X = w.user(), Z = w.user(), Y = w.user();
    const tf = (k: boolean) => bankOf([{ id: "q", stem: "The mitochondrion is the powerhouse of the cell.", kind: { Tf: { correct: k } }, points: 1, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 }], 1);
    const a = (await publish(w, X, tf(true), { license: "CC-BY-SA-4.0" })).body;
    const b = (await publish(w, Z, tf(false), { license: "CC-BY-NC-4.0" })).body;
    await get(w, Y, a.item_id);
    await get(w, Y, b.item_id);
    const r = await publish(w, Y, tf(key), { hideKey: true, license: "CC-BY-NC-4.0" });
    outs.push(JSON.stringify([r.status, r.body.error, r.body.allowed]));
  }
  assert.equal(outs[0], outs[1]);
  assert.equal(outs[0], JSON.stringify([422, "license_incompatible", []]), "SA and NC sources of the same question cannot both be satisfied");
});

// ---- E3 ---------------------------------------------------------------------------------------------------

test("x4/S3c: read evidence is durable: it survives the source's retention and purge", async () => {
  const w = new World();
  const S = w.user(), Y = w.user();
  const tf = (stem: string) => ({ id: "q", stem, kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 });
  const q = tf("Photosynthesis happens in the chloroplast of plant cells.");
  let s = (await publish(w, S, bankOf([q, tf("Filler 0 for the source bank.")]), { license: "CC-BY-NC-SA-4.0" })).body;
  await get(w, Y, s.item_id);
  for (const n of [1, 2]) {
    const qs = [q, tf(`Filler ${n} for the source bank.`)].map((x, i) => ({ ...x, published: { item_id: s.item_id, qid: s.qids[i], publish_id: s.publish_id, version: s.version } }));
    s = (await publish(w, S, bankOf(qs), { license: "CC-BY-NC-SA-4.0", base: s.publish_id }, s.item_id)).body;
  }
  assert.equal((await publish(w, Y, bankOf([q]), { license: "CC0-1.0" })).body.error, "license_incompatible", "after two republishes");
  await w.call("DELETE", `/items/${s.item_id}`, { user: S });
  w.advance(15 * DAY);
  await w.pool.daily();
  assert.equal((await publish(w, Y, bankOf([q]), { license: "CC0-1.0" })).body.error, "license_incompatible", "after the purge");
});

test("s1b: a hidden-only licence change reaches earlier importers (both grants held)", async () => {
  const w = new World();
  const A = w.user(), B = w.user();
  const qa = { id: "q", stem: "What is the boiling point of water at sea level?", kind: { Mc: { answers: [{ text: "100 C", correct: true, locked: false }, { text: "50 C", correct: false, locked: false }, { text: "0 C", correct: false, locked: false }] } }, points: null, difficulty: null, topics: [], image: null, explanation: "secret", figure: null, created: 1, modified: 1 };
  const p1 = (await publish(w, A, bankOf([qa]), { hideKey: true, license: "CC-BY-NC-4.0" })).body;
  assert.deepEqual((await imp(w, B, p1.item_id)).body.may_republish_as, ["CC-BY-NC-4.0"]);
  const p2 = await publish(w, A, bankOf([{ ...qa, published: { item_id: p1.item_id, qid: p1.qids[0], publish_id: p1.publish_id, version: 1 } }]), { hideKey: true, license: "CC-BY-4.0", base: p1.publish_id }, p1.item_id);
  assert.equal(p2.body.public_version, 1);
  const i2 = await imp(w, B, p1.item_id);
  assert.equal(i2.body.item.license, "CC-BY-4.0");
  assert.deepEqual(i2.body.may_republish_as, ["CC-BY-4.0", "CC-BY-NC-4.0"]);
});

test("S2/E3d: an original author cannot relicense an adapter's additions; reading a copy of their own work still never constrains them", async () => {
  const w = new World();
  const A = w.user(), D = w.user();
  const qa = { id: "q", stem: "Explain the role of chlorophyll in photosynthesis today", kind: { Short: { sample: "absorbs light" } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 };
  const pa = (await publish(w, A, bankOf([qa]), { license: "CC-BY-4.0" })).body;
  const ia = await imp(w, D, pa.item_id);
  const pd = (await publish(w, D, bankOf([{ ...strip(ia.body.payload.bank.questions[0]), explanation: "D's long original explanation" }]), { license: "CC-BY-SA-4.0" })).body;
  const back = await imp(w, A, pd.item_id);
  assert.deepEqual(back.body.may_republish_as, ["CC-BY-SA-4.0"]);
  assert.equal((await publish(w, A, bankOf([strip(back.body.payload.bank.questions[0])]), { license: "CC0-1.0" })).body.error, "license_incompatible");
  // ...but A's own question, without D's explanation, stays theirs.
  assert.equal((await publish(w, A, bankOf([qa]), { license: "CC0-1.0" })).status, 201);
});

test("s7/E3c: an upheld takedown voids the infringer's evidence: the rightful author is neither bound nor credited; an admin can void or restore evidence", async () => {
  const w = new World();
  const admin = w.user("admin"), A = w.user("teacher", "Real Author"), X = w.user("teacher", "Infringer");
  const work = { id: "q", stem: "Real Author's workbook question on Newton's third law, page 12", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: "workbook explanation", figure: null, created: 1, modified: 1 };
  const px = (await publish(w, X, bankOf([work]), { license: "CC-BY-NC-SA-4.0" })).body;
  await imp(w, A, px.item_id);
  const td = await w.takedown<any>(admin, await notice(w, admin), px.item_id);
  await w.admin(admin, "POST", "/admin/dmca/court-action", { action_id: td.body.action_id });
  assert.equal((await w.admin(admin, "POST", "/admin/dmca/resolve", { action_id: td.body.action_id, outcome: "remove" })).status, 200);
  const pa = await publish(w, A, bankOf([work]), { license: "CC-BY-4.0" });
  assert.equal(pa.status, 201);
  assert.deepEqual(w.q<{ chain_json: string }>(`SELECT chain_json FROM item_questions WHERE publish_id = ?`, pa.body.publish_id)[0]!.chain_json, "[]");
  assert.ok(w.q(`SELECT * FROM pool_audit WHERE action = 'evidence.void'`).length >= 1);
  // The admin route: restore it, then void by account.
  const B = w.user(), Y = w.user();
  const pb = (await publish(w, B, bankOf([{ ...work, stem: "Another question entirely, about friction." }]), { license: "CC-BY-NC-4.0" })).body;
  await get(w, Y, pb.item_id);
  const copy = { ...work, stem: "Another question entirely, about friction." };
  assert.equal((await publish(w, Y, bankOf([copy]), { license: "CC0-1.0" })).status, 422);
  assert.equal((await w.admin(admin, "POST", "/admin/pool/evidence", { user_id: Y.id, action: "void", note: "sock puppet reads" })).status, 200);
  assert.equal((await publish(w, Y, bankOf([copy]), { license: "CC0-1.0" })).status, 201);
});

test("soft check: an item only READ (not imported) still raises the silent license_soft flag", async () => {
  const w = new World();
  const A = w.user(), B = w.user();
  const q = { id: "q", stem: "A distinctive question about the Krebs cycle.", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 };
  const pa = (await publish(w, A, bankOf([q]), { license: "CC0-1.0" })).body;
  await get(w, B, pa.item_id);
  const pb = await publish(w, B, bankOf([q]), { license: "CC0-1.0" });
  assert.equal(pb.status, 201);
  assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'license_soft' AND user_id = ?`, B.id).length, 1);
});

// ---- E4 ---------------------------------------------------------------------------------------------------

test("M2/E4: a full-only image is never blocked (no guess oracle); defaults are what the item introduced; sock-puppet items do not count", async () => {
  const w = new World();
  const admin = w.user("admin"), A = w.user(), B = w.user();
  const keyImg = png(9, 9, 42);
  const up = await upload(w, A, keyImg);
  const qa = { id: "q", stem: "Read the micropipette shown and record the volume", kind: { Short: { sample: "15.7" } }, points: null, difficulty: null, topics: [], image: { file: `media/${up.sha}.png`, width: 9, height: 9 }, explanation: null, figure: null, created: 1, modified: 1 };
  const pa = (await publish(w, A, bankOf([qa]), { hideKey: true, imageOk: [] })).body;
  const prev = await w.admin<any>(admin, "POST", "/admin/dmca/takedown-preview", { item_id: pa.item_id });
  assert.deepEqual(prev.body.blob, [], "the image was never served: it is not blockable");
  await w.takedown(admin, await notice(w, admin), pa.item_id);
  for (const bytes of [keyImg, png(9, 9, 43)]) {
    const u = await upload(w, B, bytes);
    await publish(w, B, bankOf([{ ...qa, stem: `My own pipette question ${u.sha.slice(0, 6)} here`, image: { file: `media/${u.sha}.png`, width: 9, height: 9 } }]));
  }
  const mine = await w.json<any>("GET", "/items/mine", { user: B });
  assert.deepEqual(mine.body.items.map((x: Q) => x.state), ["live", "live"], "a right guess and a wrong one look the same");
  // Introduced-first: a question the item merely copied from an earlier visible item is not a default.
  const w2 = new World();
  const adm2 = w2.user("admin"), early = w2.user(), bait = w2.user();
  const shared = { ...qa, image: null, stem: "Which organelle makes ATP in eukaryotes?" };
  await publish(w2, early, bankOf([shared]));
  const pb = (await publish(w2, bait, bankOf([shared, { ...qa, image: null, stem: "Jamie Smith (period 3) failed this." }]))).body;
  const pv = await w2.admin<any>(adm2, "POST", "/admin/pool/purge-preview", { item_id: pb.item_id });
  assert.deepEqual(pv.body.match.map((m: Q) => [m.pos, m.introduced, m.selected]), [[0, false, false], [1, true, true]]);
  // Sock puppets (unlisted items) neither make a question "common" nor "copied".
  const w3 = new World();
  const adm3 = w3.user("admin"), att = w3.user();
  const puppets = [w3.user(), w3.user(), w3.user()];
  const pii = { ...qa, image: null, stem: "Student Maria Lopez in room 12 failed the test." };
  for (const p of puppets) await publish(w3, p, bankOf([pii]), { visibility: "unlisted" });
  const pc = (await publish(w3, att, bankOf([pii]))).body;
  const pv3 = await w3.admin<any>(adm3, "POST", "/admin/pool/purge-preview", { item_id: pc.item_id });
  assert.deepEqual(pv3.body.match.map((m: Q) => [m.other_authors, m.introduced, m.selected]), [[0, true, true]]);
});

// ---- E5 ---------------------------------------------------------------------------------------------------

test("M1/E5: a privacy unblock within the grace window restores everything; after it, copies are named needs_reupload (never a silent loss)", async () => {
  for (const late of [false, true]) {
    const w = new World();
    const admin = w.user("admin"), A = w.user(), B = w.user(), D = w.user();
    const img = png(8, 8, 7);
    const up = await upload(w, A, img);
    const qi = { id: "q", stem: "Student Jane Doe got this wrong, see photo", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: { file: `media/${up.sha}.png`, width: 8, height: 8 }, explanation: null, figure: null, created: 1, modified: 1 };
    const pa = (await publish(w, A, bankOf([qi]))).body;
    const ib = await imp(w, B, pa.item_id);
    const pb = (await publish(w, B, bankOf(ib.body.payload.bank.questions.map(strip)))).body;
    assert.equal((await w.purge(admin, pa.item_id)).status, 200);
    assert.equal((await w.call("GET", `/items/${pb.item_id}/blobs/${up.sha}`, { user: B })).status, 404);
    if (late) {
      w.advance(15 * DAY);
      await w.pool.daily();
    }
    const ub = await w.admin<any>(admin, "POST", "/admin/pool/unblock", { kind: "blob", hash: up.sha });
    const mh = w.q<{ hash: string }>(`SELECT hash FROM privacy_blocks WHERE kind = 'match'`)[0]!.hash;
    await w.admin(admin, "POST", "/admin/pool/unblock", { kind: "match", hash: mh });
    if (!late) {
      assert.deepEqual([ub.body.bytes_lost, ub.body.needs_reupload], [false, []]);
      assert.equal((await w.call("GET", `/items/${pb.item_id}/blobs/${up.sha}`, { user: D })).status, 200, "fully restored");
    } else {
      assert.deepEqual([ub.body.bytes_lost, ub.body.needs_reupload], [true, [pb.item_id]]);
      const mine = await w.json<any>("GET", "/items/mine", { user: B });
      assert.equal(mine.body.items[0].state, "needs_reupload");
      assert.equal(w.q(`SELECT * FROM admin_flags WHERE kind = 'needs_reupload'`).length, 1);
      // B uploads the image again and republishes: the hold is answered.
      await upload(w, B, img);
      const g = await get(w, B, pb.item_id);
      const own = g.body.payload.bank.questions.map((q: Q, i: number) => ({ ...q, published: { item_id: pb.item_id, qid: pb.qids[i], publish_id: pb.publish_id, version: 1 } }));
      assert.equal((await publish(w, B, bankOf(own), { base: pb.publish_id }, pb.item_id)).status, 200);
      assert.equal((await get(w, D, pb.item_id)).status, 200);
    }
  }
});

// ---- E6 / E7 ----------------------------------------------------------------------------------------------

test("x5/E7: a hung publish body pins nothing: length required, bodies read (with a deadline) before the heavy slot", async () => {
  const w = new World({ bodyDeadlineMs: 150 });
  const attackers = Array.from({ length: 5 }, () => w.user());
  const victim = w.user();
  const hang = (u: User, withLength: boolean) =>
    w.pool.handle(
      new Request("https://api.example.org/items", {
        method: "POST",
        headers: { Authorization: `Bearer ${u.token}`, "CF-Connecting-IP": "10.9.9.9", "Content-Type": PUBLISH_CONTENT_TYPE, ...(withLength ? { "Content-Length": "1000" } : {}) },
        body: new ReadableStream({ start: (c) => c.enqueue(new TextEncoder().encode('{"publish_id":"')) }),
        duplex: "half",
      } as RequestInit),
    ) as Promise<Response>;
  assert.equal((await hang(attackers[0]!, false)).status, 411);
  const pending = attackers.map((a) => hang(a, true));
  const v = await publish(w, victim, bankOf([{ id: "q", stem: "A victim's ordinary question about cells.", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 }]));
  assert.equal(v.status, 201, "the victim is not queued behind hung bodies");
  for (const r of await Promise.all(pending)) assert.equal(r.status, 408);
  assert.equal(bytesReserved(), 0, "every reservation released");
});

test("E6: one byte budget per isolate for reads and publishes; reads stream; reservations always return to zero", async () => {
  const w = new World();
  const a = w.user(), b = w.user();
  const p = (await publish(w, a, bankOf([{ id: "q", stem: "Streamed question body test.", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 }]))).body;
  const raw = await w.pool.handle(new Request(`https://api.example.org/items/${p.item_id}`, { headers: { Authorization: `Bearer ${b.token}`, "CF-Connecting-IP": "10.0.0.2" } }));
  assert.ok(raw!.body instanceof ReadableStream);
  assert.ok(bytesReserved() > 0, "a read holds its reservation until its stream ends");
  const text = await raw!.text();
  assert.equal(JSON.parse(text).item.id, p.item_id);
  assert.equal(bytesReserved(), 0);
  // Over budget: 503 busy (for reads and publishes alike).
  const hog = reserveBytes(ISOLATE_BYTES_BUDGET);
  assert.equal((await w.call("GET", `/items/${p.item_id}`, { user: b })).status, 503);
  assert.equal((await publish(w, a, bankOf([{ id: "q", stem: "Another one entirely.", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 }]))).status, 503);
  hog();
  await w.call("GET", `/items/${"0".repeat(32)}`, { user: b });
  assert.equal(bytesReserved(), 0, "a 404 releases too");
});

// ---- non-blocking -----------------------------------------------------------------------------------------

test("x2: past the daily publish cap, requests are refused before any validation and count against the refusal budget", async () => {
  const w = new World();
  const U = w.user();
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES (?, 'publishes', 0, 0, 'x'), (?, 'publish_refusals', 3, 0, 'x')`).run(U.id, U.id);
  const codes: string[] = [];
  for (let i = 0; i < 8; i++) {
    const r = await publish(w, U, bankOf([{ id: "q", stem: `Distinct stem number ${i} for a validation run.`, kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 }]));
    codes.push(`${r.status}:${r.body.metric}`);
  }
  assert.deepEqual(codes, ["429:publishes", "429:publishes", "429:publishes", "429:publish_refusals", "429:publish_refusals", "429:publish_refusals", "429:publish_refusals", "429:publish_refusals"]);
});

test("a republish must be schema v2 (v1 would drop question identity)", async () => {
  const w = new World();
  const a = w.user();
  const q = { id: "q", stem: "Schema version test question.", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 };
  const p = (await publish(w, a, bankOf([q], 1))).body;
  const r = await publish(w, a, bankOf([q], 1), { base: p.publish_id }, p.item_id);
  assert.deepEqual([r.status, r.body.error], [422, "schema_upgrade_required"]);
});

test("double takedown in one millisecond: one action, one strike; a put-back lifts its holds and resumes the undo window", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user(), c = w.user();
  const q = { id: "q", stem: "A distinctive question about plate tectonics.", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 };
  const p = (await publish(w, a, bankOf([q]))).body;
  const n = await notice(w, admin);
  const [t1, t2] = await Promise.all([w.takedown<any>(admin, n, p.item_id), w.takedown<any>(admin, n, p.item_id)]);
  assert.deepEqual([t1.status, t2.status].sort(), [200, 409]);
  assert.equal(w.q(`SELECT * FROM strikes`).length, 1);
  const re = (await publish(w, c, bankOf([q]))).body;
  assert.equal(w.q(`SELECT * FROM item_holds WHERE item_id = ?`, re.item_id).length, 1);
  await w.admin(admin, "POST", "/admin/dmca/withdraw", { notice_id: n });
  assert.equal(w.q(`SELECT * FROM item_holds WHERE item_id = ?`, re.item_id).length, 0, "the put-back lifted the hold its block caused");
  // Undo window: deleted with 10 days left, taken down, put back 20 days later: 10 days left again.
  const w2 = new World();
  const adm2 = w2.user("admin"), a2 = w2.user();
  const p2 = (await publish(w2, a2, bankOf([q]))).body;
  await w2.json("DELETE", `/items/${p2.item_id}`, { user: a2 });
  w2.advance(4 * DAY);
  const n2 = await notice(w2, adm2);
  await w2.takedown(adm2, n2, p2.item_id);
  w2.advance(20 * DAY);
  await w2.admin(adm2, "POST", "/admin/dmca/withdraw", { notice_id: n2 });
  const row = w2.q<{ removed_reason: string; purge_after: number }>(`SELECT removed_reason, purge_after FROM items`)[0]!;
  assert.equal(row.removed_reason, "author_deleted");
  assert.equal(row.purge_after, w2.clock + 10 * DAY);
  await w2.pool.daily();
  assert.equal((await w2.json("POST", `/items/${p2.item_id}/undelete`, { user: a2 })).status, 200);
});

test("kept items of a revoked author are hidden; deleted authors appear as 'deleted teacher' in chains", async () => {
  const w = new World();
  const k = w.user("teacher", "Ken Keep"), b = w.user(), c = w.user();
  const q = { id: "q", stem: "Ken question about density and buoyancy ok", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 };
  const pk = (await publish(w, k, bankOf([q]))).body;
  await w.json("PUT", "/items/prefs", { user: k, json: { keep_on_delete: true } });
  const ib = await imp(w, b, pk.item_id);
  const pb = (await publish(w, b, bankOf(ib.body.payload.bank.questions.map(strip)))).body;
  w.setTrust(k, "pending");
  const { deleteContentStatements } = await import("../../src/pool/index.ts");
  w.sql.prepare(`UPDATE users SET display_name = NULL, email = NULL, pw_hash = NULL, deleted_at = ? WHERE id = ?`).run(w.clock, k.id);
  await w.db.batch(deleteContentStatements(w.db, k.id, w.clock));
  assert.equal((await get(w, c, pk.item_id)).status, 404, "a revoked author's kept items do not show");
  assert.deepEqual((await get(w, c, pb.item_id)).body.questions[0].chain.map((l: Q) => l.author_name), ["deleted teacher"]);
});

test("putBlob refuses privacy-blocked bytes; preview tokens are their own key (a cursor-signed token is not one)", async () => {
  const w = new World();
  const admin = w.user("admin"), a = w.user(), r = w.user();
  const img = png(8, 8, 11);
  const up = await upload(w, a, img);
  const p = (await publish(w, a, bankOf([{ id: "q", stem: "Student Jo Bloggs in period 2, see photo", kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: { file: `media/${up.sha}.png`, width: 8, height: 8 }, explanation: null, figure: null, created: 1, modified: 1 }]))).body;
  const list = await w.json<any>("GET", "/items?limit=1", { user: r });
  void list;
  const forged = await w.admin(admin, "POST", "/admin/pool/purge", { item_id: p.item_id, confirm_preview: "eyJ4IjoxfQ.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" });
  assert.equal(forged.status, 428);
  assert.equal((await w.purge(admin, p.item_id)).status, 200);
  const again = await upload(w, w.user(), img);
  assert.deepEqual([again.status, again.body.reason], [422, "not_accepted"]);
});

test("instant holds: past half the site cap, a student-data hold needs two distinct reporters", async () => {
  const w = new World();
  const a = w.user();
  w.sql.prepare(`INSERT INTO pool_limits (user_id, metric, value, updated_at, updated_by) VALUES ('*', 'instant_holds_global', 2, 0, 'x')`).run();
  const items: string[] = [];
  for (let i = 0; i < 3; i++) items.push((await publish(w, a, bankOf([{ id: "q", stem: `Question number ${i} about rivers and deltas.`, kind: { Tf: { correct: true } }, points: null, difficulty: null, topics: [], image: null, explanation: null, figure: null, created: 1, modified: 1 }]))).body.item_id);
  await w.json("POST", `/items/${items[0]}/report`, { user: w.user(), json: { reason: "student_data" } });
  assert.equal(w.q(`SELECT * FROM item_holds WHERE item_id = ?`, items[0]).length, 1);
  await w.json("POST", `/items/${items[1]}/report`, { user: w.user(), json: { reason: "student_data" } });
  assert.equal(w.q(`SELECT * FROM item_holds WHERE item_id = ?`, items[1]).length, 0, "one reporter is not enough any more");
  await w.json("POST", `/items/${items[1]}/report`, { user: w.user(), json: { reason: "student_data" } });
  assert.equal(w.q(`SELECT * FROM item_holds WHERE item_id = ?`, items[1]).length, 1, "a second reporter makes it");
});
