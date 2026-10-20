// The share-core adapter (src/pool/core.ts): the one file that knows share-core's API. Run: npm test
import assert from "node:assert/strict";
import { test } from "node:test";
import { makeCore, rawField, type ShareCoreExports } from "../../src/pool/core.ts";
import { core, enc, shareCore } from "./harness.ts";

test("rawField slices a top-level value byte for byte (strings, escapes, nesting, integer-like keys)", async () => {
  const j = `{"a":1,"payload":{"1":"x","b":[1,{"c":"}\\"]"}],"d":1.0e21},"z":"payload"}`;
  assert.equal(rawField(j, "payload"), `{"1":"x","b":[1,{"c":"}\\"]"}],"d":1.0e21}`);
  assert.equal(rawField(j, "z"), `"payload"`);
  assert.equal(rawField(j, "a"), "1");
  assert.equal(rawField(j, "missing"), null);
  assert.equal(rawField(`[1]`, "a"), null);
  // JSON.stringify would have reordered "1" first and printed 1e+21: the stored bytes are share-core's.
  assert.notEqual(JSON.stringify(JSON.parse(rawField(j, "payload")!)), rawField(j, "payload"));
});

test("the adapter returns share-core's canonical bytes, and refusals carry codes and paths only", async () => {
  const p = `{"schema_version":1,"question":{"id":"q","stem":"Is it?","kind":{"Tf":{"correct":true}},"points":2.0,"difficulty":null,"topics":[],"image":null,"explanation":null,"figure":null,"created":1,"modified":1}}`;
  const v = core.validateForPublish(enc(p), { hideKey: false, labels: [] });
  assert.ok(v.ok);
  const direct = JSON.parse(shareCore.validate_for_publish(enc(p), JSON.stringify({ hide_key: false, labels: [] })));
  // share-core writes floats as floats ("points":2.0); JSON.stringify would write 2. The adapter keeps share-core's bytes.
  assert.match(v.json, /"points":2\.0[,}]/);
  assert.notEqual(v.json, JSON.stringify(direct.payload));
  // A JS string is refused by share-core (bytes only): the adapter never passes one.
  assert.match(shareCore.validate(p as unknown as Uint8Array, ""), /expected_bytes/);
  const bad = core.validateForPublish(enc(p.replace("Is it?", "x‮y")), { hideKey: false, labels: [] });
  assert.equal(bad.ok, false);
  if (!bad.ok) {
    assert.deepEqual(Object.keys(bad.errors[0]!).sort().filter((k) => !["code", "path", "limit", "offset16", "cp"].includes(k)), []);
    assert.ok(!JSON.stringify(bad).includes("x‮y"));
  }
  assert.deepEqual(core.schema(), { min: 1, max: 2, stripped: 2 });
});

test("a share-core that grows image_check is used instead of the fallback; unknown result fields are dropped", async () => {
  const calls: string[] = [];
  const fake = {
    ...(shareCore as unknown as ShareCoreExports),
    image_check: (b: Uint8Array) => {
      calls.push(String(b.length));
      return b.length === 3 ? `{"ok":true,"content_type":"image/png","width":7,"height":9}` : `{"ok":false,"errors":[{"code":"png_chunk","path":"","secret":"zz"}]}`;
    },
  };
  const c = makeCore(fake);
  assert.deepEqual(await c.imageInfo(new Uint8Array(3)), { contentType: "image/png", width: 7, height: 9 });
  assert.deepEqual(await c.imageInfo(new Uint8Array(4)), { refused: "png_chunk" });
  assert.deepEqual(calls, ["3", "4"]);
  // Without it, the structural checks in image.ts.
  assert.deepEqual(await core.imageInfo(enc("GIF89a")), { refused: "truncated" });
});

test("strip through the adapter: hashes per question, the seed is share-core's (two strips may differ, both keyless)", async () => {
  const words = ["apple", "river", "stone", "cloud", "ember", "maple", "orbit", "quartz"];
  const answers = words.map((text, i) => ({ text, correct: i === 3, locked: false }));
  const p = JSON.stringify({ schema_version: 1, question: { id: "q", stem: "Pick", kind: { Mc: { answers } }, points: null, difficulty: null, topics: [], image: null, explanation: "because", figure: null, created: 1, modified: 1 } });
  const orders = new Set<string>();
  for (let i = 0; i < 12; i++) {
    const s = core.strip(enc(p), { labels: [], orderAcks: [], imageOk: [] });
    assert.ok(s.ok);
    if (s.ok) {
      assert.equal(s.matchHashes.length, 1);
      assert.ok(!s.json.includes('"correct":true') && !s.json.includes("because"));
      orders.add(JSON.stringify(s.servedOrders));
    }
  }
  assert.ok(orders.size > 1, "the shuffle is random (the Worker never picks the seed)");
});
