# Adding a figure

A figure is one `FigureDef` (`src/figures/mod.rs`): a stable `id`, a `subject`, a title and blurb,
a `params` function (the controls) and a pure `render` (params to one standalone SVG string).
The Figures tab builds its controls, preview, presets, undo, share links and exports from that;
a plain figure needs no UI code.

## Steps

1. **Write the module.** `src/figures/<name>.rs` with `pub fn defs() -> Vec<FigureDef>`. Draw
   with `figures::svg::Doc` (`rect`, `line`, `text`, `arrow` ...). Keep `render` pure and
   deterministic: same params, same SVG. Fixed viewBox, white background (`Doc::finish`).
2. **Register it.** In `src/figures/mod.rs`: `pub mod <name>;` and `v.extend(<name>::defs());`
   in `registry()`. If the module has examples, add it to the `examples_for` chain.
3. **Add its FAQ.** `src/figures/faq.rs`, a `"<id>" => ...` arm and a short list of questions.
   The test `every_figure_panel_has_a_faq` fails without one.
4. **Optional:** an opening state in `start.rs::overrides` (kept apart from the `ParamDef`
   defaults on purpose, see its header), classic setups/examples, drag handles (`interact.rs`).
5. **Run the tests.** `cargo test --lib figures` and `cargo test --lib figuresui`. They render
   every registered figure with its defaults, every choice, every switch and a slider nudge, and
   open each in the workspace.

## Subjects

`Subject` (`Physics`, `Biology`, `Chemistry`, `Math`, `Computing`) picks the tab. The first four
wear a page skin matched to a reference site (`figuresui/chem_shell.rs`, gated by `skinned()`).
`Computing` has none, so its figures use the generic workspace UI. A subject with no figures
shows no tab. Figures can also be listed under another subject (`also_listed` in `mod.rs`).

## Shared pieces

Use these instead of inventing per-figure versions.

| Need | Use |
| --- | --- |
| Hide answers for a student copy | `figures::answers_param()` in `params`; draw the answer parts only while `p.b(figures::SHOW_ANSWERS)`. The switch is the preview toggle, and the **Student + key** button exports `name-student` and `name-key` side by side (`figures::answer_pair`). |
| Colour or black-and-white print | `figures::svg::style_param()` in `params`; `let fills = svg::Fills::new(p);` then `doc.defs(&fills.defs())` once, `fills.series(i)` for fills (a colour, or a hatch pattern in Print), `fills.line(i)` and `fills.dash(i)` for line series. |
| Numbers typed, pasted or generated | `ParamDef::data(key, label, columns, default)` (`Kind::Data`): 1 column is `2, 4, 5`, 2 columns is one `x, y` row per line. Read it with `figures::data::numbers(text)` / `pairs(text)`. The control takes a spreadsheet paste (tidied when the box loses focus), opens a CSV, and generates sample data from a seed. At most `data::MAX_ROWS` rows are used. |
| Label size toggle | `svg::label_size_param()` and `svg::with_label_size`. |

## Rules

- Params are stored as strings/numbers/bools and clamped by `Params::normalize`. Never panic on
  an odd value: an empty or unparsable `Kind::Data` must still render (an empty figure).
- Print style must stay grey-only: no colour fill or stroke outside `Fills`.
- A figure that computes a value which is an answer (a total, a mean) draws it only under
  `SHOW_ANSWERS`.
- New `Kind` variants touch every exhaustive `match` on `Kind` (compiler-guided) and the
  share-core mapping in `tests/share_core_fixtures.rs`, which sends `Data` as multi-line text.
  The app's real mapping, when it exists, must do the same.

## Test-only demos

`figuresui.rs::demo_defs` registers `demo-shapes` and `demo-data` under `cfg(test)`. `demo-data`
is a Computing figure that uses `Kind::Data`, `style` and `show_answers`; copy it for a minimal
example of all three.
