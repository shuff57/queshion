// Minimal HS256 JWT for short-lived access tokens. Only HS256 is ever accepted.

import { b64url, fromB64url, utf8 } from "./crypto.js";

const HEADER = b64url(utf8(JSON.stringify({ alg: "HS256", typ: "at+jwt" })));

export interface AccessClaims {
  iss: string;
  aud: string;
  sub: string;
  sid: string;
  trust: string;
  iat: number;
  exp: number;
  jti: string;
}

export async function signAccessToken(key: CryptoKey, claims: AccessClaims): Promise<string> {
  const payload = b64url(utf8(JSON.stringify(claims)));
  const input = `${HEADER}.${payload}`;
  const sig = new Uint8Array(await crypto.subtle.sign("HMAC", key, utf8(input)));
  return `${input}.${b64url(sig)}`;
}

/** Returns the claims if the token is well formed, correctly signed, unexpired and for us. */
export async function verifyAccessToken(
  key: CryptoKey,
  token: string,
  expect: { iss: string; aud: string; nowSeconds: number },
): Promise<AccessClaims | null> {
  const parts = token.split(".");
  if (parts.length !== 3) return null;
  const [h, p, s] = parts as [string, string, string];
  // The header must be byte-identical to ours: no alg confusion, no "none", no kid games.
  if (h !== HEADER) return null;
  let sig: Uint8Array;
  try {
    sig = fromB64url(s);
  } catch {
    return null;
  }
  const ok = await crypto.subtle.verify("HMAC", key, sig, utf8(`${h}.${p}`));
  if (!ok) return null;
  let c: AccessClaims;
  try {
    c = JSON.parse(new TextDecoder().decode(fromB64url(p))) as AccessClaims;
  } catch {
    return null;
  }
  if (typeof c !== "object" || c === null) return null;
  if (c.iss !== expect.iss || c.aud !== expect.aud) return null;
  if (typeof c.sub !== "string" || typeof c.sid !== "string" || typeof c.trust !== "string") return null;
  if (typeof c.exp !== "number" || typeof c.iat !== "number") return null;
  if (c.exp <= expect.nowSeconds) return null;
  if (c.iat > expect.nowSeconds + 60) return null;
  return c;
}
