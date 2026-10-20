// THE hard property (docs/shared-pool.md, docs/phase2-loop.md): no non-author read path (list,
// mine, get, import, versions, blob, report, every error body, a derived publish's answer) ever
// contains answer-key material of a hide_key publish. Seeded random banks and questions of every
// kind, stripped by the REAL share-core production wasm. Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { bankPayload, framed, genQuestion, KINDS, png, publish, publishId, questionPayload, rng, upload, World, decideLabels, type GenQuestion, type Rng, type User } from "./harness.ts";
import { PUBLISH_CONTENT_TYPE } from "../../src/pool/shared.ts";

const SEEDS = Number(process.env.POOL_PROPERTY_SEEDS ?? 40);
const LICENSES = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"];

/** Every structural trace of a key in a served stripped payload. */
function keyTraces(v: unknown, path = "$", out: string[] = []): string[] {
  if (Array.isArray(v)) v.forEach((x, i) => keyTraces(x, `${path}[${i}]`, out));
  else if (v && typeof v === "object") {
    const o = v as Record<string, unknown>;
    if ("correct" in o && o.correct !== false) out.push(`${path}.correct`);
    if ("explanation" in o && o.explanation !== null) out.push(`${path}.explanation`);
    if ("figure" in o && o.figure !== null) out.push(`${path}.figure`);
    if ("sample" in o && o.sample !== "") out.push(`${path}.sample`);
    if (Array.isArray(o.accept) && o.accept.some((a) => !Array.isArray(a) || a.length > 0)) out.push(`${path}.accept`);
    if (Array.isArray(o.pairs) && o.pairs.some((p) => (p as { right: string }).right !== "")) out.push(`${path}.pairs`);
    for (const [k, x] of Object.entries(o)) keyTraces(x, `${path}.${k}`, out);
  }
  return out;
}

function questionsIn(payload: Record<string, unknown>): Record<string, unknown>[] {
  const b = payload.bank as { questions: Record<string, unknown>[] } | undefined;
  return b ? b.questions : [payload.question as Record<string, unknown>];
}

interface Built {
  id: string;
  publishId: string;
  qs: GenQuestion[];
  payload: string;
  ticked: Set<string>;
  shas: string[];
  hide: boolean;
}

async function build(w: World, author: User, r: Rng, hide: boolean): Promise<Built> {
  const nImages = r.int(4);
  const imgs: { sha: string; ext: string }[] = [];
  for (let i = 0; i < nImages; i++) {
    const u = await upload(w, author, png(2 + r.int(6), 2 + r.int(6), r.int(1_000_000)));
    assert.equal(u.status, 201);
    imgs.push({ sha: u.sha, ext: "png" });
  }
  const single = r.bool(0.25);
  const n = single ? 1 : 1 + r.int(9);
  const qs: GenQuestion[] = [];
  for (let i = 0; i < n; i++) qs.push(genQuestion(r, i < KINDS.length && !single ? KINDS[i]! : r.pick(KINDS), i, i < imgs.length ? imgs[i] : undefined));
  const payload = single ? questionPayload(qs[0]!) : bankPayload(qs);
  const imageOk = qs.map((q, i) => (q.image && r.bool() ? i : -1)).filter((i) => i >= 0);
  const ticked = new Set(imageOk.map((i) => qs[i]!.image!));
  const res = await publish(w, author, payload, { hideKey: hide, imageOk, license: r.pick(LICENSES), meta: { title: `Bank ${r.int(1000)}`, subject: r.pick(["Biology", "Physics"]), tags: [], standards: [] } });
  assert.ok(res.status === 201, `publish: ${JSON.stringify(res.body)}`);
  return { id: res.body.item_id as string, publishId: res.body.publish_id as string, qs, payload, ticked, shas: imgs.map((x) => x.sha), hide };
}

test(`no non-author read path contains an answer key (${SEEDS} seeded worlds, real share-core)`, async () => {
  let sweeps = 0;
  let responses = 0;
  for (let seed = 1; seed <= SEEDS; seed++) {
    const r = rng(seed * 7919);
    const w = new World();
    const author = w.user("teacher", "Ms. Author");
    const other = w.user("teacher", "Mr. Other");
    const admin = w.user("admin", "Site Admin");
    const pending = w.user("pending", "Not Yet");

    // A hide_key item, sometimes first published WITH its key and then republished hiding it.
    let item = await build(w, author, r, r.bool(0.7));
    if (!item.hide) {
      const sent = JSON.parse(item.payload) as Record<string, unknown>;
      sent.schema_version = 2; // `published` is a schema-v2 field
      const qids = (await w.json<{ questions: { qid: string }[] }>("GET", `/items/${item.id}`, { user: author })).body.questions.map((q) => q.qid);
      questionsIn(sent).forEach((q, i) => (q.published = { item_id: item.id, qid: qids[i], publish_id: item.publishId, version: 1 }));
      const again = JSON.stringify(sent);
      const imageOk = item.qs.map((q, i) => (q.image && item.ticked.has(q.image) ? i : -1)).filter((i) => i >= 0);
      const re = await publish(w, author, again, { hideKey: true, imageOk, base: item.publishId }, item.id);
      assert.equal(re.status, 200, JSON.stringify(re.body));
      assert.deepEqual(re.body.qids, qids, "republish keeps the qids it was sent");
      item = { ...item, publishId: re.body.publish_id as string, payload: again, hide: true };
    }
    // A plain (key-visible) item alongside, so lists mix both kinds.
    const plain = await build(w, author, r, false);

    // ---- secrets: everything that is answer-key material for a non-author of `item` ----
    const secrets = new Set<string>(item.qs.flatMap((q) => q.secrets));
    for (const s of item.shas) if (!item.ticked.has(s)) secrets.add(s);
    for (const row of w.q<{ full_sha256: string; full_content: string }>(`SELECT full_sha256, full_content FROM item_questions WHERE publish_id IN (SELECT publish_id FROM item_publishes WHERE item_id = ?)`, item.id)) {
      secrets.add(row.full_sha256);
      secrets.add(row.full_content);
    }
    for (const row of w.q<{ full_key: string; request_sha256: string }>(`SELECT full_key, request_sha256 FROM item_publishes WHERE item_id = ?`, item.id)) {
      secrets.add(row.full_key);
      secrets.add(row.full_key.split("/").pop()!.replace(".json", ""));
      secrets.add(row.request_sha256);
    }
    // Sanity: the author really is served all of it (the sweep below could not pass vacuously).
    const mine = await w.text("GET", `/items/${item.id}`, { user: author });
    for (const s of item.qs.flatMap((q) => q.secrets)) assert.ok(mine.text.includes(s), `author is served ${s}`);

    const texts: { what: string; status: number; text: string }[] = [];
    const add = (what: string, x: { status: number; text: string }) => texts.push({ what, ...x });
    const servedBodies: string[] = [];
    for (const v of [other, admin]) {
      // Browse, with every filter, every page.
      for (const qs of ["", "?scope=public", "?scope=mine", "?subject=Biology", "?subject=Physics", ...KINDS.map((k) => `?kind=${k}`), "?limit=1"]) {
        let path: string | null = `/items${qs}`;
        while (path) {
          const x = await w.text("GET", path, { user: v });
          add(`list ${path}`, x);
          const next = (JSON.parse(x.text) as { next_cursor?: string | null }).next_cursor;
          path = next ? `/items${qs ? qs + "&" : "?"}cursor=${encodeURIComponent(next)}` : null;
        }
      }
      add("mine", await w.text("GET", "/items/mine", { user: v }));
      const got = await w.text("GET", `/items/${item.id}`, { user: v });
      add("get", got);
      assert.equal(got.status, 200);
      const imp = await w.text("POST", `/items/${item.id}/import`, { user: v });
      add("import", imp);
      assert.equal(imp.status, 200);
      for (const t of [got.text, imp.text]) {
        const body = JSON.parse(t) as { payload: Record<string, unknown>; item: { form: string }; questions: { served_sha256: string }[] };
        assert.equal(body.item.form, "stripped");
        assert.deepEqual(keyTraces(body.payload), [], `seed ${seed}: structural key traces`);
        for (const q of questionsIn(body.payload)) assert.equal(q.keyless, true);
        servedBodies.push(JSON.stringify(body.payload));
      }
      add("versions", await w.text("POST", "/items/versions", { user: v, json: { ids: [item.id, plain.id, "f".repeat(32)] } }));
      for (const s of item.shas) {
        const b = await w.call("GET", `/items/${item.id}/blobs/${s}`, { user: v });
        if (item.ticked.has(s)) assert.equal(b.status, 200, "a ticked image is served");
        else {
          assert.equal(b.status, 404, "an unticked image is never served to a non-author");
          add("blob 404", { status: b.status, text: await b.text() });
        }
      }
      add("blob bad", await w.text("GET", `/items/${item.id}/blobs/${"0".repeat(64)}`, { user: v }));
      // Every write on someone else's item: the error bodies.
      const stripped = JSON.parse(imp.text).payload as Record<string, unknown>;
      add(
        "put other",
        await w.text("PUT", `/items/${item.id}`, {
          user: v,
          body: framed({ publish_id: publishId(), base_publish_id: item.publishId, hide_key: true, license: "CC-BY-4.0", attestation: { accepted: true, text_rev: "2026-10-19" }, labels: [], meta: {} }, JSON.stringify(stripped)),
          contentType: PUBLISH_CONTENT_TYPE,
        }),
      );
      add("patch other", await w.text("PATCH", `/items/${item.id}`, { user: v, json: { visibility: "unlisted" } }));
      add("delete other", await w.text("DELETE", `/items/${item.id}`, { user: v }));
      add("undelete other", await w.text("POST", `/items/${item.id}/undelete`, { user: v }));
      add("counter other", await w.text("POST", `/items/${item.id}/counter-notice`, { user: v, json: { signature: "x" } }));
      add("report", await w.text("POST", `/items/${item.id}/report`, { user: v, json: { reason: "wrong_answer", note: "looks off" } }));
      add("upload", await w.text("PUT", `/blobs/${"a".repeat(64)}`, { user: v, body: png(3, 3, 99), contentType: "image/png" }));
      // The importer republishes the keyless copy (hide_key is required for keyless questions).
      const copy = JSON.stringify(stripped);
      const derived = await publish(w, v, copy, { hideKey: true, labels: decideLabels(copy), license: "CC-BY-NC-SA-4.0" });
      add("derived publish", { status: derived.status, text: JSON.stringify(derived.body) });
      if (derived.status === 201) {
        const dget = await w.text("GET", `/items/${derived.body.item_id as string}`, { user: v });
        add("derived get (own)", dget);
      }
    }
    // A pending account is refused everywhere, with a body that says nothing.
    for (const [m, p] of [["GET", `/items/${item.id}`], ["POST", `/items/${item.id}/import`], ["GET", "/items"], ["GET", `/items/${item.id}/blobs/${item.shas[0] ?? "0".repeat(64)}`]] as const) {
      const x = await w.text(m, p, { user: pending });
      assert.equal(x.status, 403);
      add(`pending ${m} ${p}`, x);
    }
    // Shuffled once at publish: every non-author read is byte-identical.
    assert.ok(servedBodies.every((b) => b === servedBodies[0]), "one stored stripped form for everyone");

    for (const t of texts) {
      responses++;
      for (const s of secrets) assert.ok(!t.text.includes(s), `seed ${seed}: ${t.what} (${t.status}) leaks key material ${s.slice(0, 24)}`);
    }
    sweeps++;
  }
  assert.equal(sweeps, SEEDS);
  assert.ok(responses > SEEDS * 40, `swept ${responses} responses`);
});

test("guessed keys get identical answers: the hard check is no key oracle", async () => {
  const w = new World();
  const a = w.user("teacher", "Ms. A");
  const b = w.user("teacher", "Mr. B");
  const r = rng(5);
  const tf = genQuestion(r, "Tf", 0);
  const mc = genQuestion(r, "Mc", 1);
  const orig = await publish(w, a, bankPayload([tf, mc]), { hideKey: true, license: "CC-BY-SA-4.0" });
  assert.equal(orig.status, 201);
  const imp = await w.json<{ payload: { bank: { questions: Record<string, unknown>[] } } }>("POST", `/items/${orig.body.item_id as string}/import`, { user: b });
  const served = imp.body.payload.bank.questions;
  // B fills in every possible key and tries to publish each under an incompatible licence.
  const answers: string[] = [];
  for (const tfKey of [true, false])
    for (let k = 0; k < (served[1]!.kind as { Mc: { answers: unknown[] } }).Mc.answers.length; k++) {
      const qs = structuredClone(served);
      for (const q of qs) {
        delete q.keyless;
        q.explanation = null;
      }
      (qs[0]!.kind as { Tf: { correct: boolean } }).Tf.correct = tfKey;
      (qs[1]!.kind as { Mc: { answers: { correct: boolean }[] } }).Mc.answers.forEach((x, j) => (x.correct = j === k));
      const payload = JSON.stringify({ schema_version: 2, bank: { id: "b", name: "guess", questions: qs, created: 1, modified: 1 } });
      const res = await publish(w, b, payload, { hideKey: false, license: "CC-BY-4.0" });
      answers.push(`${res.status} ${JSON.stringify(res.body)}`);
    }
  assert.ok(answers.length >= 4);
  assert.ok(answers.every((x) => x === answers[0]), `every guess answered alike:\n${answers.join("\n")}`);
  assert.match(answers[0]!, /^422 .*license_incompatible.*"positions":\[0,1\]/);
});

/** A served payload with every per-publish random choice removed: ids, times, and the shuffle order. */
function canonicalServed(v: unknown): unknown {
  if (Array.isArray(v)) {
    const xs = v.map(canonicalServed);
    return xs.every((x) => typeof x === "string") ? [...xs].sort() : [...xs].sort((a, b) => (JSON.stringify(a) < JSON.stringify(b) ? -1 : 1));
  }
  if (v && typeof v === "object") {
    const o: Record<string, unknown> = {};
    for (const [k, x] of Object.entries(v as Record<string, unknown>)) if (!["id", "created", "modified"].includes(k)) o[k] = canonicalServed(x);
    return o;
  }
  return v;
}
const DROP = new Set(["id", "item_id", "publish_id", "qid", "committed_at", "served_sha256", "next_cursor"]);
function observable(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(observable);
  if (v && typeof v === "object") return Object.fromEntries(Object.entries(v as Record<string, unknown>).filter(([k]) => !DROP.has(k)).map(([k, x]) => [k, k === "payload" ? canonicalServed(x) : observable(x)]));
  return v;
}

/** Change the key of a full (keyed) question in place; true if anything changed. */
function changeKey(q: Record<string, any>, r: Rng): boolean {
  const k = q.kind;
  if (k.Tf) k.Tf.correct = !k.Tf.correct;
  else if (k.Mc) {
    const n = k.Mc.answers.length;
    const was = k.Mc.answers.findIndex((a: { correct: boolean }) => a.correct);
    k.Mc.answers.forEach((a: { correct: boolean }, j: number) => (a.correct = j === (was + 1 + r.int(n - 1)) % n));
  } else if (k.Matching) {
    const p = k.Matching.pairs;
    [p[0].right, p[1].right] = [p[1].right, p[0].right];
  } else if (k.Short) k.Short.sample = `${k.Short.sample} (corrected)`;
  else if (k.Blank) k.Blank.accept[0] = [`${k.Blank.accept[0][0]}x`];
  else return false;
  if (q.explanation) q.explanation = `${q.explanation} corrected`;
  return true;
}

test("A1: correcting the key of an imported question and republishing it hidden changes nothing a non-author can observe", async () => {
  let compared = 0;
  for (let seed = 1; seed <= 24; seed++) {
    const r = rng(seed * 104729);
    const kind = ["Tf", "Mc", "Matching", "Short", "Blank"][seed % 5]!;
    // The source either shows its key (B imports the full form) or hides it (B fills in a key).
    const srcHidden = seed % 2 === 0;
    const views: string[] = [];
    for (const correct of [false, true]) {
      const w = new World();
      const A = w.user("teacher", "Ms. A");
      const B = w.user("teacher", "Mr. B");
      const Eve = w.user("teacher", "Eve");
      const rr = rng(seed); // the same questions in both worlds
      const qs = [genQuestion(rr, kind, 0), genQuestion(rr, "Tf", 1)];
      const src = await publish(w, A, bankPayload(qs), { hideKey: srcHidden, license: "CC-BY-4.0" });
      assert.equal(src.status, 201);
      const imp = await w.json<any>("POST", `/items/${src.body.item_id}/import`, { user: B });
      const copy = imp.body.payload.bank.questions.map((q: Record<string, any>) => {
        const x = structuredClone(q);
        if (srcHidden) {
          // A keyless import: B adds a key (the original one in world 1, a different one in world 2).
          const orig = qs.find((g) => (g.json.stem as string) === x.stem)!.json as Record<string, any>;
          x.kind = structuredClone(orig.kind);
          delete x.keyless;
        }
        return x;
      });
      if (correct) assert.ok(changeKey(copy[0], r), "a key was changed");
      const payload = JSON.stringify({ schema_version: 2, bank: { id: "b", name: "B's bank", questions: copy, created: 1, modified: 1 } });
      const pb = await publish(w, B, payload, { hideKey: true, license: "CC-BY-4.0", meta: { title: "B", subject: "Bio", tags: [], standards: [] } });
      assert.equal(pb.status, 201, JSON.stringify(pb.body));
      const id = pb.body.item_id as string;
      const seen: unknown[] = [];
      for (const [m, p, o] of [
        ["GET", `/items/${id}`, {}],
        ["POST", `/items/${id}/import`, {}],
        ["GET", `/items?subject=Bio`, {}],
        ["GET", `/items?kind=${kind}`, {}],
        ["POST", "/items/versions", { json: { ids: [id] } }],
        ["POST", `/items/${id}/report`, { json: { reason: "other" } }],
      ] as const) {
        const t = await w.text(m, p, { ...o, user: Eve });
        const body = JSON.parse(t.text);
        if (body.items) body.items = body.items.filter((x: { author_name: string }) => x.author_name === "Mr. B");
        seen.push([t.status, observable(body)]);
        if (body.payload) seen.push(["served length", JSON.stringify(canonicalServed(body.payload)).length]);
      }
      views.push(JSON.stringify(seen));
    }
    assert.equal(views[0], views[1], `seed ${seed} (${kind}, source ${srcHidden ? "hidden" : "shown"}): a non-author can tell the key changed`);
    compared++;
  }
  assert.equal(compared, 24);
});
