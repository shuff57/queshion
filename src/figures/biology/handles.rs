//! Direct-manipulation handles for the biology figures (see `figures::interact`). Geometry is
//! read back from each renderer's last layout so handles sit exactly on the drawn elements.
use super::{gel, pipette, popdyn, punnett};
use crate::figures::interact::{Handle, Pt};
use crate::figures::Params;

pub fn handles(id: &str, p: &Params, size: Pt) -> Option<Vec<Handle>> {
    Some(match id {
        "population-growth" => growth(p, size),
        "predator-prey" => predprey(p, size),
        "punnett-square" => punnett_h(p, size),
        "pedigree" => pedigree_h(p, size),
        "gel-electrophoresis" => gel_h(p, size),
        "micropipette-reading" => pipette_h(p, size),
        _ => return None,
    })
}

fn title_zone(size: Pt, y: f64) -> Handle {
    Handle::label((size.0 / 2.0, y), (260.0, 26.0), "title", "Double-click to edit the title")
}

// ---------------------------------------------------------------- population growth

/// Census points as (time, size) in the order the renderer parses them.
fn census(p: &Params) -> Vec<(f64, f64)> {
    p.s("census")
        .split([',', ';', '\n'])
        .filter_map(|t| {
            let (a, b) = t.split_once(':').or_else(|| t.split_once(' '))?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .collect()
}

fn set_census(p: &mut Params, idx: usize, v: (f64, f64)) {
    let mut pts = census(p);
    if let Some(slot) = pts.get_mut(idx) {
        *slot = v;
    }
    let s: Vec<String> = pts.iter().map(|(t, n)| format!("{}:{}", fmt(*t), fmt(*n))).collect();
    p.set_s("census", &s.join(", "));
}

fn fmt(v: f64) -> String {
    if v.abs() >= 100.0 { format!("{}", v.round() as i64) } else { format!("{}", (v * 10.0).round() / 10.0) }
}

fn growth(p: &Params, size: Pt) -> Vec<Handle> {
    let mut hs = vec![title_zone(size, 22.0)];
    let axes = popdyn::axes_of(p, false);
    if let Some(a) = axes.last() {
        hs.push(Handle::label((a.x + a.w / 2.0, a.y + a.h + 44.0), (200.0, 22.0), "x_title", "Double-click to edit the x-axis title"));
    }
    if let Some(a0) = axes.first() {
        hs.push(Handle::label((a0.x - 52.0, a0.y + a0.h / 2.0), (22.0, 150.0), "y_title", "Double-click to edit the y-axis title"));
    }
    if p.b("blank") {
        return hs;
    }
    let Some(a) = axes.iter().find(|a| a.kind == 0).copied() else { return hs };
    let mode = p.s("growth").to_string();
    let (n0, r, k, tspan) = (p.f("n0"), p.f("r"), p.f("k"), p.f("tspan"));
    let has_k = mode != "Exponential (J-curve)";
    if a.inside((0.0, n0)) {
        hs.push(Handle::diamond((a.px(0.0), a.py(n0)), "Drag to set the starting size N0", move |q, t| {
            q.set_f("n0", a.vy(t.1).max(1.0));
        }).with_cursor(crate::figures::interact::Cursor::ResizeV));
    }
    if has_k && a.inside((0.0, k)) {
        let x = a.px(a.x1 * 0.9);
        hs.push(Handle::square((x, a.py(k)), "Drag up or down to set the carrying capacity K", move |q, t| {
            q.set_f("k", a.vy(t.1).max(10.0));
        }).with_cursor(crate::figures::interact::Cursor::ResizeV));
    }
    if matches!(mode.as_str(), "Logistic (S-curve)" | "Exponential (J-curve)" | "Exponential and logistic") && n0 < k {
        let tm = tspan * 0.4;
        let pt = (tm, popdyn::growth_value(&mode, n0, r, k, tm));
        if a.inside(pt) {
            let m2 = mode.clone();
            hs.push(Handle::knob((a.px(pt.0), a.py(pt.1)), "Drag to change the growth rate r", move |q, t| {
                let target = a.vy(t.1);
                let cur = popdyn::growth_value(&m2, q.f("n0"), q.f("r"), q.f("k"), tm);
                if (a.py(cur) - t.1).abs() < 0.5 {
                    return;
                }
                q.set_f("r", popdyn::growth_r_for(&m2, q.f("n0"), q.f("k"), tm, target));
            }));
        }
    }
    for (i, (t, n)) in census(p).into_iter().enumerate() {
        if a.inside((t, n)) {
            hs.push(Handle::knob((a.px(t), a.py(n)), "Drag this census point", move |q, to| {
                set_census(q, i, (a.vx(to.0).max(0.0), a.vy(to.1).max(0.0)));
            }));
        }
    }
    if has_k && p.b("mark_k") {
        hs.push(Handle::label((a.x + 90.0, a.py(k) + 12.0), (150.0, 18.0), "k_label", "Double-click to rename the K line"));
    }
    hs
}

// ---------------------------------------------------------------- predator-prey

fn predprey(p: &Params, size: Pt) -> Vec<Handle> {
    let mut hs = vec![title_zone(size, 22.0)];
    let axes = popdyn::axes_of(p, true);
    let Some(a) = axes.first().copied() else { return hs };
    hs.push(Handle::label((a.x + a.w / 2.0, a.y + a.h + 44.0), (200.0, 22.0), "x_title", "Double-click to edit the x-axis title"));
    hs.push(Handle::label((a.x - 52.0, a.y + a.h / 2.0), (22.0, 150.0), "y_title", "Double-click to edit the y-axis title"));
    if !p.s("pair").starts_with("Custom") {
        return hs; // presets fix the start, averages and cycle
    }
    let (prey0, pred0) = (p.f("prey0"), p.f("pred0"));
    if a.kind == 10 {
        if a.inside((prey0, pred0)) {
            hs.push(Handle::knob((a.px(prey0), a.py(pred0)), "Drag the starting populations", move |q, t| {
                q.set_f("prey0", a.vx(t.0).max(0.1));
                q.set_f("pred0", a.vy(t.1).max(0.1));
            }));
        }
        return hs;
    }
    let show_prey = p.s("show") != "Predators only";
    let show_pred = p.s("show") != "Prey only";
    // predators may have their own scale on the right (kind 11)
    let ap = axes.iter().find(|x| x.kind == 11).copied().unwrap_or(a);
    if show_prey && a.inside((0.0, prey0)) {
        hs.push(Handle::knob((a.px(0.0), a.py(prey0)), "Drag to set the starting prey", move |q, t| {
            q.set_f("prey0", a.vy(t.1).max(0.1));
        }).with_cursor(crate::figures::interact::Cursor::ResizeV));
    }
    if show_pred && ap.inside((0.0, pred0)) {
        hs.push(Handle::diamond((ap.px(0.0), ap.py(pred0)), "Drag to set the starting predators", move |q, t| {
            q.set_f("pred0", ap.vy(t.1).max(0.1));
        }).with_cursor(crate::figures::interact::Cursor::ResizeV));
    }
    if p.s("source") == "Populations" {
        let xr = a.px(a.x1 * 0.94);
        let (pa, qa) = (p.f("prey_avg"), p.f("pred_avg"));
        if show_prey && a.inside((0.0, pa)) {
            hs.push(Handle::square((xr, a.py(pa)), "Drag to set the average prey", move |q, t| {
                q.set_f("prey_avg", a.vy(t.1).max(1.0));
            }).with_cursor(crate::figures::interact::Cursor::ResizeV));
        }
        if show_pred && ap.inside((0.0, qa)) {
            hs.push(Handle::square((xr - 22.0, ap.py(qa)), "Drag to set the average predators", move |q, t| {
                q.set_f("pred_avg", ap.vy(t.1).max(1.0));
            }).with_cursor(crate::figures::interact::Cursor::ResizeV));
        }
        let c = p.f("cycle");
        if c <= a.x1 {
            hs.push(Handle::bar((a.px(c), a.py(0.0)), false, "Drag along the time axis to set the cycle length", move |q, t| {
                q.set_f("cycle", a.vx(t.0).max(0.5));
            }));
        }
    }
    hs
}

// ---------------------------------------------------------------- punnett

fn punnett_h(p: &Params, size: Pt) -> Vec<Handle> {
    let mut hs = vec![title_zone(size, if p.s("title").trim().is_empty() { 14.0 } else { 40.0 })];
    let Some(l) = punnett::layout_of(p) else { return hs };
    let cross = punnett::cross_of(p);
    let genes = if cross == 1 { 2 } else { 1 };
    let top_c = (l.gx + l.cell * l.nc as f64 / 2.0, l.p1_y - 10.0);
    let side_c = (l.ox + 30.0, l.pb_y);
    for g in 0..genes {
        let (w, off) = match cross {
            1 => (36.0, (g as f64 - 0.5) * 36.0),
            2 => (64.0, 0.0),
            _ => (44.0, 0.0),
        };
        for (parent, c) in [(0usize, top_c), (1usize, side_c)] {
            hs.push(Handle::zone((c.0 + off, c.1), (w, 34.0), "Click to cycle this parent's genotype", move |q| {
                punnett::cycle_gene(q, parent, g)
            }));
        }
    }
    if p.s("cells") == "Some blank" && p.b("filled") {
        for j in 0..l.nr {
            for i in 0..l.nc {
                let c = (l.gx + l.cell * (i as f64 + 0.5), l.gy + l.cell * (j as f64 + 0.5));
                let idx = j * l.nc + i;
                hs.push(Handle::zone(c, (l.cell - 8.0, l.cell - 8.0), "Click to blank or fill this cell", move |q| punnett::toggle_blank_cell(q, idx)));
            }
        }
    }
    hs
}

// ---------------------------------------------------------------- pedigree

/// Only the title here: clicking a person selects them for the Family card's person panel, which
/// the workspace draws itself (`figuresui::pedigree_panel`) because selection is not a param.
fn pedigree_h(p: &Params, size: Pt) -> Vec<Handle> {
    vec![title_zone(size, if p.s("title").trim().is_empty() { 14.0 } else { 34.0 })]
}

// ---------------------------------------------------------------- gel

fn gel_h(p: &Params, size: Pt) -> Vec<Handle> {
    let mut hs = vec![title_zone(size, if p.s("title").trim().is_empty() { 12.0 } else { 32.0 })];
    let lay = gel::layout_of(p);
    for (li, lane) in lay.lanes.iter().enumerate() {
        if lane.ladder {
            continue;
        }
        for (bi, (bp, amt)) in lane.bands.iter().enumerate() {
            let pos = (gel::lane_x(&lay, li), gel::band_y(&lay, *bp));
            let l2 = lay.clone();
            let l3 = lay.clone();
            let amt = *amt;
            let h = Handle::bar(pos, true, "Drag up or down to change this band's fragment size; click to cycle its amount", move |q, t| {
                gel::edit_band(q, li, bi, |bp, a| {
                    if (gel::band_y(&l2, bp) - t.1).abs() < 1.0 { (bp, a) } else { (gel::bp_at(&l2, t.1), a) }
                });
            })
            .with_half(20.0, 5.0)
            .with_click(move |q| {
                let _ = &l3;
                gel::edit_band(q, li, bi, |bp, a| {
                    let next = if a < 0.9 { 1.0 } else if a < 1.5 { 2.0 } else { 0.5 };
                    (bp, next)
                });
            });
            let _ = amt;
            hs.push(h);
        }
    }
    hs
}

// ---------------------------------------------------------------- pipette

fn pipette_h(p: &Params, size: Pt) -> Vec<Handle> {
    let sp = pipette::spec(p.s("size"));
    let vol = p.f("volume").clamp(sp.min, sp.max);
    let (rail_x, rail_bot, rail_h) = (292.0, 470.0, 340.0);
    let y_of = move |v: f64| rail_bot - (v - sp.min) / (sp.max - sp.min) * rail_h;
    let (min, max, unit) = (sp.min, sp.max, sp.unit);
    let mut hs = vec![
        title_zone(size, 22.0),
        Handle::bar((rail_x, y_of(vol)), true, "Drag up or down to set the volume", move |q, t| {
            let cur = q.f("volume").clamp(min, max);
            if (y_of(cur) - t.1).abs() < 0.5 {
                return;
            }
            let f = ((rail_bot - t.1) / rail_h).clamp(0.0, 1.0);
            q.set_f("volume", ((min + f * (max - min)) / unit).round() * unit);
        }),
    ];
    // Each display digit: click adds 1 to that digit (wraps), scroll up/down steps it.
    for i in 0..3usize {
        let weight = unit * 10f64.powi(2 - i as i32);
        let step = move |q: &mut Params, n: f64| {
            let cur = q.f("volume").clamp(min, max);
            let mut v = cur + n * weight;
            if v > max + 1e-9 {
                v -= 10.0 * weight;
            }
            if v < min - 1e-9 {
                v += 10.0 * weight;
            }
            q.set_f("volume", v.clamp(min, max));
        };
        let c = (150.0, 150.0 + 14.0 + 28.0 * i as f64);
        hs.push(Handle::zone(c, (34.0, 28.0), "Click or scroll to step this digit", move |q| step(q, 1.0)).with_scroll(move |q, d| step(q, d.signum())));
    }
    if p.b("answer") {
        hs.push(Handle::label((size.0 / 2.0, size.1 - 14.0), (240.0, 24.0), "answer_text", "Double-click to edit the answer text"));
    }
    hs
}
