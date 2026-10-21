//! Business and finance figures (the Business and finance tab): a break-even chart, the time
//! value of money, a loan amortisation table, an organisation chart, a SWOT grid and a marketing
//! funnel. Original drawings on the shared pieces (`chart`, `svg::Doc`, `Fills`, `show_answers`,
//! print rules, and the money formatter from Accounting). What is given (the costs, the rate, the
//! structure, the counts) is the question; the break-even point, present values, repayments,
//! classifications and conversion rates are the answers, so a student copy (Show answers off)
//! leaves them out and is the same size as its key.
use super::{Example, FigureDef, Params};

mod breakeven;
mod funnel;
mod loan;
mod orgchart;
mod swot;
mod tvm;

pub fn defs() -> Vec<FigureDef> {
    vec![breakeven::def(), tvm::def(), loan::def(), orgchart::def(), swot::def(), funnel::def()]
}

/// Params that only matter in some states of a figure.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    orgchart::relevant(fig, key, p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    breakeven::examples(id)
        .or_else(|| tvm::examples(id))
        .or_else(|| loan::examples(id))
        .or_else(|| orgchart::examples(id))
        .or_else(|| swot::examples(id))
        .or_else(|| funnel::examples(id))
}
