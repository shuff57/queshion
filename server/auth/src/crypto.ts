// Small WebCrypto helpers. Everything here runs on Workers, Node 20+ and browsers.

const enc = new TextEncoder();

export function utf8(s: string): Uint8Array {
  return enc.encode(s);
}

export function b64url(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]!);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function fromB64url(s: string): Uint8Array {
  if (!/^[A-Za-z0-9_-]*$/.test(s)) throw new Error("bad base64url");
  const pad = s.length % 4 === 0 ? "" : "=".repeat(4 - (s.length % 4));
  const bin = atob(s.replace(/-/g, "+").replace(/_/g, "/") + pad);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Standard alphabet, unpadded: the encoding PHC strings use for salt and hash. */
export function b64(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]!);
  return btoa(bin).replace(/=+$/, "");
}

export function fromB64(s: string): Uint8Array {
  if (!/^[A-Za-z0-9+/]*$/.test(s)) throw new Error("bad base64");
  const pad = s.length % 4 === 0 ? "" : "=".repeat(4 - (s.length % 4));
  const bin = atob(s + pad);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export function randomBytes(n: number): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(n));
}

/** 256-bit random token, base64url (43 chars). */
export function randomToken(): string {
  return b64url(randomBytes(32));
}

export const TOKEN_RE = /^[A-Za-z0-9_-]{43}$/;

export async function sha256(s: string): Promise<string> {
  const d = await crypto.subtle.digest("SHA-256", utf8(s));
  return b64url(new Uint8Array(d));
}

/** Constant-time comparison of two byte arrays. Length is not secret. */
export function timingSafeEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  const subtle = crypto.subtle as SubtleCrypto & { timingSafeEqual?: (a: ArrayBufferView, b: ArrayBufferView) => boolean };
  if (typeof subtle.timingSafeEqual === "function") return subtle.timingSafeEqual(a, b);
  let diff = 0;
  for (let i = 0; i < a.length; i++) diff |= a[i]! ^ b[i]!;
  return diff === 0;
}

export interface Keys {
  /** Signs access tokens (HS256). */
  access: CryptoKey;
  /** Derives a refresh token's successor: HMAC(refresh, predecessor). */
  refresh: CryptoKey;
  /** Salted hash of an email address kept on a deleted account's tombstone. */
  email: CryptoKey;
  /** Keys rate-limit counters so they never hold a raw IP or email. */
  limit: CryptoKey;
  /** Signs known-device tokens. */
  device: CryptoKey;
  /** Signs short-lived sudo tokens (re-entered password). */
  sudo: CryptoKey;
}

const keyCache = new Map<string, Promise<Keys>>();

/**
 * Derives independent HMAC-SHA256 keys from one server secret with HKDF, so a single
 * secret can be configured without reusing a key across purposes.
 */
export function deriveKeys(secret: string): Promise<Keys> {
  let p = keyCache.get(secret);
  if (!p) {
    p = (async () => {
      const ikm = await crypto.subtle.importKey("raw", utf8(secret), "HKDF", false, ["deriveKey"]);
      const derive = (info: string, usages: ("sign" | "verify")[]) =>
        crypto.subtle.deriveKey(
          { name: "HKDF", hash: "SHA-256", salt: utf8("d1-auth/v1"), info: utf8(info) },
          ikm,
          { name: "HMAC", hash: "SHA-256", length: 256 },
          false,
          usages,
        );
      return {
        access: await derive("access-token", ["sign", "verify"]),
        refresh: await derive("refresh-successor", ["sign"]),
        email: await derive("tombstone-email", ["sign"]),
        limit: await derive("rate-limit-key", ["sign"]),
        device: await derive("known-device", ["sign", "verify"]),
        sudo: await derive("sudo", ["sign", "verify"]),
      };
    })();
    keyCache.set(secret, p);
  }
  return p;
}

export async function hmac(key: CryptoKey, data: string): Promise<string> {
  const sig = await crypto.subtle.sign("HMAC", key, utf8(data));
  return b64url(new Uint8Array(sig));
}
