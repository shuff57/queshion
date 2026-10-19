# Phase 1 spike: Argon2id in a Worker

Measured with `server/share-core` (RustCrypto `argon2` 0.6, wasm-pack release build) served from
`server/worker/src/spike.ts` under `wrangler dev` 4.135 (local workerd), on the dev machine,
2026-10-19. Local workerd is not Cloudflare's edge: production CPU time per hash may differ, so
re-measure after the first deploy.

| Measurement | Result |
|---|---|
| wasm module size | 40 KB (release, wasm-opt) |
| 1 hash, m=19456 KiB (19 MiB), t=2, p=1 | about 25-35 ms warm, about 107 ms on the first (cold) call |
| 40 simultaneous requests, same params | 1.0 s wall, about 25 ms each, `peakInflight` = 1 |
| wasm linear memory after m=19 MiB | 21 MB |
| wasm linear memory after m=46 MiB | 69 MB (high-water mark; wasm memory never shrinks) |
| m=64 MiB, t=3 | 147 ms, memory stayed at 69 MB |

## What this settles

- **Concurrency does not multiply memory inside one isolate.** The hash is a synchronous wasm
  call, so requests in an isolate run one at a time (`peakInflight` stayed 1 with 40 concurrent
  requests). Memory is the high-water mark of one hash, not the sum of the concurrent ones. The
  spec's "serialize hashing with a semaphore" is therefore unnecessary and has been removed. Separate
  isolates each get their own 128 MB, so they do not compete either.
- **The real cost is CPU time, not memory.** Each login attempt costs about 25-100 ms of CPU. The
  free plan's 10 ms CPU limit cannot run it (confirms the paid-plan requirement). The pre-hash rate
  limit is what stops an attacker from burning CPU, so it stays.
- **Chosen parameters:** Argon2id, m=19456 KiB, t=2, p=1 (the OWASP minimum configuration), 16-byte
  random salt, 32-byte tag. Parameters are stored with each hash, so they can be raised later and
  hashes upgraded on the next login. m up to 46 MiB fits comfortably (69 MB wasm memory).
- Bundle cost is negligible (40 KB), so putting Argon2 in `share-core` costs nothing.

## Still to measure

- Sustained concurrent load on the edge. Per-request CPU time on the edge is measured below
  ("Edge measurement"); workerd timers do not advance during synchronous CPU, so those figures come
  from `wrangler tail` CPU stats.

## Edge measurement (2026-10-19, staging)

Deployed to `https://vershion-pool-staging.shuff57.workers.dev` (Worker `vershion-pool-staging`, D1
`vershion-pool-staging`, config `server/worker/wrangler.staging.toml`, log-mode mail). One probe
account ran sign-up, verify, 3 logins, 2 wrong passwords, 2 unknown-email logins, refresh and `/me`.
CPU and wall time are from `wrangler tail --format json` (`cpuTime`, `wallTime`, milliseconds); one
sample per request, so treat them as indicative, not as a benchmark.

| Request | CPU ms | Wall ms |
|---|---|---|
| `POST /auth/signup` (cold, 1 hash) | 81 | 259 |
| `POST /auth/verify` (1 hash) | 107 | 413 |
| `POST /auth/login` 200 (3 runs) | 58, 102, 160 | 435-598 |
| `POST /auth/login` 401, wrong password (2 runs) | 119, 84 | 550, 444 |
| `POST /auth/login` 401, unknown email (2 runs) | 95, 85 | 1227, 460 |
| `POST /auth/refresh` | 5 | 104 |
| `GET /me` | 6 | 60 |

What this shows:
- A request that hashes costs roughly 60-160 ms of CPU on the edge, about 2-4x the local figure
  (25-35 ms) once D1 calls, JWT signing and variance are included. Requests that do not hash cost
  5-6 ms. The paid plan's CPU limit (30 s by default) is not a constraint; the free plan's 10 ms is,
  as expected.
- Wrong-password and unknown-email logins cost about the same CPU (84-119 vs 85-95 ms), so the
  dummy-hash design holds on the edge. Wall time varies more (444-1227 ms) because of D1 and network
  latency; that variance is larger than the CPU difference between the two cases.
- Cost: at about 100 ms CPU per login the Workers Standard plan's included CPU time covers hundreds
  of thousands of logins a month; the per-IP and per-account limits are what bound abuse.

Not measured: sustained concurrent load, and a warm-vs-cold breakdown beyond the first request.
