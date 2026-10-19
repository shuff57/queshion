# Phase 1 gauntlet loop

Paste the block below into a fresh Claude Code session opened in this repo. It builds phase 1 of
`docs/shared-pool.md` and loops until a blind critic prefers our auth to the reference.

```
Build phase 1 of docs/shared-pool.md in this repo under server/: a Cloudflare Worker with D1, email and password auth, sessions with HMAC refresh-token rotation and a reuse grace window, trust levels, an admin approval queue, rate limiting, and the app's sign-in screen. Start with an Argon2id spike using the RustCrypto argon2 crate compiled to wasm inside the share-core crate, and record measured time and memory under concurrent logins before choosing parameters. Keep the auth code as its own self-contained package inside server/ with no verSHion-specific coupling, a README, tests and a permissive license, because it will be published publicly later. Do not publish or push it.

The bar is Supabase Auth, github.com/supabase/auth. Clone it first and compare against its real code, tests and behavior, not a description of it.

Break this into the smallest pieces that can be improved and judged on their own: sign-up and verify, login, refresh rotation, password reset, rate limits, approval queue, sign-in screen. For each piece, fan out a builder and a separate critic with fresh context. The critic puts our behavior next to Supabase Auth's code and tests for the same flow, blind, says which is more secure and correct, and names the single biggest remaining gap. Then it goes back to the builder.

The critic should be a harsh critic. Praise is not useful. Our own tests must pass, with no account enumeration, no refresh token in a web response body and correct cookie flags. If ours does not win, it keeps going.

/loop on each piece until the critic picks ours blind. Do not stop before that.

Keep a live progress page updating as the work evolves so I can watch it.

Fan out subagents and ultracode.
```

## Notes

- The bar is Supabase Auth because it has refresh-token rotation with a reuse interval, which matches
  the spec's 45-second grace window. No Rust auth crate was mature enough to use or to measure against
  (see the search in the session that produced this file: better-auth-rs is alpha, rustauth is
  unmaintained, the rest have under 500 downloads).
- Argon2 comes from the RustCrypto `argon2` crate, inside `share-core`, so the spike measures the
  code that ships.
- The loop must leave publishing to the owner. Before the auth package goes public: check the
  license, remove anything verSHion-specific, and scan history for secrets.
- Run the phase 2 critic review (keyless form, hashes, publish order, licenses) before starting phase 2.
