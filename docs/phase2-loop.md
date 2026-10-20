# Phase 2 gauntlet loop

Paste the block below into a fresh Claude Code session opened in this repo, on the branch
`shared-pool-phase1` (or its successor). It builds phase 2 of `docs/shared-pool.md` (publish, browse,
import) and loops until the blind critics prefer our work to the references.

```
Build phase 2 of docs/shared-pool.md in this repo: publish, browse and import of banks and questions for public and unlisted items, with versioned immutable storage in R2 and D1, the share-core crate that strips answer keys server-side, content hashes and the license compatibility table, image blob rules and quotas, the publish-time review in the app, import with attribution and update notices, reports with takedown and R2 purge, and the Rust sharing panel and pool browser. Start by having a critic review the phase 2 sections of the spec and fix the spec before any code. Build on top of server/ and src/share/; do not touch the owner's uncommitted files (src/screens.rs, src/figuresui.rs, src/fonts.rs, src/figures/*), do not deploy, publish or push.

The bars are Moodle's question bank sharing, github.com/moodle/moodle, for behavior, and OWASP ASVS Level 2 chapters V4 access control and V12 files, owasp.org, for security. Clone Moodle and fetch the real checklist first and compare against them directly, not against a description of them.

Break this into the smallest pieces that can be improved and judged on their own: the strip function, the publish path, the blob rules, the import path, the license check, reports and takedown, the sharing panel, the pool browser. For each piece, fan out a builder and two separate critics with fresh context, one comparing behavior with Moodle and one auditing against ASVS. Each critic puts our work next to its bar blind, says which is better, and names the single biggest remaining gap. Then it goes back to the builder.

The critics should be harsh. Praise is not useful. Our tests must pass, no answer key may be reachable by a non-author through any read, search, preview or hash, and a student must never see a teacher's key. If ours does not win, it keeps going.

/loop on each piece until both critics pick ours blind. Do not stop before that.

Keep a live progress page updating as the work evolves so I can watch it.

Fan out subagents and ultracode.
```

## Notes

- Phase 2 sections of the spec still need their own critic pass (keyless form, hashes, publish order,
  license table); the prompt makes that the first step.
- The measurable half is the test suites (server and Rust) plus a property test that no non-author read
  path ever contains a key, built on the same `strip()` that feeds the search index and previews.
- Standing rules from phase 1 carry over: nothing is committed, pushed, deployed or published by the
  loop; the owner's uncommitted files stay untouched.
- Circles, discovery, ratings and extra sign-in methods are later loops (spec phases 3 to 6).
