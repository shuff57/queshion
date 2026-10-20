//! Population growth and predator-prey generators.
use super::plot::*;
use super::*;

const NPTS: usize = 401;

/// Axes of the most recent render, so interaction handles use the exact same geometry.
#[derive(Clone, Copy, Debug)]
pub struct AxInfo {
    /// 0 size-over-time, 1 rate-over-time, 2 per-capita-over-time, 3 rate-vs-N, 4 per-capita-vs-N; 9 = pp time; 10 = pp phase
    pub kind: u8,
    pub x: f64, pub y: f64, pub w: f64, pub h: f64,
    pub x0: f64, pub x1: f64, pub y0: f64, pub y1: f64,
    /// Gridline steps (count by) of the two axes.
    pub xs: f64, pub ys: f64,
}

impl AxInfo {
    pub fn px(&self, v: f64) -> f64 { self.x + (v - self.x0) / (self.x1 - self.x0) * self.w }
    pub fn py(&self, v: f64) -> f64 { self.y + self.h - (v - self.y0) / (self.y1 - self.y0) * self.h }
    pub fn vx(&self, px: f64) -> f64 { self.x0 + (px - self.x) / self.w * (self.x1 - self.x0) }
    pub fn vy(&self, py: f64) -> f64 { self.y0 + (self.y + self.h - py) / self.h * (self.y1 - self.y0) }
    pub fn inside(&self, v: (f64, f64)) -> bool {
        v.0 >= self.x0 && v.0 <= self.x1 && v.1 >= self.y0 && v.1 <= self.y1
    }
}

thread_local! {
    static LAST: std::cell::RefCell<Vec<AxInfo>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Renders `p` (growth or predator-prey) and returns the plot areas it drew.
pub fn axes_of(p: &Params, pp: bool) -> Vec<AxInfo> {
    LAST.with(|l| l.borrow_mut().clear());
    if pp { let _ = pp_render(p); } else { let _ = growth_render(p); }
    LAST.with(|l| l.borrow().clone())
}

fn stash(kind: u8, a: &Axes) {
    LAST.with(|l| l.borrow_mut().push(AxInfo { kind, x: a.x, y: a.y, w: a.w, h: a.h, x0: a.x0, x1: a.x1, y0: a.y0, y1: a.y1, xs: a.xs, ys: a.ys }));
}

/// Pure model value used by the handles (closed forms only).
pub fn growth_value(mode: &str, n0: f64, r: f64, k: f64, t: f64) -> f64 {
    match mode {
        "Exponential (J-curve)" => n0 * (r * t).exp(),
        _ => logistic(n0, r, k, t),
    }
}

/// Inverse of `growth_value` in r: growth rate that puts the curve through (t, n).
pub fn growth_r_for(mode: &str, n0: f64, k: f64, t: f64, n: f64) -> f64 {
    let t = t.max(1e-6);
    if mode == "Exponential (J-curve)" {
        return (n.max(1e-9) / n0).ln() / t;
    }
    let n = n.clamp(n0 * 1.0001, k * 0.9999);
    let arg = (k / n - 1.0) * n0 / (k - n0).max(1e-9);
    -arg.max(1e-12).ln() / t
}

fn color_hex(name: &str) -> &'static str {
    match name {
        "Red" => "#dc2626",
        "Green" => "#15803d",
        "Black" => "#111111",
        "Orange" => "#ea580c",
        "Purple" => "#7c3aed",
        _ => "#2563eb",
    }
}

/// Width of `s` in bold Arial/Helvetica at `size` px (the reference measures labels the same way).
fn bold_width(s: &str, size: f64) -> f64 {
    const W: [u16; 95] = [
        278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611,
        975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556,
        333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
    ];
    s.chars().map(|c| if (' '..='~').contains(&c) { W[c as usize - 32] as f64 } else { 556.0 }).sum::<f64>() * size / 1000.0
}

fn minor_of(s: &str) -> i64 {
    match s { "2 per block" => 2, "4 per block" => 4, "5 per block" => 5, _ => 1 }
}

// ------------------------------------------------------------- population growth

pub fn growth_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("growth", "Growth", &["Logistic (S-curve)", "Exponential (J-curve)", "Exponential and logistic", "Overshoot and oscillation", "Boom and crash"], "Logistic (S-curve)"),
        ParamDef::float("n0", "Starting size, N₀", 1.0, 1_000_000.0, 1.0, 20.0),
        ParamDef::float("r", "Growth rate, r (per time unit)", 0.01, 5.0, 0.05, 0.5),
        ParamDef::float("k", "Carrying capacity, K", 10.0, 1_000_000.0, 10.0, 1000.0),
        ParamDef::float("tspan", "Time span", 1.0, 1000.0, 1.0, 20.0),
        ParamDef::choice("unit", "Time in", &["Hours", "Days", "Weeks", "Months", "Years", "Generations"], "Years"),
        ParamDef::text("organism", "Organism plural, for the axis titles", ""),
        ParamDef::choice("graph", "Show", &[
            "Population size over time", "Growth rate over time", "Per capita growth rate over time",
            "Population size and growth rate", "Population size and per capita rate", "Growth rate and per capita rate",
            "Growth rate against population size", "Per capita rate against population size", "Both rates against population size",
        ], "Population size over time"),
        ParamDef::choice("curve_color", "Curve color", &["Blue", "Red", "Green", "Black", "Orange", "Purple"], "Blue"),
        ParamDef::list("census", "Census data points (time:size)", ""),
        ParamDef::bool("mark_k", "Carrying capacity, K dashed line", true),
        ParamDef::bool("mark_infl", "Inflection point where growth is fastest", false),
        ParamDef::text("x_title", "x-axis title (blank = automatic)", ""),
        ParamDef::text("y_title", "y-axis title (blank = automatic)", ""),
        ParamDef::bool("fit", "Fit the axes as these change", true),
        ParamDef::float("x_max", "To", 1.0, 100_000.0, 1.0, 20.0),
        ParamDef::float("y_max", "To", 0.001, 10_000_000.0, 10.0, 1100.0),
        ParamDef::choice("minor", "Minor gridlines", &["None", "2 per block", "4 per block", "5 per block"], "None"),
        ParamDef::bool("blank", "Blank graph (axes only, for students to draw on)", false),
        ParamDef::text("title", "Chart title (blank = none)", ""),
        ParamDef::float("curve_width", "Curve thickness", 1.0, 7.0, 0.5, 3.0),
        ParamDef::text("k_label", "Label on the carrying-capacity line", "Carrying capacity (K)"),
        // ---- added for parity with the reference controls
        ParamDef::float("tau", "Overshoot: lag, tau (0 = automatic, r*tau = 1.3)", 0.0, 100.0, 0.1, 0.0),
        ParamDef::bool("draw_curve", "Draw the curve", true),
        ParamDef::bool("census_auto", "Show census points on the population graph", false),
        ParamDef::float("census_every", "Count every", 0.1, 100.0, 0.1, 1.0),
        ParamDef::choice("census_scatter", "Scatter", &["None, on the curve", "About 5%", "About 10%", "About 20%"], "None, on the curve"),
        ParamDef::int("census_seed", "Census: new counts (seed)", 1, 9999, 1),
        ParamDef::bool("census_table", "List them in a table beside the graph", false),
        ParamDef::choice("k_label_style", "Carrying capacity label", &["Text", "Blank line", "None"], "Text"),
        ParamDef::bool("mark_phases", "Growth phases above the graph", false),
        ParamDef::choice("title_style", "Chart title", &["Text", "Blank line", "None"], "Text"),
        ParamDef::choice("xtitle_style", "Time axis", &["Text", "Blank line", "None"], "Text"),
        ParamDef::choice("ytitle_style", "Population size axis", &["Text", "Blank line", "None"], "Text"),
        ParamDef::text("rate_title", "Growth rate axis title (blank = automatic)", ""),
        ParamDef::text("pc_title", "Per capita rate axis title (blank = automatic)", ""),
        ParamDef::float("x_min", "From", -100_000.0, 100_000.0, 1.0, 0.0),
        ParamDef::float("x_step", "Count by", 0.0, 100_000.0, 0.1, 0.0),
        ParamDef::choice("x_numbers", "Numbers", &["Automatic", "Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"], "Automatic"),
        ParamDef::choice("x_end", "Label at the right end", &["None", "Text"], "None"),
        ParamDef::text("x_end_text", "x-axis end label text", "t"),
        ParamDef::choice("x_lo_end", "Left end", &["None", "Triangle arrow", "Line arrow", "Circle"], "None"),
        ParamDef::choice("x_hi_end", "Right end", &["None", "Triangle arrow", "Line arrow", "Circle"], "None"),
        ParamDef::float("y_min", "From", -10_000_000.0, 10_000_000.0, 1.0, 0.0),
        ParamDef::float("y_step", "Count by", 0.0, 10_000_000.0, 0.1, 0.0),
        ParamDef::choice("y_numbers", "Numbers", &["Automatic", "Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"], "Automatic"),
        ParamDef::choice("y_end", "Label at the top end", &["None", "Text"], "None"),
        ParamDef::text("y_end_text", "y-axis end label text", "N"),
        ParamDef::choice("y_lo_end", "Bottom end", &["None", "Triangle arrow", "Line arrow", "Circle"], "None"),
        ParamDef::choice("y_hi_end", "Top end", &["None", "Triangle arrow", "Line arrow", "Circle"], "None"),
        ParamDef::float("y2_max", "Second graph y-axis maximum (when not fitting, 0 = fit)", 0.0, 10_000_000.0, 0.1, 0.0),
        ParamDef::float("y2_step", "Second graph y-axis count by (0 = automatic)", 0.0, 10_000_000.0, 0.01, 0.0),
    ]
}

struct Series {
    name: String,
    ys: Vec<f64>,
    color: String,
    dash: &'static str,
    fit: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Panel { TimeN, TimeRate, TimePer, NRate, NPer }

fn panels_of(g: &str) -> Vec<Panel> {
    use Panel::*;
    match g {
        "Growth rate over time" => vec![TimeRate],
        "Per capita growth rate over time" => vec![TimePer],
        "Population size and growth rate" => vec![TimeN, TimeRate],
        "Population size and per capita rate" => vec![TimeN, TimePer],
        "Growth rate and per capita rate" => vec![TimeRate, TimePer],
        "Growth rate against population size" => vec![NRate],
        "Per capita rate against population size" => vec![NPer],
        "Both rates against population size" => vec![NRate, NPer],
        _ => vec![TimeN],
    }
}

fn logistic(n0: f64, r: f64, k: f64, t: f64) -> f64 { k / (1.0 + (k - n0) / n0 * (-r * t).exp()) }

fn model(mode: &str, n0: f64, r: f64, k: f64, tspan: f64, col: &str, tau_in: f64) -> Vec<Series> {
    let ts: Vec<f64> = (0..NPTS).map(|i| tspan * i as f64 / (NPTS - 1) as f64).collect();
    let mk = |name: &str, ys: Vec<f64>, color: &str, dash: &'static str, fit: bool| Series { name: name.into(), ys, color: color.into(), dash, fit };
    match mode {
        "Exponential (J-curve)" => vec![mk("Exponential", ts.iter().map(|t| n0 * (r * t).exp()).collect(), col, "", true)],
        "Exponential and logistic" => {
            let other = if col == "#dc2626" { "#2563eb" } else { "#dc2626" };
            vec![
                mk("Exponential", ts.iter().map(|t| n0 * (r * t).exp()).collect(), other, "8 5", false),
                mk("Logistic", ts.iter().map(|t| logistic(n0, r, k, *t)).collect(), col, "", true),
            ]
        }
        "Overshoot and oscillation" => {
            // Delayed logistic: dN/dt = r N (1 - N(t - tau)/K), with r*tau = 1.3 (damped oscillation about K).
            let tau = if tau_in > 0.0 { tau_in } else { 1.3 / r };
            let sub = 20usize;
            let dt = tspan / ((NPTS - 1) * sub) as f64;
            let lag = (tau / dt).round().max(1.0) as usize;
            let mut hist = vec![n0];
            for i in 0..(NPTS - 1) * sub {
                let n = hist[i];
                let nd = if i >= lag { hist[i - lag] } else { n0 };
                hist.push((n + dt * r * n * (1.0 - nd / k)).max(0.0));
            }
            vec![mk("Population", (0..NPTS).map(|i| hist[i * sub]).collect(), col, "", true)]
        }
        "Boom and crash" => {
            // Exponential boom to K, then a crash as resources collapse.
            let peak = if n0 < k { ((k / n0).ln() / r).clamp(0.05, tspan * 0.9) } else { 0.0 };
            let top = n0 * (r * peak).exp();
            let ys = ts.iter().map(|t| if *t <= peak { n0 * (r * t).exp() } else { top * (-1.6 * r * (t - peak)).exp() }).collect();
            vec![mk("Population", ys, col, "", true)]
        }
        _ => vec![mk("Logistic", ts.iter().map(|t| logistic(n0, r, k, *t)).collect(), col, "", true)],
    }
}

fn derivative(ts: &[f64], ys: &[f64]) -> Vec<f64> {
    let n = ys.len();
    (0..n).map(|i| {
        let (a, b) = (i.saturating_sub(1), (i + 1).min(n - 1));
        (ys[b] - ys[a]) / (ts[b] - ts[a])
    }).collect()
}

fn parse_points(s: &str) -> Vec<(f64, f64)> {
    s.split([',', ';', '\n'])
        .filter_map(|p| {
            let (a, b) = p.split_once(':').or_else(|| p.split_once(' '))?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .collect()
}

/// Census counts generated from the model: (t, N) every `every` time units, optionally scattered.
fn auto_census(ts: &[f64], ys: &[f64], tspan: f64, every: f64, scatter: f64, seed: i64) -> Vec<(f64, f64)> {
    let mut rng = Rng::new(seed.max(1) as u64);
    let every = every.max(tspan / 400.0).max(1e-6);
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        let t = i as f64 * every;
        if t > tspan + 1e-9 || out.len() > 400 { break; }
        let f = (t / tspan * (ts.len() - 1) as f64).clamp(0.0, (ts.len() - 1) as f64);
        let (j, fr) = (f.floor() as usize, f - f.floor());
        let v = if j + 1 < ys.len() { ys[j] * (1.0 - fr) + ys[j + 1] * fr } else { ys[ys.len() - 1] };
        // Box-Muller normal, always advance the generator so counts are stable per index
        let (u1, u2) = (rng.f().max(1e-12), rng.f());
        let z = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
        let n = if i == 0 { v } else { v * (1.0 + scatter * z.clamp(-2.5, 2.5)) };
        out.push((t, n.max(0.0).round()));
        i += 1;
    }
    out
}

pub fn growth_render(p: &Params) -> String {
    let mode = p.s("growth").to_string();
    let n0 = p.f("n0").max(1e-6);
    let r = p.f("r").max(1e-6);
    let k = p.f("k").max(1.0);
    let tspan = p.f("tspan").max(1e-6);
    let unit = p.s("unit").to_lowercase();
    let unit1 = unit.trim_end_matches('s').to_string();
    let org = p.s("organism").trim().to_string();
    let col = color_hex(p.s("curve_color"));
    let blank = p.b("blank");
    let draw_curve = p.b("draw_curve") || !p.0.contains_key("draw_curve");
    let panels = panels_of(p.s("graph"));
    let series = model(&mode, n0, r, k, tspan, col, p.f("tau"));
    let ts: Vec<f64> = (0..NPTS).map(|i| tspan * i as f64 / (NPTS - 1) as f64).collect();
    let rates: Vec<Vec<f64>> = series.iter().map(|s| derivative(&ts, &s.ys)).collect();
    let pers: Vec<Vec<f64>> = series.iter().zip(&rates).map(|(s, r)| s.ys.iter().zip(r).map(|(n, d)| if *n > 1e-9 { d / n } else { 0.0 }).collect()).collect();
    let has_k = mode != "Exponential (J-curve)";
    let logistic_like = matches!(mode.as_str(), "Logistic (S-curve)" | "Exponential and logistic");
    let mut census = parse_points(p.s("census"));
    let auto: Vec<(f64, f64)> = if p.b("census_auto") {
        let sc = match p.s("census_scatter") { "About 5%" => 0.05, "About 10%" => 0.10, "About 20%" => 0.20, _ => 0.0 };
        let main = series.iter().position(|s| s.fit).unwrap_or(0);
        auto_census(&ts, &series[main].ys, tspan, p.f("census_every").max(0.01), sc, p.i("census_seed"))
    } else { vec![] };
    census.extend(auto.iter().copied());
    let title_style = p.s("title_style");
    let title = if title_style == "None" || title_style == "Blank line" { String::new() } else { p.s("title").trim().to_string() };
    let title_blank = title_style == "Blank line";
    let has_title = !title.is_empty() || title_blank;
    let t_inf = if logistic_like && n0 < k / 2.0 { Some(((k - n0) / n0).ln() / r) } else { None };
    let phases = p.b("mark_phases") && t_inf.is_some() && !blank && panels.contains(&Panel::TimeN);
    let has_ends = p.s("y_end") == "Text" || p.s("y_hi_end") != "None" && !p.s("y_hi_end").is_empty();

    let fit = p.b("fit");
    let ydata_of = |pk: &Panel, si: usize| -> &Vec<f64> {
        match pk { Panel::TimeN => &series[si].ys, Panel::TimeRate | Panel::NRate => &rates[si], _ => &pers[si] }
    };
    let range_of = |pi: usize, pk: &Panel| -> (f64, f64, f64, f64, f64, f64) {
        let is_time = matches!(pk, Panel::TimeN | Panel::TimeRate | Panel::TimePer);
        // fit
        let (mut ymin, mut ymax) = (0.0f64, 0.0f64);
        let mut xmax_n = 0.0f64;
        for (si, s) in series.iter().enumerate() {
            if !s.fit { continue; }
            for (i, v) in ydata_of(pk, si).iter().enumerate() {
                ymax = ymax.max(*v);
                ymin = ymin.min(*v);
                xmax_n = xmax_n.max(s.ys[i]);
            }
        }
        if matches!(pk, Panel::TimeN) {
            for (_, n) in &census { ymax = ymax.max(*n); }
        }
        if ymax <= 0.0 { ymax = 1.0; }
        if matches!(pk, Panel::TimeN) && has_k { ymax = ymax.max(k); }
        if matches!(pk, Panel::NRate | Panel::NPer) && has_k { xmax_n = xmax_n.max(k); }
        let ycap = |ymax: f64, ymin: f64| -> (f64, f64, f64) {
            let step = nice_step((ymax - ymin) * 1.05, 10.0);
            let top = ((ymax * 1.05) / step).ceil() * step;
            let bot = if ymin < -1e-9 { (ymin * 1.05 / step).floor() * step } else { 0.0 };
            (bot, top.max(step), step)
        };
        let (mut yb, mut yt, mut ys) = ycap(ymax, ymin);
        let (mut xlo, x1, xs);
        if fit {
            let xrange = if is_time { tspan } else { xmax_n.max(1e-6) };
            xs = nice_step(xrange, 20.0);
            x1 = (xrange / xs - 1e-9).ceil() * xs;
            xlo = 0.0;
        } else {
            let xr = if is_time { p.f("x_max").max(1e-6) } else { p.f("x_max").max(1e-6) };
            xlo = p.f("x_min");
            if xlo >= xr { xlo = 0.0; }
            xs = if p.f("x_step") > 0.0 && (xr - xlo) / p.f("x_step") <= 200.0 { p.f("x_step") } else { nice_step(xr - xlo, 20.0) };
            x1 = xlo + ((xr - xlo) / xs - 1e-9).ceil() * xs;
        }
        if !fit && pi == 0 {
            let top_v = p.f("y_max").max(1e-6);
            let lo = if p.f("y_min") < top_v { p.f("y_min") } else { 0.0 };
            ys = if p.f("y_step") > 0.0 && (top_v - lo) / p.f("y_step") <= 200.0 { p.f("y_step") } else { nice_step(top_v - lo, 10.0) };
            yb = lo;
            yt = lo + ((top_v - lo) / ys - 1e-9).ceil() * ys;
        }
        if !fit && pi == 1 && p.f("y2_max") > 0.0 {
            let top_v = p.f("y2_max");
            ys = if p.f("y2_step") > 0.0 && (top_v - yb) / p.f("y2_step") <= 200.0 { p.f("y2_step") } else { nice_step(top_v - yb, 10.0) };
            yt = yb + ((top_v - yb) / ys - 1e-9).ceil() * ys;
        }
        if yt <= yb { yt = yb + ys; }
        (yb, yt, ys, xlo, x1, xs)
    };
    let ranges: Vec<(f64, f64, f64, f64, f64, f64)> = panels.iter().enumerate().map(|(pi, pk)| range_of(pi, pk)).collect();
    // census table beside the graph
    let table: Vec<(f64, f64)> = if p.b("census_table") && !blank { census.clone() } else { vec![] };
    let single = panels.len() == 1;
    // the reference's graph paper: 32 px squares, a 640 px wide plot, the left margin sized to the
    // widest y number (0.6 em of 14 px per character)
    let ny0 = (((ranges[0].1 - ranges[0].0) / ranges[0].2).round().max(1.0)) as f64;
    let ah = if single { (ny0 * 32.0).clamp(160.0, 480.0) } else { 205.0 };
    let rows_per = (((ah - 28.0f64) / 17.0).floor() as usize).max(4);
    let ntab = if table.is_empty() { 0 } else { table.len().div_ceil(rows_per) };
    let tab_w = 150.0;
    let ychars = ranges.iter().zip(&panels).map(|(r, _)| y_label_chars(r.0, r.1, r.2, numbers_every(p.s("y_numbers")))).max().unwrap_or(1);
    let (x0, aw) = (ref_left(ychars), 640.0f64);
    let x_end_extra = if p.s("x_end") == "Text" || p.s("x_hi_end") != "None" && !p.s("x_hi_end").is_empty() { 20.0 } else { 0.0 };
    let w = x0 + aw + 22.4 + x_end_extra + if ntab > 0 { 10.0 + ntab as f64 * (tab_w + 12.0) } else { 0.0 };
    let top = 14.0 + if has_title { 34.0 } else { 0.0 } + if phases { 26.0 } else { 0.0 } + if has_ends { 16.0 } else { 0.0 };
    let stride = ah + 66.8 + 20.0;
    let legend_h = if series.len() > 1 { 26.6 } else { 0.0 };
    let h = top + stride * panels.len() as f64 - 20.0 + legend_h;
    let mut d = Doc::new(w, h);
    if !title.is_empty() { bold(&mut d, (x0 + aw / 2.0).max(w / 2.0 - if ntab > 0 { ntab as f64 * 81.0 } else { 0.0 }), 32.0, 19.0, "middle", &title); }
    else if title_blank { d.line(x0 + aw / 2.0 - 140.0, 36.0, x0 + aw / 2.0 + 140.0, 36.0, INK, 1.3); }

    let tx = if p.s("x_title").trim().is_empty() { format!("Time ({unit})") } else { p.s("x_title").trim().to_string() };
    let noun = if org.is_empty() { String::new() } else { org.clone() };
    let opts = AxOpts {
        xnum: numbers_every(p.s("x_numbers")),
        ynum: numbers_every(p.s("y_numbers")),
        xend: if p.s("x_end") == "Text" { p.s("x_end_text").trim().to_string() } else { String::new() },
        yend: if p.s("y_end") == "Text" { p.s("y_end_text").trim().to_string() } else { String::new() },
        x_lo: p.s("x_lo_end").to_string(), x_hi: p.s("x_hi_end").to_string(),
        y_lo: p.s("y_lo_end").to_string(), y_hi: p.s("y_hi_end").to_string(),
        xt_mode: p.s("xtitle_style").to_string(), yt_mode: p.s("ytitle_style").to_string(),
    };

    for (pi, pk) in panels.iter().enumerate() {
        let y0 = top + stride * pi as f64;
        let ydata = |si: usize| -> &Vec<f64> { ydata_of(pk, si) };
        let is_time = matches!(pk, Panel::TimeN | Panel::TimeRate | Panel::TimePer);
        let (yb, yt, ys, xlo, x1, xs) = ranges[pi];
        let a = Axes { x: x0, y: y0, w: aw, h: ah, x0: xlo, x1, y0: yb, y1: yt, xs, ys };

        let ytitle_auto = match pk {
            Panel::TimeN => if noun.is_empty() { "Population size (N)".to_string() } else { format!("Number of {noun} (N)") },
            Panel::TimeRate | Panel::NRate => if p.s("rate_title").trim().is_empty() { format!("Growth rate, dN/dt (per {unit1})") } else { p.s("rate_title").trim().to_string() },
            _ => if p.s("pc_title").trim().is_empty() { format!("Per capita growth rate (per {unit1})") } else { p.s("pc_title").trim().to_string() },
        };
        let yt_text = if pi == 0 && !p.s("y_title").trim().is_empty() && matches!(pk, Panel::TimeN) { p.s("y_title").trim().to_string() }
            else if pi == 0 && !p.s("y_title").trim().is_empty() && p.s("rate_title").trim().is_empty() && p.s("pc_title").trim().is_empty() { p.s("y_title").trim().to_string() }
            else { ytitle_auto };
        let xt_text = if is_time { tx.clone() } else if noun.is_empty() { "Population size (N)".to_string() } else { format!("Number of {noun} (N)") };
        let mut o = opts.clone();
        if !is_time && o.xend == "t" { o.xend = "N".into(); }
        if !matches!(pk, Panel::TimeN) && o.yend == "N" { o.yend = String::new(); }
        // the population-size title style only governs the N axis; rate axes always show text
        if !matches!(pk, Panel::TimeN) { o.yt_mode = "Text".into(); }
        if !is_time { o.xt_mode = "Text".into(); }
        draw_axes_ex(&mut d, &a, minor_of(p.s("minor")), &xt_text, &yt_text, &o);
        stash(match pk { Panel::TimeN => 0, Panel::TimeRate => 1, Panel::TimePer => 2, Panel::NRate => 3, Panel::NPer => 4 }, &a);
        if blank { continue; }

        let clip = format!("clip{pi}");
        begin_clip(&mut d, &a, &clip);
        let kstyle = p.s("k_label_style");
        if has_k && matches!(pk, Panel::TimeN) && p.b("mark_k") {
            dashed(&mut d, &[(a.px(a.x0), a.py(k)), (a.px(a.x1), a.py(k))], INK, 2.0, "7 5");
        }
        if has_k && matches!(pk, Panel::NRate | Panel::NPer) && p.b("mark_k") {
            dashed(&mut d, &[(a.px(k), a.py(a.y0)), (a.px(k), a.py(a.y1))], INK, 1.6, "7 5");
        }
        if phases && matches!(pk, Panel::TimeN) {
            let ti = t_inf.unwrap_or(0.0);
            let slope = r * k / 4.0;
            let (t1, t2) = ((ti + (n0 - k / 2.0) / slope).max(0.0), ti + (k / 2.0) / slope);
            for t in [t1, t2] {
                if t > a.x0 && t < a.x1 { dashed(&mut d, &[(a.px(t), a.y), (a.px(t), a.y + a.h)], INK, 1.2, "2 3"); }
            }
        }
        if draw_curve {
            for (si, s) in series.iter().enumerate() {
                let yv = ydata(si);
                let pts: Vec<(f64, f64)> = if is_time {
                    ts.iter().zip(yv).map(|(t, y)| a.pt((*t, *y))).collect()
                } else {
                    s.ys.iter().zip(yv).map(|(n, y)| a.pt((*n, *y))).collect()
                };
                let cw = p.f("curve_width").clamp(1.0, 7.0);
                if s.dash.is_empty() { d.polyline(&pts, &s.color, cw); } else { dashed(&mut d, &pts, &s.color, cw, s.dash); }
            }
        }
        if matches!(pk, Panel::TimeN) {
            for (t, n) in &census {
                let (cx, cy) = a.pt((*t, *n));
                d.circle(cx, cy, 4.2, INK, "#fff", 1.0);
            }
        }
        if p.b("mark_infl") {
            if let Some(ti) = t_inf {
                let pt = match pk {
                    Panel::TimeN => Some((ti, k / 2.0)),
                    Panel::TimeRate => Some((ti, r * k / 4.0)),
                    Panel::NRate => Some((k / 2.0, r * k / 4.0)),
                    _ => None,
                };
                if let Some(pt) = pt {
                    let (cx, cy) = a.pt(pt);
                    dashed(&mut d, &[(a.px(a.x0), cy), (cx, cy), (cx, a.py(a.y0))], INK, 1.2, "5 4");
                    d.circle(cx, cy, 5.5, "#fff", INK, 2.0);
                }
            }
        }
        end_clip(&mut d);
        if has_k && matches!(pk, Panel::TimeN) && p.b("mark_k") {
            match kstyle {
                "None" => {}
                "Blank line" => { d.line(a.x + 8.0, a.py(k) + 20.0, a.x + 170.0, a.py(k) + 20.0, INK, 1.3); }
                _ => {
                    // the reference paints a white plate under the label, then the text with a white halo,
                    // so the dashed line and the gridlines never run through the words
                    let text = if p.s("k_label").trim().is_empty() { "Carrying capacity (K)" } else { p.s("k_label").trim() };
                    let (x, y, size) = (a.x + 8.0, a.py(k) + 21.7, 14.7);
                    d.raw(&format!(r##"<g fill="#fff"><rect x="{:.3}" y="{:.3}" width="{:.4}" height="{:.3}"/></g>"##, x - 2.0, y - size * 0.78, bold_width(text, size) + 4.0, size * 1.02));
                    txt(&mut d, x, y, size, "start", SANS, r##"font-weight="bold" stroke="#fff" stroke-width="4" paint-order="stroke" stroke-linejoin="round""##, text);
                }
            }
        }
        if has_k && matches!(pk, Panel::NRate | Panel::NPer) && p.b("mark_k") && kstyle != "None" {
            bold(&mut d, a.px(k) - 5.0, a.y + 16.0, 13.0, "end", "K");
        }
        if p.b("mark_infl") && matches!(pk, Panel::TimeN) {
            if let Some(ti) = t_inf {
                let (cx, cy) = a.pt((ti, k / 2.0));
                bold(&mut d, (cx + 12.0).min(a.x + a.w - 170.0), cy + 18.0, 13.0, "start", "Inflection point (N = K/2)");
            }
        }
        if phases && matches!(pk, Panel::TimeN) {
            let ti = t_inf.unwrap_or(0.0);
            let slope = r * k / 4.0;
            let (t1, t2) = ((ti + (n0 - k / 2.0) / slope).max(a.x0), (ti + (k / 2.0) / slope).min(a.x1));
            let by = a.y - 10.0 - if has_ends { 16.0 } else { 0.0 };
            for (lo, hi, name) in [(a.x0, t1, "Lag phase"), (t1, t2, "Exponential phase"), (t2, a.x1, "Stationary phase")] {
                if hi - lo <= 1e-9 { continue; }
                let (xa, xb) = (a.px(lo) + 2.0, a.px(hi) - 2.0);
                d.polyline(&[(xa, by + 6.0), (xa, by), (xb, by), (xb, by + 6.0)], INK, 1.2);
                let fs = if (xb - xa) < 110.0 { 11.0 } else { 13.0 };
                bold(&mut d, (xa + xb) / 2.0, by - 5.0, fs, "middle", name);
            }
        }
    }
    // census table(s)
    if ntab > 0 {
        let tx0 = x0 + aw + 22.0 + x_end_extra;
        let ty0 = top;
        let rows: Vec<&(f64, f64)> = table.iter().collect();
        for ti in 0..ntab {
            let x = tx0 + ti as f64 * (tab_w + 12.0);
            let chunk: Vec<&&(f64, f64)> = rows.iter().skip(ti * rows_per).take(rows_per).collect();
            let th = 30.0 + chunk.len() as f64 * 17.0 + 6.0;
            d.rect(x, ty0, tab_w, 30.0, "#e5e7eb", "none", 0.0);
            d.rect(x, ty0, tab_w, th, "none", INK, 1.2);
            d.line(x + tab_w / 2.0, ty0, x + tab_w / 2.0, ty0 + th, INK, 1.0);
            d.line(x, ty0 + 30.0, x + tab_w, ty0 + 30.0, INK, 1.0);
            let ucap = { let mut c = unit.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() };
            let _ = ucap;
            bold(&mut d, x + tab_w / 4.0, ty0 + 13.0, 10.5, "middle", "Time");
            bold(&mut d, x + tab_w / 4.0, ty0 + 25.0, 10.5, "middle", &format!("({unit})"));
            bold(&mut d, x + tab_w * 0.75, ty0 + 13.0, 10.5, "middle", "Population");
            bold(&mut d, x + tab_w * 0.75, ty0 + 25.0, 10.5, "middle", "size (N)");
            for (ri, (t, n)) in chunk.iter().map(|v| **v).enumerate() {
                let y = ty0 + 30.0 + 15.0 + ri as f64 * 17.0;
                txt(&mut d, x + tab_w / 2.0 - 8.0, y, 11.5, "end", SANS, "", &fnum((t * 1000.0).round() / 1000.0));
                txt(&mut d, x + tab_w - 8.0, y, 11.5, "end", SANS, "", &fnum(n.round()));
            }
        }
    }
    if series.len() > 1 && !blank && draw_curve {
        let items: Vec<(String, String, String)> = series.iter().map(|s| (s.name.clone(), s.color.clone(), s.dash.replace(' ', ","))).collect();
        let items: Vec<(String, String, String)> = items.into_iter().map(|(n, c, dsh)| (n, c, if dsh.is_empty() { String::new() } else { "8 5".into() })).collect();
        legend(&mut d, x0 + aw / 2.0, h - 23.4, &items);
    }
    d.finish()
}

/// "Start from an example" entries for population growth (reference gallery + classic setups).
/// The reference's one-line summaries under the Population Growth section headers.
pub fn growth_section_summary(title: &str, p: &Params) -> Option<String> {
    let num = |v: f64| {
        let t = format!("{:.4}", (v * 10_000.0).round() / 10_000.0);
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let ends = |lo: &str, hi: &str| -> String {
        let nm = |s: &str| s.to_lowercase();
        let plain = |s: &str| s.is_empty() || s == "None";
        if plain(lo) && plain(hi) { "plain ends".into() }
        else if lo == hi { format!("{} at both ends", nm(lo)) }
        else if plain(lo) { format!("{} at the right end", nm(hi)) }
        else if plain(hi) { format!("{} at the left end", nm(lo)) }
        else { format!("{} and {} ends", nm(lo), nm(hi)) }
    };
    Some(match title {
        "Graphs" => p.s("graph").to_string(),
        "Curve" => if p.0.contains_key("draw_curve") && !p.b("draw_curve") { "Hidden, for students to draw".into() } else { p.s("curve_color").to_string() },
        "Census data" => {
            if !p.b("census_auto") && p.s("census").trim().is_empty() { return Some("None".into()); }
            let unit = p.s("unit").to_lowercase();
            let sc = match p.s("census_scatter") { "About 5%" => "\u{b1}5% noise", "About 10%" => "\u{b1}10% noise", "About 20%" => "\u{b1}20% noise", _ => "on the curve" };
            let mut t = format!("Every {} {} \u{b7} {}", num(p.f("census_every")), if unit.is_empty() { "years".into() } else { unit }, sc);
            if p.b("census_table") { t.push_str(" \u{b7} with a table"); }
            t
        }
        "Marked on the graph" => {
            let mut v: Vec<&str> = Vec::new();
            if p.b("mark_k") { v.push("Carrying capacity"); }
            if p.b("mark_infl") { v.push("inflection point"); }
            if p.b("mark_phases") { v.push("phases"); }
            if v.is_empty() { "Nothing".into() } else { v.join(" \u{b7} ") }
        }
        "Titles" => {
            let mut v: Vec<String> = Vec::new();
            let q = |t: &str| format!("\u{201c}{t}\u{201d}");
            let unit = p.s("unit").to_lowercase();
            if p.s("title_style") == "Text" && !p.s("title").trim().is_empty() { v.push(q(p.s("title").trim())); }
            let tx = if p.s("x_title").trim().is_empty() { format!("Time ({})", if unit.is_empty() { "years" } else { &unit }) } else { p.s("x_title").trim().to_string() };
            let org = p.s("organism").trim();
            let ty = if !p.s("y_title").trim().is_empty() { p.s("y_title").trim().to_string() } else if org.is_empty() { "Population size (N)".to_string() } else { format!("Number of {org} (N)") };
            match p.s("xtitle_style") { "Text" => v.push(q(&tx)), "Blank line" => v.push("blank time line".into()), _ => {} }
            match p.s("ytitle_style") { "Text" => v.push(q(&ty)), "Blank line" => v.push("blank size line".into()), _ => {} }
            if v.is_empty() { "None".into() } else { v.join(" \u{b7} ") }
        }
        "x-axis" | "y-axis" => {
            let x = title == "x-axis";
            let ax = axes_of(p, false).into_iter().next()?;
            let (lo, hi, st) = if x { (ax.x0, ax.x1, ax.xs) } else { (ax.y0, ax.y1, ax.ys) };
            let (k, end, l, h) = if x { ("x", p.s("x_end"), p.s("x_lo_end"), p.s("x_hi_end")) } else { ("y", p.s("y_end"), p.s("y_lo_end"), p.s("y_hi_end")) };
            let nn = ((hi - lo) / st).round().max(1.0) as i64;
            let auto = ((nn as f64) / if x { 11.0 } else { 12.0 }).ceil().max(1.0) as i64;
            let every = numbers_every(p.s(&format!("{k}_numbers"))).unwrap_or(auto);
            let nums = match every { 0 => "no numbers".to_string(), 1 => "numbered".to_string(), n => format!("numbered every {n}") };
            let label = if end == "Text" { format!("label \u{201c}{}\u{201d}", p.s(&format!("{k}_end_text")).trim()) } else { "no label".to_string() };
            format!("{} to {} \u{b7} by {} \u{b7} {nums} \u{b7} {label} \u{b7} {}", num(lo), num(hi), num(st), ends(l, h))
        }
        "Grid" => match p.s("minor") { "2 per block" | "4 per block" | "5 per block" => format!("Minor gridlines, {}", p.s("minor")), _ => "No minor gridlines".into() },
        _ => return None,
    })
}

pub fn growth_examples() -> Vec<Example> {
    let st = |v: &str| Value::S(v.into());
    vec![
        Example { name: "Logistic growth curve with carrying capacity (S-curve)", params: vec![
            ("r", Value::F(0.4)), ("k", Value::F(500.0)), ("tspan", Value::F(30.0)), ("organism", st("deer")),
            ("mark_infl", Value::B(true)), ("y_title", st("Number of deer")), ("rate_title", st("Growth rate (deer/year)")),
        ] },
        Example { name: "Exponential vs. logistic growth graph (J-curve and S-curve)", params: vec![
            ("growth", st("Exponential and logistic")), ("n0", Value::F(10.0)),
            ("fit", Value::B(false)), ("x_max", Value::F(20.0)), ("x_step", Value::F(1.0)), ("y_max", Value::F(1300.0)), ("y_step", Value::F(100.0)),
        ] },
        Example { name: "Growth rate and per capita growth rate vs. population size", params: vec![
            ("n0", Value::F(10.0)), ("graph", st("Both rates against population size")), ("mark_infl", Value::B(true)),
            ("fit", Value::B(false)), ("x_max", Value::F(1100.0)), ("x_step", Value::F(50.0)), ("y_max", Value::F(160.0)), ("y_step", Value::F(20.0)),
            ("y2_max", Value::F(0.7)), ("y2_step", Value::F(0.1)),
        ] },
        Example { name: "Yeast population growth curve with census data and growth phases", params: vec![
            ("n0", Value::F(10.0)), ("r", Value::F(0.54)), ("k", Value::F(665.0)), ("tspan", Value::F(18.0)), ("unit", st("Hours")),
            ("organism", st("yeast cells")), ("census_auto", Value::B(true)), ("census_scatter", st("About 5%")), ("census_seed", Value::I(5)),
            ("mark_phases", Value::B(true)), ("y_title", st("Yeast cells (thousands)")),
            ("rate_title", st("Growth rate (yeast cells/hour)")), ("pc_title", st("Per capita growth rate (per hour)")),
        ] },
        Example { name: "Bacteria doubling every hour, with a census table", params: vec![
            ("growth", st("Exponential (J-curve)")), ("n0", Value::F(100.0)), ("r", Value::F(0.6931472)), ("tspan", Value::F(10.0)), ("unit", st("Hours")),
            ("organism", st("bacteria")), ("census_auto", Value::B(true)), ("census_table", Value::B(true)), ("y_title", st("Number of bacteria")),
        ] },
        Example { name: "Overshoot and oscillation around K", params: vec![
            ("growth", st("Overshoot and oscillation")), ("r", Value::F(0.6)), ("tau", Value::F(2.0)), ("tspan", Value::F(40.0)),
            ("census_auto", Value::B(true)), ("census_every", Value::F(5.0)),
        ] },
        Example { name: "Blank population growth graph for students to draw on", params: vec![("draw_curve", Value::B(false)), ("mark_k", Value::B(false))] },
    ]
}

/// The reference's "Classic setups" (Population card), value for value from its share links;
/// a setup that gives axis ranges turns axis fitting off, as choosing one does on the reference.
pub fn growth_classics() -> Vec<Example> {
    let st = |v: &str| Value::S(v.into());
    let f = Value::F;
    let no_fit = ("fit", Value::B(false));
    vec![
        Example { name: "Yeast in a flask", params: vec![
            ("n0", f(10.0)), ("r", f(0.54)), ("k", f(665.0)), ("tspan", f(18.0)), ("unit", st("Hours")), ("organism", st("yeast cells")),
            ("census_auto", Value::B(true)), ("census_scatter", st("About 5%")), ("mark_phases", Value::B(true)),
            ("x_title", st("Time (hours)")), ("y_title", st("Yeast cells (thousands)")),
            ("rate_title", st("Growth rate (yeast cells/hour)")), ("pc_title", st("Per capita growth rate (per hour)")),
            no_fit.clone(), ("x_max", f(18.0)), ("y_max", f(800.0)),
        ] },
        Example { name: "Deer on an island", params: vec![
            ("r", f(0.4)), ("k", f(500.0)), ("tspan", f(30.0)), ("organism", st("deer")), ("census_every", f(5.0)), ("mark_infl", Value::B(true)),
            ("y_title", st("Number of deer")), ("rate_title", st("Growth rate (deer/year)")),
            no_fit.clone(), ("x_max", f(30.0)), ("x_step", f(2.0)), ("y_max", f(550.0)), ("y_step", f(50.0)),
        ] },
        Example { name: "Bacteria doubling every hour", params: vec![
            ("growth", st("Exponential (J-curve)")), ("n0", f(100.0)), ("r", f(0.6931472)), ("tspan", f(10.0)), ("unit", st("Hours")),
            ("organism", st("bacteria")), ("census_auto", Value::B(true)), ("census_table", Value::B(true)),
            ("x_title", st("Time (hours)")), ("y_title", st("Number of bacteria")),
            ("rate_title", st("Growth rate (bacteria/hour)")), ("pc_title", st("Per capita growth rate (per hour)")),
            no_fit.clone(), ("x_max", f(10.0)), ("x_step", f(0.5)), ("y_max", f(120_000.0)), ("y_step", f(10_000.0)),
        ] },
        Example { name: "Exponential and logistic growth", params: vec![
            ("growth", st("Exponential and logistic")), ("n0", f(10.0)), ("census_every", f(2.0)), no_fit.clone(), ("y_max", f(1300.0)),
        ] },
        Example { name: "Per capita growth rate against N", params: vec![
            ("growth", st("Exponential and logistic")), ("n0", f(10.0)), ("graph", st("Per capita rate against population size")), ("census_every", f(2.0)),
            no_fit.clone(), ("x_max", f(1300.0)), ("x_step", f(50.0)), ("x_numbers", st("Every 5th line")), ("y_max", f(0.65)), ("y_step", f(0.05)),
        ] },
        Example { name: "Growth rate and per capita rate", params: vec![
            ("n0", f(10.0)), ("graph", st("Both rates against population size")), ("census_every", f(2.0)), ("mark_infl", Value::B(true)),
            no_fit.clone(), ("x_max", f(1100.0)), ("x_step", f(50.0)), ("y_max", f(160.0)), ("y_step", f(20.0)), ("y2_max", f(0.7)), ("y2_step", f(0.1)),
        ] },
        Example { name: "Population size and growth rate over time", params: vec![
            ("n0", f(10.0)), ("graph", st("Population size and growth rate")), ("census_every", f(2.0)), ("mark_infl", Value::B(true)),
            no_fit.clone(), ("y_max", f(1200.0)), ("y_step", f(200.0)), ("y2_max", f(160.0)),
        ] },
        Example { name: "Overshoot and oscillation around K", params: vec![
            ("growth", st("Overshoot and oscillation")), ("r", f(0.6)), ("tau", f(2.0)), ("tspan", f(40.0)), ("census_every", f(5.0)),
            no_fit.clone(), ("x_max", f(40.0)), ("x_step", f(2.0)), ("y_max", f(1800.0)), ("y_step", f(200.0)),
        ] },
        Example { name: "Boom and crash", params: vec![
            ("growth", st("Boom and crash")), ("n0", f(30.0)), ("k", f(6000.0)), ("tspan", f(30.0)), ("organism", st("reindeer")), ("census_every", f(5.0)),
            ("y_title", st("Number of reindeer")), ("rate_title", st("Growth rate (reindeer/year)")),
            no_fit, ("x_max", f(30.0)), ("x_step", f(2.0)), ("y_max", f(6500.0)), ("y_step", f(500.0)),
        ] },
        Example { name: "Blank graph for students to draw on", params: vec![("draw_curve", Value::B(false)), ("census_every", f(2.0))] },
    ]
}

// ---------------------------------------------------------------- predator-prey

pub const OWN_PAIR: &str = "Your own pair";

const PAIRS: [&str; 9] = [
    "Snowshoe hare and Canada lynx",
    "Rabbit and wolf",
    "Rabbit and red fox",
    "Moose and wolf (Isle Royale)",
    "Sea lion and orca",
    "Snake and eagle",
    "Mouse and owl",
    "Aphid and ladybug",
    "Your own pair",
];

/// The animals a named pair stands for (prey, predator), None for Custom.
pub fn preset_names(pair: &str) -> Option<(&'static str, &'static str)> {
    preset(pair).map(|(a, b, ..)| (a, b))
}

pub fn pp_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("pair", "Pair", &PAIRS, PAIRS[0]),
        ParamDef::text("prey", "Prey", "Hares"),
        ParamDef::text("pred", "Predators", "Lynx"),
        ParamDef::choice("source", "Work the cycle out from", &["Populations", "Model rates (α β γ δ)"], "Populations"),
        ParamDef::float("prey0", "Prey at the start", 0.1, 1_000_000.0, 1.0, 20.0),
        ParamDef::float("pred0", "Predators at the start", 0.1, 1_000_000.0, 1.0, 12.0),
        ParamDef::float("prey_avg", "Average prey", 1.0, 1_000_000.0, 1.0, 50.0),
        ParamDef::float("pred_avg", "Average predators", 1.0, 1_000_000.0, 1.0, 20.0),
        ParamDef::float("cycle", "Cycle length (years)", 0.5, 500.0, 0.5, 10.0),
        ParamDef::float("alpha", "Model: prey growth, alpha", 0.01, 5.0, 0.01, 0.67),
        ParamDef::float("beta", "Model: predation, beta", 0.0001, 1.0, 0.0001, 0.0337),
        ParamDef::float("gamma", "Model: predator death, gamma", 0.01, 5.0, 0.01, 0.67),
        ParamDef::float("delta", "Model: predator gain, delta", 0.0001, 1.0, 0.0001, 0.0135),
        ParamDef::float("tshow", "Time shown", 1.0, 1000.0, 1.0, 40.0),
        ParamDef::choice("unit", "Units", &["Years", "Months", "Weeks", "Days"], "Years"),
        ParamDef::choice("graph", "Show", &["Over time", "Predators against prey"], "Over time"),
        ParamDef::choice("show", "Show", &["Both", "Prey only", "Predators only"], "Both"),
        ParamDef::choice("style", "Style", &["Color", "Black and white"], "Color"),
        ParamDef::choice("data", "Data", &["Smooth curves", "Points each time unit", "Census counts"], "Smooth curves"),
        ParamDef::choice("marks", "Marked", &["Nothing", "Peaks"], "Nothing"),
        ParamDef::text("x_title", "x-axis title (blank = automatic)", ""),
        ParamDef::text("y_title", "y-axis title (blank = automatic)", ""),
        ParamDef::choice("minor", "Minor gridlines", &["None", "2 per block", "4 per block", "5 per block"], "None"),
        ParamDef::text("title", "Chart title (blank = none)", ""),
        ParamDef::float("line_width", "Line thickness", 1.0, 7.0, 0.5, 3.0),
        ParamDef::bool("legend", "Legend", true),
        // ---- added for parity with the reference controls
        ParamDef::choice("y_axis", "y-axis", &["Automatic", "One for both", "Predators on the right"], "Automatic"),
        ParamDef::float("census_every", "Census: counted every (time units)", 0.1, 100.0, 0.1, 1.0),
        ParamDef::float("census_noise", "Census: counts off by about (%)", 0.0, 50.0, 1.0, 10.0),
        ParamDef::int("census_seed", "Census: new counts (seed)", 1, 9999, 1),
        ParamDef::bool("census_join", "Join the counts with lines (solid prey, dashed predators)", true),
        ParamDef::bool("mark_lag", "Lag from a prey peak to the predator peak after it", false),
        ParamDef::bool("mark_period", "Period from one peak to the next", false),
        ParamDef::choice("lag_labels", "Lag and period labels", &["Name and length", "Name only", "Blank for students"], "Name and length"),
        ParamDef::choice("title_style", "Chart title", &["Text", "Blank line", "None"], "Text"),
        ParamDef::choice("xtitle_style", "x-axis title", &["Text", "Blank line", "None"], "Text"),
        ParamDef::choice("ytitle_style", "y-axis title", &["Text", "Blank line", "None"], "Text"),
        ParamDef::bool("x_fit", "Fit to the time shown", true),
        ParamDef::float("x_min", "x-axis from (when not fitting)", -10_000.0, 10_000.0, 1.0, 0.0),
        ParamDef::float("x_max", "x-axis to (when not fitting)", 0.1, 10_000.0, 1.0, 40.0),
        ParamDef::float("x_step", "x-axis count by (0 = automatic)", 0.0, 10_000.0, 0.1, 0.0),
        ParamDef::choice("x_numbers", "x-axis numbers", &["Automatic", "Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"], "Automatic"),
        ParamDef::choice("x_end", "x-axis label at the right end", &["None", "Text"], "None"),
        ParamDef::text("x_end_text", "x-axis end label text", "t"),
        ParamDef::choice("x_lo_end", "x-axis left end", &["None", "Triangle arrow", "Line arrow", "Circle"], "None"),
        ParamDef::choice("x_hi_end", "x-axis right end", &["None", "Triangle arrow", "Line arrow", "Circle"], "None"),
        ParamDef::bool("y_fit", "Fit to the populations", true),
        ParamDef::float("y_min", "y-axis from (when not fitting)", -1_000_000.0, 1_000_000.0, 1.0, 0.0),
        ParamDef::float("y_max", "y-axis to (when not fitting)", 0.1, 10_000_000.0, 1.0, 120.0),
        ParamDef::float("y_step", "y-axis count by (0 = automatic)", 0.0, 10_000_000.0, 0.1, 0.0),
        ParamDef::choice("y_numbers", "y-axis numbers", &["Automatic", "Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"], "Automatic"),
        ParamDef::choice("y_end", "y-axis label at the top end", &["None", "Text"], "None"),
        ParamDef::text("y_end_text", "y-axis end label text", "N"),
        ParamDef::choice("y_lo_end", "y-axis bottom end", &["None", "Triangle arrow", "Line arrow", "Circle"], "None"),
        ParamDef::choice("y_hi_end", "y-axis top end", &["None", "Triangle arrow", "Line arrow", "Circle"], "None"),
    ]
}

/// Lotka-Volterra with RK4. Returns (t, prey, pred) samples at `n` uniform steps over `tend`.
fn lv(a: f64, b: f64, g: f64, dl: f64, x0: f64, y0: f64, tend: f64, n: usize) -> Vec<(f64, f64, f64)> {
    let f = |x: f64, y: f64| (a * x - b * x * y, dl * x * y - g * y);
    let sub = 4usize;
    let h = tend / (n * sub) as f64;
    let (mut x, mut y) = (x0, y0);
    let mut out = vec![(0.0, x, y)];
    for i in 1..=n * sub {
        let (k1x, k1y) = f(x, y);
        let (k2x, k2y) = f(x + 0.5 * h * k1x, y + 0.5 * h * k1y);
        let (k3x, k3y) = f(x + 0.5 * h * k2x, y + 0.5 * h * k2y);
        let (k4x, k4y) = f(x + h * k3x, y + h * k3y);
        x += h / 6.0 * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
        y += h / 6.0 * (k1y + 2.0 * k2y + 2.0 * k3y + k4y);
        if i % sub == 0 { out.push((i as f64 * h, x, y)); }
    }
    out
}

fn local_max_times(v: &[(f64, f64, f64)], pick: fn(&(f64, f64, f64)) -> f64) -> Vec<(f64, f64)> {
    let mut o = Vec::new();
    for i in 1..v.len().saturating_sub(1) {
        let (a, b, c) = (pick(&v[i - 1]), pick(&v[i]), pick(&v[i + 1]));
        if b > a && b >= c { o.push((v[i].0, b)); }
    }
    o
}

fn preset(pair: &str) -> Option<(&'static str, &'static str, f64, f64, f64, &'static str, &'static str)> {
    // prey, predator, avg prey, avg predator, cycle, units, y title
    match pair {
        "Snowshoe hare and Canada lynx" => Some(("Hares", "Lynx", 50.0, 20.0, 10.0, "Years", "Number of animals (thousands)")),
        "Rabbit and red fox" => Some(("Rabbits", "Foxes", 400.0, 60.0, 6.0, "Years", "Number of animals")),
        "Mouse and owl" => Some(("Mice", "Owls", 200.0, 30.0, 4.0, "Years", "Number of animals")),
        "Aphid and ladybug" => Some(("Aphids", "Ladybugs", 500.0, 80.0, 20.0, "Days", "Number of insects")),
        "Rabbit and wolf" => Some(("Rabbits", "Wolves", 600.0, 30.0, 8.0, "Years", "Rabbits")),
        "Moose and wolf (Isle Royale)" => Some(("Moose", "Wolves", 1000.0, 25.0, 30.0, "Years", "Moose")),
        "Sea lion and orca" => Some(("Sea lions", "Orcas", 2000.0, 20.0, 20.0, "Years", "Sea lions")),
        "Snake and eagle" => Some(("Snakes", "Eagles", 300.0, 12.0, 12.0, "Years", "Snakes")),
        _ => None,
    }
}

/// Starting populations and time shown for the reference pairs added later (prey0, pred0, time shown).
fn preset_start(pair: &str) -> Option<(f64, f64, f64)> {
    match pair {
        "Rabbit and wolf" => Some((300.0, 20.0, 32.0)),
        "Moose and wolf (Isle Royale)" => Some((600.0, 20.0, 90.0)),
        "Sea lion and orca" => Some((1200.0, 14.0, 60.0)),
        "Snake and eagle" => Some((150.0, 9.0, 48.0)),
        _ => None,
    }
}

/// A pair pick fills the page's boxes with that pair's numbers; typing in one of those boxes while
/// a named pair is picked turns the pair into "Your own pair" and keeps everything else as shown.
pub fn pp_panel_changed(key: &str, p: &mut Params) {
    let pair = p.s("pair").to_string();
    let Some((a, b, pa, qa, cy, u, _)) = preset(&pair) else { return };
    let fill = |p: &mut Params| {
        p.set_s("prey", a);
        p.set_s("pred", b);
        p.set_f("prey_avg", pa);
        p.set_f("pred_avg", qa);
        p.set_f("cycle", cy);
        p.set_s("unit", u);
        let (x0, y0) = preset_start(&pair).map(|(x, y, _)| (x, y)).unwrap_or((pa * 0.4, qa * 0.6));
        p.set_f("prey0", x0);
        p.set_f("pred0", y0);
    };
    if key == "pair" {
        fill(p);
        if let Some((_, _, t)) = preset_start(&pair) {
            p.set_f("tshow", t);
        }
    } else if matches!(key, "prey" | "pred" | "prey_avg" | "pred_avg" | "cycle" | "unit" | "prey0" | "pred0") {
        let typed = p.0.get(key).cloned();
        fill(p);
        if let Some(v) = typed {
            p.0.insert(key.to_string(), v);
        }
        p.set_s("pair", PAIRS[8]);
    }
}

/// Mean spacing of successive peaks (0 when fewer than two).
fn cycle_est(pk: &[(f64, f64)]) -> f64 {
    if pk.len() < 2 { return 0.0; }
    (pk[pk.len() - 1].0 - pk[0].0) / (pk.len() - 1) as f64
}

/// Peak times by parabolic refinement of local maxima in a sampled series.
fn peaks(v: &[(f64, f64, f64)], pick: fn(&(f64, f64, f64)) -> f64) -> Vec<(f64, f64)> {
    let mut o = Vec::new();
    for i in 1..v.len().saturating_sub(1) {
        let (a, b, c) = (pick(&v[i - 1]), pick(&v[i]), pick(&v[i + 1]));
        if b > a && b >= c {
            let den = a - 2.0 * b + c;
            let off = if den.abs() > 1e-12 { (0.5 * (a - c) / den).clamp(-0.5, 0.5) } else { 0.0 };
            let dt = v[i + 1].0 - v[i].0;
            o.push((v[i].0 + off * dt, b));
        }
    }
    o
}

/// What a predator-prey figure models once its pair preset is applied: names, averages, cycle,
/// unit, starting populations and the Lotka-Volterra rates.
struct PpSetup {
    prey: String, pred: String, pavg: f64, qavg: f64, cycle: f64, unit: String, ytitle: String,
    auto_right: bool, x0: f64, y0: f64, alpha: f64, beta: f64, gamma: f64, delta: f64,
}

fn pp_setup(p: &Params) -> PpSetup {
    let (mut prey, mut pred) = (p.s("prey").trim().to_string(), p.s("pred").trim().to_string());
    let (mut pavg, mut qavg, mut cycle) = (p.f("prey_avg").max(1.0), p.f("pred_avg").max(1.0), p.f("cycle").max(0.1));
    let mut unit = p.s("unit").to_string();
    let mut ytitle = p.s("y_title").trim().to_string();
    let pair = p.s("pair");
    let mut auto_right = false;
    if let Some((a, b, c, dd, e, u, yt)) = preset(pair) {
        prey = a.into(); pred = b.into(); pavg = c; qavg = dd; cycle = e; unit = u.into();
        if ytitle.is_empty() { ytitle = yt.into(); }
        auto_right = preset_start(pair).is_some();
    }
    let (x0, y0) = if let Some((a, b, _)) = preset_start(pair) { (a, b) }
        else if preset(pair).is_some() { (pavg * 0.4, qavg * 0.6) }
        else { (p.f("prey0").max(0.01), p.f("pred0").max(0.01)) };
    // the reference's default (hares and lynx) counts thousands of animals
    if ytitle.is_empty() && prey == "Hares" && pred == "Lynx" && pavg == 50.0 && qavg == 20.0 { ytitle = "Number of animals (thousands)".into(); }
    if prey.is_empty() { prey = "Prey".into(); }
    if pred.is_empty() { pred = "Predators".into(); }
    let (alpha, beta, gamma, delta);
    if p.s("source").starts_with("Model rates") {
        alpha = p.f("alpha"); beta = p.f("beta"); gamma = p.f("gamma"); delta = p.f("delta");
    } else {
        // alpha = gamma = w; time scaling is exact, so measure the period at w = 1 and rescale.
        let (b1, d1) = (1.0 / qavg, 1.0 / pavg);
        let sim = lv(1.0, b1, 1.0, d1, x0, y0, 60.0, 6000);
        let mx = local_max_times(&sim, |t| t.1);
        let p1 = if mx.len() >= 2 { mx[1].0 - mx[0].0 } else { std::f64::consts::TAU };
        let w = p1 / cycle;
        alpha = w; gamma = w; beta = w / qavg; delta = w / pavg;
    }
    PpSetup { prey, pred, pavg, qavg, cycle, unit, ytitle, auto_right, x0, y0, alpha, beta, gamma, delta }
}

/// `v` to three significant figures, without trailing zeros (16.8, 111, 0.0337, 1.6, 10).
pub fn sig3(v: f64) -> String {
    if !v.is_finite() { return "–".into(); }
    if v == 0.0 { return "0".into(); }
    let mag = v.abs().log10().floor() as i32;
    let dec = 2 - mag;
    let f = 10f64.powi(dec);
    let r = (v * f).round() / f;
    // rounding up can add a digit (99.96 -> 100): recount
    let dec = (2 - (r.abs().max(1e-300).log10().floor() as i32)).max(0) as usize;
    let s = format!("{:.*}", dec, r);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// "1 year" / "1.6 years": the unit is plural unless the number shown is exactly 1.
fn with_unit(v: &str, unit_plural: &str) -> String {
    let one = unit_plural.trim_end_matches('s');
    format!("{v} {}", if v == "1" { one } else { unit_plural })
}

/// The predator-prey card's live readout, as on the reference: the cycle length and the lag
/// from a prey peak to the next predator peak, each population's range and average, and (when
/// worked out from the populations) the model rates α β γ δ.
pub fn pp_readout(p: &Params) -> Vec<String> {
    let st = pp_setup(p);
    let unit = st.unit.to_lowercase();
    // long enough for a few full cycles: the small-swing period is 2π/√(αγ), big swings run longer
    let t0 = std::f64::consts::TAU / (st.alpha * st.gamma).max(1e-9).sqrt();
    let mut out = Vec::new();
    let mut tend = 4.0 * t0;
    let (mut pk_prey, mut pk_pred, mut sim) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..4 {
        sim = lv(st.alpha, st.beta, st.gamma, st.delta, st.x0, st.y0, tend, 8000);
        pk_prey = peaks(&sim, |v| v.1);
        pk_pred = peaks(&sim, |v| v.2);
        if pk_prey.len() >= 2 { break; }
        tend *= 4.0;
    }
    if pk_prey.len() >= 2 {
        let cycle = pk_prey[1].0 - pk_prey[0].0;
        // lengths as the lag / period marks write them: whole units from 10 up, else tenths
        let len = |v: f64| fnum(if v >= 10.0 { v.round() } else { (v * 10.0).round() / 10.0 });
        let mut line = format!("A cycle every {}", with_unit(&len(cycle), &unit));
        if let Some(q) = pk_pred.iter().find(|(t, _)| *t > pk_prey[0].0) {
            line.push_str(&format!("; the predators peak {} after the prey", with_unit(&len(q.0 - pk_prey[0].0), &unit)));
        }
        out.push(line);
    }
    if !sim.is_empty() {
        let range = |f: fn(&(f64, f64, f64)) -> f64| sim.iter().map(f).fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
        let (plo, phi) = range(|v| v.1);
        let (qlo, qhi) = range(|v| v.2);
        // over a whole cycle the averages are exactly γ/δ (prey) and α/β (predators)
        out.push(format!("{}: {} to {}, averaging {}", st.prey, sig3(plo), sig3(phi), sig3(st.gamma / st.delta.max(1e-12))));
        out.push(format!("{}: {} to {}, averaging {}", st.pred, sig3(qlo), sig3(qhi), sig3(st.alpha / st.beta.max(1e-12))));
    }
    if !p.s("source").starts_with("Model rates") {
        out.push(format!("α = {}, β = {}, γ = {}, δ = {}", sig3(st.alpha), sig3(st.beta), sig3(st.gamma), sig3(st.delta)));
    }
    let _ = (st.pavg, st.qavg, st.cycle);
    out
}

/// Turns one predator-prey axis's "fit" off, first copying the fitted range into its From / To /
/// Count by boxes so the graph doesn't jump (only the over-time graph uses them).
pub fn pp_unfit(p: &Params, x_axis: bool) -> Params {
    let mut q = p.clone();
    let mut fitted = p.clone();
    fitted.set_b(if x_axis { "x_fit" } else { "y_fit" }, true);
    if let Some(a) = axes_of(&fitted, true).into_iter().find(|a| a.kind == 9) {
        if x_axis {
            q.set_f("x_min", a.x0); q.set_f("x_max", a.x1); q.set_f("x_step", a.xs);
        } else {
            q.set_f("y_min", a.y0); q.set_f("y_max", a.y1); q.set_f("y_step", a.ys);
        }
    }
    q.set_b(if x_axis { "x_fit" } else { "y_fit" }, false);
    q
}

/// The population-growth card's live readout, as on the reference: the inflection point and the
/// lag and stationary phases of a logistic curve, the doubling time of an exponential one, and
/// how far an overshooting one swings.
pub fn growth_readout(p: &Params) -> Vec<String> {
    let mode = p.s("growth");
    let (n0, r, k) = (p.f("n0").max(1e-6), p.f("r").max(1e-6), p.f("k").max(1.0));
    let unit = p.s("unit").to_lowercase();
    let unit1 = unit.trim_end_matches('s').to_string();
    let org = p.s("organism").trim();
    let who = if org.is_empty() { "individuals" } else { org };
    let mut out = Vec::new();
    if matches!(mode, "Logistic (S-curve)" | "Exponential and logistic") && n0 < k / 2.0 {
        let ti = ((k - n0) / n0).ln() / r;
        let slope = r * k / 4.0;
        out.push(format!("Inflection point: N = K/2 = {} at t = {}, growing fastest, {} {who} per {unit1}", fnum((k / 2.0 * 100.0).round() / 100.0), with_unit(&sig3(ti), &unit), sig3(slope)));
        // where the tangent at the inflection point meets N₀ and K
        let (t1, t2) = ((ti - (k / 2.0 - n0) / slope).max(0.0), ti + (k / 2.0) / slope);
        out.push(format!("Lag phase until t = {}; stationary phase from t = {}", sig3(t1), sig3(t2)));
    }
    if matches!(mode, "Exponential (J-curve)" | "Exponential and logistic") {
        out.push(format!("Doubles every {}", with_unit(&sig3(std::f64::consts::LN_2 / r), &unit)));
    }
    if mode == "Overshoot and oscillation" {
        let tau = if p.f("tau") > 0.0 { p.f("tau") } else { 1.3 / r };
        let rt = r * tau;
        let how = if rt < (-1.0f64).exp() { "too small to overshoot K" } else if rt < std::f64::consts::FRAC_PI_2 { "overshoots K, the swings dying away" } else { "swings around K that keep going" };
        out.push(format!("rτ = {}: {how}", sig3(rt)));
    }
    out
}

/// "Fit now": the axes fitted to the current curve, written into the x- and y-axis boxes (and the
/// second graph's), with fitting left as it was.
pub fn growth_fit_now(p: &Params) -> Params {
    let mut fitted = p.clone();
    fitted.set_b("fit", true);
    let axes = axes_of(&fitted, false);
    let mut q = p.clone();
    if let Some(a) = axes.first() {
        q.set_f("x_min", a.x0); q.set_f("x_max", a.x1); q.set_f("x_step", a.xs);
        q.set_f("y_min", a.y0); q.set_f("y_max", a.y1); q.set_f("y_step", a.ys);
    }
    if let Some(a) = axes.get(1) {
        q.set_f("y2_max", a.y1); q.set_f("y2_step", a.ys);
    }
    q
}

pub fn pp_render(p: &Params) -> String {
    let PpSetup { prey, pred, unit, ytitle, auto_right, x0, y0, alpha, beta, gamma, delta, .. } = pp_setup(p);
    let pair = p.s("pair");
    let tshow = p.f("tshow").max(0.1);
    let n = 800usize;
    let data = lv(alpha, beta, gamma, delta, x0, y0, tshow, n);
    let (cprey, cpred, dash_pred) = if p.s("style") == "Black and white" { (INK, INK, "9 6") } else { ("#2563eb", "#dc2626", "9 6") };
    let show_prey = p.s("show") != "Predators only";
    let show_pred = p.s("show") != "Prey only";
    let phase = p.s("graph") == "Predators against prey";
    let title_style = p.s("title_style");
    let title = if title_style == "None" || title_style == "Blank line" { String::new() } else { p.s("title").trim().to_string() };
    let right = !phase && show_prey && show_pred && match p.s("y_axis") { "Predators on the right" => true, "One for both" => false, _ => auto_right };
    let census = p.s("data") == "Census counts";

    // census counts: sampled every `census_every`, scattered by about census_noise %
    let counts: Vec<(f64, f64, f64)> = if census {
        let every = p.f("census_every").max(tshow / 400.0).max(0.01);
        let noise = p.f("census_noise").clamp(0.0, 50.0) / 100.0;
        let mut rng = Rng::new(p.i("census_seed").max(1) as u64);
        let mut v = Vec::new();
        let mut i = 0usize;
        loop {
            let t = i as f64 * every;
            if t > tshow + 1e-9 || v.len() > 500 { break; }
            let idx = ((t / tshow) * n as f64).round() as usize;
            let s = data[idx.min(data.len() - 1)];
            let mut g = || { let (u1, u2) = (rng.f().max(1e-12), rng.f()); ((-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()).clamp(-2.5, 2.5) };
            let (a1, a2) = (g(), g());
            v.push((t, (s.1 * (1.0 + noise * a1)).max(0.0), (s.2 * (1.0 + noise * a2)).max(0.0)));
            i += 1;
        }
        v
    } else { vec![] };

    let maxprey = data.iter().map(|v| v.1).chain(counts.iter().map(|v| v.1)).fold(0.0, f64::max);
    let maxpred = data.iter().map(|v| v.2).chain(counts.iter().map(|v| v.2)).fold(0.0, f64::max);
    let minor = minor_of(p.s("minor"));
    let xt_auto = format!("Time ({})", unit.to_lowercase());
    let opts = AxOpts {
        xnum: numbers_every(p.s("x_numbers")),
        ynum: numbers_every(p.s("y_numbers")),
        xend: if p.s("x_end") == "Text" { p.s("x_end_text").trim().to_string() } else { String::new() },
        yend: if p.s("y_end") == "Text" { p.s("y_end_text").trim().to_string() } else { String::new() },
        x_lo: p.s("x_lo_end").to_string(), x_hi: p.s("x_hi_end").to_string(),
        y_lo: p.s("y_lo_end").to_string(), y_hi: p.s("y_hi_end").to_string(),
        xt_mode: p.s("xtitle_style").to_string(), yt_mode: p.s("ytitle_style").to_string(),
    };
    // y range helper: (from, to, step)
    let yrange = |vmax: f64| -> (f64, f64, f64) {
        if p.b("y_fit") || !p.0.contains_key("y_fit") {
            let ys = nice_step(vmax * 1.05, 10.0);
            (0.0, ((vmax * 1.05) / ys).ceil().max(1.0) * ys, ys)
        } else {
            let (lo, hi) = (p.f("y_min"), p.f("y_max").max(p.f("y_min") + 1e-6));
            let st = if p.f("y_step") > 0.0 && (hi - lo) / p.f("y_step") <= 200.0 { p.f("y_step") } else { nice_step(hi - lo, 10.0) };
            (lo, lo + ((hi - lo) / st - 1e-9).ceil() * st, st)
        }
    };

    // the reference's graph paper: 32 px squares, a 640 px plot, margins sized to the numbers
    let vmax0 = if phase { maxpred } else { match (show_prey, show_pred) { (true, true) if !right => maxprey.max(maxpred), (true, _) => maxprey, _ => maxpred } };
    let (yb0, yt0, ys0) = if phase { let ys = nice_step(maxpred * 1.05, 10.0); (0.0, ((maxpred * 1.05) / ys).ceil() * ys, ys) } else { yrange(vmax0) };
    let ah = (((yt0 - yb0) / ys0).round().max(1.0) * 32.0).clamp(160.0, 480.0);
    let ychars = y_label_chars(yb0, yt0, ys0, numbers_every(p.s("y_numbers")));
    let (ax_x, aw) = (ref_left(ychars), 640.0f64);
    let w = ax_x + aw + 22.4 + if right { 50.0 } else { 0.0 };
    let has_title = !title.is_empty() || title_style == "Blank line";
    let ends = p.s("y_end") == "Text" || !matches!(p.s("y_hi_end"), "None" | "");
    let top = 14.0 + if has_title { 34.0 } else { 0.0 } + if ends { 16.0 } else { 0.0 };
    let h = top + ah + 66.8 + if p.b("legend") || phase { 26.6 } else { 0.0 };
    let mut d = Doc::new(w, h);
    if !title.is_empty() { bold(&mut d, ax_x + aw / 2.0, 32.0, 19.0, "middle", &title); }
    else if title_style == "Blank line" { d.line(ax_x + aw / 2.0 - 140.0, 36.0, ax_x + aw / 2.0 + 140.0, 36.0, INK, 1.3); }

    if !phase {
        let vmax = match (show_prey, show_pred) { (true, true) if !right => maxprey.max(maxpred), (true, _) => maxprey, _ => maxpred };
        let (yb, yt, ys) = yrange(vmax);
        let (xlo, x1, xs) = if p.b("x_fit") || !p.0.contains_key("x_fit") {
            let xs = nice_step(tshow, 20.0);
            (0.0, (tshow / xs - 1e-9).ceil() * xs, xs)
        } else {
            let (lo, hi) = (p.f("x_min"), p.f("x_max").max(p.f("x_min") + 1e-6));
            let st = if p.f("x_step") > 0.0 && (hi - lo) / p.f("x_step") <= 200.0 { p.f("x_step") } else { nice_step(hi - lo, 20.0) };
            (lo, lo + ((hi - lo) / st - 1e-9).ceil() * st, st)
        };
        let a = Axes { x: ax_x, y: top, w: aw, h: ah, x0: xlo, x1, y0: yb, y1: yt, xs, ys };
        let xt = if p.s("x_title").trim().is_empty() { xt_auto } else { p.s("x_title").trim().to_string() };
        let ya = if right { if p.s("y_title").trim().is_empty() && preset_start(pair).is_none() { prey.clone() } else { ytitle.clone() } }
            else if ytitle.is_empty() { "Number of animals".to_string() } else { ytitle.clone() };
        let mut o = opts.clone();
        if o.xnum.is_none() {
            let nx = ((a.x1 - a.x0) / a.xs).round() as i64;
            o.xnum = Some(if nx >= 16 { 5 } else if nx >= 8 { 2 } else { 1 });
        }
        draw_axes_ex(&mut d, &a, minor, &xt, &ya, &o);
        stash(9, &a);
        // right-hand predator axis: same gridlines, its own numbers
        let ap = if right {
            let (_, pt, _) = yrange(maxpred);
            let ny = ((a.y1 - a.y0) / a.ys).round().max(1.0);
            let pstep = nice_step(pt, ny);
            let ap = Axes { x: ax_x, y: top, w: aw, h: ah, x0: xlo, x1, y0: 0.0, y1: pstep * ny, xs, ys: pstep };
            let ey = o.ynum.unwrap_or(((ny) / 12.0).ceil().max(1.0) as i64);
            let mut maxl = 1usize;
            if ey > 0 {
                for i in (0..=ny as i64).filter(|i| i % ey == 0) {
                    let v = i as f64 * pstep;
                    let l = fnum(v);
                    maxl = maxl.max(l.chars().count());
                    bold(&mut d, ax_x + aw + 7.0, ap.py(v) + 4.0, 12.0, "start", &l);
                }
            }
            if o.yt_mode != "None" {
                let cx = ax_x + aw + 18.0 + maxl as f64 * 7.4;
                let cy = top + ah / 2.0;
                let t = if ytitle.is_empty() || preset_start(pair).is_some() || p.s("y_title").trim().is_empty() { pred.clone() } else { pred.clone() };
                txt(&mut d, cx, cy, 15.0, "middle", SANS, &format!(r#"font-weight="bold" transform="rotate(-90 {cx:.1} {cy:.1})""#), &t);
            }
            LAST.with(|l| l.borrow_mut().push(AxInfo { kind: 11, x: ap.x, y: ap.y, w: ap.w, h: ap.h, x0: ap.x0, x1: ap.x1, y0: ap.y0, y1: ap.y1, xs: ap.xs, ys: ap.ys }));
            ap
        } else { Axes { ..a } };
        begin_clip(&mut d, &a, "ppclip");
        let lw = p.f("line_width").clamp(1.0, 7.0);
        let sample: Vec<(f64, f64, f64)> = if census {
            counts.clone()
        } else if p.s("data") == "Points each time unit" {
            let step = tshow / n as f64;
            (0..=(tshow.floor() as usize)).map(|t| data[((t as f64 / step).round() as usize).min(data.len() - 1)]).collect()
        } else {
            data.clone()
        };
        let draw_lines = !census || p.b("census_join") || !p.0.contains_key("census_join");
        if draw_lines {
            if show_prey { d.polyline(&sample.iter().map(|v| a.pt((v.0, v.1))).collect::<Vec<_>>(), cprey, if census { lw * 0.6 } else { lw }); }
            if show_pred { dashed(&mut d, &sample.iter().map(|v| ap.pt((v.0, v.2))).collect::<Vec<_>>(), cpred, if census { lw * 0.6 } else { lw }, if census { "5 4" } else { dash_pred }); }
        }
        if p.s("data") == "Points each time unit" {
            for (f, on) in [(0usize, show_prey), (1, show_pred)] {
                if !on { continue; }
                for v in &sample {
                    let (cx, cy) = if f == 0 { a.pt((v.0, v.1)) } else { ap.pt((v.0, v.2)) };
                    d.circle(cx, cy, 3.2, if f == 0 { cprey } else { cpred }, "#fff", 0.8);
                }
            }
        }
        if census {
            for v in &sample {
                if show_prey { let (cx, cy) = a.pt((v.0, v.1)); d.circle(cx, cy, 4.0, cprey, "#fff", 0.8); }
                if show_pred { let (cx, cy) = ap.pt((v.0, v.2)); d.rect(cx - 4.5, cy - 4.5, 9.0, 9.0, "#fff", cpred, 1.8); }
            }
        }
        // peaks of the smooth model (used by peaks, lag and period marks)
        let pk_prey = peaks(&data, |v| v.1);
        let pk_pred = peaks(&data, |v| v.2);
        // with census counts, each peak is the highest count near the model's peak
        let half = cycle_est(&pk_prey).max(1e-6) / 4.0;
        let snap = |list: &Vec<(f64, f64)>, prey_side: bool| -> Vec<(f64, f64)> {
            if !census { return list.clone(); }
            list.iter().filter_map(|(t, _)| {
                sample.iter().filter(|v| (v.0 - t).abs() <= half)
                    .max_by(|x, y| { let (a1, b1) = if prey_side { (x.1, y.1) } else { (x.2, y.2) }; a1.partial_cmp(&b1).unwrap() })
                    .map(|v| (v.0, if prey_side { v.1 } else { v.2 }))
            }).collect()
        };
        let pk_prey = snap(&pk_prey, true);
        let pk_pred = snap(&pk_pred, false);
        let peak_at = |t: f64, prey_side: bool| -> (f64, f64) {
            if census {
                let s = sample.iter().min_by(|x, y| (x.0 - t).abs().partial_cmp(&(y.0 - t).abs()).unwrap()).copied().unwrap_or((t, 0.0, 0.0));
                if prey_side { a.pt((s.0, s.1)) } else { ap.pt((s.0, s.2)) }
            } else {
                let i = (((t / tshow) * n as f64).round() as usize).min(data.len() - 1);
                if prey_side { a.pt((t, data[i].1)) } else { ap.pt((t, data[i].2)) }
            }
        };
        if p.s("marks") == "Peaks" {
            for (on, col, list, side) in [(show_prey, cprey, &pk_prey, true), (show_pred, cpred, &pk_pred, false)] {
                if !on { continue; }
                for (t, _) in list.iter() {
                    let (cx, cy) = peak_at(*t, side);
                    dashed(&mut d, &[(cx, cy + 7.0), (cx, a.y + a.h)], INK, 1.2, "2 3");
                    if census { d.circle(cx, cy, 8.0, "none", INK, 1.6); } else { d.circle(cx, cy, 5.0, "#fff", col, 2.2); }
                }
            }
        }
        end_clip(&mut d);
        // lag and period arrows (drawn above the curves, inside the plot)
        let lab_mode = p.s("lag_labels");
        let unit1 = { let u = unit.to_lowercase(); u.trim_end_matches('s').to_string() };
        let fmt_len = |v: f64| -> String {
            let r = if v >= 10.0 { v.round() } else { (v * 10.0).round() / 10.0 };
            format!("{} {}", fnum(r), if (r - 1.0).abs() < 1e-9 { unit1.clone() } else { unit.to_lowercase() })
        };
        let arrow = |d: &mut Doc, t1: f64, t2: f64, yy: f64, name: &str, len: f64| {
            let (xa, xb) = (a.px(t1), a.px(t2));
            dashed(d, &[(xa, yy - 6.0), (xa, a.y + a.h)], INK, 1.0, "2 3");
            dashed(d, &[(xb, yy - 6.0), (xb, a.y + a.h)], INK, 1.0, "2 3");
            let (lo, hi) = (xa.min(xb), xa.max(xb));
            d.line(lo, yy, hi, yy, INK, 1.4);
            for (x, s) in [(lo, 1.0), (hi, -1.0)] {
                d.poly(&[(x, yy), (x + s * 8.0, yy - 4.0), (x + s * 8.0, yy + 4.0)], INK, INK, 1.0);
            }
            let label = match lab_mode { "Name only" => name.to_string(), "Blank for students" => format!("{name}: ________"), _ => format!("{name}: {}", fmt_len(len)) };
            let lw = label.chars().count() as f64 * 7.5;
            let cx = ((lo + hi) / 2.0).clamp(a.x + lw / 2.0 + 4.0, a.x + a.w - lw / 2.0 - 4.0);
            d.raw(&format!(r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="17" fill="#fff" opacity="0.85"/>"##, cx - lw / 2.0 - 3.0, yy - 22.0, lw + 6.0));
            bold(d, cx, yy - 8.0, 13.0, "middle", &label);
        };
        if p.b("mark_lag") && show_prey && show_pred {
            if let Some((tp, _)) = pk_prey.iter().find(|(t, _)| pk_pred.iter().any(|(q, _)| q > t)) {
                let tq = pk_pred.iter().find(|(q, _)| q > tp).map(|v| v.0).unwrap_or(*tp);
                if tq <= a.x1 { arrow(&mut d, *tp, tq, a.y + 36.0, "Lag", tq - tp); }
            }
        }
        if p.b("mark_period") {
            let list = if show_prey { &pk_prey } else { &pk_pred };
            if list.len() >= 2 {
                let i = if list.len() >= 3 { 1 } else { 0 };
                let (t1, t2) = (list[i].0, list[i + 1].0);
                if t2 <= a.x1 { arrow(&mut d, t1, t2, a.y + 64.0, "Period", t2 - t1); }
            }
        }
        let mut items = Vec::new();
        if show_prey { items.push((prey.clone(), cprey.to_string(), String::new())); }
        if show_pred { items.push((pred.clone(), cpred.to_string(), dash_pred.to_string())); }
        if p.b("legend") { legend(&mut d, ax_x + aw / 2.0, top + ah + 70.0, &items); }
    } else {
        let xmax = maxprey;
        let ymax = maxpred;
        let xs = nice_step(xmax * 1.05, 10.0);
        let ys = nice_step(ymax * 1.05, 10.0);
        let a = Axes { x: ax_x, y: top, w: aw, h: ah, x0: 0.0, x1: ((xmax * 1.05) / xs).ceil() * xs, y0: 0.0, y1: ((ymax * 1.05) / ys).ceil() * ys, xs, ys };
        let xt = if p.s("x_title").trim().is_empty() { format!("{prey} (number)") } else { p.s("x_title").trim().to_string() };
        let yt = if p.s("y_title").trim().is_empty() { format!("{pred} (number)") } else { ytitle.clone() };
        draw_axes_ex(&mut d, &a, minor, &xt, &yt, &opts);
        stash(10, &a);
        let pts: Vec<(f64, f64)> = data.iter().map(|v| a.pt((v.1, v.2))).collect();
        d.polyline(&pts, cprey, p.f("line_width").clamp(1.0, 7.0) * 0.87);
        if pts.len() > 40 {
            let (p0, p1) = (pts[20], pts[24]);
            d.arrow(p0.0, p0.1, p1.0 + (p1.0 - p0.0) * 2.0, p1.1 + (p1.1 - p0.1) * 2.0, cprey, 2.6);
        }
        let (sx, sy) = a.pt((x0, y0));
        d.circle(sx, sy, 5.0, INK, "#fff", 1.5);
        bold(&mut d, sx + 9.0, sy - 8.0, 12.0, "start", "start");
        bold(&mut d, ax_x + aw / 2.0, top + ah + 74.92, 14.7, "middle", "Time runs along the loop (arrow shows direction)");
    }
    d.finish()
}

/// "Start from an example" entries for predator-prey (reference gallery).
pub fn pp_examples() -> Vec<Example> {
    let st = |v: &str| Value::S(v.into());
    vec![
        Example { name: "Snowshoe hare and lynx population cycles", params: vec![("pair", st("Snowshoe hare and Canada lynx"))] },
        Example { name: "Hare and lynx census counts with peaks, lag and period", params: vec![
            ("pair", st("Snowshoe hare and Canada lynx")), ("data", st("Census counts")), ("marks", st("Peaks")),
            ("mark_lag", Value::B(true)), ("mark_period", Value::B(true)), ("y_axis", st("Predators on the right")),
            ("y_title", st("Hares (thousands)")),
        ] },
        Example { name: "Moose and wolf population cycles graph (Isle Royale)", params: vec![("pair", st("Moose and wolf (Isle Royale)")), ("tshow", Value::F(90.0))] },
        Example { name: "Predator-prey phase plane loop", params: vec![("pair", st("Snowshoe hare and Canada lynx")), ("graph", st("Predators against prey"))] },
        Example { name: "Lag and period worksheet (blank for students)", params: vec![
            ("pair", st("Snowshoe hare and Canada lynx")), ("marks", st("Peaks")), ("mark_lag", Value::B(true)), ("mark_period", Value::B(true)),
            ("lag_labels", st("Blank for students")), ("style", st("Black and white")),
        ] },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gp(over: &[(&str, Value)]) -> Params {
        let mut p = Params::defaults(&growth_params());
        for (k, v) in over { p.0.insert(k.to_string(), v.clone()); }
        p
    }

    #[test]
    fn sig3_formats_like_the_reference() {
        for (v, want) in [(16.83, "16.8"), (111.2, "111"), (6.72, "6.72"), (44.6, "44.6"), (0.6743, "0.674"), (0.03372, "0.0337"), (1.6004, "1.6"), (10.0, "10"), (7.7836, "7.78"), (11.78, "11.8"), (0.375, "0.375"), (99.96, "100"), (1500.0, "1500"), (12345.0, "12300"), (0.0, "0"), (-2.346, "-2.35")] {
            assert_eq!(sig3(v), want, "{v}");
        }
    }

    /// The readouts match the reference's numbers for its opening figure and each growth model.
    #[test]
    fn growth_readout_matches_the_reference() {
        let p = gp(&[]);
        assert_eq!(growth_readout(&p), vec![
            "Inflection point: N = K/2 = 500 at t = 7.78 years, growing fastest, 125 individuals per year".to_string(),
            "Lag phase until t = 3.94; stationary phase from t = 11.8".to_string(),
        ]);
        let p = gp(&[("organism", s("deer")), ("unit", s("Days")), ("k", Value::F(3000.0))]);
        assert_eq!(growth_readout(&p)[0], "Inflection point: N = K/2 = 1500 at t = 10 days, growing fastest, 375 deer per day");
        assert_eq!(growth_readout(&p)[1], "Lag phase until t = 6.06; stationary phase from t = 14");
        // starting above K/2 there is no inflection point
        assert!(growth_readout(&gp(&[("n0", Value::F(600.0))])).is_empty());
        assert_eq!(growth_readout(&gp(&[("growth", s("Exponential (J-curve)"))])), vec!["Doubles every 1.39 years".to_string()]);
        assert_eq!(growth_readout(&gp(&[("growth", s("Exponential and logistic"))])).len(), 3);
        assert_eq!(growth_readout(&gp(&[("growth", s("Exponential (J-curve)")), ("r", Value::F(0.6931))])), vec!["Doubles every 1 year".to_string()]);
        let over = |r: f64| growth_readout(&gp(&[("growth", s("Overshoot and oscillation")), ("r", Value::F(r)), ("tau", Value::F(2.5))]))[0].clone();
        assert_eq!(over(0.14), "rτ = 0.35: too small to overshoot K");
        assert_eq!(over(0.15), "rτ = 0.375: overshoots K, the swings dying away");
        assert_eq!(over(0.62), "rτ = 1.55: overshoots K, the swings dying away");
        assert_eq!(over(0.64), "rτ = 1.6: swings around K that keep going");
        assert!(growth_readout(&gp(&[("growth", s("Boom and crash"))])).is_empty());
    }

    /// "Fit now" writes the fitted axes into the boxes: with fitting off the graph then draws
    /// exactly as the fitted one, for one- and two-graph layouts.
    #[test]
    fn growth_fit_now_freezes_the_fitted_axes() {
        for over in [
            vec![],
            vec![("k", Value::F(3000.0))],
            vec![("growth", s("Exponential (J-curve)")), ("tspan", Value::F(12.0))],
            vec![("graph", s("Population size and growth rate"))],
            vec![("graph", s("Growth rate and per capita rate")), ("growth", s("Overshoot and oscillation"))],
            vec![("graph", s("Both rates against population size"))],
        ] {
            let mut p = gp(&over);
            p.set_b("fit", true);
            let fitted = growth_render(&p);
            let mut q = growth_fit_now(&p);
            assert!(q.b("fit"), "Fit now leaves fitting as it was");
            q.set_b("fit", false);
            assert_eq!(growth_render(&q), fitted, "{over:?}");
        }
        // with fitting off, Fit now catches the axes up with a changed K
        let mut p = gp(&[("fit", Value::B(false)), ("k", Value::F(3000.0))]);
        assert_eq!(p.f("y_max"), 1100.0);
        p = growth_fit_now(&p);
        assert!(!p.b("fit"));
        assert_eq!((p.f("y_min"), p.f("y_max"), p.f("y_step")), (0.0, 3500.0, 500.0));
        assert_eq!((p.f("x_min"), p.f("x_max")), (0.0, 20.0));
    }

    fn pp(over: &[(&str, Value)]) -> Params {
        let mut p = Params::defaults(&pp_params());
        for (k, v) in over { p.0.insert(k.to_string(), v.clone()); }
        p
    }

    /// The predator-prey readout matches the reference for the hare and lynx opening figure.
    #[test]
    fn pp_readout_matches_the_reference() {
        let r = pp_readout(&pp(&[]));
        assert_eq!(r[0], "A cycle every 10 years; the predators peak 1.6 years after the prey");
        assert_eq!(r[1], "Hares: 16.8 to 111, averaging 50");
        assert_eq!(r[2], "Lynx: 6.72 to 44.6, averaging 20");
        assert_eq!(r[3], "α = 0.674, β = 0.0337, γ = 0.674, δ = 0.0135");
        // model rates typed in: no rates line, averages from the rates
        let r = pp_readout(&pp(&[("source", s("Model rates (α β γ δ)")), ("alpha", Value::F(0.674)), ("beta", Value::F(0.0337)), ("gamma", Value::F(0.674)), ("delta", Value::F(0.0135))]));
        assert_eq!(r.len(), 3);
        assert_eq!(r[0], "A cycle every 10 years; the predators peak 1.6 years after the prey");
        assert_eq!(r[1], "Hares: 16.8 to 111, averaging 49.9");
        // a preset pair uses its own names, units and cycle
        let r = pp_readout(&pp(&[("pair", s("Aphid and ladybug"))]));
        assert!(r[0].starts_with("A cycle every 20 days"), "{}", r[0]);
        assert!(r[1].starts_with("Aphids: ") && r[2].starts_with("Ladybugs: "), "{r:?}");
        // a short cycle in one unit reads "1 year"
        assert!(pp_readout(&pp(&[("pair", s("Your own pair")), ("cycle", Value::F(1.0))]))[0].starts_with("A cycle every 1 year;"));
    }

    /// Turning an axis's fit off keeps the fitted range in its boxes (no jump).
    #[test]
    fn pp_unfit_keeps_the_axes() {
        let p = pp(&[]);
        let before = pp_render(&p);
        let q = pp_unfit(&p, true);
        assert!(!q.b("x_fit") && q.b("y_fit"));
        let q = pp_unfit(&q, false);
        assert!(!q.b("y_fit"));
        assert_eq!((q.f("x_min"), q.f("x_max"), q.f("x_step")), (0.0, 40.0, 2.0));
        assert_eq!((q.f("y_min"), q.f("y_max"), q.f("y_step")), (0.0, 120.0, 10.0));
        assert_eq!(pp_render(&q), before);
    }

    #[test]
    fn growth_variants() {
        render_case("population-growth", "default", &[]);
        render_case("population-growth", "both", &[
            ("growth", s("Exponential and logistic")), ("graph", s("Population size and growth rate")),
            ("mark_infl", Value::B(true)), ("census", s("0:20, 4:150, 8:520, 12:900")), ("organism", s("deer")),
            ("minor", s("4 per block")), ("title", s("Deer on an island")),
        ]);
        render_case("population-growth", "overshoot", &[("growth", s("Overshoot and oscillation")), ("graph", s("Both rates against population size")), ("curve_color", s("Red"))]);
        render_case("population-growth", "boom", &[("growth", s("Boom and crash")), ("graph", s("Growth rate and per capita rate")), ("k", Value::F(5000.0))]);
        render_case("population-growth", "blank", &[("blank", Value::B(true)), ("fit", Value::B(false)), ("x_max", Value::F(10.0)), ("y_max", Value::F(500.0))]);
        render_case("population-growth", "parity", &[
            ("census_auto", Value::B(true)), ("census_scatter", s("About 10%")), ("census_table", Value::B(true)), ("mark_infl", Value::B(true)),
            ("mark_phases", Value::B(true)), ("x_end", s("Text")), ("y_end", s("Text")), ("x_hi_end", s("Triangle arrow")), ("y_hi_end", s("Line arrow")),
            ("title_style", s("Blank line")), ("xtitle_style", s("Blank line")), ("ytitle_style", s("None")), ("k_label_style", s("Blank line")),
            ("x_lo_end", s("Circle")), ("y_lo_end", s("Circle")),
        ]);
        for ex in growth_examples() { render_case("population-growth", "example", &ex.params); }
        let a = render_case("population-growth", "k500", &[("k", Value::F(500.0))]);
        let b = render_case("population-growth", "k2000", &[("k", Value::F(2000.0))]);
        assert_ne!(a, b);
    }

    #[test]
    fn predprey_variants() {
        render_case("predator-prey", "default", &[]);
        render_case("predator-prey", "foxes", &[("pair", s("Rabbit and red fox")), ("marks", s("Peaks")), ("data", s("Points each time unit")), ("tshow", Value::F(30.0)), ("minor", s("2 per block"))]);
        render_case("predator-prey", "phase", &[("graph", s("Predators against prey")), ("style", s("Black and white"))]);
        render_case("predator-prey", "rates", &[("source", s("Model rates (α β γ δ)")), ("show", s("Prey only"))]);
        render_case("predator-prey", "parity", &[("data", s("Census counts")), ("marks", s("Peaks")), ("mark_lag", Value::B(true)),
            ("mark_period", Value::B(true)), ("y_axis", s("Predators on the right")), ("title_style", s("Blank line")),
            ("x_fit", Value::B(false)), ("x_max", Value::F(30.0)), ("x_step", Value::F(5.0)), ("y_fit", Value::B(false)), ("y_max", Value::F(150.0))]);
        for ex in pp_examples() { render_case("predator-prey", "example", &ex.params); }
        for pr in PAIRS { render_case("predator-prey", "pair", &[("pair", s(pr))]); }
        // cycle length is honoured: prey peaks should be ~cycle apart
        let sim = lv(1.0, 1.0 / 20.0, 1.0, 1.0 / 50.0, 20.0, 12.0, 60.0, 6000);
        assert!(local_max_times(&sim, |t| t.1).len() >= 2);
    }
}
