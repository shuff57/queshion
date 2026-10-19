// Text that a person typed and an admin will read. Control and format characters let an applicant
// fake labels (line breaks), hide or reorder text (bidi overrides U+202A-202E, isolates
// U+2066-2069), or smuggle zero-width characters (U+200B-200F). They are refused, not stripped, so
// what is stored is exactly what was typed.

/** Cc, Cf (incl. bidi controls and zero-width), lone surrogates, private use, unassigned, line/paragraph separators. */
const FORBIDDEN = /[\p{Cc}\p{Cf}\p{Cs}\p{Co}\p{Cn}\p{Zl}\p{Zp}]/u;

export function hasForbiddenChars(s: string): boolean {
  return FORBIDDEN.test(s);
}

/**
 * A short line of display text: NFKC-normalized, runs of whitespace collapsed to one space,
 * trimmed, 1..max characters, and free of control and format characters. Null when invalid.
 */
export function cleanText(v: unknown, max: number): string | null {
  if (typeof v !== "string") return null;
  if (hasForbiddenChars(v)) return null;
  const s = v.normalize("NFKC").replace(/\s+/gu, " ").trim();
  if (s.length === 0 || [...s].length > max || hasForbiddenChars(s)) return null;
  return s;
}

const MAX_PROFILE_JSON = 4096;
const MAX_PROFILE_STRING = 1000;

/** Every string anywhere in an app-defined profile is free of control and format characters, and the whole is small. */
export function profileIsSafe(profile: Record<string, unknown> | null): boolean {
  if (profile === null) return true;
  let ok = true;
  const walk = (v: unknown, depth: number): void => {
    if (!ok) return;
    if (depth > 4) ok = false;
    else if (typeof v === "string") ok = !hasForbiddenChars(v) && [...v].length <= MAX_PROFILE_STRING;
    else if (Array.isArray(v)) v.forEach((x) => walk(x, depth + 1));
    else if (v && typeof v === "object") for (const [k, x] of Object.entries(v)) (hasForbiddenChars(k) ? (ok = false) : walk(x, depth + 1));
  };
  walk(profile, 0);
  return ok && JSON.stringify(profile).length <= MAX_PROFILE_JSON;
}
