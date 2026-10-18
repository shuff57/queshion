# parity-leftovers progress

Baseline on `parity-loop` (from `web-port` b89fea2): 528 unit + 6 integration tests, 3 ignored;
`cargo check --workspace` reported 9 dead-code warnings.

- Step 1: zero compiler warnings. Removed dead wrappers/fields in `bankexport.rs`, `figuresui.rs`,
  `bankui.rs`, `examui.rs`, `figures/biology/{pedigree,plot,punnett}.rs` and
  `figuresui/composite_canvas.rs`; test-only helpers (`popover_copy_text`, `SnapDoc::from_parts`,
  `Canvas::label_centres`) are now `#[cfg(test)]`. `cargo check --workspace`: 0 warnings.
  `cargo test --workspace`: 528 passed, 6 integration passed, 0 failed, 3 ignored.
- Step 2a: CSV bank export. `bankexport::bank_csv` (RFC 4180, CRLF; columns `#`, `type`,
  `points`, `question`, `answer`, `choices`, `explanation`, `topics`, `difficulty`) plus a
  `Save CSV file…` button in the export dialog using `platform::save_path`/`write` (native and
  wasm). Three new tests (rows/answers, quoting, empty bank). Native check 0 warnings; wasm
  check passes with 4 pre-existing wasm-only warnings (`ai.rs`, `importui.rs`, `screens.rs`).
  `cargo test --workspace`: 531 passed, 6 integration passed, 0 failed, 3 ignored.
