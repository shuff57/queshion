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
    LAST.with(|l| l.borrow_mut().push(AxInfo { kind, x: a.x, y: a.y, w: a.w, h: a.h, x0: a.x0, x1: a.x1, y0: a.y0, y1: a.y1 }));
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
        _ => "#2563eb",
    }
}

fn minor_of(s: &str) -> i64 {
    match s { "2 per block" => 2, "4 per block" => 4, "5 per block" => 5, _ => 1 }
}

// ------------------------------------------------------------- population growth

pub fn growth_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("growth", "Growth", &["Logistic (S-curve)", "Exponential (J-curve)", "Exponential and logistic", "Overshoot and oscillation", "Boom and crash"], "Logistic (S-curve)"),
        ParamDef::float("n0", "Starting size, N0", 1.0, 1_000_000.0, 1.0, 20.0),
        ParamDef::float("r", "Growth rate, r (per time unit)", 0.01, 5.0, 0.05, 0.5),
        ParamDef::float("k", "Carrying capacity, K", 10.0, 1_000_000.0, 10.0, 1000.0),
        ParamDef::float("tspan", "Time span", 1.0, 1000.0, 1.0, 20.0),
        ParamDef::choice("unit", "Time in", &["Hours", "Days", "Weeks", "Months", "Years", "Generations"], "Years"),
        ParamDef::text("organism", "Organism plural, for axis titles (e.g. deer)", ""),
        ParamDef::choice("graph", "Graph", &[
            "Population size over time", "Growth rate over time", "Per capita growth rate over time",
            "Population size and growth rate", "Population size and per capita rate", "Growth rate and per capita rate",
            "Growth rate against population size", "Per capita rate against population size", "Both rates against population size",
        ], "Population size over time"),
        ParamDef::choice("curve_color", "Curve color", &["Blue", "Red", "Green", "Black"], "Blue"),
        ParamDef::list("census", "Census data points (time:size)", ""),
        ParamDef::bool("mark_k", "Mark the carrying capacity", true),
        ParamDef::bool("mark_infl", "Mark the inflection point (logistic)", false),
        ParamDef::text("x_title", "x-axis title (blank = automatic)", ""),
        ParamDef::text("y_title", "y-axis title (blank = automatic)", ""),
        ParamDef::bool("fit", "Fit the axes automatically", true),
        ParamDef::float("x_max", "x-axis maximum (when not fitting)", 1.0, 100_000.0, 1.0, 20.0),
        ParamDef::float("y_max", "y-axis maximum (when not fitting, first panel)", 1.0, 10_000_000.0, 10.0, 1100.0),
        ParamDef::choice("minor", "Minor gridlines", &["None", "2 per block", "4 per block", "5 per block"], "None"),
        ParamDef::bool("blank", "Blank graph (axes only, for students to draw on)", false),
        ParamDef::text("title", "Chart title (blank = none)", ""),
        ParamDef::float("curve_width", "Curve thickness", 1.0, 7.0, 0.5, 3.0),
        ParamDef::text("k_label", "Label on the carrying-capacity line", "Carrying capacity (K)"),
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

fn model(mode: &str, n0: f64, r: f64, k: f64, tspan: f64, col: &str) -> Vec<Series> {
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
            let tau = 1.3 / r;
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
    let panels = panels_of(p.s("graph"));
    let series = model(&mode, n0, r, k, tspan, col);
    let ts: Vec<f64> = (0..NPTS).map(|i| tspan * i as f64 / (NPTS - 1) as f64).collect();
    let rates: Vec<Vec<f64>> = series.iter().map(|s| derivative(&ts, &s.ys)).collect();
    let pers: Vec<Vec<f64>> = series.iter().zip(&rates).map(|(s, r)| s.ys.iter().zip(r).map(|(n, d)| if *n > 1e-9 { d / n } else { 0.0 }).collect()).collect();
    let has_k = mode != "Exponential (J-curve)";
    let census = parse_points(p.s("census"));
    let title = p.s("title").trim().to_string();

    let (w, x0, aw) = (660.0, 112.0, 510.0);
    let single = panels.len() == 1;
    let ah = if single { 300.0 } else { 205.0 };
    let top = 28.0 + if title.is_empty() { 0.0 } else { 34.0 };
    let stride = ah + 82.0;
    let legend_h = if series.len() > 1 { 34.0 } else { 0.0 };
    let h = top + stride * panels.len() as f64 + legend_h + 6.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() { bold(&mut d, w / 2.0, 32.0, 19.0, "middle", &title); }

    let tx = if p.s("x_title").trim().is_empty() { format!("Time ({unit})") } else { p.s("x_title").trim().to_string() };
    let noun = if org.is_empty() { String::new() } else { org.clone() };

    for (pi, pk) in panels.iter().enumerate() {
        let y0 = top + stride * pi as f64;
        // values to plot
        let ydata = |si: usize| -> &Vec<f64> {
            match pk { Panel::TimeN => &series[si].ys, Panel::TimeRate | Panel::NRate => &rates[si], _ => &pers[si] }
        };
        let is_time = matches!(pk, Panel::TimeN | Panel::TimeRate | Panel::TimePer);
        // fit
        let (mut ymin, mut ymax) = (0.0f64, 0.0f64);
        let mut xmax_n = 0.0f64;
        for (si, s) in series.iter().enumerate() {
            if !s.fit { continue; }
            for (i, v) in ydata(si).iter().enumerate() {
                ymax = ymax.max(*v);
                ymin = ymin.min(*v);
                xmax_n = xmax_n.max(s.ys[i]);
            }
        }
        if ymax <= 0.0 { ymax = 1.0; }
        if matches!(pk, Panel::TimeN) && has_k { ymax = ymax.max(k); }
        if matches!(pk, Panel::NRate | Panel::NPer) && has_k { xmax_n = xmax_n.max(k); }
        let fit = p.b("fit");
        let ycap = |ymax: f64, ymin: f64| -> (f64, f64, f64) {
            let step = nice_step((ymax - ymin) * 1.05, 10.0);
            let top = ((ymax * 1.05) / step).ceil() * step;
            let bot = if ymin < -1e-9 { (ymin * 1.05 / step).floor() * step } else { 0.0 };
            (bot, top.max(step), step)
        };
        let (mut yb, mut yt, mut ys) = ycap(ymax, ymin);
        let xrange = if is_time { if fit { tspan } else { p.f("x_max").max(1e-6) } } else { xmax_n.max(1e-6) };
        if pi == 0 && !fit && matches!(pk, Panel::TimeN) {
            yt = p.f("y_max").max(1e-6);
            ys = nice_step(yt, 10.0);
            yt = (yt / ys).ceil() * ys;
            yb = 0.0;
        }
        let xs = nice_step(xrange, 10.0);
        let x1 = (xrange / xs).ceil() * xs;
        let a = Axes { x: x0, y: y0, w: aw, h: ah, x0: 0.0, x1, y0: yb, y1: yt, xs, ys };

        let ytitle_auto = match pk {
            Panel::TimeN => if noun.is_empty() { "Population size (N)".to_string() } else { format!("Number of {noun} (N)") },
            Panel::TimeRate | Panel::NRate => format!("Growth rate, dN/dt (per {unit1})"),
            _ => format!("Per capita growth rate (per {unit1})"),
        };
        let yt_text = if pi == 0 && !p.s("y_title").trim().is_empty() { p.s("y_title").trim().to_string() } else { ytitle_auto };
        let xt_text = if is_time { tx.clone() } else if noun.is_empty() { "Population size (N)".to_string() } else { format!("Number of {noun} (N)") };
        draw_axes(&mut d, &a, minor_of(p.s("minor")), &xt_text, &yt_text);
        stash(match pk { Panel::TimeN => 0, Panel::TimeRate => 1, Panel::TimePer => 2, Panel::NRate => 3, Panel::NPer => 4 }, &a);
        if blank { continue; }

        let clip = format!("clip{pi}");
        begin_clip(&mut d, &a, &clip);
        if has_k && matches!(pk, Panel::TimeN) && p.b("mark_k") {
            dashed(&mut d, &[(a.px(0.0), a.py(k)), (a.px(a.x1), a.py(k))], INK, 1.6, "7 5");
        }
        if has_k && matches!(pk, Panel::NRate | Panel::NPer) && p.b("mark_k") {
            dashed(&mut d, &[(a.px(k), a.py(a.y0)), (a.px(k), a.py(a.y1))], INK, 1.6, "7 5");
        }
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
        if matches!(pk, Panel::TimeN) {
            for (t, n) in &census {
                let (cx, cy) = a.pt((*t, *n));
                d.circle(cx, cy, 4.2, INK, "#fff", 1.0);
            }
        }
        if p.b("mark_infl") && mode.starts_with("Logistic") && n0 < k / 2.0 {
            let ti = (k - n0) / n0;
            let t_inf = ti.ln() / r;
            match pk {
                Panel::TimeN => { let (cx, cy) = a.pt((t_inf, k / 2.0)); d.circle(cx, cy, 5.0, "#fff", INK, 2.0); }
                Panel::TimeRate => { let (cx, cy) = a.pt((t_inf, r * k / 4.0)); d.circle(cx, cy, 5.0, "#fff", INK, 2.0); }
                Panel::NRate => { let (cx, cy) = a.pt((k / 2.0, r * k / 4.0)); d.circle(cx, cy, 5.0, "#fff", INK, 2.0); }
                _ => {}
            }
        }
        end_clip(&mut d);
        if has_k && matches!(pk, Panel::TimeN) && p.b("mark_k") {
            bold(&mut d, a.x + 8.0, a.py(k) + 17.0, 13.0, "start", if p.s("k_label").trim().is_empty() { "Carrying capacity (K)" } else { p.s("k_label").trim() });
        }
        if has_k && matches!(pk, Panel::NRate | Panel::NPer) && p.b("mark_k") {
            bold(&mut d, a.px(k) - 5.0, a.y + 16.0, 13.0, "end", "K");
        }
        if p.b("mark_infl") && mode.starts_with("Logistic") && n0 < k / 2.0 && matches!(pk, Panel::TimeN) {
            let t_inf = ((k - n0) / n0).ln() / r;
            let (cx, cy) = a.pt((t_inf, k / 2.0));
            bold(&mut d, (cx + 12.0).min(a.x + a.w - 110.0), cy + 22.0, 13.0, "start", "Inflection (K/2)");
        }
    }
    if series.len() > 1 && !blank {
        let items: Vec<(String, String, String)> = series.iter().map(|s| (s.name.clone(), s.color.clone(), s.dash.replace(' ', ","))).collect();
        let items: Vec<(String, String, String)> = items.into_iter().map(|(n, c, dsh)| (n, c, if dsh.is_empty() { String::new() } else { "8 5".into() })).collect();
        legend(&mut d, x0 + aw / 2.0, h - 20.0, &items);
    }
    d.finish()
}

// ---------------------------------------------------------------- predator-prey

const PAIRS: [&str; 5] = [
    "Custom (use the fields below)",
    "Snowshoe hare and Canada lynx",
    "Rabbit and red fox",
    "Mouse and owl",
    "Aphid and ladybug",
];

pub fn pp_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("pair", "Pair (presets override names, averages and cycle)", &PAIRS, PAIRS[0]),
        ParamDef::text("prey", "Prey", "Hares"),
        ParamDef::text("pred", "Predators", "Lynx"),
        ParamDef::choice("source", "Work the cycle out from", &["Populations", "Model rates"], "Populations"),
        ParamDef::float("prey0", "Prey at the start", 0.1, 1_000_000.0, 1.0, 20.0),
        ParamDef::float("pred0", "Predators at the start", 0.1, 1_000_000.0, 1.0, 12.0),
        ParamDef::float("prey_avg", "Average prey", 1.0, 1_000_000.0, 1.0, 50.0),
        ParamDef::float("pred_avg", "Average predators", 1.0, 1_000_000.0, 1.0, 20.0),
        ParamDef::float("cycle", "Cycle length (time units)", 0.5, 500.0, 0.5, 10.0),
        ParamDef::float("alpha", "Model: prey growth, alpha", 0.01, 5.0, 0.01, 0.67),
        ParamDef::float("beta", "Model: predation, beta", 0.0001, 1.0, 0.0001, 0.0337),
        ParamDef::float("gamma", "Model: predator death, gamma", 0.01, 5.0, 0.01, 0.67),
        ParamDef::float("delta", "Model: predator gain, delta", 0.0001, 1.0, 0.0001, 0.0135),
        ParamDef::float("tshow", "Time shown", 1.0, 1000.0, 1.0, 40.0),
        ParamDef::choice("unit", "Units", &["Years", "Months", "Weeks", "Days"], "Years"),
        ParamDef::choice("graph", "Graph", &["Over time", "Predators against prey"], "Over time"),
        ParamDef::choice("show", "Show", &["Both", "Prey only", "Predators only"], "Both"),
        ParamDef::choice("style", "Style", &["Color", "Black and white"], "Color"),
        ParamDef::choice("data", "Data", &["Smooth curves", "Points each time unit"], "Smooth curves"),
        ParamDef::choice("marks", "Marked", &["Nothing", "Peaks"], "Nothing"),
        ParamDef::text("x_title", "x-axis title (blank = automatic)", ""),
        ParamDef::text("y_title", "y-axis title (blank = automatic)", ""),
        ParamDef::choice("minor", "Minor gridlines", &["None", "2 per block", "4 per block", "5 per block"], "None"),
        ParamDef::text("title", "Chart title (blank = none)", ""),
        ParamDef::float("line_width", "Line thickness", 1.0, 7.0, 0.5, 3.0),
        ParamDef::bool("legend", "Legend", true),
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
        _ => None,
    }
}

pub fn pp_render(p: &Params) -> String {
    let (mut prey, mut pred) = (p.s("prey").trim().to_string(), p.s("pred").trim().to_string());
    let (mut pavg, mut qavg, mut cycle) = (p.f("prey_avg").max(1.0), p.f("pred_avg").max(1.0), p.f("cycle").max(0.1));
    let mut unit = p.s("unit").to_string();
    let mut ytitle = p.s("y_title").trim().to_string();
    if let Some((a, b, c, dd, e, u, yt)) = preset(p.s("pair")) {
        prey = a.into(); pred = b.into(); pavg = c; qavg = dd; cycle = e; unit = u.into();
        if ytitle.is_empty() { ytitle = yt.into(); }
    }
    let (x0, y0) = if preset(p.s("pair")).is_some() { (pavg * 0.4, qavg * 0.6) } else { (p.f("prey0").max(0.01), p.f("pred0").max(0.01)) };
    if prey.is_empty() { prey = "Prey".into(); }
    if pred.is_empty() { pred = "Predators".into(); }
    let tshow = p.f("tshow").max(0.1);
    let (alpha, beta, gamma, delta);
    if p.s("source") == "Model rates" {
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
    let n = 800usize;
    let data = lv(alpha, beta, gamma, delta, x0, y0, tshow, n);
    let (cprey, cpred, dash_pred) = if p.s("style") == "Black and white" { (INK, INK, "9 6") } else { ("#2563eb", "#dc2626", "9 6") };
    let show_prey = p.s("show") != "Predators only";
    let show_pred = p.s("show") != "Prey only";
    let phase = p.s("graph") == "Predators against prey";
    let title = p.s("title").trim().to_string();

    let (w, ax_x, aw, ah) = (680.0, 112.0, 530.0, 360.0);
    let top = 28.0 + if title.is_empty() { 0.0 } else { 34.0 };
    let h = top + ah + 128.0;
    let mut d = Doc::new(w, h);
    if !title.is_empty() { bold(&mut d, w / 2.0, 32.0, 19.0, "middle", &title); }

    let maxprey = data.iter().map(|v| v.1).fold(0.0, f64::max);
    let maxpred = data.iter().map(|v| v.2).fold(0.0, f64::max);
    let vmax = match (show_prey, show_pred) { (true, true) => maxprey.max(maxpred), (true, false) => maxprey, _ => maxpred };
    let minor = minor_of(p.s("minor"));
    let xt_auto = format!("Time ({})", unit.to_lowercase());
    let (ya, yb, name_y) = if ytitle.is_empty() { ("Number of animals".to_string(), 0, 0) } else { (ytitle.clone(), 0, 0) };
    let _ = (yb, name_y);

    if !phase {
        let ys = nice_step(vmax * 1.05, 10.0);
        let yt = ((vmax * 1.05) / ys).ceil() * ys;
        let xs = nice_step(tshow, 8.0);
        let x1 = (tshow / xs).ceil() * xs;
        let a = Axes { x: ax_x, y: top, w: aw, h: ah, x0: 0.0, x1, y0: 0.0, y1: yt, xs, ys };
        let xt = if p.s("x_title").trim().is_empty() { xt_auto } else { p.s("x_title").trim().to_string() };
        draw_axes(&mut d, &a, minor, &xt, &ya);
        stash(9, &a);
        begin_clip(&mut d, &a, "ppclip");
        let sample: Vec<(f64, f64, f64)> = if p.s("data") == "Points each time unit" {
            let step = tshow / n as f64;
            (0..=(tshow.floor() as usize)).map(|t| data[((t as f64 / step).round() as usize).min(data.len() - 1)]).collect()
        } else {
            data.clone()
        };
        let pts = |f: fn(&(f64, f64, f64)) -> f64| -> Vec<(f64, f64)> { sample.iter().map(|v| a.pt((v.0, f(v)))).collect() };
        let lw = p.f("line_width").clamp(1.0, 7.0);
        if show_prey { d.polyline(&pts(|v| v.1), cprey, lw); }
        if show_pred { dashed(&mut d, &pts(|v| v.2), cpred, lw, dash_pred); }
        if p.s("data") == "Points each time unit" {
            for (f, on) in [(0usize, show_prey), (1, show_pred)] {
                if !on { continue; }
                for v in &sample {
                    let (cx, cy) = a.pt((v.0, if f == 0 { v.1 } else { v.2 }));
                    d.circle(cx, cy, 3.2, if f == 0 { cprey } else { cpred }, "#fff", 0.8);
                }
            }
        }
        if p.s("marks") == "Peaks" {
            for (on, col, pick) in [(show_prey, cprey, (|v: &(f64, f64, f64)| v.1) as fn(&(f64, f64, f64)) -> f64), (show_pred, cpred, |v: &(f64, f64, f64)| v.2)] {
                if !on { continue; }
                for (t, v) in local_max_times(&sample, pick) {
                    let (cx, cy) = a.pt((t, v));
                    d.circle(cx, cy, 5.0, "#fff", col, 2.2);
                }
            }
        }
        end_clip(&mut d);
        let mut items = Vec::new();
        if show_prey { items.push((prey.clone(), cprey.to_string(), String::new())); }
        if show_pred { items.push((pred.clone(), cpred.to_string(), dash_pred.to_string())); }
        if p.b("legend") { legend(&mut d, ax_x + aw / 2.0, top + ah + 90.0, &items); }
    } else {
        let xmax = maxprey;
        let ymax = maxpred;
        let xs = nice_step(xmax * 1.05, 10.0);
        let ys = nice_step(ymax * 1.05, 10.0);
        let a = Axes { x: ax_x, y: top, w: aw, h: ah, x0: 0.0, x1: ((xmax * 1.05) / xs).ceil() * xs, y0: 0.0, y1: ((ymax * 1.05) / ys).ceil() * ys, xs, ys };
        let xt = if p.s("x_title").trim().is_empty() { format!("{prey} (number)") } else { p.s("x_title").trim().to_string() };
        let yt = if p.s("y_title").trim().is_empty() { format!("{pred} (number)") } else { ytitle.clone() };
        draw_axes(&mut d, &a, minor, &xt, &yt);
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
        bold(&mut d, ax_x + aw / 2.0, top + ah + 90.0, 13.0, "middle", "Time runs along the loop (arrow shows direction)");
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let a = render_case("population-growth", "k500", &[("k", Value::F(500.0))]);
        let b = render_case("population-growth", "k2000", &[("k", Value::F(2000.0))]);
        assert_ne!(a, b);
    }

    #[test]
    fn predprey_variants() {
        render_case("predator-prey", "default", &[]);
        render_case("predator-prey", "foxes", &[("pair", s("Rabbit and red fox")), ("marks", s("Peaks")), ("data", s("Points each time unit")), ("tshow", Value::F(30.0)), ("minor", s("2 per block"))]);
        render_case("predator-prey", "phase", &[("graph", s("Predators against prey")), ("style", s("Black and white"))]);
        render_case("predator-prey", "rates", &[("source", s("Model rates")), ("show", s("Prey only"))]);
        // cycle length is honoured: prey peaks should be ~cycle apart
        let sim = lv(1.0, 1.0 / 20.0, 1.0, 1.0 / 50.0, 20.0, 12.0, 60.0, 6000);
        assert!(local_max_times(&sim, |t| t.1).len() >= 2);
    }
}
