// One payload through a fresh wasm instance; prints {mode, bytes, growth, ms, code} as JSON.
// Usage: node tests/node/mem.mjs <pkg dir> <mode> [validate|strip]
// The modes are the critics' hostile inputs (scratchpad critic11b/node/mem.mjs) plus the legit bank.
import { readFileSync } from "node:fs";
const [pkg, mode, fn = "strip"] = process.argv.slice(2);
const m = await import(pkg + "/share_core.js");
m.initSync({ module: readFileSync(pkg + "/share_core_bg.wasm") });
const mem0 = m.memory_bytes();
let p;
const bank = (qs) => `{"schema_version":1,"bank":{"id":"b","name":"n","created":1,"modified":2,"questions":[${qs.join(",")}]}}`;
if (mode === "many_choices") {
  const L = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
  const part = (k) => `{"stem":"p","kind":{"Mc":{"answers":[${Array.from({ length: 26 }, (_, j) => `{"text":"${L[j]}. w${k}${j}${"x".repeat(25)}"${j === 0 ? ',"correct":true' : ""}}`).join(",")}]}}}`;
  p = bank(Array.from({ length: 41 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Multipart":{"parts":[${Array.from({ length: 50 }, (_, k) => part(k)).join(",")}]}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode === "params") {
  const ps = Array.from({ length: 180 }, (_, k) => `"p${k}":"xxxxxx"`).join(",");
  p = bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":1,"modified":1,"figure":{"id":"f","params":{${ps}}}}`));
} else if (mode === "nfc") {
  const N = String.fromCodePoint(0x1d160);
  const qs = [];
  let len = 200;
  for (let i = 0; i < 1000; i++) {
    const ans = Array.from({ length: 26 }, (_, j) => `{"text":"${N.repeat(169)}${String.fromCharCode(97 + j)}","correct":${j === 0}}`).join(",");
    const s = `{"id":"q${i}","stem":"${N.repeat(682)}","explanation":"${N.repeat(682)}","kind":{"Mc":{"answers":[${ans}]}},"points":null,"difficulty":null,"created":1,"modified":1}`;
    const n = new TextEncoder().encode(s).length;
    if (len + n > 4_190_000) break;
    len += n + 1;
    qs.push(s);
  }
  p = bank(qs);
} else if (mode === "objs") {
  p = `{"schema_version":1,"question":{"x":[${'{"":0},'.repeat(248_999)}{"":0}]}}`;
} else if (mode === "objs2") {
  const unit = '{"":{"":{"":{"":{"":{"":{"":{"":{"":{"":{"":{"":0}}}}}}}}}}}},';
  p = `{"schema_version":1,"question":{"x":[${unit.repeat(Math.floor(499_000 / 13))}0]}}`;
} else if (mode === "strs") {
  p = `{"schema_version":1,"question":{"x":[${'"",'.repeat(498_999)}""]}}`;
} else if (mode === "kept_objects") {
  const parts = Array(50).fill('{"stem":"p","kind":{"Tf":{"correct":true}}}').join(",");
  p = bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Multipart":{"parts":[${parts}]}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode === "tiny_answers") {
  const ans = Array(26).fill('{"text":"a"}').join(",");
  p = bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Mc":{"answers":[${ans}]}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode === "labels") {
  const lab = "A. ".repeat(640), lab2 = "B. ".repeat(640);
  p = bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"s","kind":{"Mc":{"answers":[{"text":"${lab}x","correct":true},{"text":"${lab2}y"}]}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode === "long_strings") {
  p = bank(Array.from({ length: 1000 }, (_, i) => `{"id":"q${i}","stem":"${"s".repeat(4000)}","kind":{"Tf":{"correct":true}},"points":null,"difficulty":null,"created":1,"modified":1}`));
} else if (mode === "legit") {
  p = JSON.stringify({
    schema_version: 1,
    bank: {
      id: "b", name: "Big", created: 0, modified: 0,
      questions: Array.from({ length: 1000 }, (_, i) => ({
        id: `q${i}`, stem: "S".repeat(600), points: 1, difficulty: null, created: 0, modified: 0, explanation: "E".repeat(400),
        kind: { Mc: { answers: Array.from({ length: 26 }, (_, j) => ({ text: `planet ${j} ${"x".repeat(60)}`, correct: j === 0, locked: false })) } },
      })),
    },
  });
}
const bytes = new TextEncoder().encode(p);
const t = performance.now();
// The author's decisions (Keep for every label finding) come from the preview call, inside the
// measurement: the ceiling covers the whole session.
let opts = "";
if (fn === "strip") {
  const f = JSON.parse(m.label_findings(bytes, ""));
  if (f.ok) opts = JSON.stringify({ labels: f.label_findings.map((x) => ({ question: x.question, part: x.part ?? undefined, choice: "keep" })) });
}
const call = { strip: () => m.strip(bytes, opts), validate: () => m.validate(bytes, ""), label_findings: () => m.label_findings(bytes, ""), validate_for_publish: () => m.validate_for_publish(bytes, '{"hide_key":true}') };
const r = JSON.parse(call[fn]());
const ms = performance.now() - t;
// The instance must still work afterwards.
const again = JSON.parse(m.validate(new TextEncoder().encode('{"schema_version":1}'), ""));
console.log(JSON.stringify({ mode, fn, bytes: bytes.length, growth: m.memory_bytes() - mem0, ms: Math.round(ms), ok: r.ok, code: r.ok ? null : r.errors[0].code, alive: again.ok === false }));
