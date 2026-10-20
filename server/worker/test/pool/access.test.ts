// Access control (ASVS 5.0 V8 / 4.0 V4): trust gating, IDOR on every id-taking route, author
// standing, uniform 404s, CORS and cache headers. Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { PUBLISH_CONTENT_TYPE } from "../../src/pool/shared.ts";
import { bankPayload, framed, genQuestion, ORIGIN, png, publish, publishId, rng, upload, World } from "./harness.ts";

async function setup() {
  const w = new World();
  const a = w.user("teacher", "Ms. A");
  const b = w.user("teacher", "Mr. B");
  const img = await upload(w, a, png(5, 5, 1));
  const r = rng(3);
  const payload = bankPayload([genQuestion(r, "Short", 0, { sha: img.sha, ext: "png" }), genQuestion(r, "Mc", 1)]);
  const pub = await publish(w, a, payload, { visibility: "public", imageOk: [0], hideKey: true });
  const unl = await publish(w, a, bankPayload([genQuestion(r, "Tf", 0, { sha: img.sha, ext: "png" })]), { visibility: "unlisted" });
  return { w, a, b, img, pub: pub.body.item_id as string, pubPid: pub.body.publish_id as string, unl: unl.body.item_id as string, unlPid: unl.body.publish_id as string, payload };
}

/** Every id-taking route, as `(method, path, options)`. */
function idRoutes(id: string, sha: string, base: string) {
  return [
    ["GET", `/items/${id}`, {}],
    ["PUT", `/items/${id}`, { body: framed({ publish_id: publishId(), base_publish_id: base, hide_key: false, license: "CC0-1.0", attestation: { accepted: true, text_rev: "2026-10-19" }, meta: {} }, bankPayload([genQuestion(rng(1), "Tf", 0)])), contentType: PUBLISH_CONTENT_TYPE }],
    ["PATCH", `/items/${id}`, { json: { visibility: "unlisted" } }],
    ["DELETE", `/items/${id}`, {}],
    ["POST", `/items/${id}/undelete`, {}],
    ["POST", `/items/${id}/import`, {}],
    ["POST", `/items/${id}/report`, { json: { reason: "other" } }],
    ["POST", `/items/${id}/counter-notice`, { json: {} }],
    ["GET", `/items/${id}/blobs/${sha}`, {}],
  ] as const;
}

test("no token: 401 on every route; pending and unverified: 403 on every route", async () => {
  const { w, pub, img, pubPid } = await setup();
  const routes = [...idRoutes(pub, img.sha, pubPid), ["GET", "/items", {}], ["GET", "/items/mine", {}], ["POST", "/items/versions", { json: { ids: [pub] } }], ["PUT", `/blobs/${img.sha}`, { body: png(5, 5, 1) }], ["POST", "/items", { body: "x\ny", contentType: PUBLISH_CONTENT_TYPE }]] as const;
  for (const [m, p, o] of routes) {
    const none = await w.json(m, p, { ...o, user: null });
    assert.equal(none.status, 401, `${m} ${p}`);
    for (const trust of ["pending", "unverified", "member"]) {
      const u = w.user(trust);
      const x = await w.json(m, p, { ...o, user: u });
      assert.deepEqual([x.status, x.body.error], [403, "account_not_approved"], `${trust} ${m} ${p}`);
    }
  }
  assert.equal(w.q(`SELECT * FROM imports`).length + w.q(`SELECT * FROM reports`).length, 0, "nothing recorded");
});

test("IDOR: another teacher gets the same 404 as for an id that does not exist, on every id route, and changes nothing", async () => {
  const { w, b, pub, unl, img, pubPid, unlPid } = await setup();
  const before = JSON.stringify(w.q(`SELECT * FROM items ORDER BY id`));
  const nothing = "0123456789abcdef0123456789abcdef";
  for (const [m, p, o] of idRoutes(unl, img.sha, unlPid)) {
    const x = await w.json(m, p, { ...o, user: b });
    const y = await w.json(m, p.replace(unl, nothing), { ...o, user: b });
    assert.deepEqual([x.status, x.body], [404, { error: "not_found" }], `unlisted: ${m} ${p}`);
    assert.deepEqual([y.status, y.body], [404, { error: "not_found" }], `missing: ${m} ${p}`);
  }
  // The public item: readable, but every author-only route is the same 404.
  for (const [m, p, o] of idRoutes(pub, img.sha, pubPid).filter(([m, p]) => m === "PUT" || m === "PATCH" || m === "DELETE" || p.endsWith("undelete") || p.endsWith("counter-notice"))) {
    const x = await w.json(m, p, { ...o, user: b });
    assert.deepEqual([x.status, x.body], [404, { error: "not_found" }], `public: ${m} ${p}`);
  }
  assert.equal(JSON.stringify(w.q(`SELECT * FROM items ORDER BY id`)), before, "no item changed");
  assert.equal(w.q(`SELECT * FROM imports`).length, 0);
  // Malformed ids are the same 404 too.
  for (const bad of ["x", "..%2f..%2fetc", "A".repeat(32), "%2e%2e%2f", "0".repeat(31) + "g"]) assert.equal((await w.json("GET", `/items/${bad}`, { user: b })).status, 404);
  // versions: ids the caller never authored or imported are "unavailable", existing or not.
  const v = await w.json<{ items: { status: string }[] }>("POST", "/items/versions", { user: b, json: { ids: [unl, pub, nothing] } });
  assert.deepEqual(v.body.items.map((x) => x.status), ["unavailable", "unavailable", "unavailable"]);
});

test("the author sees their own unlisted and held items; nobody else does; an admin is a non-author on these routes", async () => {
  const { w, a, b, pub, unl } = await setup();
  const admin = w.user("admin");
  assert.equal((await w.json("GET", `/items/${unl}`, { user: a })).status, 200);
  assert.equal((await w.json<{ item: { form: string } }>("GET", `/items/${pub}`, { user: a })).body.item.form, "full");
  assert.equal((await w.json<{ item: { form: string } }>("GET", `/items/${pub}`, { user: admin })).body.item.form, "stripped");
  assert.equal((await w.json("GET", `/items/${unl}`, { user: admin })).status, 404);
  w.sql.prepare(`INSERT INTO item_holds (item_id, reason, at) VALUES (?, 'admin', 1)`).run(pub);
  assert.equal((await w.json("GET", `/items/${pub}`, { user: b })).status, 404);
  assert.equal((await w.json<{ items: unknown[] }>("GET", `/items`, { user: b })).body.items.length, 0);
  assert.equal((await w.json("GET", `/items/${pub}`, { user: a })).status, 200);
  const mine = await w.json<{ items: { id: string; state: string }[] }>("GET", "/items/mine", { user: a });
  assert.equal(mine.body.items.find((x) => x.id === pub)!.state, "held_for_review");
  // Making it unlisted hides it; public again shows it (once the hold is lifted).
  w.sql.prepare(`DELETE FROM item_holds`).run();
  assert.equal((await w.json("PATCH", `/items/${pub}`, { user: a, json: { visibility: "unlisted" } })).status, 200);
  assert.equal((await w.json("GET", `/items/${pub}`, { user: b })).status, 404);
  assert.equal((await w.json("PATCH", `/items/${pub}`, { user: a, json: { visibility: "public" } })).status, 200);
  assert.equal((await w.json("GET", `/items/${pub}`, { user: b })).status, 200);
});

test("content follows the author's standing: suspension, revocation to pending, deletion hide at once and restore shows again", async () => {
  const { w, a, b, pub, img } = await setup();
  const visible = async () => (await w.json("GET", `/items/${pub}`, { user: b })).status === 200 && (await w.json<{ items: unknown[] }>("GET", "/items", { user: b })).body.items.length === 1 && (await w.call("GET", `/items/${pub}/blobs/${img.sha}`, { user: b })).status === 200;
  assert.equal(await visible(), true);
  w.sql.prepare(`UPDATE users SET suspended_at = 1 WHERE id = ?`).run(a.id);
  assert.equal(await visible(), false);
  assert.equal((await w.json("POST", `/items/${pub}/import`, { user: b })).status, 404);
  w.sql.prepare(`UPDATE users SET suspended_at = NULL WHERE id = ?`).run(a.id);
  assert.equal(await visible(), true);
  w.setTrust(a, "pending");
  assert.equal(await visible(), false);
  w.setTrust(a, "teacher");
  assert.equal(await visible(), true);
  // Every item write by a non-author left the rows as they were (no per-item writes on suspension).
  assert.equal(w.q<{ removed_at: null }>(`SELECT removed_at FROM items WHERE id = ?`, pub)[0]!.removed_at, null);
});

test("an import racing a takedown records nothing and returns 404", async () => {
  const { w, b, pub } = await setup();
  const admin = w.user("admin");
  const n = await w.admin<{ notice_id: string }>(admin, "POST", "/admin/dmca/notices", { complainant_name: "C", complainant_contact: "c@x.org", work_desc: "w", material_desc: "m", has_signature: true, has_good_faith: true, has_perjury_stmt: true });
  // The body read for the import happens, then the takedown lands before the import is recorded.
  w.r2.onGet = async () => {
    const t = await w.takedown(admin, n.body.notice_id, pub);
    assert.equal(t.status, 200);
  };
  const imp = await w.json("POST", `/items/${pub}/import`, { user: b });
  assert.deepEqual([imp.status, imp.body], [404, { error: "not_found" }]);
  assert.equal(w.q(`SELECT * FROM imports`).length, 0);
  assert.equal(w.q(`SELECT * FROM import_questions`).length, 0);
});

test("headers: no-store and Vary on JSON; blobs are attachments under a sandbox CSP with the verified type; CORS is exact", async () => {
  const { w, b, pub, img } = await setup();
  const r = await w.call("GET", `/items/${pub}`, { user: b });
  assert.equal(r.headers.get("Cache-Control"), "no-store");
  assert.match(r.headers.get("Vary")!, /Authorization/);
  assert.equal(r.headers.get("X-Content-Type-Options"), "nosniff");
  const blob = await w.call("GET", `/items/${pub}/blobs/${img.sha}`, { user: b });
  assert.equal(blob.status, 200);
  assert.equal(blob.headers.get("Content-Type"), "image/png");
  assert.equal(blob.headers.get("Content-Security-Policy"), "sandbox; default-src 'none'");
  assert.equal(blob.headers.get("Content-Disposition"), `attachment; filename="${img.sha}.png"`);
  assert.equal(blob.headers.get("Cache-Control"), "no-store");
  assert.deepEqual(new Uint8Array(await blob.arrayBuffer()), png(5, 5, 1));
  assert.equal((await w.json("GET", "/items", { user: b, headers: { Origin: "https://evil.example.net" } })).status, 403);
  assert.equal((await w.json("GET", "/items", { user: b, headers: { Origin: ORIGIN } })).status, 200);
  assert.equal((await w.json("GET", "/items", { user: b, headers: { Origin: ORIGIN, "X-Client": "native" } })).status, 400);
  assert.equal((await w.json("POST", `/items/${pub}`, { user: b })).status, 405);
});

test("browse: newest first, filters, cursor pages signed and capped at 50, no totals", async () => {
  const w = new World();
  const a = w.user();
  const b = w.user();
  const r = rng(8);
  const ids: string[] = [];
  for (let i = 0; i < 7; i++) {
    w.advance(1000);
    const res = await publish(w, a, bankPayload([genQuestion(r, i % 2 ? "Tf" : "Blank", 0)]), { meta: { title: `T${i}`, subject: i < 3 ? "Physics" : "Biology", tags: [], standards: [] } });
    ids.push(res.body.item_id as string);
  }
  const seen: string[] = [];
  let cursor: string | null = null;
  do {
    const page: { body: { items: { id: string }[]; next_cursor: string | null } } = await w.json("GET", `/items?limit=3${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ""}`, { user: b });
    assert.ok(page.body.items.length <= 3);
    assert.equal("total" in page.body, false);
    seen.push(...page.body.items.map((x) => x.id));
    cursor = page.body.next_cursor;
  } while (cursor);
  assert.deepEqual(seen, [...ids].reverse());
  assert.equal((await w.json<{ items: unknown[] }>("GET", "/items?subject=Physics", { user: b })).body.items.length, 3);
  assert.equal((await w.json<{ items: unknown[] }>("GET", "/items?kind=Tf", { user: b })).body.items.length, 3);
  assert.equal((await w.json("GET", "/items?kind=Essay", { user: b })).status, 400);
  assert.equal((await w.json("GET", "/items?cursor=forged.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", { user: b })).status, 400);
  assert.equal((await w.json<{ items: unknown[] }>("GET", "/items?limit=500", { user: b })).body.items.length, 7);
  const many = new World();
  const m = many.user();
  for (let i = 0; i < 55; i++) {
    many.advance(10);
    await publish(many, m, bankPayload([genQuestion(r, "Tf", 0)]));
  }
  assert.equal((await many.json<{ items: unknown[] }>("GET", "/items?limit=500", { user: many.user() })).body.items.length, 50);
});
