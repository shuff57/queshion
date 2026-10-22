# Figures roadmap

What exists, what is being built, and the new subject tabs planned after it. Every figure follows
`docs/new-figure.md`: original design, text spec the figure agent can write, answers hideable with
`show_answers`, paired Student + key export, grey-only Print style, FAQ and examples.

## Built (63 + the 12 of this round)

Physics, Biology, Chemistry, Math (geometry, stats), Computing: logic circuit, graph, tree,
number bases, data structure, flowchart, truth table and Karnaugh map.

## This round (all 12 done)

1. Truth table and Karnaugh map
2. State machine (finite automaton)
3. Trace table
4. Sorting passes
5. Recursion tree
6. Network topology and ER diagrams
7. Probability tree
8. Venn diagram
9. Binomial and Poisson bars with a shaded region
10. Confidence interval on a number line
11. Regression residuals (pairs with the scatterplot)
12. RC charging curve and Kirchhoff two-loop circuit (Physics)

## New subject tabs

A subject is a tab in the Figures screen (`Subject::ALL` in `src/figures/mod.rs`). Each new one
needs the enum variant, its label, the gallery card order and a module under `src/figures/`.
Money figures share a small helper for currency formatting and column alignment.

### Accounting (tab added; shared money helpers in `src/figures/accounting.rs`)
- T-account: debits and credits per account, balanced off with totals and balance b/d (done)
- Journal entry: date, accounts, debit, credit, narration; balanced check as the answer (done)
- Trial balance: accounts listed, totals, the out-of-balance difference as the answer (done)
- Income statement and balance sheet: standard layouts from a list of accounts and amounts (done)
- Accounting equation: assets = liabilities + equity with transactions applied one by one (done)
- Depreciation schedule: straight-line, double-declining and sum of the years' digits, book value column (done)
- Bank reconciliation: two columns, adjustments, the reconciled balance (done)
- Cash-flow statement skeleton: operating, investing, financing sections (done)

### Economics
- Supply and demand: curves from intercept and slope, shifts, equilibrium and surplus shading (done)
- Elasticity: price change to quantity change, midpoint formula worked out (done)
- Production possibilities frontier: points inside, on and outside, opportunity cost (done)
- Cost curves: MC, ATC, AVC, with the profit-maximising quantity marked (done)
- Market structures: monopoly revenue and cost with the profit rectangle (done)
- Circular flow diagram: households, firms, markets, government, flows labelled or blank (done)
- Aggregate demand and supply: shifts and the new equilibrium (done)
- Lorenz curve and Gini coefficient from income shares (done)

### Earth science
- Rock cycle and water cycle: labelled arrows between stages, labels blankable (done)
- Plate boundaries: convergent, divergent, transform cross-sections (done)
- Layers of the Earth: concentric layers with thickness and state (done)
- Strata cross-section with fault and fold, for relative dating (done)
- Moon phases and seasons: orbit diagrams with the lit half shaded (done)
- Topographic contours from a small height grid (done)
- Hertzsprung-Russell diagram with stars plotted (done)

### Geography
- Climate graph: rainfall bars and temperature line on twin axes (done)
- Population pyramid from age-band counts (done)
- Grid references and compass bearings on a blank grid (done)
- Map scale bar and distance workings (done)

### Music
- Staff with clef, key signature and notes from a text spec (`C4 q, E4 q, G4 h`) (done)
- Circle of fifths with keys blankable (done)
- Scale and chord diagrams, guitar chord boxes, piano keyboard with keys marked (done)
- Rhythm grid and time-signature bar counting (done)

### Business and finance
- Break-even chart: fixed cost, total cost, revenue, break-even point (done)
- Time value of money timeline with cash flows and discount factors (done)
- Loan amortisation table: payment, interest, principal, balance (done)
- Org chart, SWOT grid, marketing funnel (done)

### Language and humanities
- Sentence diagram, plot arc, graphic organisers (T-chart, KWL, Venn comparison) (done)
- Timeline with dated events, cause-and-effect web, family tree (done)
- Government structure and trade-route charts (boxes and arrows, no maps) (done)

### Health, PE and technology
- Body systems with labelled blanks (simplified original outlines) (done)
- Food plate, heart-rate zones, court and field layouts with positions (done)
- Engineering: block diagram, free-body, gear train, orthographic views (done); truss not built

### Art, psychology and sociology
- Colour wheel and perspective grids (done)
- Maslow's hierarchy, brain regions (simplified), experimental design flow (done)

### Statistics extras (cheap: reuse the existing chart code)
- Sampling distribution, hypothesis-test rejection region, chi-square table

### Build order and limits
1. Earth science, Geography climate graphs, Business and finance, Music staff: the closest fit to
   what exists (diagrams, charts, tables from a text spec).
2. Language and humanities, Health and technology, Art and psychology: boxes-and-arrows layouts
   built on the graph and tree code.
3. Real maps and detailed anatomy are out of scope: the drawing is geometric, so those would need
   simplified original outlines, never traced or copied artwork.

Ask before starting any of these tabs; each needs a `Subject` variant, label and gallery order.

## Rules that keep the tabs consistent
- Numbers in a figure come from params and are rounded in one place, so a student copy and its
  key are the same size.
- Money uses a shared formatter (thousands separators, negatives in brackets by default).
- Every answer cell is hidden together with its heading intact under `show_answers`.
