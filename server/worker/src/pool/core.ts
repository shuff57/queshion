// The ONE adapter between the pool and share-core's wasm API (server/share-core/src/wasm.rs).
//
// share-core takes UTF-8 **bytes** (a Uint8Array; a JS string is refused with `expected_bytes`) and a
// JSON options string, and returns one JSON object: `{"ok": true, ...}` or `{"ok": false, "errors":
// [{code, path, limit?, offset?, offset16?, cp?}]}`. Errors never contain input text. If share-core's
// API changes (names, option keys, result fields), this file is the only one to change.
//
// The pool never builds, edits or trusts a stripped form itself: `strip` is share-core's, with a seed
// share-core draws internally. Payload JSON is passed on as the exact canonical bytes share-core wrote
// (sliced out of its result, never re-serialised by JavaScript).
import { checkImage, type ImageInfo } from "./image.ts";

/** The share-core wasm exports the pool uses (the package's module namespace, already initialised). */
export interface ShareCoreExports {
  validate(payload: Uint8Array, options: string): string;
  validate_for_publish(payload: Uint8Array, options: string): string;
  strip(payload: Uint8Array, options: string): string;
  label_findings(payload: Uint8Array, options: string): string;
  match_hash(payload: Uint8Array, options: string): string;
  full_hash(payload: Uint8Array, options: string): string;
  validate_meta(payload: Uint8Array, options: string): string;
  schema_versions(): string;
  /** Not in share-core yet: when it lands (`{"ok", "content_type", "width", "height"}`), it replaces ./image.ts. */
  image_check?(bytes: Uint8Array, options: string): string;
}

/** A share-core error, reduced to the fields that carry no input text. */
export interface CoreError {
  code: string;
  path: string;
  limit?: number;
  offset16?: number;
  cp?: string;
}

/** A question (and optionally a Multipart part) by position in the payload. */
export interface Pos {
  question: number;
  part?: number;
}

export interface LabelDecision {
  question: number;
  part?: number;
  choice: "strip" | "keep";
}

export type Fail = {
  ok: false;
  errors: CoreError[];
  /** Present when the problem is an undecided label: the author's preview (their own text). */
  label_findings?: unknown[];
  /** Present when a preserved order was not acknowledged. */
  needs_order_ack?: Pos[];
  order_reasons?: unknown[];
};

/** A validated payload: the canonical JSON share-core wrote, and the same parsed (read-only use). */
export interface Validated {
  ok: true;
  readonly json: string;
  bytes: Uint8Array;
  payload: PayloadJson;
  dropped: unknown[];
}

export interface Stripped extends Validated {
  matchHashes: string[];
  strippedHashes: string[];
  needsOrderAck: Pos[];
  orderReasons: unknown[];
  labelsRemoved: Pos[];
  labelFindings: unknown[];
  servedOrders: unknown[];
  imagesKept: number[];
  imagesRemoved: number[];
  figuresRemoved: number[];
}

/** The wire envelope (docs/shared-pool.md "Import": `{"schema_version", "bank" | "question"}`). */
export interface PayloadJson {
  schema_version: number;
  bank?: BankJson;
  question?: QuestionJson;
}
export interface BankJson {
  id: string;
  name: string;
  questions: QuestionJson[];
  created: number;
  modified: number;
  [k: string]: unknown;
}
export interface QuestionJson {
  id: string;
  kind: Record<string, unknown>;
  image?: { file: string; width: number; height: number } | null;
  difficulty?: string | null;
  topics?: string[];
  published?: { item_id: string; qid?: string; publish_id: string; version: number };
  origin?: { item_id: string; qid?: string; publish_id: string; version: number };
  created: number;
  modified: number;
  [k: string]: unknown;
}

export interface Meta {
  title: string;
  subject: string;
  course: string;
  grade: string;
  tags: string[];
  standards: string[];
}

export interface Core {
  schema(): { min: number; max: number; stripped: number };
  /** For uploads: strict parse, limits, characters, key completeness, label decisions (hide_key). */
  validateForPublish(bytes: Uint8Array, o: { hideKey: boolean; labels: LabelDecision[] }): Validated | Fail;
  /** Validates for a hide_key publish and strips (share-core draws the shuffle seed). */
  strip(bytes: Uint8Array, o: { labels: LabelDecision[]; orderAcks: Pos[]; imageOk: number[] }): Stripped | Fail;
  /** match_hash per question (key-free; safe to compare across items). */
  matchHashes(bytes: Uint8Array): string[] | Fail;
  /** full_hash per question (a key oracle for small answer spaces: author-only, never compared). */
  fullHashes(bytes: Uint8Array): string[] | Fail;
  validateMeta(meta: unknown): Meta | Fail;
  /** Structural image check (format sniffed from the bytes); null when refused. */
  imageInfo(bytes: Uint8Array): Promise<ImageInfo | { refused: string }>;
}

const enc = new TextEncoder();

/**
 * The raw JSON text of top-level `key` in a JSON object `json`, or null. A small scanner (strings
 * and nesting only), so the payload share-core wrote is reused byte for byte: JavaScript's
 * JSON.stringify would reorder integer-like keys and print some numbers differently.
 */
export function rawField(json: string, key: string): string | null {
  const want = JSON.stringify(key);
  let i = 0;
  const n = json.length;
  const ws = () => {
    while (i < n && " \t\r\n".includes(json[i]!)) i++;
  };
  const skipString = () => {
    // json[i] === '"'
    i++;
    while (i < n) {
      const c = json[i]!;
      if (c === "\\") i += 2;
      else if (c === '"') {
        i++;
        return;
      } else i++;
    }
    throw new Error("unterminated string");
  };
  const skipValue = () => {
    ws();
    const c = json[i];
    if (c === '"') return skipString();
    if (c === "{" || c === "[") {
      let depth = 0;
      while (i < n) {
        const d = json[i]!;
        if (d === '"') {
          skipString();
          continue;
        }
        if (d === "{" || d === "[") depth++;
        else if (d === "}" || d === "]") {
          depth--;
          if (depth === 0) {
            i++;
            return;
          }
        }
        i++;
      }
      throw new Error("unbalanced");
    }
    while (i < n && !",}]".includes(json[i]!)) i++;
  };
  ws();
  if (json[i] !== "{") return null;
  i++;
  for (;;) {
    ws();
    if (json[i] === "}") return null;
    const ks = i;
    skipString();
    const k = json.slice(ks, i);
    ws();
    if (json[i] !== ":") return null;
    i++;
    ws();
    const vs = i;
    skipValue();
    if (k === want) return json.slice(vs, i);
    ws();
    if (json[i] === ",") i++;
    else return null;
  }
}

function cleanErrors(raw: unknown): CoreError[] {
  if (!Array.isArray(raw)) return [{ code: "share_core_error", path: "" }];
  return raw.slice(0, 100).map((e) => {
    const x = (e ?? {}) as Record<string, unknown>;
    const out: CoreError = { code: typeof x.code === "string" ? x.code : "invalid", path: typeof x.path === "string" ? x.path : "" };
    if (typeof x.limit === "number") out.limit = x.limit;
    if (typeof x.offset16 === "number") out.offset16 = x.offset16;
    if (typeof x.cp === "string" && /^U\+[0-9A-F]{4,6}$/.test(x.cp)) out.cp = x.cp;
    return out;
  });
}

function fail(r: Record<string, unknown>): Fail {
  const f: Fail = { ok: false, errors: cleanErrors(r.errors) };
  if (Array.isArray(r.label_findings)) f.label_findings = r.label_findings;
  if (Array.isArray(r.needs_order_ack)) f.needs_order_ack = r.needs_order_ack as Pos[];
  if (Array.isArray(r.order_reasons)) f.order_reasons = r.order_reasons;
  return f;
}

function opts(o: Record<string, unknown>): string {
  return JSON.stringify(o);
}

/** Labels as share-core expects them (`part` omitted when absent). */
function labelsOut(labels: LabelDecision[]): LabelDecision[] {
  return labels.map((l) => (l.part === undefined ? { question: l.question, choice: l.choice } : { question: l.question, part: l.part, choice: l.choice }));
}

function posOut(p: Pos[]): Pos[] {
  return p.map((x) => (x.part === undefined ? { question: x.question } : { question: x.question, part: x.part }));
}

export function makeCore(m: ShareCoreExports): Core {
  const call = (out: string): Record<string, unknown> => {
    const r = JSON.parse(out) as Record<string, unknown>;
    if (typeof r !== "object" || r === null) throw new Error("share-core returned a non-object");
    return r;
  };
  const validated = (out: string, r: Record<string, unknown>): Validated => {
    const json = rawField(out, "payload");
    if (json === null) throw new Error("share-core result has no payload");
    // Only the bytes are kept (the JSON text is decoded on demand): a 4 MB payload is not held twice.
    const bytes = enc.encode(json);
    return {
      ok: true,
      get json() {
        return new TextDecoder().decode(bytes);
      },
      bytes,
      payload: r.payload as PayloadJson,
      dropped: (r.dropped as unknown[]) ?? [],
    };
  };
  const hashes = (out: string): string[] | Fail => {
    const r = call(out);
    if (r.ok !== true) return fail(r);
    const h = r.hashes;
    if (!Array.isArray(h) || !h.every((x) => typeof x === "string" && /^[0-9a-f]{64}$/.test(x))) throw new Error("share-core hashes malformed");
    return h as string[];
  };
  let schema: { min: number; max: number; stripped: number } | null = null;
  return {
    schema() {
      if (!schema) {
        const r = JSON.parse(m.schema_versions()) as { min: number; max: number; stripped: number };
        schema = { min: r.min, max: r.max, stripped: r.stripped };
      }
      return schema;
    },
    validateForPublish(bytes, o) {
      const out = m.validate_for_publish(bytes, opts({ hide_key: o.hideKey, labels: labelsOut(o.labels) }));
      const r = call(out);
      return r.ok === true ? validated(out, r) : fail(r);
    },
    strip(bytes, o) {
      const out = m.strip(bytes, opts({ labels: labelsOut(o.labels), order_acks: posOut(o.orderAcks), image_ok: o.imageOk }));
      const r = call(out);
      if (r.ok !== true) return fail(r);
      const v = validated(out, r);
      const arr = <T>(k: string) => (Array.isArray(r[k]) ? (r[k] as T[]) : []);
      const s: Stripped = {
        ...v,
        matchHashes: arr<string>("match_hashes"),
        strippedHashes: arr<string>("stripped_hashes"),
        needsOrderAck: arr<Pos>("needs_order_ack"),
        orderReasons: arr("order_reasons"),
        labelsRemoved: arr<Pos>("labels_removed"),
        labelFindings: arr("label_findings"),
        servedOrders: arr("served_orders"),
        imagesKept: arr<number>("images_kept"),
        imagesRemoved: arr<number>("images_removed"),
        figuresRemoved: arr<number>("figures_removed"),
      };
      const n = (v.payload.bank?.questions.length ?? (v.payload.question ? 1 : 0));
      if (s.matchHashes.length !== n || s.strippedHashes.length !== n) throw new Error("share-core strip hashes do not match the question count");
      return s;
    },
    matchHashes: (bytes) => hashes(m.match_hash(bytes, "")),
    fullHashes: (bytes) => hashes(m.full_hash(bytes, "")),
    validateMeta(meta) {
      const r = call(m.validate_meta(enc.encode(JSON.stringify(meta ?? {})), ""));
      if (r.ok !== true) return fail(r);
      const x = r.meta as Meta;
      return { title: x.title ?? "", subject: x.subject ?? "", course: x.course ?? "", grade: x.grade ?? "", tags: x.tags ?? [], standards: x.standards ?? [] };
    },
    async imageInfo(bytes) {
      if (typeof m.image_check === "function") {
        const r = call(m.image_check(bytes, ""));
        if (r.ok !== true) return { refused: cleanErrors(r.errors)[0]?.code ?? "image_refused" };
        return { contentType: r.content_type as ImageInfo["contentType"], width: Number(r.width), height: Number(r.height) };
      }
      return checkImage(bytes);
    },
  };
}

/** True when `x` is a share-core refusal. */
export function isFail(x: unknown): x is Fail {
  return typeof x === "object" && x !== null && (x as { ok?: unknown }).ok === false;
}

/** The questions of a payload, in order. */
export function questionsOf(p: PayloadJson): QuestionJson[] {
  return p.bank ? p.bank.questions : p.question ? [p.question] : [];
}

/** The kind tag of a question (`"Mc"`, `"Tf"`, ...). */
export function kindTag(q: QuestionJson): string {
  return Object.keys(q.kind ?? {})[0] ?? "";
}
