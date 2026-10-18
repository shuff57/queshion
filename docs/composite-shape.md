# Composite shape spec

The composite-shape figure is one text param, `spec`. Each row is a piece or a mark. Rows are
separated by `;` or new lines. The canvas editor, the typed rows under **Advanced: spec**, the
**Start with** chips, presets, saved figures and share links all read and write this same text
(`src/figures/composite.rs`). A row the editor never touched is written back exactly as typed,
including rows it does not understand.

Coordinates are model units with y pointing up.

## Pieces

| Row | Shape |
| --- | --- |
| `rect X Y W H` | rectangle, bottom-left corner at (X, Y) |
| `tri X1 Y1 X2 Y2 X3 Y3` | triangle |
| `poly X Y X Y ...` | polygon (3 or more corners) |
| `circle CX CY R` | circle |
| `semi CX CY R u/d/l/r` | semicircle that bulges up, down, left or right |
| `quarter CX CY R ne/nw/se/sw` | quarter circle filling that quadrant |

A piece row can end with a shading word (`blue`, `red`, `green`, `yellow`) and with `| TEXT`,
which is written at the piece's centre, e.g. `rect 0 0 6 4 blue | A`.

## Marks

| Row | Mark |
| --- | --- |
| `dim X1 Y1 X2 Y2 [TEXT]` | dimension line; with no text it shows the measured length. The label goes on the right of the direction of travel. |
| `dash X1 Y1 X2 Y2` | dashed height or other line (the Lines tool) |
| `ticks X1 Y1 X2 Y2 N` | 1-3 congruence ticks across the middle of a side |
| `arrows X1 Y1 X2 Y2 N` | 1-3 parallel arrows |
| `text X Y WORDS` | free text |
| `ra X Y NE/NW/SE/SW` | right-angle square at a corner square to the grid |

## Label-mode rows

The Label tool writes these rows. A row is only written when something is set, so older specs
never contain them and still draw exactly as before.

| Row | Meaning |
| --- | --- |
| `side X1 Y1 X2 Y2 STYLE [THICKNESS]` | line of the outline side from (X1, Y1) to (X2, Y2). STYLE is `solid`, `dashed`, `dotted` or `hidden`. THICKNESS is `thin`, `regular` (the default, so it is left out) or `thick`. A plain solid regular side has no row. |
| `angle X Y [ext] [measure] [arcs N] [square] [SHADE] [\| TEXT]` | angle mark at the corner (X, Y). Without `ext` it marks the interior angle. With `ext` it marks the exterior angle, between one side carried on past the corner (drawn as a short dashed line) and the other side. `measure` labels the angle with its size, rounded by the `angle_dec` setting; `\| TEXT` gives a typed label instead. `arcs N` draws 1-3 congruence arcs, `square` draws a right-angle square, and a shading word fills the wedge. |
| `point X Y [NAME]` | point marker on a corner, with its name written outside the shape |

Words after the coordinates can come in any order and any case. Unknown words are ignored.
Angle and point rows only draw where the outline still has that corner, so changing the shape
never breaks the figure. Side, angle and point rows move with their piece and follow a dragged
corner, as dims and right angles do.

### Dragged labels

A `dim`, `angle`, `point` or `text` row may carry one `@DX,DY` word (anywhere after the keyword),
written when its label is dragged on the canvas: the label is drawn DX, DY model units from its
own place, along the screen axes (x right, y down) of the unturned figure. The offset grows with
the zoom and turns with the figure, so the label keeps its spot beside its side or corner. A row
without it draws its label where it always did. Dragging back to the start removes the word.

An interior right angle set from the angle popover becomes an `ra` row when the corner is square
to the grid, or `angle X Y square` otherwise.

Examples:

```
rect 0 0 6 4; tri 0 4 6 4 3 7; side 6 0 6 4 dashed thick; side 0 0 6 0 dotted thin
angle 0 4 arcs 2 red | x; angle 6 0 ext measure; point 0 0 A
```

## Figure settings (params)

| Param | Meaning |
| --- | --- |
| `unit` | unit added to measured dims |
| `dec` | round lengths to 0-3 decimal places |
| `angle_dec` | round angle measures to 0 (whole), 1 (tenths) or 2 (hundredths) places |
| `label_size` | Large / Medium / Small |
| `shade`, `fill_color` | shading of unshaded pieces |
| `rotate` | the figure's Turn, -180 to 180 degrees counter-clockwise, about the middle of the pieces. Labels stay upright. **Straighten** sets it back to 0. |

## Canvas

- **Start with**: an empty canvas shows the chips Rectangle, Triangle, Semicircle, Circle,
  L-shape, House and Arch.
- **Lines** (the Line tool): the figure is greyed and its snap targets are marked, with circles
  on corners and where pieces meet and diamonds on side midpoints.
- **Label**: shows the figure as it will be exported, turned and without the grid. Click a side
  to set its label, ticks, arrows, line and thickness. Click inside a corner for its angle, or
  just outside it for the exterior angle. Click the corner itself for its point. The Turn slider (drag, -180 to 180), its
  number box and **Straighten** sit at the bottom right.
- **Moving labels** (Select and Label tools): the pointer shows "Drag to move this label" over a
  dimension, angle, point or text label; dragging it writes its `@DX,DY` offset as one undo step.
  A click without a drag still selects the mark (Select) or opens its part's popover (Label).
- QA hook: `QUESHION_COMP="tool:label,side:N|angle:N|ext:N|corner:N"` opens a popover for outline
  side or corner N, for screenshots.
