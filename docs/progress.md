# web-port progress

- Phase 2.0: lib/bin split (src/lib.rs, src/app.rs, thin eframe adapter in main.rs); added assets/loading-logo.svg; rexie dep for Phase 3. 528+6 tests passed.
- Phase 2.2a: civil_from_days moved to src/time.rs (dcab082). 528+6 tests passed. NOTE: a second session (vershion-web-build) shares this tree and owns store/web.rs, web_main.rs, index.html, Trunk.toml, ai.rs transport; crates/ move held until it reports clear.
- Phase 2.2b: heading_scale moved equation.rs -> richtext.rs. 528+6 tests passed. Loop now works in worktree verSHion-loop on branch web-port-loop (other session shares verSHion/web-port).
