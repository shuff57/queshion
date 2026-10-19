import type { RateLimitResult, RateLimitRule, RateLimitStore } from "./types.js";

const SUB_BUCKETS = 12;

interface Bucket {
  window_start: number;
  count: number;
}

/** Sub-bucket width for a rule, in ms. */
function bucketMs(rule: RateLimitRule): number {
  return Math.max(1, Math.round((rule.windowSeconds * 1000) / SUB_BUCKETS));
}

/**
 * The windowed count at time `at`: every bucket inside (at - window, at] counts fully, except the
 * oldest, partly elapsed one, which is weighted by how much of it is still inside.
 */
function estimateAt(buckets: Bucket[], rule: RateLimitRule, at: number): number {
  const w = rule.windowSeconds * 1000;
  const b = bucketMs(rule);
  const edge = at - w;
  let sum = 0;
  for (const x of buckets) {
    if (x.window_start + b <= edge) continue; // wholly outside
    sum += x.window_start <= edge ? (x.count * (x.window_start + b - edge)) / b : x.count;
  }
  return sum;
}

/**
 * Decides one hit given the buckets as they are after counting it. When refused, Retry-After is
 * the first moment the same estimate (with the retry's own hit) is back within the limit. The
 * estimate only falls as time passes, so a bisection finds it exactly.
 */
export function decide(buckets: Bucket[], rule: RateLimitRule, now: number): RateLimitResult {
  if (estimateAt(buckets, rule, now) <= rule.limit) return { allowed: true, retryAfter: 0 };
  // A retry lands in a later bucket: the denied hit is rolled back and the retry re-adds one.
  let lo = now;
  let hi = now + rule.windowSeconds * 1000 + bucketMs(rule);
  for (let i = 0; i < 40; i++) {
    const mid = (lo + hi) / 2;
    if (estimateAt(buckets, rule, mid) <= rule.limit) hi = mid;
    else lo = mid;
  }
  return { allowed: false, retryAfter: Math.max(1, Math.ceil((hi - now) / 1000)) };
}

/**
 * Sliding-window counter in D1. Each window is split into 12 sub-buckets; the estimate is the sum
 * of the buckets inside the window, with only the oldest, partly elapsed bucket weighted by its
 * overlap. So the count over any span of one window is exact to within about 1/12 of the limit,
 * which the classic two-bucket estimate cannot promise (it can admit nearly twice the limit when
 * hits bunch up before a boundary).
 * The increment is one atomic UPSERT, so concurrent requests cannot overshoot the limit. A denied
 * hit is rolled back, so a lockout ends once the counted hits age out rather than being extended
 * by every rejected try.
 */
export class D1RateLimitStore implements RateLimitStore {
  constructor(private readonly db: D1Database) {}

  async hit(key: string, rule: RateLimitRule, now: number): Promise<RateLimitResult> {
    const w = rule.windowSeconds * 1000;
    const b = bucketMs(rule);
    const cur = Math.floor(now / b) * b;
    const oldest = Math.floor((now - w) / b) * b;
    const [, rows] = await this.db.batch<Bucket>([
      this.db
        .prepare(
          `INSERT INTO rate_limits (key, window_start, count, expires_at) VALUES (?1, ?2, 1, ?3)
           ON CONFLICT (key, window_start) DO UPDATE SET count = count + 1`,
        )
        .bind(key, cur, cur + b + w),
      this.db
        .prepare(`SELECT window_start, count FROM rate_limits WHERE key = ?1 AND window_start >= ?2 AND window_start <= ?3 ORDER BY window_start`)
        .bind(key, oldest, cur),
    ]);
    return decide(rows!.results, rule, now);
  }

  /** Undo a hit (a refused request, or a success that should not count). */
  async undo(key: string, rule: RateLimitRule, now: number): Promise<void> {
    const b = bucketMs(rule);
    const cur = Math.floor(now / b) * b;
    await this.db
      .prepare(`UPDATE rate_limits SET count = count - 1 WHERE key = ?1 AND window_start = ?2 AND count > 0`)
      .bind(key, cur)
      .run();
  }
}

/**
 * The same estimator over an in-process map. For tests and simulations (it is exact and fast), or
 * a single-isolate deployment; it is NOT shared between Worker isolates, so production uses D1.
 */
export class MemoryRateLimitStore implements RateLimitStore {
  private readonly rows = new Map<string, Map<number, number>>();

  async hit(key: string, rule: RateLimitRule, now: number): Promise<RateLimitResult> {
    const b = bucketMs(rule);
    const cur = Math.floor(now / b) * b;
    const oldest = Math.floor((now - rule.windowSeconds * 1000) / b) * b;
    let m = this.rows.get(key);
    if (!m) this.rows.set(key, (m = new Map()));
    m.set(cur, (m.get(cur) ?? 0) + 1);
    for (const start of m.keys()) if (start < oldest) m.delete(start);
    const buckets = [...m.entries()].sort((x, y) => x[0] - y[0]).map(([window_start, count]) => ({ window_start, count }));
    return decide(buckets, rule, now);
  }

  async undo(key: string, rule: RateLimitRule, now: number): Promise<void> {
    const b = bucketMs(rule);
    const cur = Math.floor(now / b) * b;
    const m = this.rows.get(key);
    const c = m?.get(cur) ?? 0;
    if (m && c > 0) m.set(cur, c - 1);
  }
}

/**
 * Groups IPv6 addresses by prefix: /64 by default (one subscriber usually holds a whole /64);
 * /48 for login-failure counting, because one customer can often rotate through 65,536 /64s.
 */
export function ipBucket(ip: string, prefix: 64 | 48 = 64): string {
  if (!ip.includes(":")) return ip;
  const zone = ip.indexOf("%");
  const addr = (zone >= 0 ? ip.slice(0, zone) : ip).toLowerCase();
  if (addr.includes(".")) return addr; // IPv4-mapped; rare enough to keep whole
  const [head, tail] = addr.split("::");
  const h = head ? head.split(":") : [];
  const t = tail !== undefined && tail !== "" ? tail.split(":") : [];
  const fill = addr.includes("::") ? new Array(Math.max(0, 8 - h.length - t.length)).fill("0") : [];
  const groups = [...h, ...fill, ...t].map((g) => g.replace(/^0+(?=.)/, ""));
  return groups.slice(0, prefix / 16).join(":") + `::/${prefix}`;
}
