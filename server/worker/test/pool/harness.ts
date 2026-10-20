// Test harness for the pool: D1 over node:sqlite (SQLite is what D1 runs), an in-memory R2, a stub
// authenticate that reads the account's CURRENT trust from D1 (like the auth package), and the REAL
// share-core production wasm package (server/share-core/pkg).
import { readdirSync, readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import { deflateSync } from "node:zlib";
import { createPool, makeCore, type Pool, type ShareCoreExports } from "../../src/pool/index.ts";
import type { PoolNotice } from "../../src/pool/shared.ts";
import { PUBLISH_CONTENT_TYPE } from "../../src/pool/shared.ts";

const here = new URL("../..", import.meta.url).pathname;
const pkg = `${here}../share-core/pkg/`;

export const shareCore = (await import(`${pkg}share_core.js`)) as ShareCoreExports & { initSync(o: { module: Uint8Array }): unknown };
shareCore.initSync({ module: readFileSync(`${pkg}share_core_bg.wasm`) });
export const core = makeCore(shareCore);
export const enc = (s: string) => new TextEncoder().encode(s);

// ---- D1 --------------------------------------------------------------------------------------------

export function freshDb(): DatabaseSync {
  const db = new DatabaseSync(":memory:");
  for (const f of readdirSync(`${here}migrations`).filter((x) => x.endsWith(".sql")).sort()) db.exec(readFileSync(`${here}migrations/${f}`, "utf8"));
  return db;
}

const D1_MAX_PARAMS = 100;

/** D1's surface over node:sqlite: ?NNN parameters, first/all/run, RETURNING, atomic batch. */
export function d1(db: DatabaseSync, stats = { statements: 0 }): D1Database {
  const exec = (sql: string, args: unknown[], mode: "first" | "all" | "run") => {
    if (args.length > D1_MAX_PARAMS) throw new Error(`D1: too many bound parameters (${args.length})`);
    for (const a of args) if (a === undefined) throw new Error(`D1: undefined bound in ${sql.slice(0, 80)}`);
    stats.statements++;
    const st = db.prepare(sql);
    const vals = args.map((a) => (typeof a === "boolean" ? (a ? 1 : 0) : a)) as never[];
    if (mode === "first") {
      const r = st.get(...vals);
      return r ? { ...r } : null;
    }
    if (mode === "all" || /\bRETURNING\b/i.test(sql) || /^\s*(SELECT|WITH)\b/i.test(sql)) {
      const rows = st.all(...vals).map((x) => ({ ...x }));
      return { results: rows, success: true, meta: { changes: /^\s*(SELECT|WITH)\b/i.test(sql) ? 0 : rows.length } };
    }
    const r = st.run(...vals);
    return { results: [], success: true, meta: { changes: Number(r.changes) } };
  };
  const stmt = (sql: string, args: unknown[] = []): unknown => ({
    sql,
    args,
    bind: (...a: unknown[]) => stmt(sql, a),
    first: async () => exec(sql, args, "first"),
    all: async () => exec(sql, args, "all"),
    run: async () => exec(sql, args, "run"),
  });
  return {
    prepare: (sql: string) => stmt(sql),
    batch: async (ss: { sql: string; args: unknown[] }[]) => {
      db.exec("SAVEPOINT batch");
      try {
        const out = ss.map((x) => exec(x.sql, x.args, "run"));
        db.exec("RELEASE batch");
        return out;
      } catch (e) {
        db.exec("ROLLBACK TO batch");
        db.exec("RELEASE batch");
        throw e;
      }
    },
  } as unknown as D1Database;
}

// ---- R2 --------------------------------------------------------------------------------------------

export class MemR2 {
  objects = new Map<string, { bytes: Uint8Array; contentType?: string }>();
  /** Runs (once) before the next get of a matching key: simulates a concurrent request landing mid-flight. */
  onGet: ((key: string) => Promise<void>) | null = null;
  /** Runs (once) before the next put whose key starts with `onPutPrefix`. */
  onPut: ((key: string) => Promise<void>) | null = null;
  onPutPrefix = "items/";
  async put(key: string, value: Uint8Array | ArrayBuffer | string, o?: { httpMetadata?: { contentType?: string } }) {
    const hook = this.onPut;
    if (hook && key.startsWith(this.onPutPrefix)) {
      this.onPut = null;
      await hook(key);
    }
    const bytes = typeof value === "string" ? enc(value) : value instanceof Uint8Array ? new Uint8Array(value) : new Uint8Array(value);
    this.objects.set(key, { bytes, contentType: o?.httpMetadata?.contentType });
    return { key, size: bytes.length };
  }
  async get(key: string) {
    const hook = this.onGet;
    if (hook) {
      this.onGet = null;
      await hook(key);
    }
    const o = this.objects.get(key);
    if (!o) return null;
    const bytes = o.bytes;
    return {
      key,
      size: bytes.length,
      text: async () => new TextDecoder().decode(bytes),
      arrayBuffer: async () => bytes.slice().buffer,
      // Streamed in 64 KB views of the stored bytes (no copy), like a network body.
      get body() {
        let o = 0;
        return new ReadableStream<Uint8Array>({
          pull(c) {
            if (o >= bytes.length) return c.close();
            c.enqueue(bytes.subarray(o, Math.min(o + 65536, bytes.length)));
            o += 65536;
          },
        });
      },
    };
  }
  async head(key: string) {
    return this.objects.has(key) ? { key, size: this.objects.get(key)!.bytes.length } : null;
  }
  async delete(keys: string | string[]) {
    for (const k of Array.isArray(keys) ? keys : [keys]) this.objects.delete(k);
  }
  keys(prefix = ""): string[] {
    return [...this.objects.keys()].filter((k) => k.startsWith(prefix)).sort();
  }
}

// ---- the world -------------------------------------------------------------------------------------

export const ORIGIN = "https://app.example.org";
export const API = "https://api.example.org";
export const SECRET = "test-secret-0123456789-0123456789-0123456789";

export interface User {
  id: string;
  token: string;
  name: string;
}

const allow = { hit: async () => ({ allowed: true, retryAfter: 0 }), undo: async () => {} };

/** A fixed-window counter (the auth package's stores are tested there; this checks the pool calls one). */
export class CountingStore {
  counts = new Map<string, number>();
  async hit(key: string, rule: { limit: number }) {
    const n = (this.counts.get(key) ?? 0) + 1;
    this.counts.set(key, n);
    return n <= rule.limit ? { allowed: true, retryAfter: 0 } : { allowed: false, retryAfter: 30 };
  }
  async undo(key: string) {
    this.counts.set(key, Math.max(0, (this.counts.get(key) ?? 0) - 1));
  }
}

export class World {
  sql = freshDb();
  stats = { statements: 0 };
  db = d1(this.sql, this.stats);
  r2 = new MemR2();
  clock = Date.UTC(2026, 9, 20, 12, 0, 0);
  notices: PoolNotice[] = [];
  pending: Promise<unknown>[] = [];
  tokens = new Map<string, string>();
  pool: Pool;
  private n = 0;

  constructor(o: { rateStore?: CountingStore; bodyDeadlineMs?: number } = {}) {
    this.pool = createPool({
      db: this.db,
      r2: this.r2 as unknown as R2Bucket,
      core,
      secret: SECRET,
      allowedOrigins: [ORIGIN],
      now: () => this.clock,
      rateStore: o.rateStore ?? allow,
      bodyDeadlineMs: o.bodyDeadlineMs,
      ipBucket: (ip) => ip,
      // Like the auth package: the token's session must be live and trust is read from D1 now.
      authenticate: async (req) => {
        const m = /^Bearer (.+)$/.exec(req.headers.get("Authorization") ?? "");
        const id = m ? this.tokens.get(m[1]!) : undefined;
        if (!id) return null;
        const u = this.sql.prepare(`SELECT trust FROM users WHERE id = ? AND deleted_at IS NULL AND suspended_at IS NULL`).get(id) as { trust: string } | undefined;
        return u ? { userId: id, sessionFamily: "f", trust: u.trust } : null;
      },
      notify: async (n) => void this.notices.push(n),
      background: (p) => void this.pending.push(p),
    });
  }

  advance(ms: number) {
    this.clock += ms;
  }

  user(trust = "teacher", name?: string): User {
    const i = ++this.n;
    const id = `${String(i).padStart(8, "0")}-0000-4000-8000-${String(i).padStart(12, "0")}`;
    this.sql
      .prepare(`INSERT INTO users (id, email, email_verified_at, pw_hash, display_name, trust, created_at) VALUES (?, ?, 1, 'x', ?, ?, 1)`)
      .run(id, `u${i}@school.example.org`, name ?? `Teacher ${i}`, trust);
    const token = `tok-${i}-${Math.random().toString(36).slice(2)}`;
    this.tokens.set(token, id);
    return { id, token, name: name ?? `Teacher ${i}` };
  }

  setTrust(u: User, trust: string) {
    this.sql.prepare(`UPDATE users SET trust = ? WHERE id = ?`).run(trust, u.id);
  }

  async call(method: string, path: string, o: { user?: User | null; json?: unknown; body?: BodyInit; contentType?: string; headers?: Record<string, string>; ip?: string | null } = {}): Promise<Response> {
    const h = new Headers(o.headers ?? {});
    if (o.user) h.set("Authorization", `Bearer ${o.user.token}`);
    if (o.ip !== null) h.set("CF-Connecting-IP", o.ip ?? "10.0.0.1");
    let body: BodyInit | undefined = o.body;
    if (o.json !== undefined) {
      body = JSON.stringify(o.json);
      h.set("Content-Type", "application/json");
    }
    if (o.contentType) h.set("Content-Type", o.contentType);
    // A real client sends Content-Length on the wire for a byte body; node's Request does not add it.
    if (!h.has("Content-Length") && (typeof body === "string" || body instanceof Uint8Array)) h.set("Content-Length", String(typeof body === "string" ? new TextEncoder().encode(body).length : body.length));
    const res = await this.pool.handle(new Request(`${API}${path}`, { method, headers: h, body }));
    if (!res) throw new Error(`no route: ${method} ${path}`);
    // Drain a streamed body as a client would (it releases the isolate's read reservation).
    const bytes = new Uint8Array(await res.arrayBuffer());
    return new Response(res.status === 204 ? null : bytes, { status: res.status, headers: res.headers });
  }

  /** A request and its body as text (status + text, for the leak sweep). */
  async text(method: string, path: string, o: Parameters<World["call"]>[2] = {}): Promise<{ status: number; text: string; headers: Headers }> {
    const r = await this.call(method, path, o);
    return { status: r.status, text: await r.text(), headers: r.headers };
  }

  async json<T = Record<string, unknown>>(method: string, path: string, o: Parameters<World["call"]>[2] = {}): Promise<{ status: number; body: T }> {
    const r = await this.call(method, path, o);
    const t = await r.text();
    return { status: r.status, body: (t ? JSON.parse(t) : null) as T };
  }

  /** auth_audit entries the admin routes asked for (the auth package writes them in production). */
  adminAudits: unknown[] = [];

  /** Calls a pool admin route's handler as the auth package's guard would (admin, known device, sudo). */
  async admin<T = Record<string, unknown>>(admin: User, method: string, path: string, body: Record<string, unknown> | null = null, query = ""): Promise<{ status: number; body: T; text: string }> {
    const route = this.pool.adminRoutes.find((r) => r.path === path && r.method === method);
    if (!route) throw new Error(`no admin route ${method} ${path}`);
    const res = await route.handler(new Request(`${API}${path}${query}`, { method }), {
      admin: { userId: admin.id, sessionFamily: "f", trust: "admin", sudo: true },
      body,
      now: this.clock,
      audit: (e) => {
        this.adminAudits.push(e);
        return this.db.prepare(`SELECT 1 AS audited`);
      },
    });
    const text = await res.text();
    return { status: res.status, body: (text ? JSON.parse(text) : null) as T, text };
  }

  /** A privacy purge as an admin does it: preview, then confirm with the preview's token. */
  async purge<T = Record<string, unknown>>(admin: User, itemId: string, sel: Record<string, unknown> = {}) {
    const p = await this.admin<Record<string, unknown>>(admin, "POST", "/admin/pool/purge-preview", { item_id: itemId, ...sel });
    if (p.status !== 200) return p as unknown as { status: number; body: T; text: string };
    return this.admin<T>(admin, "POST", "/admin/pool/purge", { item_id: itemId, ...sel, confirm_preview: p.body.confirm_preview });
  }

  /** A DMCA takedown as an admin does it: preview, then confirm with the preview's token. */
  async takedown<T = Record<string, unknown>>(admin: User, noticeId: unknown, itemId: unknown, extra: Record<string, unknown> = {}) {
    const p = await this.admin<Record<string, unknown>>(admin, "POST", "/admin/dmca/takedown-preview", { item_id: itemId, ...extra });
    if (p.status !== 200) return p as unknown as { status: number; body: T; text: string };
    return this.admin<T>(admin, "POST", "/admin/dmca/takedown", { notice_id: noticeId, item_id: itemId, ...extra, confirm_preview: p.body.confirm_preview });
  }

  q<T = Record<string, unknown>>(sql: string, ...args: unknown[]): T[] {
    return this.sql.prepare(sql).all(...(args as never[])).map((x) => ({ ...x })) as T[];
  }
}

// ---- payloads --------------------------------------------------------------------------------------

export function framed(meta: unknown, payload: Uint8Array | string): Uint8Array {
  const m = enc(JSON.stringify(meta) + "\n");
  const p = typeof payload === "string" ? enc(payload) : payload;
  const out = new Uint8Array(m.length + p.length);
  out.set(m, 0);
  out.set(p, m.length);
  return out;
}

export function publishId(): string {
  return Buffer.from(crypto.getRandomValues(new Uint8Array(16))).toString("base64url");
}

export interface PublishOpts {
  hideKey?: boolean;
  license?: string;
  visibility?: "public" | "unlisted";
  imageOk?: number[];
  meta?: Record<string, unknown>;
  base?: string | null;
  publishId?: string;
  labels?: unknown[];
  orderAcks?: unknown[];
  /** Decide labels and acknowledge preserved orders the way the app would (default true). */
  auto?: boolean;
}

/** The label decisions an author would make (strip where allowed), computed client-side with share-core. */
export function decideLabels(payload: string): unknown[] {
  const f = JSON.parse(shareCore.label_findings(enc(payload), "")) as { ok: boolean; label_findings?: { question: number; part?: number; strip_allowed: boolean }[] };
  return (f.label_findings ?? []).map((x) => (x.part === undefined ? { question: x.question, choice: x.strip_allowed ? "strip" : "keep" } : { question: x.question, part: x.part, choice: x.strip_allowed ? "strip" : "keep" }));
}

export async function publish(w: World, user: User, payload: string, o: PublishOpts = {}, itemId: string | null = null): Promise<{ status: number; body: Record<string, unknown> }> {
  const hide = o.hideKey ?? false;
  const meta: Record<string, unknown> = {
    publish_id: o.publishId ?? publishId(),
    hide_key: hide,
    license: o.license ?? "CC-BY-4.0",
    attestation: { accepted: true, text_rev: "2026-10-19" },
    labels: o.labels ?? (hide && o.auto !== false ? decideLabels(payload) : []),
    order_acks: o.orderAcks ?? [],
    image_ok: o.imageOk ?? [],
    meta: o.meta ?? { title: "Unit test", subject: "Biology", course: "", grade: "", tags: [], standards: [] },
  };
  if (itemId === null) meta.visibility = o.visibility ?? "public";
  else meta.base_publish_id = o.base ?? null;
  const send = () => w.json(itemId ? "PUT" : "POST", itemId ? `/items/${itemId}` : "/items", { user, body: framed(meta, payload), contentType: PUBLISH_CONTENT_TYPE });
  let r = await send();
  if (r.status === 422 && r.body.error === "order_ack_required" && o.auto !== false && !o.orderAcks) {
    meta.order_acks = r.body.needs_order_ack;
    r = await send();
  }
  return r;
}

// ---- random questions (seeded) ---------------------------------------------------------------------

export function rng(seed: number) {
  let s = seed >>> 0;
  const next = () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  return {
    next,
    int: (n: number) => Math.floor(next() * n),
    pick: <T>(xs: T[]) => xs[Math.floor(next() * xs.length)]!,
    bool: (p = 0.5) => next() < p,
  };
}
export type Rng = ReturnType<typeof rng>;

const WORDS = ["cell", "orbit", "nucleus", "vector", "enzyme", "photon", "lattice", "tundra", "isotope", "glacier", "meiosis", "torque", "osmosis", "quasar", "alloy", "delta"];

/** A token no hex hash or id can contain (letters g-z), unique per call: what a leak sweep looks for. */
export function secret(r: Rng, tag: string): string {
  let s = "";
  for (let i = 0; i < 14; i++) s += "ghijklmnopqrstuvwxyz"[r.int(20)];
  return `${tag}${s}`;
}

export interface GenQuestion {
  json: Record<string, unknown>;
  /** Strings that are answer-key material: must never reach a non-author of a hide_key publish. */
  secrets: string[];
  kind: string;
  image?: string;
}

function words(r: Rng, n: number): string {
  return Array.from({ length: n }, () => r.pick(WORDS)).join(" ");
}

/** A valid question of `kind` with key material in every key-bearing slot. */
export function genQuestion(r: Rng, kind: string, i: number, image?: { sha: string; ext: string }): GenQuestion {
  const secrets: string[] = [];
  const uniq = (n: number, tag: string) => Array.from({ length: n }, (_, k) => `${tag}${k} ${words(r, 2)}`);
  let k: Record<string, unknown>;
  let stem = `Question ${i}: ${words(r, 5)}?`;
  const mc = (n: number, labelled: boolean) => {
    const texts = uniq(n, "choice");
    const correct = r.int(n);
    return texts.map((t, j) => ({ text: labelled ? `${String.fromCharCode(65 + j)}. ${t}` : t, correct: j === correct || (r.bool(0.15) && j !== 0), locked: r.bool(0.1) }));
  };
  switch (kind) {
    case "Mc":
      k = { Mc: { answers: mc(2 + r.int(5), r.bool(0.3)) } };
      break;
    case "Tf":
      k = { Tf: { correct: r.bool() } };
      break;
    case "Matching": {
      const n = 2 + r.int(4);
      const pairs = Array.from({ length: n }, (_, j) => ({ left: `left${j} ${words(r, 2)}`, right: `right${j} ${words(r, 2)}` }));
      k = { Matching: { pairs, distractors: r.bool() ? [`distractor ${words(r, 2)}`] : [] } };
      break;
    }
    case "Blank": {
      const n = 1 + r.int(3);
      stem = `Fill: ${Array.from({ length: n }, () => `${words(r, 2)} ___`).join(", ")}.`;
      const accept = Array.from({ length: n }, () => Array.from({ length: 1 + r.int(2) }, () => secret(r, "ACC")));
      for (const a of accept) secrets.push(...a);
      k = { Blank: { accept } };
      break;
    }
    case "Short": {
      const s = secret(r, "SAMPLE");
      secrets.push(s);
      k = { Short: { sample: s } };
      break;
    }
    default: {
      const s = secret(r, "PSAMPLE");
      secrets.push(s);
      k = {
        Multipart: {
          parts: [
            { stem: `Part a ${words(r, 3)}`, kind: { Mc: { answers: mc(3, r.bool(0.3)) } } },
            { stem: `Part b ${words(r, 3)}`, kind: { Tf: { correct: r.bool() } } },
            { stem: `Part c ${words(r, 3)}`, kind: { Short: { sample: s } } },
          ],
        },
      };
    }
  }
  const explanation = r.bool(0.7) ? secret(r, "EXPL") : null;
  if (explanation) secrets.push(explanation);
  const figure = r.bool(0.3) ? { id: "micropipette-reading", params: { volume: 10 + r.int(50), answer: true, answer_text: secret(r, "FIG") } } : null;
  if (figure) secrets.push(figure.params.answer_text);
  return {
    kind,
    image: image?.sha,
    secrets,
    json: {
      id: `q${i}local`,
      stem,
      kind: k,
      points: r.bool() ? 1 + r.int(5) : null,
      difficulty: r.pick([null, "Low", "Medium", "High"]),
      topics: r.bool(0.3) ? [`topic ${words(r, 1)}`] : [],
      image: image ? { file: `media/${image.sha}.${image.ext}`, width: 1, height: 1 } : null,
      explanation,
      figure,
      created: 1760000000000 + i,
      modified: 1760000000500 + i,
    },
  };
}

export const KINDS = ["Mc", "Tf", "Matching", "Blank", "Short", "Multipart"];

export function bankPayload(qs: GenQuestion[], name = "Generated bank"): string {
  return JSON.stringify({ schema_version: 1, bank: { id: "blocal", name, questions: qs.map((q) => q.json), created: 1760000000000, modified: 1760000000000 } });
}
export function questionPayload(q: GenQuestion): string {
  return JSON.stringify({ schema_version: 1, question: q.json });
}

// ---- images ----------------------------------------------------------------------------------------

const crcTable = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc(b: Uint8Array): number {
  let c = 0xffffffff;
  for (const x of b) c = crcTable[(c ^ x) & 0xff]! ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
export function chunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(12 + data.length);
  const dv = new DataView(out.buffer);
  dv.setUint32(0, data.length);
  out.set(enc(type), 4);
  out.set(data, 8);
  dv.setUint32(8 + data.length, crc(out.subarray(4, 8 + data.length)));
  return out;
}
export function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((a, p) => a + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

/** A real w x h greyscale PNG whose pixels depend on `seed` (distinct hashes). */
export function png(w: number, h: number, seed = 0, extra: Uint8Array[] = []): Uint8Array {
  const ihdr = new Uint8Array(13);
  const dv = new DataView(ihdr.buffer);
  dv.setUint32(0, w);
  dv.setUint32(4, h);
  ihdr.set([8, 0, 0, 0, 0], 8);
  const raw = new Uint8Array((w + 1) * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) raw[y * (w + 1) + 1 + x] = (x * 7 + y * 13 + seed) & 0xff;
  return concat(new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), ...extra, chunk("IDAT", deflateSync(raw)), chunk("IEND", new Uint8Array(0)));
}

export async function sha256(b: Uint8Array): Promise<string> {
  return Buffer.from(await crypto.subtle.digest("SHA-256", b)).toString("hex");
}

export async function upload(w: World, user: User, bytes: Uint8Array): Promise<{ status: number; body: Record<string, unknown>; sha: string }> {
  const sha = await sha256(bytes);
  const r = await w.json("PUT", `/blobs/${sha}`, { user, body: bytes, contentType: "application/octet-stream" });
  return { ...r, sha };
}
