// Runs the wasm builds under Node.
//   node tests/node/check.mjs [pkg] [test-pkg]
// * `pkg` (default ./pkg): the production build (`wasm-pack build --release --target web`). Checks
//   the exports (no `strip_with_seed`), invariants of `strip` with its internal seed, refusals, and
//   a wasm-memory CEILING: each hostile input and the legitimate 4 MB bank run in a fresh instance
//   (tests/node/mem.mjs) and must grow linear memory by less than 40 MB.
// * `test-pkg` (default ./target/pkg-test): the same crate built with `--features test-seed`
//   (`wasm-pack build --release --target web --out-dir target/pkg-test -- --features test-seed`).
//   Checks byte-for-byte equality with the native goldens for the fixed seed.
import { readFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const pkgDir = resolve(process.argv[2] || join(root, "pkg"));
const testDir = resolve(process.argv[3] || join(root, "target", "pkg-test"));
const enc = (s) => new TextEncoder().encode(s);
const CEILING = 40 * 1024 * 1024;

let failures = 0;
let checks = 0;
const check = (cond, msg) => {
  checks++;
  if (!cond) {
    failures++;
    console.error("FAIL:", msg);
  }
};
const canon = (v) => JSON.stringify(sortKeys(v));
function sortKeys(v) {
  if (Array.isArray(v)) return v.map(sortKeys);
  if (v && typeof v === "object") return Object.fromEntries(Object.keys(v).sort().map((k) => [k, sortKeys(v[k])]));
  return v;
}
const names = [
  "question_mc", "question_mc_multi", "question_tf", "question_matching", "question_blank",
  "question_short", "question_multipart", "question_figure_image", "question_matching_shared", "bank_all_kinds", "bank_no_details",
];
const payloadOf = (name) => {
  const inner = readFileSync(join(root, "fixtures", "model", `${name}.json`), "utf8");
  const key = name.startsWith("bank") ? "bank" : "question";
  return { key, inner, payload: `{"schema_version":1,"${key}":${inner}}` };
};
const hashes = JSON.parse(readFileSync(join(root, "fixtures", "hashes.json"), "utf8"));

// ---- production build -----------------------------------------------------------------------------
const m = await import(join(pkgDir, "share_core.js"));
m.initSync({ module: readFileSync(join(pkgDir, "share_core_bg.wasm")) });
check(m.strip_with_seed === undefined, "the production build does not export strip_with_seed");
check(m.strip.length === 2, "strip takes (payload bytes, options): no seed parameter");
// Decisions for every label finding: Strip where allowed (what the author would pick here).
const decide = (bytes) => {
  const f = JSON.parse(m.label_findings(bytes, ""));
  return JSON.stringify({ labels: f.label_findings.map((x) => ({ question: x.question, part: x.part ?? undefined, choice: x.strip_allowed ? "strip" : "keep" })) });
};
// strip as an author who acknowledges every preserved order (the ack is required).
const stripAck = (fn, bytes, opts, ...rest) => {
  let r = JSON.parse(fn(bytes, ...rest, opts));
  if (r.ok === false && r.errors[0].code === "order_ack_required") {
    const o = opts ? JSON.parse(opts) : {};
    o.order_acks = r.needs_order_ack;
    r = JSON.parse(fn(bytes, ...rest, JSON.stringify(o)));
  }
  return r;
};
// A JS string where bytes are expected: a clear refusal, never invalid_json.
for (const fn of ["validate", "validate_for_publish", "strip", "match_hash", "full_hash", "validate_meta", "label_findings"]) {
  const r = JSON.parse(m[fn]('{"schema_version":1}', ""));
  check(r.ok === false && r.errors[0].code === "expected_bytes", `${fn}(string) -> expected_bytes: ${JSON.stringify(r)}`);
}

for (const name of names) {
  const { key, inner, payload } = payloadOf(name);
  const v = JSON.parse(m.validate(enc(payload), ""));
  check(v.ok === true && canon(v.payload[key]) === canon(JSON.parse(inner)), `${name}: validate output equals the app JSON`);
  const a = stripAck(m.strip, enc(payload), decide(enc(payload)));
  const b = stripAck(m.strip, enc(payload), decide(enc(payload)));
  check(a.ok && b.ok, `${name}: strip`);
  check(!JSON.stringify(a.payload).includes('"correct":true'), `${name}: no correct flag`);
  check(canon(a.match_hashes) === canon(b.match_hashes), `${name}: match hashes independent of the internal seed`);
  check(canon(JSON.parse(m.match_hash(enc(payload), "")).hashes) === canon(hashes[name].match_hash), `${name}: match_hash`);
  check(canon(JSON.parse(m.full_hash(enc(payload), "")).hashes) === canon(hashes[name].full_hash), `${name}: full_hash`);
}
// The internal seed differs per call: a 26-way shuffle comes out in more than one order.
const big = JSON.stringify({ schema_version: 1, question: { id: "q", stem: "Pick", kind: { Mc: { answers: Array.from({ length: 26 }, (_, i) => ({ text: `planet ${i}`, correct: i === 0 })) } }, points: null, difficulty: null, created: 0, modified: 0 } });
const orders = new Set(Array.from({ length: 5 }, () => JSON.stringify(stripAck(m.strip, enc(big), "").payload)));
check(orders.size > 1, "strip draws a fresh seed each call");

const bad = (payload, code) => {
  const r = JSON.parse(m.validate(typeof payload === "string" ? enc(payload) : payload, ""));
  check(r.ok === false && r.errors[0].code === code, `refuse ${code}: ${JSON.stringify(r).slice(0, 200)}`);
  return r;
};
const tf = (stem) => JSON.stringify({ schema_version: 1, question: { id: "q", stem, kind: { Tf: { correct: true } }, points: null, difficulty: null, created: 0, modified: 0 } });
const r1 = bad(tf("a‮b"), "forbidden_char");
check(r1.errors[0].cp === "U+202E" && r1.errors[0].offset === 1, "forbidden_char carries cp and offset");
bad(tf("a⁦b"), "forbidden_char");
bad(tf("ㅤ"), "forbidden_char");
bad(tf(" ​ "), "blank_text");
bad(tf("x".repeat(8193)), "too_long");
bad('{"schema_version":1,"question":{"id":"q","id":"r"}}', "duplicate_key");
bad("[".repeat(100000), "too_deep");
// JSON.stringify writes a lone surrogate as the escape \ud800, which the strict reader refuses;
// raw bytes that are not UTF-8 are refused before parsing.
bad(tf("a\ud800b"), "invalid_json");
bad(new Uint8Array([0x7b, 0xff, 0x7d]), "invalid_utf8");
check(JSON.parse(m.validate(enc(tf("Count \u{1F468}‍\u{1F469}‍\u{1F467} ❤️")), "")).ok, "emoji ZWJ sequence and VS16 accepted");
check(JSON.parse(m.validate(enc(tf("می‌خواهم")), "")).ok, "Persian ZWNJ accepted");
const oor = JSON.parse(m.strip(enc(tf("x")), JSON.stringify({ image_ok: [99] })));
check(oor.ok === false && oor.errors[0].code === "image_ok_out_of_range", "image_ok out of range");
check(JSON.parse(m.validate_meta(enc('{"title":"T","tags":["X"]}'), "")).meta.tags[0] === "x", "meta");
const lf = JSON.parse(m.strip(enc(JSON.stringify({ schema_version: 1, question: { id: "q", stem: "Pick", kind: { Mc: { answers: ["A) Venus", "B) Mars"].map((t, i) => ({ text: t, correct: i === 0 })) } }, points: null, difficulty: null, created: 0, modified: 0 } })), ""));
check(lf.ok === false && lf.errors[0].code === "labels_need_decision" && lf.label_findings[0].choices[0].after === "Venus", "labels need a decision, with the preview");
check(JSON.parse(m.schema_versions()).stripped === 2, "schema_versions");
check(m.argon2id(enc("pw"), new Uint8Array(16), 8, 1, 1).length === 32, "argon2id still exported");

// ---- memory ceiling, each case in a fresh instance --------------------------------------------------
for (const mode of ["many_choices", "params", "nfc", "objs", "objs2", "strs", "kept_objects", "tiny_answers", "labels", "long_strings", "legit"]) {
  for (const fn of mode === "many_choices" ? ["validate", "strip", "label_findings", "validate_for_publish"] : ["validate", "strip"]) {
    const out = JSON.parse(execFileSync(process.execPath, [join(root, "tests", "node", "mem.mjs"), pkgDir, mode, fn], { encoding: "utf8" }));
    console.log(`  ${mode.padEnd(13)} ${fn.padEnd(8)} ${String(out.bytes).padStart(8)} B in  +${(out.growth / 1048576).toFixed(1).padStart(5)} MB  ${String(out.ms).padStart(5)} ms  ${out.ok ? "ok" : out.code}`);
    check(out.growth < CEILING, `${mode}/${fn}: memory growth ${out.growth} >= ${CEILING}`);
    check(out.alive, `${mode}/${fn}: instance still works`);
    if (mode === "legit") check(out.ok, "legit bank accepted");
  }
}

// ---- test-seed build: byte-for-byte against the native goldens -------------------------------------
if (existsSync(join(testDir, "share_core.js"))) {
  const t = await import(join(testDir, "share_core.js"));
  t.initSync({ module: readFileSync(join(testDir, "share_core_bg.wasm")) });
  for (const name of names) {
    const { payload } = payloadOf(name);
    const s = stripAck(t.strip_with_seed, enc(payload), decide(enc(payload)), new Uint8Array(32).fill(7));
    const golden = JSON.parse(readFileSync(join(root, "fixtures", "stripped", `${name}.json`), "utf8"));
    check(s.ok === true && canon(s.payload) === canon(golden), `${name}: wasm strip_with_seed equals the native golden`);
  }
} else {
  console.log("(test-seed build not found; golden equality skipped)");
  failures++;
}

console.log(`wasm memory after checks: ${m.memory_bytes()} bytes`);
console.log(`${checks - failures}/${checks} checks passed`);
process.exit(failures ? 1 : 0);
