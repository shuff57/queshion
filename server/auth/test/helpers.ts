import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { createAuth, type Auth, type AuthConfig, type AuthEvent, type MailMessage } from "../src/index.js";
import type { Argon2idHasher, Argon2Params } from "../src/types.js";

export const API = "https://api.example.org";
export const ORIGIN = "https://app.example.org";
export const OTHER_ORIGIN = "https://evil.example.net";
export const GOOD_PW = "violet-harbor-tractor-91";

/** Fast deterministic stand-in for Argon2id that counts its calls. */
export class StubHasher implements Argon2idHasher {
  calls = 0;
  /** Runs after the next hash, once: simulates another request committing mid-flight. */
  onHash: (() => Promise<void>) | null = null;
  async argon2id(password: Uint8Array, salt: Uint8Array, p: Argon2Params): Promise<Uint8Array> {
    this.calls++;
    const hook = this.onHash;
    this.onHash = null;
    if (hook) await hook();
    const pre = new TextEncoder().encode(`${p.m},${p.t},${p.p}|`);
    const buf = new Uint8Array(pre.length + salt.length + 1 + password.length);
    buf.set(pre, 0);
    buf.set(salt, pre.length);
    buf.set(password, pre.length + salt.length + 1);
    return new Uint8Array(await crypto.subtle.digest("SHA-256", buf));
  }
}

export class RecordingMailer {
  sent: MailMessage[] = [];
  async send(m: MailMessage) {
    this.sent.push(m);
  }
  last(kind?: MailMessage["kind"], to?: string): MailMessage | undefined {
    return [...this.sent].reverse().find((m) => (!kind || m.kind === kind) && (!to || m.to === to));
  }
}

let seq = 0;
export function uniqueEmail(tag = "u"): string {
  return `${tag}.${Date.now().toString(36)}.${++seq}.${Math.random().toString(36).slice(2, 8)}@school.example.org`;
}

function randomIp(): string {
  const b = crypto.getRandomValues(new Uint8Array(3));
  return `10.${b[0]}.${b[1]}.${b[2]}`;
}

export interface CallOpts {
  body?: unknown;
  rawBody?: string;
  headers?: Record<string, string>;
  origin?: string | null;
  native?: boolean;
  cookie?: string;
  bearer?: string;
  /** null sends no CF-Connecting-IP header. */
  ip?: string | null;
  contentType?: string | null;
}

export class Harness {
  hasher = new StubHasher();
  mailer = new RecordingMailer();
  events: AuthEvent[] = [];
  clock = Date.UTC(2026, 9, 19, 12, 0, 0);
  ip = randomIp();
  secret = `test-secret-${crypto.randomUUID()}-${crypto.randomUUID()}`;
  auth: Auth;

  constructor(overrides: Partial<AuthConfig> = {}) {
    this.auth = this.build(overrides);
  }

  build(overrides: Partial<AuthConfig> = {}): Auth {
    this.auth = createAuth({
      db: env.DB,
      hasher: this.hasher,
      mailer: this.mailer,
      secret: this.secret,
      allowedOrigins: [ORIGIN],
      trust: { levels: ["unverified", "pending", "member", "admin"], afterVerify: "pending", requestApproval: true },
      now: () => this.clock,
      onEvent: (e) => this.events.push(e),
      ...overrides,
    });
    return this.auth;
  }

  advance(seconds: number) {
    this.clock += seconds * 1000;
  }

  async call(method: string, path: string, o: CallOpts = {}): Promise<Response> {
    const h = new Headers(o.headers ?? {});
    const origin = o.origin === undefined ? (o.native ? null : ORIGIN) : o.origin;
    if (origin !== null) h.set("Origin", origin);
    if (o.native) h.set("X-Client", "native");
    if (o.cookie) h.set("Cookie", o.cookie);
    if (o.bearer) h.set("Authorization", `Bearer ${o.bearer}`);
    if (o.ip !== null) h.set("CF-Connecting-IP", o.ip ?? this.ip);
    let body: string | undefined;
    if (o.rawBody !== undefined) body = o.rawBody;
    else if (o.body !== undefined) body = JSON.stringify(o.body);
    if (body !== undefined && o.contentType !== null) h.set("Content-Type", o.contentType ?? "application/json");
    const req = new Request(`${API}${path}`, { method, headers: h, body });
    const ctx = createExecutionContext();
    const res = await this.auth.handle(req, ctx);
    await waitOnExecutionContext(ctx);
    if (!res) throw new Error(`no route for ${method} ${path}`);
    return res;
  }

  /** Signs up and returns the verification token mailed out. */
  async signup(email: string, password = GOOD_PW, extra: Record<string, unknown> = {}): Promise<string> {
    const res = await this.call("POST", "/auth/signup", { body: { email, password, display_name: "Ms. Test", ...extra } });
    if (res.status !== 202) throw new Error(`signup failed: ${res.status} ${await res.text()}`);
    const m = this.mailer.last("verify", email);
    if (!m?.token) throw new Error("no verify mail");
    return m.token;
  }

  /** Signs up and confirms, choosing the password and profile at confirmation (the one-step verify). */
  async signupVerified(email: string, password = GOOD_PW): Promise<void> {
    const token = await this.signup(email, password);
    const res = await this.call("POST", "/auth/verify", { body: { token, password, display_name: "Ms. Test" } });
    if (res.status !== 200) throw new Error(`verify failed: ${res.status}`);
  }

  async loginWeb(email: string, password = GOOD_PW): Promise<{ res: Response; access: string; refresh: string }> {
    const res = await this.call("POST", "/auth/login", { body: { email, password } });
    const body = (await res.clone().json()) as { access_token: string };
    return { res, access: body.access_token, refresh: cookieValue(res, "__Host-refresh") ?? "" };
  }

  async loginNative(email: string, password = GOOD_PW): Promise<{ res: Response; access: string; refresh: string }> {
    const res = await this.call("POST", "/auth/login", { body: { email, password }, native: true });
    const body = (await res.clone().json()) as { access_token: string; refresh_token: string };
    return { res, access: body.access_token, refresh: body.refresh_token };
  }

  refreshWeb(refresh: string, o: CallOpts = {}) {
    return this.call("POST", "/auth/refresh", { cookie: `__Host-refresh=${refresh}`, ...o });
  }

  refreshNative(refresh: string, o: CallOpts = {}) {
    return this.call("POST", "/auth/refresh", { native: true, body: { refresh_token: refresh }, ...o });
  }
}

export function cookieValue(res: Response, name: string): string | null {
  const sc = res.headers.get("Set-Cookie");
  if (!sc) return null;
  const m = new RegExp(`(?:^|, )${name.replace(/[-]/g, "\\-")}=([^;]*)`).exec(sc);
  return m ? m[1]! : null;
}

/** Status, headers (minus Date) and body text, for comparing two responses byte for byte. */
export async function snapshot(res: Response): Promise<{ status: number; headers: [string, string][]; body: string }> {
  const headers = [...res.headers.entries()].filter(([k]) => k !== "date").sort(([a], [b]) => a.localeCompare(b));
  return { status: res.status, headers, body: await res.text() };
}

export async function sha256b64url(s: string): Promise<string> {
  const d = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(s)));
  let bin = "";
  for (const b of d) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** Most hits in any 24 h span (minutes since start, ascending). */
export function worst24h(hits: number[]): number {
  let w = 0;
  for (let i = 0, j = 0; i < hits.length; i++) {
    while (hits[i]! - hits[j]! >= 24 * 60) j++;
    w = Math.max(w, i - j + 1);
  }
  return w;
}

export { env };
