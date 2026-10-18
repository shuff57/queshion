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
queshion://figure/<figure-id>?p=<payload>
```

- `<figure-id>` is the figure's stable slug, e.g. `motion-graphs` or `punnett-square`. These are
  the same ids that saved question figures (`FigureSpec.id`) and presets use.
- `<payload>` is base64url (RFC 4648 §5, `A–Z a–z 0–9 - _`, no `=` padding) of a compact JSON
  object holding **only the params that differ from the figure's opening state** (what it shows
  when you first open it from the browser). The reference generators work the same way: an
  unchanged figure gets a bare link, `queshion://figure/waves`, and one change stays short, e.g.
  `queshion://figure/motion-graphs?p=eyJsZXR0ZXJzIjp0cnVlfQ` for `{"letters":true}`.
- JSON values are the same types a saved spec uses: numbers for sliders/counts, `true`/`false`
  for switches, and strings for choices, text, lists, colours and dragged label offsets.

Implementation: `src/figures/share.rs` (encode/decode, base64url) and `Workspace::share_link` /
`Workspace::apply_link` in `src/figuresui.rs`.

## Reading a link

Pasted text is handled leniently. Whitespace and line breaks are dropped (mail clients wrap long
links), surrounding `<…>` or quotes are stripped, the `queshion://` prefix may be missing, and
other `&key=value` pairs are ignored.

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
- A link stores differences from the opening state at the time it was made. If a later version
  changes a figure's opening state, a link that left that param untouched opens with the new
  opening value. Every param the sender changed travels explicitly.

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
