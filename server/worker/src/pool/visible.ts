// THE visible-derivation boundary (critic round 5, structural rule).
//
// Anything a non-author can see, or that decides an outcome a non-author can observe (which source a
// chain credits, the "modified" flag, the licence a copy may carry, may_republish_as, whether a
// source counts as evidence), is computed ONLY by comparing a `Subject` with a served source here.
// When the publish hides its key, its Subject carries no full-content hash at all: there is nothing
// key-bearing to compare, so no visible result can depend on the key. Full-content comparisons are
// possible only for a publish that shows its key (whose key is public anyway).

/** The new question, as far as visible derivations may look at it. */
export interface Subject {
  /** Key-free facts (match_hash, points, difficulty, topics, served image). */
  readonly keyfree: string;
  /** Full content (key included): only for a publish that shows its key; null when hidden. */
  readonly full: string | null;
}

/** A served source question (an import or a read). */
export interface Served {
  form: "full" | "stripped";
  content_hash: string;
  keyfree_hash: string;
}

export function subjectFor(hideKey: boolean, keyfree: string, full: string): Subject {
  return Object.freeze({ keyfree, full: hideKey ? null : full });
}

/**
 * Is the subject the same question as was served by `h`? Full content (explanation, points, image
 * and key included) only when both the subject shows its key and the source was served in full;
 * otherwise key-free facts only.
 */
export function sameAs(s: Subject, h: Served): boolean {
  if (s.full !== null && h.form === "full") return s.full === h.content_hash;
  return s.keyfree === h.keyfree_hash;
}

/** The author's own earlier work with this match_hash (for the "my own work" rule). */
export interface OwnWork {
  /** Full contents of their earlier publishes of such a question. */
  full: Set<string>;
}

/**
 * Evidence admissibility (critics round 4 D3 + round 5 E3d). A source counts unless it is the
 * publisher's own lineage (its item, or a chain through one) or came later than their own first
 * publish of the question. Such a source still counts when the publisher took ITS additions: a
 * publish that shows its key and equals the source's full served form, which is not the
 * publisher's own earlier content (an original author copying an adapter's explanation verbatim).
 */
export function admissibleWith(h: Served & { item_id: string; chain_items: string[]; first_seq: number }, s: Subject, mine: Set<string>, myFirstSeq: number | undefined, own: OwnWork | undefined): boolean {
  const derivedFromMine = mine.has(h.item_id) || h.chain_items.some((x) => mine.has(x));
  const later = myFirstSeq !== undefined && h.first_seq >= myFirstSeq;
  if (!derivedFromMine && !later) return true;
  if (mine.has(h.item_id)) return false;
  return s.full !== null && h.form === "full" && s.full === h.content_hash && !(own?.full.has(h.content_hash) ?? false);
}
