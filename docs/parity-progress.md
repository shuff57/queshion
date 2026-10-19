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
- Step 2b: Aiken and Blackboard exports. `bankexport::bank_aiken` (single-answer MC only) and
  `bankexport::bank_blackboard` (Blackboard Learn 7.3+ tab-delimited: MC, TF, FIB, ESS, with
  aligned incorrect-answer columns, flattened tabs/newlines and a skipped count). Two buttons
  in the export dialog save `.aiken.txt` / `.blackboard.txt` and report skipped questions in
  the toast; a muted line says what each format holds. Four new tests. Native check 0 warnings;
  wasm unchanged (same 4 pre-existing warnings). `cargo test --workspace`: 535 passed,
  6 integration passed, 0 failed, 3 ignored.
- Step 3: inline equation panel. `EqReq` now carries the anchor (the field frame or preview chip
  that asked) and the opening frame; `equation_panel` replaces the centered `Modal` with an
  anchored foreground `egui::Area` clamped by the pure `panel_pos` (flips above/left near the
  edges). Outside-click cancels except on the opening click; Enter/Esc/buttons unchanged. Four
  `panel_pos` tests. Native check 0 warnings; wasm unchanged. `cargo test --workspace`:
  539 passed, 6 integration passed, 0 failed, 3 ignored.
- Step 4: keyboard shortcuts. `app::shortcut_action(screen, has_draft)` resolves Ctrl/Cmd+N to
  new exam on Home/Exams or new bank on Banks, handled in `App::handle_shortcuts` before the
  screen draws. The bank question editor now cancels on Esc (when no text field has focus) and
  saves a valid draft on Ctrl/Cmd+Enter; both are suppressed while the equation panel is open.
  The "How it works" sheet lists the shortcuts. Two matrix tests. Native check 0 warnings; wasm
  unchanged. `cargo test --workspace`: 541 passed, 6 integration passed, 0 failed, 3 ignored.
