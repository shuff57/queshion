import { COMMON_PASSWORDS, COMMON_BASE_WORDS } from "./common-passwords.js";
import { b64, fromB64, randomBytes, timingSafeEqual, utf8 } from "./crypto.js";
import type { Argon2idHasher, Argon2Params } from "./types.js";

export const MAX_PASSWORD_CHARS = 512;
const SALT_BYTES = 16;
const TAG_BYTES = 32;

/** NIST SP 800-63B: normalize Unicode so the same password typed on two keyboards matches. */
export function normalizePassword(pw: string): string {
  return pw.normalize("NFKC");
}

const LEET: Record<string, string> = { "0": "o", "1": "i", "3": "e", "4": "a", "5": "s", "7": "t", "@": "a", "$": "s" };

/** Undo substitutions inside a word only (p@ssw0rd -> password); leading/trailing digits stay digits. */
function deLeet(s: string): string {
  const m = /^([^\p{L}]*)(.*?)([^\p{L}]*)$/u.exec(s);
  if (!m) return s;
  return m[1] + m[2]!.replace(/[013457@$]/g, (c) => LEET[c]!) + m[3];
}

function isRunOrRepeat(s: string): boolean {
  if (s.length < 3) return false;
  const step = s.charCodeAt(1) - s.charCodeAt(0);
  if (Math.abs(step) > 1) return false;
  for (let i = 2; i < s.length; i++) if (s.charCodeAt(i) - s.charCodeAt(i - 1) !== step) return false;
  return true;
}

/**
 * Cheap, synchronous policy check (no I/O): length, the built-in common-password list,
 * "common word plus digits/symbols", runs like 1234567890, and the user's own email.
 * Returns a reason code or null.
 */
export function checkPasswordPolicy(
  password: string,
  email: string,
  minLength: number,
  extra: ReadonlySet<string>,
): string | null {
  const chars = [...password].length;
  if (chars < minLength) return "too_short";
  if (chars > MAX_PASSWORD_CHARS) return "too_long";
  const lower = password.toLowerCase();
  if (COMMON_PASSWORDS.has(lower) || extra.has(lower)) return "common";
  const core = deLeet(lower).replace(/^[^\p{L}]+/u, "").replace(/[^\p{L}]+$/u, "");
  if (COMMON_BASE_WORDS.has(core) || extra.has(core)) return "common";
  if (isRunOrRepeat(lower)) return "common";
  if (new Set(lower).size <= 2) return "common";
  const local = email.split("@")[0] ?? "";
  if (lower === email || lower.includes(email) || (local.length >= 4 && lower.includes(local))) return "contains_email";
  return null;
}

export interface StoredHash {
  phc: string;
  params: Argon2Params;
}

export async function hashPassword(hasher: Argon2idHasher, password: string, params: Argon2Params): Promise<StoredHash> {
  const salt = randomBytes(SALT_BYTES);
  const tag = await hasher.argon2id(utf8(password), salt, params);
  if (tag.length !== TAG_BYTES) throw new Error("hasher returned a tag of the wrong length");
  return { phc: `$argon2id$v=19$m=${params.m},t=${params.t},p=${params.p}$${b64(salt)}$${b64(tag)}`, params };
}

const PHC_RE = /^\$argon2id\$v=19\$m=(\d{1,7}),t=(\d{1,3}),p=(\d{1,3})\$([A-Za-z0-9+/]{16,})\$([A-Za-z0-9+/]{16,})$/;

export function parsePhc(phc: string): { params: Argon2Params; salt: Uint8Array; tag: Uint8Array } | null {
  const m = PHC_RE.exec(phc);
  if (!m) return null;
  return {
    params: { m: Number(m[1]), t: Number(m[2]), p: Number(m[3]) },
    salt: fromB64(m[4]!),
    tag: fromB64(m[5]!),
  };
}

export async function verifyPassword(hasher: Argon2idHasher, phc: string, password: string): Promise<boolean> {
  const parsed = parsePhc(phc);
  if (!parsed) return false;
  const tag = await hasher.argon2id(utf8(password), parsed.salt, parsed.params);
  return timingSafeEqual(tag, parsed.tag);
}

/**
 * Full-cost hash whose result is discarded, for an email that has no account. A cheaper
 * stand-in would itself reveal which emails exist.
 */
export async function dummyVerify(hasher: Argon2idHasher, password: string, params: Argon2Params): Promise<false> {
  const tag = await hasher.argon2id(utf8(password), randomBytes(SALT_BYTES), params);
  timingSafeEqual(tag, randomBytes(TAG_BYTES));
  return false;
}

export function sameParams(a: Argon2Params, b: Argon2Params): boolean {
  return a.m === b.m && a.t === b.t && a.p === b.p;
}

export function paramsJson(p: Argon2Params): string {
  return JSON.stringify({ alg: "argon2id", v: 19, m: p.m, t: p.t, p: p.p });
}
