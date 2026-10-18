# Figure share links

The Figures workspace can turn the figure on screen into a short link and open one back up.
It all happens offline. The link holds the figure's settings, so no server or account is needed.

## Using it

- **Copy link**: toolbar **Share link → Copy link** copies the link to the clipboard and shows
  "Link copied…". Send it any way you like: chat, mail, or a note in a question bank.
- **Open from link**: paste a link into the box under **Share link** (when a figure is open), or
  into **Open link…** on the Figures browser, then press **Open from link** or Enter. queShion
  switches to that figure and applies the settings. This counts as one Undo step.

## Format

```
queshion://figure/<figure-id>?p=<payload>&v=3
```

- `<figure-id>` is the figure's stable slug, e.g. `motion-graphs` or `punnett-square`. These are
  the same ids that saved question figures (`FigureSpec.id`) and presets use.
- `<payload>` is base64url (RFC 4648 §5, `A–Z a–z 0–9 - _`, no `=` padding) of a compact JSON
  object holding **only the params that differ from the figure's opening state** (what it shows
  when you first open it from the browser). The reference generators work the same way: an
  unchanged figure gets a bare link, `queshion://figure/waves?v=3`, and one change stays short, e.g.
  `queshion://figure/motion-graphs?p=eyJsZXR0ZXJzIjp0cnVlfQ&v=3` for `{"letters":true}`.
- `v` is the link format version (3 since P4, 2 since round 5). Links without it are version 1.
  The payload format is the same in every version; only the opening state it is laid over differs
  (see Compatibility).
- JSON values are the same types a saved spec uses: numbers for sliders/counts, `true`/`false`
  for switches, and strings for choices, text, lists, colours and dragged label offsets.

Implementation: `src/figures/share.rs` (encode/decode, base64url) and `Workspace::share_link` /
`Workspace::apply_link` in `src/figuresui.rs`.

## Reading a link

Pasted text is handled leniently. Whitespace and line breaks are dropped (mail clients wrap long
links), surrounding `<…>` or quotes are stripped, the `queshion://` prefix may be missing, and
other `&key=value` pairs are ignored. A link copied out of a web page or HTML mail, where `&`
appears as `&amp;` (or `&#38;`), reads the same, version included, and the scheme, `figure/` and
the figure id may be in any case (`Queshion://Figure/Motion-Graphs`); the payload stays
case-sensitive.

The changed params are laid over the figure's opening state, and the result goes through the
same path as a saved question figure (`params_from_spec`):

- unknown keys are ignored, and a value of the wrong type falls back to the default;
- old keys are migrated (`figures::migrate`), so links made before a figure's inputs changed
  still open;
- numbers are clamped into range, and unknown choice values fall back to the default.

A link to a figure id this build doesn't have, or a damaged payload, shows an error under the
box. Nothing else changes.

## Compatibility

- Saved question figures, presets (`figure-presets.json`) and the **Copy spec** / **Apply pasted
  spec** JSON are unchanged. Share links are an extra format, not a replacement.
- A link stores differences from the opening state at the time it was made. Every param the
  sender changed travels explicitly. When an opening state changes, the link version goes up and
  `figures::start::added_in` lists each key under the version that changed it. A link of an
  older version reads those keys with the opening value they had then (`start::older_values`:
  the plain default, or the earlier override), so it opens exactly as it did when made:
  - v2 (round 5): vector diagram resultant dashed. A v1 link keeps the solid resultant.
  - v3 (P4), the reference generators' first screens: orbital diagram without the configuration
    line (`config`), angles without point letters (`letters`), parallel lines with no angle
    marked (`marked`, `unknown`), volume by displacement with grey marbles and no arrow
    (`marble_color`, `arrow`, both new params), pH reading as the bench meter with its probe in a
    beaker (`meter`, new), kite mirrored so A is on the right (`flip`), and length reading with
    one magnifier above the object's right end (`mag_show` = "Ruler and one magnifier above";
    v1/v2 links keep "Ruler and magnifiers").
  The new params default to the old drawing, so saved figures, presets and examples render
  exactly as before.
- The side-panel cards (gel lanes, free-body forces and Motion, vectors, line-spectrum strips,
  particle kinds, population readouts and **Fit now**; `figures::cards`) edit the same params the
  generators always read, so links, saved figures and presets keep their meaning and did not
  change the link version. Older layouts move into the card form only when edited: a gel's classic
  `setup` is copied into its `lanes` text, and a line spectrum's per-element ticks and custom row
  are copied into its `strips` list. Until then they render exactly as before.

## QA hooks

- `QUESHION_FIG_LINK=<link>` opens a link at startup, as **Open from link** does.
- `QUESHION_FIG_OPENLINK=1` shows the browser's **Open link…** box.
- `QUESHION_FIG_EXAMPLES=1` (with `QUESHION_FIG=<id>`) opens the example gallery.

## Composite shape

The composite-shape figure opens on a blank canvas with its **Start with** chips, but its
opening state for links (and **Reset all**) is still the arch it always had. A link made before
the blank start, or one that never touched the pieces, opens the same arch it did then. A blank
canvas travels as `{"spec":""}`. The Label-mode rows (`side`, `angle`, `point`) live inside
`spec`, and the figure's Turn and angle rounding are the params `rotate` and `angle_dec`. Both
default to 0, so older links leave them out. See `docs/composite-shape.md`.
