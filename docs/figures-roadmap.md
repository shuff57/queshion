# Figures roadmap

What exists, what is being built, and the new subject tabs planned after it. Every figure follows
`docs/new-figure.md`: original design, text spec the figure agent can write, answers hideable with
`show_answers`, paired Student + key export, grey-only Print style, FAQ and examples.

## Built (63 + this round)

Physics, Biology, Chemistry, Math (geometry, stats), Computing: logic circuit, graph, tree,
number bases, data structure, flowchart, truth table and Karnaugh map.

## This round (in order)

1. Truth table and Karnaugh map (done)
2. State machine (finite automaton): states, labelled arrows, trace an input string
3. Trace table: variables across, one row per step, blank cells
4. Sorting passes: array after each pass of bubble, selection, insertion
5. Recursion tree: call tree for fib(n), factorial and similar, return values as answers
6. Network and ER diagrams
7. Probability tree: branches, probabilities, product per path
8. Venn diagram: two or three sets with counts
9. Binomial / discrete distribution bars with a shaded region
10. Confidence interval on a number line
11. Regression residuals (pairs with the scatterplot)
12. Circuits: capacitor and RC curve, Kirchhoff loops

## New subject tabs

A subject is a tab in the Figures screen (`Subject::ALL` in `src/figures/mod.rs`). Each new one
needs the enum variant, its label, the gallery card order and a module under `src/figures/`.
Money figures share a small helper for currency formatting and column alignment.

### Accounting
- T-account: debits and credits per account, running balance, blank cells for students
- Journal entry: date, accounts, debit, credit, narration; balanced check as the answer
- Trial balance: accounts listed, totals, the out-of-balance difference as the answer
- Income statement and balance sheet: standard layouts from a list of accounts and amounts
- Accounting equation: assets = liabilities + equity with transactions applied one by one
- Depreciation schedule: straight-line and declining balance, book value column
- Bank reconciliation: two columns, adjustments, the reconciled balance
- Cash-flow statement skeleton: operating, investing, financing sections

### Economics
- Supply and demand: curves from intercept and slope, shifts, equilibrium and surplus shading
- Elasticity: price change to quantity change, midpoint formula worked out
- Production possibilities frontier: points inside, on and outside, opportunity cost
- Cost curves: MC, ATC, AVC, with the profit-maximising quantity marked
- Market structures: monopoly revenue and cost with the profit rectangle
- Circular flow diagram: households, firms, markets, government, flows labelled or blank
- Aggregate demand and supply: shifts and the new equilibrium
- Lorenz curve and Gini coefficient from income shares

### Later candidates (ask before starting)
- Business and finance: break-even chart, time value of money timeline, loan amortisation
- Statistics extras: sampling distribution, hypothesis test region, chi-square table
- Earth science, geography, music (staff and chords), language (sentence diagram)

## Rules that keep the tabs consistent
- Numbers in a figure come from params and are rounded in one place, so a student copy and its
  key are the same size.
- Money uses a shared formatter (thousands separators, negatives in brackets by default).
- Every answer cell is hidden together with its heading intact under `show_answers`.
