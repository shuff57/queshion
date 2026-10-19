# web-port progress

- Phase 2.0: lib/bin split (src/lib.rs, src/app.rs, thin eframe adapter in main.rs); added assets/loading-logo.svg; rexie dep for Phase 3. 528+6 tests passed.
- Phase 2.2a: civil_from_days moved to src/time.rs (dcab082). 528+6 tests passed. NOTE: a second session (vershion-web-build) shares this tree and owns store/web.rs, web_main.rs, index.html, Trunk.toml, ai.rs transport; crates/ move held until it reports clear.
- 2026-10-18: hayro PDF page rendering committed on web-port (ac5c904). Native + wasm check clean; 541 unit + 6 integration tests pass; sample PDF matches pdftoppm. This tree is clean and has no uncommitted edits — free for the fast-forward and the crates/ move.
