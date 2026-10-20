// Provenance and licence grants, shared by the publish check and the import response so that the
// two can never disagree (critic A4). Everything here is built from server records: the account's
// own import_questions rows and the item's previous publish. A client-declared origin is a hint
// that counts only when it names a question this account really imported, at that version.
import type { QuestionJson } from "./core.ts";
import { chainGrants, type Grant } from "./license.ts";
import { ID_RE, sha256Hex, type Ctx } from "./shared.ts";
import { admissibleWith, sameAs, type OwnWork, type Subject } from "./visible.ts";

export interface ChainLink {
  author_name: string;
  item_id: string;
  qid: string;
  version: number;
  license: string;
  modified: boolean;
}

/** One question this account was served (an import, or a plain read: a 'view'). */
export interface ImportHit {
  match_hash: string;
  qid: string;
  item_id: string;
  content_hash: string;
  keyfree_hash: string;
  chain_json: string;
  author_name: string;
  license: string;
  version: number;
  at: number;
  /** The form this account was served: decides what the licence comparison may look at. */
  form: "full" | "stripped";
  /** question_ids.seq of the source question (for the "it came first" rule); 0 when purged. */
  source_first_seq: number;
  stem_hash: string | null;
  kind: "import" | "view";
}

/**
 * Everything `?1` was served, imports and reads alike, as recorded when it was served (durable: no
 * retention or purge of the source can erase it; round 5 E3a), minus voided evidence, one row per
 * licence the account was shown (a CC grant is irrevocable).
 */
const SERVED = `WITH served AS (
  SELECT iq.match_hash, iq.qid, iq.item_id, iq.content_hash, iq.keyfree_hash, iq.chain_json, iq.author_name, lic.value AS license, im.version, im.at, im.form,
         COALESCE(iq.source_first_seq, 0) AS source_first_seq, iq.stem_hash, im.kind
    FROM import_questions iq JOIN imports im ON im.user_id = iq.user_id AND im.publish_id = iq.publish_id AND im.item_id = iq.item_id
    JOIN json_each(COALESCE(im.licenses_json, json_array(im.license))) lic
   WHERE iq.user_id = ?1 AND im.author_id <> ?1 AND iq.void_at IS NULL)`;
const COLS = `match_hash, qid, item_id, content_hash, keyfree_hash, chain_json, author_name, license, version, at, form, source_first_seq, stem_hash, kind`;

/** Every question served to `me` with one of `hashes` (match_hash ignores the key), excluding the account's own items. */
export async function importHits(c: Ctx, me: string, hashes: string[]): Promise<ImportHit[]> {
  if (hashes.length === 0) return [];
  const r = await c.db
    .prepare(`${SERVED} SELECT ${COLS} FROM served WHERE match_hash IN (SELECT value FROM json_each(?2))`)
    .bind(me, JSON.stringify([...new Set(hashes)]))
    .all<ImportHit>();
  return r.results;
}

/** Served questions whose loose stem hash matches (a soft signal only). */
export async function stemHits(c: Ctx, me: string, stems: string[]): Promise<Set<string>> {
  if (stems.length === 0) return new Set();
  const r = await c.db
    .prepare(`${SERVED} SELECT DISTINCT stem_hash FROM served WHERE stem_hash IN (SELECT value FROM json_each(?2))`)
    .bind(me, JSON.stringify([...new Set(stems)]))
    .all<{ stem_hash: string }>();
  return new Set(r.results.map((x) => x.stem_hash));
}

/** Declared origins that name a served question at the version served; keyed `item/qid/version`. */
export async function declaredImportHits(c: Ctx, me: string, declared: ({ item: string; qid: string; version: number } | null)[]): Promise<Map<string, ImportHit>> {
  const want = declared.filter((d): d is { item: string; qid: string; version: number } => d !== null && ID_RE.test(d.item) && ID_RE.test(d.qid));
  const out = new Map<string, ImportHit>();
  if (want.length === 0) return out;
  const r = await c.db
    .prepare(
      `${SERVED} SELECT ${COLS} FROM served
        WHERE EXISTS (SELECT 1 FROM json_each(?2) j WHERE j.value ->> '$.item' = served.item_id AND j.value ->> '$.qid' = served.qid AND j.value ->> '$.version' = served.version)`,
    )
    .bind(me, JSON.stringify(want))
    .all<ImportHit>();
  for (const h of r.results) out.set(`${h.item_id}/${h.qid}/${h.version}`, h);
  return out;
}

/** The newest served copy of each `(item, qid)` (a kept question's recorded origin); keyed `item/qid`. */
export async function carriedImportHits(c: Ctx, me: string, refs: { item: string; qid: string }[]): Promise<Map<string, ImportHit>> {
  const out = new Map<string, ImportHit>();
  if (refs.length === 0) return out;
  const r = await c.db
    .prepare(
      `${SERVED} SELECT ${COLS} FROM served
        WHERE EXISTS (SELECT 1 FROM json_each(?2) j WHERE j.value ->> '$.item' = served.item_id AND j.value ->> '$.qid' = served.qid)
        ORDER BY version, at`,
    )
    .bind(me, JSON.stringify(refs))
    .all<ImportHit>();
  for (const h of r.results) out.set(`${h.item_id}/${h.qid}`, h); // ordered: the newest wins
  return out;
}

/** What `me` authored, for the evidence rules: their item ids, per match_hash their first publish (seq) and their full contents. */
export interface AuthorCtx {
  mine: Set<string>;
  firstAt: Map<string, number>;
  own: Map<string, OwnWork>;
}

export async function authorContext(c: Ctx, me: string, hashes: string[]): Promise<AuthorCtx> {
  const items = await c.db.prepare(`SELECT id FROM items WHERE author_id = ?1`).bind(me).all<{ id: string }>();
  const firstAt = new Map<string, number>();
  const own = new Map<string, OwnWork>();
  if (hashes.length > 0) {
    const r = await c.db
      .prepare(
        `SELECT iq.match_hash, MIN(qi.seq) AS at, json_group_array(iq.full_content) AS fulls FROM item_questions iq
           JOIN item_publishes p ON p.publish_id = iq.publish_id AND p.author_id = ?1 AND p.status IN ('current', 'previous')
           JOIN question_ids qi ON qi.qid = iq.qid
          WHERE iq.match_hash IN (SELECT value FROM json_each(?2)) GROUP BY iq.match_hash`,
      )
      .bind(me, JSON.stringify([...new Set(hashes)]))
      .all<{ match_hash: string; at: number; fulls: string }>();
    for (const x of r.results) {
      firstAt.set(x.match_hash, x.at);
      own.set(x.match_hash, { full: new Set(JSON.parse(x.fulls) as string[]) });
    }
  }
  return { mine: new Set(items.results.map((x) => x.id)), firstAt, own };
}

/** The evidence rules, applied through the visible-derivation boundary (visible.ts). */
export function admissible(h: ImportHit, ctx: AuthorCtx, s: Subject): boolean {
  const chainItems = (JSON.parse(h.chain_json) as ChainLink[]).map((l) => l.item_id);
  return admissibleWith({ ...h, chain_items: chainItems, first_seq: h.source_first_seq }, s, ctx.mine, ctx.firstAt.get(h.match_hash), ctx.own.get(h.match_hash));
}

/** Grants of admissible sources for a subject; "identical" decided by visible.ts alone. */
export function grantsFor(sources: ImportHit[], s: Subject): Grant[] {
  return evidenceGrants(sources.map((h) => ({ h, licMod: !sameAs(s, h) })));
}

/**
 * Grants from admissible sources (critic D3 iii): when a source is identical to what is published
 * (licence sense), only the identical sources grant; derivatives that merely share the question do
 * not narrow a verbatim copy. Otherwise every source does (an adaptation must satisfy them all).
 */
export function evidenceGrants(sources: { h: ImportHit; licMod: boolean }[]): Grant[] {
  const identical = sources.filter((s) => !s.licMod);
  return (identical.length > 0 ? identical : sources).flatMap((s) => linkGrants(JSON.parse(s.h.chain_json) as ChainLink[], { item: s.h.item_id, license: s.h.license, modified: s.licMod }));
}

/** A loose stem hash: case-folded letters and digits only. */
export async function stemHash(stem: unknown): Promise<string | null> {
  if (typeof stem !== "string") return null;
  const norm = stem.normalize("NFKC").toLowerCase().replace(/[^\p{L}\p{N}]+/gu, " ").trim();
  if (norm.length < 12) return null; // too short to mean anything
  return sha256Hex(new TextEncoder().encode(`vershion/stem/v1\n${norm}`));
}

export async function ownItemSet(c: Ctx, me: string, ids: string[]): Promise<Set<string>> {
  const want = [...new Set(ids.filter((x) => ID_RE.test(x)))];
  if (want.length === 0) return new Set();
  const r = await c.db.prepare(`SELECT id FROM items WHERE author_id = ?1 AND id IN (SELECT value FROM json_each(?2))`).bind(me, JSON.stringify(want)).all<{ id: string }>();
  return new Set(r.results.map((x) => x.id));
}

/** The grants a chain carries (earlier links as adaptations) plus its immediate source link. */
export function linkGrants(earlier: ChainLink[], immediate: Grant | null): Grant[] {
  return [...chainGrants(earlier), ...(immediate && immediate.item !== "" ? [immediate] : [])];
}

/**
 * Hash of the key-free facts of a question as served to non-authors: its match_hash (key-free by
 * construction in share-core), points, difficulty, topics, and the image if it is served. Never
 * the key, the explanation, a figure or an unticked image.
 */
export async function keyfreeHash(matchHash: string, served: QuestionJson, imgOf: (q: QuestionJson) => string | null): Promise<string> {
  const topics = [...(Array.isArray(served.topics) ? served.topics : [])].sort();
  const facts = JSON.stringify(["vershion/keyfree/v1", matchHash, served.points ?? null, served.difficulty ?? null, topics, imgOf(served)]);
  return sha256Hex(new TextEncoder().encode(facts));
}
