# Fonts

Every font verSHion ships is in `assets/fonts/`, and every one is licensed under the SIL Open
Font License 1.1. The licence texts sit beside the fonts, and `assets/fonts/ATTRIBUTION.md`
says where each file came from. verSHion bundles nothing under an unknown or non-embeddable
licence, and nothing is downloaded from anywhere else at build or run time.

**Web build.** The desktop app compiles every face in. The web build compiles in only the four
Source Serif 4 faces (equaSHion compiles the same files in, so they cost nothing) and VS Sans
Fallback, a Latin cut of Source Sans 3 Regular renamed as the OFL requires
(`tools/make_sans_fallback.py`), used only if the real UI face cannot be fetched. Trunk copies
`assets/fonts/` (licences included) to `fonts/` next to index.html, and the page fetches the
other faces from there before the app starts. See `src/fonts.rs` (`Face`, marked `embed` or
`fetch`) and docs/progress.md, "web fonts".

| Family | Faces | Licence | Used for |
|---|---|---|---|
| Source Serif 4 (Adobe, 4.005) | Regular, Bold, It, BoldIt (compiled into the binary by `src/fonts.rs`); Semibold (read at startup) | OFL 1.1, `assets/fonts/OFL-SourceSerif4.txt` | Printed exams: PDF body and headings, the on-page exam preview, card and export thumbnails, and headings across the app |
| Source Sans 3 (Adobe) | Regular, It, Semibold, Bold (read at startup) | OFL 1.1, `assets/fonts/OFL-SourceSans3.txt` | App interface text |
| Liberation Mono (Red Hat / Google, 2.1.5) | Regular (compiled in by `src/fonts.rs`, `fonts::Style::Mono`) | OFL 1.1 (Reserved Font Name "Liberation"), `assets/fonts/OFL-LiberationMono.txt` | `` `code` `` spans in PDFs (font F10, embedded as a subset only when a document has code) |
| STIX Two Math (The STIX Fonts Project, 2.x) | Regular (compiled in by the `equashion-font` crate; verSHion reuses those bytes, `fonts::fallback_bytes`) | OFL 1.1, `assets/fonts/OFL-STIXTwoMath.txt` | Equations (equaSHion), and in PDFs the fallback for symbols Source Serif 4 lacks (`⇒ ⇔ ↔ ∈ ∉ ⊂ ∪ ∩ ∠ ⊥ ∥ ≡ ∝ ∴ ⋅` ...), embedded as a Type 3 font of only the glyphs drawn |
| egui defaults (Ubuntu-Light, Hack, Noto Emoji, emoji-icon-font) | as shipped in egui (`default_fonts` feature) | Ubuntu Font Licence 1.0, Hack (MIT / Bitstream Vera), OFL; see the `epaint_default_fonts` crate's `fonts/` folder; emoji-icon-font is MIT | Fallback glyphs behind the house faces on screen only; never embedded in exports |

Equations on screen come from equaSHion's own fonts (STIX Two Math and others, all OFL; see
`../equaSHion/assets/fonts`). verSHion does not change them. In exports, `$math$` still
degrades to Unicode text (`equation::stem_to_plain`), which now prints in full (see below).

## Print serif: one set of faces everywhere

Test Parrot prints exams in a serif. verSHion uses **Source Serif 4** with four real faces,
so italic text uses the italic design (single-storey *a*, its own advance widths) and is not
the upright face slanted.

- **PDF.** F1..F4 are Regular, Bold, Italic and Bold Italic. Each is embedded as a TrueType
  subset (`FontFile2` with FlateDecode; only the glyphs the document draws keep their
  outlines). The text uses WinAnsiEncoding. Line breaking measures text with
  `fonts::metrics(..).widths`, the advance widths read from each font's own `hmtx` through
  its `cmap`, and the PDF writes those same numbers into `/Widths`, so what is measured is
  exactly what is drawn. `pdffonts exam.pdf` lists four `SourceSerif4-*` fonts, each marked
  `emb yes sub yes`. The unit tests in `src/export.rs` and `src/fonts.rs` check that the
  widths match, that the font programs embed and parse, and that the subsets keep the glyphs
  the document uses.
- **Characters beyond WinAnsi (P3).** Source Serif 4 has about 1 200 characters: Greek,
  Cyrillic, `π − √ ≤ ≥ ≈ ≠ ∞ ∑ ∫ ∂ ∆`, arrows `← ↑ → ↓`, super/subscript digits and more.
  Text that WinAnsi can encode still goes into F1..F4 exactly as before (a document without
  other characters is byte-identical to the P2 output). Any other character the face has is
  drawn through that face's **Type0 / CIDFontType2 font with Identity-H encoding** (F5..F8,
  one per face, only when used): the string holds glyph ids, `/W` holds the same advances the
  line breaker measured with, `/CIDToGIDMap /Identity`, and a **ToUnicode CMap** so the text
  copies and searches correctly. The CID font shares its face's font descriptor and subset
  program, so each face is embedded once. A character Source Serif 4 lacks falls back to
  **STIX Two Math** (F9): a Type 3 font whose glyph procedures are the STIX outlines (cubic
  paths, 1/1000 em), with its own ToUnicode CMap, holding only the glyphs drawn (at most 256
  per document). The Type 3 font carries one extra empty glyph named `m`, 0.6 em wide, because
  poppler sizes Type 3 text from the width of its `m`; without it `pdftotext` moved the
  symbols off their line. A character neither face has prints as `?`, as before.
  `pdffonts` then also lists `SourceSerif4-* CID TrueType Identity-H emb yes sub yes uni yes`
  and `F9 Type 3 emb yes uni yes`. Tests: `export::unicode_beyond_winansi_prints_in_cid_and_fallback_fonts`
  (every symbol drawn, none as `?`, which font draws what, `/W` equals the measured advances,
  the subset keeps the outlines), `export::pdftotext_round_trips_unicode` (runs poppler's
  `pdftotext` when it is installed and finds every symbol, in reading order) and
  `export::winansi_only_documents_are_unchanged_by_the_cid_fonts`. The export preview and the
  card thumbnails decode the `<...>` strings back to characters (`examui::shown_text`).
- **DOCX.** Word documents cannot reasonably carry fonts, so the DOCX names
  **Times New Roman**, which every Word install has, for body and headings in every paper
  style. `word/fontTable.xml` gives "Liberation Serif" as its alternate name (it is
  metric-compatible, and LibreOffice substitutes it) and the `roman` family, so a machine
  without Times falls back to another serif rather than a sans. Bold and italic are real
  `w:b` / `w:i` run properties, so Word and LibreOffice pick the real bold and italic faces.
- **On-page preview and editor.** egui has no style linking, so `theme.rs` registers
  `serif-italic`, `serif-bold-italic` and `sans-italic` families. `theme::italic_family`
  maps a family to its italic, and the stem renderer (`equation::word_galley`) and the
  WYSIWYG stem editor use that italic family instead of egui's synthetic slant. The slant
  is only a fallback for when the house fonts are not loaded. The serif faces come from the
  same compiled-in bytes as the PDF. A test checks that egui's advances for each face equal
  the PDF's width tables. egui also kerns and forms ligatures, so a long preview line can be
  a hair narrower than the PDF.
- **Thumbnails.** Exam cards and the export preview rasterise the PDF's page operators as
  SVG with resvg. The four faces are loaded into resvg's font database, and the SVG asks
  for `'Source Serif 4'`. The family name must be quoted, because an unquoted name ending
  in a digit is invalid CSS and makes resvg draw no text; a test guards this.

## Mono

Round 7 added `` `code` `` spans (`docs/stem-format.md`).

- **PDF.** Code prints in **Liberation Mono Regular 2.1.5** (SIL OFL 1.1, Reserved Font Name
  "Liberation"; copied unmodified from Fedora's `liberation-mono-fonts` package, licence text
  in `assets/fonts/OFL-LiberationMono.txt`). It is metric-compatible with Courier New, the face
  the DOCX names, so a code span measures the same in both.
  - Font F10 is a TrueType subset with WinAnsiEncoding, FixedPitch flag and `/Widths` from the
    font's own advances, like F1..F4. It is written only when the document draws code, so
    other documents are byte-identical to the round-6 output.
  - Code prints at 0.92 of the surrounding size (the on-screen preview's ratio), on a light
    gray tint (`0.92 g`). The width the line breaker measures includes that scale.
  - There is one mono face. Bold or italic code prints in the regular mono, as on screen.
  - A code character outside WinAnsi prints through the regular serif's CID font F5, or the
    STIX fallback F9, so the export preview can still read it back.
  - Tests: `fonts::mono_face_covers_winansi_with_one_advance`,
    `export::r7_markup_tests::code_prints_in_the_embedded_mono` and
    `documents_without_code_embed_no_mono_and_keep_the_font_table`.
- **DOCX.** Code runs set `w:rFonts` to **Courier New**, which every Word install has, at the
  same 0.92 size, shaded `EBEBEB`. Only when a document has code, `word/fontTable.xml` lists
  Courier New, with Liberation Mono as its alternate, the modern family and fixed pitch, so
  LibreOffice substitutes the same mono the PDF uses.
- **On screen.** The editor and preview draw code in egui's monospace family (Hack, from
  egui's default fonts).

## Adding a font

Only add OFL, Apache-2.0 or public-domain fonts that allow embedding. Put the font file and
its licence text in `assets/fonts/`, add a line to `ATTRIBUTION.md`, and record the font in
the table above.
