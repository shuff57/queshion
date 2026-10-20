// Licences and the inheritance table (docs/shared-pool.md "Sharing model", "Licenses inherit, per
// question"). A CC grant is per version; a copy keeps the licence of the publish it came from.

export const LICENSES = ["CC-BY-4.0", "CC0-1.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0", "circle-only"] as const;
export type License = (typeof LICENSES)[number];

export function isLicense(x: unknown): x is License {
  return typeof x === "string" && (LICENSES as readonly string[]).includes(x);
}

/**
 * Origin licence -> licences a derived publish may carry. "Stricter" never means a different
 * condition set: BY-SA cannot become BY-NC-SA. circle-only material cannot be re-shared by an
 * importer at all (its author can still republish their own).
 */
const MAY: Record<License, readonly License[]> = {
  "CC0-1.0": LICENSES,
  "CC-BY-4.0": ["CC-BY-4.0", "CC-BY-SA-4.0", "CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"],
  "CC-BY-SA-4.0": ["CC-BY-SA-4.0"],
  "CC-BY-NC-4.0": ["CC-BY-NC-4.0", "CC-BY-NC-SA-4.0"],
  "CC-BY-NC-SA-4.0": ["CC-BY-NC-SA-4.0"],
  "circle-only": [],
};

/** May material under `origin` be published by someone else under `target`? */
export function mayRepublish(origin: string, target: string): boolean {
  return isLicense(origin) && isLicense(target) && MAY[origin].includes(target);
}

/** The licences an importer may publish a copy under (for the import response). */
export function republishOptions(origin: string): readonly License[] {
  return isLicense(origin) ? MAY[origin] : [];
}

/** Licences allowed by every one of `origins` (empty when none is). */
export function allowedByAll(origins: Iterable<string>): License[] {
  let out: License[] = [...LICENSES];
  for (const o of origins) out = out.filter((t) => mayRepublish(o, t));
  return out;
}

/**
 * Revisions of the rights-attestation wording the server accepts. The app shows the newest; an
 * older one stays accepted until the app versions that show it are gone.
 */
export const ATTEST_TEXT_REVS = ["2026-10-19"] as const;

/**
 * One grant a question carries: the licence of an upstream publish of item `item`, and whether the
 * copy changed it. A verbatim copy keeps its licence (CC 4.0 lets you add terms only to your own
 * adaptation), except that CC0 material may be relabelled freely; an adaptation follows the table.
 */
export interface Grant {
  item: string;
  license: string;
  modified: boolean;
}

export function grantOptions(g: Grant): readonly License[] {
  if (!isLicense(g.license) || g.license === "circle-only") return [];
  if (g.modified || g.license === "CC0-1.0") return MAY[g.license];
  return [g.license];
}

/**
 * THE licence rule, used by both the import response (`may_republish_as`) and the publish check, so
 * the two can never disagree. Within one item: union of the verbatim grants if there are any,
 * otherwise intersection; across different items: intersection. No grants: any licence.
 */
export function allowedFor(grants: Grant[]): License[] {
  const byItem = new Map<string, Grant[]>();
  for (const g of grants) byItem.set(g.item, [...(byItem.get(g.item) ?? []), g]);
  let out: License[] = [...LICENSES];
  for (const gs of byItem.values()) {
    // Within one item (critic C4): a verbatim copy of some version may use any of the grants of the
    // versions it is identical to (union of the verbatim grants only). An adaptation must satisfy
    // every version it drew on (intersection): adding an NC-only explanation to a v2 never makes it
    // CC-BY just because v1 was.
    const verbatim = gs.filter((g) => !g.modified);
    let ok: Set<License>;
    if (verbatim.length > 0) ok = new Set(verbatim.flatMap((g) => [...grantOptions(g)]));
    else {
      ok = new Set(LICENSES);
      for (const g of gs) {
        const o = new Set(grantOptions(g));
        ok = new Set([...ok].filter((l) => o.has(l)));
      }
    }
    out = out.filter((l) => ok.has(l));
  }
  return out;
}

/** A bank: every question's grants must allow the licence. */
export function allowedForAll(perQuestion: Grant[][]): License[] {
  let out: License[] = [...LICENSES];
  for (const g of perQuestion) {
    const a = new Set(allowedFor(g));
    out = out.filter((l) => a.has(l));
  }
  return out;
}

/** Grants carried by a stored chain (oldest first): every earlier link counts as adapted. */
export function chainGrants(chain: { item_id: string; license: string }[]): Grant[] {
  return chain.filter((l) => l.item_id !== "" && l.license !== "").map((l) => ({ item: l.item_id, license: l.license, modified: true }));
}
