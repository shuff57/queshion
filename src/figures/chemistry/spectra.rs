//! Line (atomic) spectra, photoelectron spectra and mass spectra.
use super::data::*;
use super::*;
use crate::figures::{ParamDef, Params, Subject};

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "line-spectrum", subject: Subject::Chemistry, title: "Line Spectrum",
            blurb: "Atomic emission or absorption line spectra for common elements, on a 400-700 nm scale.",
            params: line_params, render: line_render },
        FigureDef { id: "photoelectron-spectrum", subject: Subject::Chemistry, title: "Photoelectron Spectrum",
            blurb: "PES peaks for the first 18 elements: one peak per subshell, height set by the electron count.",
            params: pes_params, render: pes_render },
        FigureDef { id: "mass-spectrum", subject: Subject::Chemistry, title: "Mass Spectrum",
            blurb: "Isotope abundance bars for an element, a diatomic molecule of it, or your own isotope list.",
            params: ms_params, render: ms_render },
    ]
}

// ---------------------------------------------------------------------------------------------
// Line spectrum
// ---------------------------------------------------------------------------------------------

type Lines = &'static [(f64, f64)];
const LINE_DATA: [(&str, &str, Lines); 7] = [
    ("hydrogen", "Hydrogen", &[(410.2, 0.3), (434.0, 0.45), (486.1, 0.8), (656.3, 1.0)]),
    ("helium", "Helium", &[(447.1, 0.6), (471.3, 0.3), (492.2, 0.3), (501.6, 0.8), (587.6, 1.0), (667.8, 0.5)]),
    ("neon", "Neon", &[(585.2, 0.7), (588.2, 0.5), (603.0, 0.4), (614.3, 0.6), (626.6, 0.5), (640.2, 1.0), (650.6, 0.6), (692.9, 0.5)]),
    ("sodium", "Sodium", &[(498.3, 0.2), (568.8, 0.3), (589.0, 1.0), (589.6, 1.0), (615.4, 0.2), (616.1, 0.2)]),
    ("mercury", "Mercury", &[(404.7, 0.6), (435.8, 1.0), (546.1, 1.0), (577.0, 0.6), (579.1, 0.6)]),
    ("lithium", "Lithium", &[(460.3, 0.3), (610.4, 0.4), (670.8, 1.0)]),
    ("calcium", "Calcium", &[(422.7, 1.0), (445.5, 0.3), (558.9, 0.3), (616.2, 0.5), (643.9, 0.6)]),
];

fn line_params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("style", "Style", &["Emission", "Absorption", "Print"], "Emission"),
        ParamDef::choice("brightness", "Brightness", &["All equal", "Relative"], "All equal"),
        ParamDef::bool("labels", "Label wavelengths", false),
        ParamDef::text("title", "Title (optional)", ""),
        ParamDef::list("custom", "Extra custom row: wavelengths in nm (nm or nm:brightness)", ""),
    ];
    let labels: [&'static str; 7] = ["Hydrogen", "Helium", "Neon", "Sodium", "Mercury", "Lithium", "Calcium"];
    for (i, (k, _, _)) in LINE_DATA.iter().enumerate() { v.push(ParamDef::bool(k, labels[i], *k == "hydrogen")); }
    v
}

/// Rows drawn: the ticked elements, then an optional custom row.
fn line_rows(p: &Params) -> Vec<(String, Vec<(f64, f64)>)> {
    let mut rows: Vec<(String, Vec<(f64, f64)>)> = LINE_DATA.iter().filter(|e| p.b(e.0)).map(|e| (e.1.to_string(), e.2.to_vec())).collect();
    let custom = custom_lines(p.s("custom"));
    if !custom.is_empty() { rows.push(("Custom".to_string(), custom)); }
    rows
}

/// "656.3, 486:0.5" -> [(nm, brightness)].
fn custom_lines(s: &str) -> Vec<(f64, f64)> {
    s.split(|c| c == ',' || c == ';').filter_map(|t| {
        let mut it = t.trim().split(':');
        let nm = it.next()?.trim().parse::<f64>().ok()?;
        let b = it.next().and_then(|b| b.trim().parse::<f64>().ok()).unwrap_or(1.0);
        Some((nm, b.clamp(0.0, 1.0)))
    }).collect()
}

fn line_render(p: &Params) -> String {
    let rows = line_rows(p);
    let style = p.s("style");
    let rel = p.s("brightness") == "Relative";
    let (x0, x1) = (110.0, 650.0);
    let rowh = 52.0;
    let n = rows.len().max(1);
    let w = 700.0;
    let h = 40.0 + rowh * n as f64 + 60.0;
    let mut d = Doc::new(w, h);
    title_at(&mut d, p, w / 2.0, 18.0, 15.0);
    let px = |nm: f64| x0 + (nm - 400.0) / 300.0 * (x1 - x0);
    let top0 = 30.0;
    for (r, e) in rows.iter().enumerate() {
        let y = top0 + r as f64 * rowh;
        let sh = 38.0;
        tx(&mut d, x0 - 12.0, y + sh / 2.0 + 5.0, 14.0, "end", INK, false, &e.0);
        match style {
            "Emission" => { d.rect(x0, y, x1 - x0, sh, "#000", "#000", 1.0); }
            "Absorption" => {
                let steps = 270;
                for k in 0..steps {
                    let nm = 400.0 + 300.0 * (k as f64 + 0.5) / steps as f64;
                    let wpx = (x1 - x0) / steps as f64;
                    d.rect(x0 + k as f64 * wpx, y, wpx + 0.6, sh, &hex(wavelength_rgb(nm)), "none", 0.0);
                }
            }
            _ => { d.rect(x0, y, x1 - x0, sh, "#fff", INK, 1.2); }
        }
        let mut last_label = -100.0;
        for &(nm, inten) in e.1.iter() {
            if !(400.0..=700.0).contains(&nm) { continue; }
            let x = px(nm);
            let o = if rel { 0.25 + 0.75 * inten } else { 1.0 };
            let col = match style { "Emission" => hex(wavelength_rgb(nm)), _ => "#000000".into() };
            d.raw(&format!(r#"<line x1="{x:.2}" y1="{y:.2}" x2="{x:.2}" y2="{:.2}" stroke="{col}" stroke-width="{}" stroke-opacity="{o:.2}"/>"#, y + sh, if style == "Print" { 1.6 } else { 2.0 }));
            if p.b("labels") && nm - last_label > 4.5 {
                tr(&mut d, x + 3.0, y - 3.0, 9.0, -90.0, "start", &format!("{nm}"));
                last_label = nm;
            }
        }
        if style == "Print" { }
    }
    // axis
    let ay = top0 + rowh * n as f64 + 4.0;
    d.line(x0, ay, x1, ay, INK, 1.6);
    for k in 0..=6 {
        let nm = 400.0 + 50.0 * k as f64;
        d.line(px(nm), ay, px(nm), ay + 6.0, INK, 1.4);
        tx(&mut d, px(nm), ay + 22.0, 13.0, "middle", INK, false, &format!("{}", nm as i64));
    }
    for k in 0..30 { let nm = 400.0 + 10.0 * k as f64; if k % 5 != 0 { d.line(px(nm), ay, px(nm), ay + 3.0, INK, 0.8); } }
    tx(&mut d, (x0 + x1) / 2.0, ay + 44.0, 14.0, "middle", INK, false, "Wavelength (nm)");
    d.finish()
}

// ---------------------------------------------------------------------------------------------
// PES
// ---------------------------------------------------------------------------------------------

fn pes_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("element", "Element (Z = 1-18)", &ELEMENT_CHOICES[..18], "6 C - Carbon"),
        ParamDef::bool("labels", "Label peaks with subshells", true),
        ParamDef::bool("counts", "Show electrons per peak", false),
        ParamDef::choice("source", "Peaks from", &["Element", "Custom peaks"], "Element"),
        ParamDef::list("custom", "Custom peaks (energy MJ/mol : electrons : label)", "28.6:2:1s, 1.72:2:2s, 1.09:2:2p"),
        ParamDef::text("title", "Title (optional)", ""),
    ]
}

/// Peaks as (label, electrons, binding energy).
fn pes_peaks(p: &Params) -> Vec<(String, u8, f64)> {
    if p.s("source") == "Custom peaks" {
        let v: Vec<(String, u8, f64)> = p.s("custom").split(|c| c == ',' || c == ';').filter_map(|t| {
            let parts: Vec<&str> = t.trim().split(':').collect();
            let en = parts.first()?.trim().parse::<f64>().ok()?;
            let e = parts.get(1)?.trim().parse::<f64>().ok()?.round().clamp(1.0, 14.0) as u8;
            Some((parts.get(2).map(|l| l.trim().to_string()).unwrap_or_default(), e, en.clamp(0.05, 2000.0)))
        }).collect();
        if !v.is_empty() { return v; }
    }
    let z = z_of(p.s("element")).min(18);
    pes_energies(z).into_iter().map(|(code, e, en)| (format!("{}{}", code / 10, ['s', 'p', 'd', 'f'][(code % 10) as usize]), e, en)).collect()
}

const PES_X0: f64 = 70.0;
const PES_X1: f64 = 640.0;
const PES_Y0: f64 = 40.0;
const PES_Y1: f64 = 320.0;

/// (lo, hi, unit): log-axis bounds and pixels per electron.
fn pes_frame(peaks: &[(String, u8, f64)]) -> (f64, f64, f64) {
    let emax = peaks.iter().map(|x| x.2).fold(0.0, f64::max);
    let emin = peaks.iter().map(|x| x.2).fold(f64::MAX, f64::min);
    let (lo, hi) = ((emin / 2.2).min(0.3), emax * 2.2);
    let maxe = peaks.iter().map(|x| x.1).max().unwrap_or(1) as f64;
    (lo, hi, (PES_Y1 - PES_Y0 - 30.0) / maxe.max(4.0))
}

fn pes_render(p: &Params) -> String {
    let peaks = pes_peaks(p);
    let (lo, hi, unit) = pes_frame(&peaks);
    let (x0, x1, y0, y1) = (PES_X0, PES_X1, PES_Y0, PES_Y1);
    let px = |e: f64| x0 + ((hi.ln() - e.ln()) / (hi.ln() - lo.ln())) * (x1 - x0);
    let mut d = Doc::new(700.0, 420.0);
    title_at(&mut d, p, 355.0, 22.0, 16.0);
    d.line(x0, y1, x1, y1, INK, 1.8);
    d.line(x0, y0 - 10.0, x0, y1, INK, 1.8);
    let ticks = [0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0];
    for &t in ticks.iter().filter(|&&t| t >= lo && t <= hi) {
        d.line(px(t), y1, px(t), y1 + 6.0, INK, 1.4);
        tx(&mut d, px(t), y1 + 22.0, 12.0, "middle", INK, false, &if t < 1.0 { num(t, 1) } else { format!("{}", t as i64) });
    }
    tx(&mut d, (x0 + x1) / 2.0, y1 + 50.0, 14.0, "middle", INK, false, "Binding energy (MJ/mol)");
    tr(&mut d, 22.0, (y0 + y1) / 2.0, 14.0, -90.0, "middle", "Relative number of electrons");
    // curve: sum of gaussians on the log axis
    let sigma = 5.5;
    let samples = 600;
    let mut pts = Vec::new();
    for i in 0..=samples {
        let x = x0 + (x1 - x0) * i as f64 / samples as f64;
        let mut yv = 0.0;
        for (_, e, en) in &peaks {
            let (e, en) = (*e, *en);
            let c = px(en);
            yv += e as f64 * unit * (-((x - c).powi(2)) / (2.0 * sigma * sigma)).exp();
        }
        pts.push((x, y1 - yv));
    }
    d.polyline(&pts, "#1f5fa8", 2.4);
    for (l, e, en) in &peaks {
        let (e, en) = (*e, *en);
        let c = px(en);
        let top = y1 - e as f64 * unit;
        if p.b("labels") {
            tx(&mut d, c, top - 8.0 - if p.b("counts") { 14.0 } else { 0.0 }, 13.0, "middle", INK, true, l);
        }
        if p.b("counts") { tx(&mut d, c, top - 8.0, 12.0, "middle", "#555", false, &format!("({e})")); }
    }
    d.finish()
}

// ---------------------------------------------------------------------------------------------
// Mass spectrum
// ---------------------------------------------------------------------------------------------

fn ms_params() -> Vec<ParamDef> {
    let names: Vec<&'static str> = ISOTOPES.iter().map(|e| e.0).collect();
    vec![
        ParamDef::choice("source", "Isotopes from", &["Element", "Custom isotopes"], "Element"),
        ParamDef::choice("element", "Element", &names, "Chlorine"),
        ParamDef::text("custom", "Custom isotopes (mass:abundance, ...)", "35:75.8, 37:24.2"),
        ParamDef::bool("molecule", "Diatomic molecule (X2)", false),
        ParamDef::bool("relative", "Scale tallest peak to 100", true),
        ParamDef::bool("values", "Print the value above each bar", false),
        ParamDef::text("title", "Title (optional)", ""),
    ]
}

/// Raw isotopes (mass, abundance) of the chosen source, sorted by mass.
fn ms_iso(p: &Params) -> Vec<(u32, f64)> {
    let mut iso: Vec<(u32, f64)> = Vec::new();
    if p.s("source") == "Custom isotopes" {
        for tok in p.s("custom").split(|c| c == ',' || c == ';' || c == '\n') {
            let mut it = tok.split(|c| c == ':' || c == '=');
            if let (Some(a), Some(b)) = (it.next(), it.next()) {
                if let (Ok(m), Ok(ab)) = (a.trim().parse::<u32>(), b.trim().trim_end_matches('%').parse::<f64>()) { if ab > 0.0 { iso.push((m, ab)); } }
            }
        }
    } else if let Some(e) = ISOTOPES.iter().find(|e| e.0 == p.s("element")) {
        iso = e.1.to_vec();
    }
    iso.sort_by_key(|x| x.0);
    iso
}

/// Displayed bars (mass, height 0..100) for raw isotopes `iso`.
fn ms_bars(p: &Params, iso: &[(u32, f64)]) -> Vec<(u32, f64)> {
    let tot: f64 = iso.iter().map(|x| x.1).sum();
    let mut bars: Vec<(u32, f64)> = iso.iter().map(|&(m, a)| (m, a / tot * 100.0)).collect();
    if p.b("molecule") {
        let mut v: Vec<(u32, f64)> = Vec::new();
        for (i, &(m1, a1)) in bars.iter().enumerate() {
            for &(m2, a2) in bars.iter().skip(i) {
                let pr = a1 / 100.0 * a2 / 100.0 * if m1 == m2 { 1.0 } else { 2.0 } * 100.0;
                match v.iter_mut().find(|x| x.0 == m1 + m2) { Some(x) => x.1 += pr, None => v.push((m1 + m2, pr)) }
            }
        }
        v.sort_by_key(|x| x.0);
        bars = v;
    }
    if p.b("relative") {
        let maxv = bars.iter().map(|x| x.1).fold(0.0, f64::max);
        for b in bars.iter_mut() { b.1 = b.1 / maxv * 100.0; }
    }
    bars
}

const MS_X0: f64 = 80.0;
const MS_X1: f64 = 650.0;
const MS_Y0: f64 = 40.0;
const MS_Y1: f64 = 340.0;

fn ms_render(p: &Params) -> String {
    let iso = ms_iso(p);
    if iso.is_empty() {
        let mut d = Doc::new(420.0, 120.0);
        tx(&mut d, 210.0, 66.0, 15.0, "middle", "#b03030", false, "Enter isotopes as mass:abundance, e.g. 35:75.8, 37:24.2");
        return d.finish();
    }
    let bars = ms_bars(p, &iso);
    let relative = p.b("relative");
    let (mlo, mhi) = (bars.first().unwrap().0 as f64 - 2.0, bars.last().unwrap().0 as f64 + 2.0);
    let (x0, x1, y0, y1) = (MS_X0, MS_X1, MS_Y0, MS_Y1);
    let px = |m: f64| x0 + (m - mlo) / (mhi - mlo) * (x1 - x0);
    let py = |v: f64| y1 - v / 100.0 * (y1 - y0);
    let mut d = Doc::new(700.0, 430.0);
    title_at(&mut d, p, 365.0, 22.0, 16.0);
    for k in 0..=5 {
        let v = 20.0 * k as f64;
        d.line(x0 - 5.0, py(v), x0, py(v), INK, 1.2);
        tx(&mut d, x0 - 10.0, py(v) + 4.5, 12.0, "end", INK, false, &format!("{}", v as i64));
        if k > 0 { d.line(x0, py(v), x1, py(v), "#e6e6e6", 0.8); }
    }
    d.line(x0, y0 - 10.0, x0, y1, INK, 1.8);
    d.line(x0, y1, x1, y1, INK, 1.8);
    let span = mhi - mlo;
    let step = if span <= 16.0 { 1.0 } else if span <= 40.0 { 2.0 } else if span <= 100.0 { 5.0 } else { 10.0 };
    let mut m = (mlo / step).ceil() * step;
    while m <= mhi {
        d.line(px(m), y1, px(m), y1 + 6.0, INK, 1.3);
        tx(&mut d, px(m), y1 + 22.0, 12.0, "middle", INK, false, &format!("{}", m as i64));
        m += step;
    }
    tx(&mut d, (x0 + x1) / 2.0, y1 + 52.0, 14.0, "middle", INK, false, if p.b("molecule") { "m/z (molecular ion)" } else { "Mass number (m/z)" });
    tr(&mut d, 24.0, (y0 + y1) / 2.0, 14.0, -90.0, "middle", if relative { "Relative abundance" } else { "Abundance (%)" });
    for &(mass, v) in &bars {
        let x = px(mass as f64);
        let bw = ((x1 - x0) / span * 0.5).clamp(4.0, 16.0);
        d.rect(x - bw / 2.0, py(v), bw, y1 - py(v), "#1f5fa8", "#12396b", 1.0);
        if p.b("values") {
            let t = if v >= 10.0 { num(v, 1) } else { num(v, 2) };
            tx(&mut d, x, py(v) - 6.0, 11.0, "middle", INK, false, &t);
        }
    }
    d.finish()
}

// ---------------------------------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------------------------------

use crate::figures::interact::{Cursor, Handle};

pub(super) fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    match id {
        "line-spectrum" => Some(line_handles(p)),
        "photoelectron-spectrum" => Some(pes_handles(p)),
        "mass-spectrum" => Some(ms_handles(p)),
        _ => None,
    }
}

fn line_handles(p: &Params) -> Vec<Handle> {
    let (x0, x1, rowh, top0, sh) = (110.0, 650.0, 52.0, 30.0, 38.0);
    let mut v = Vec::new();
    let mut r = 0;
    for e in LINE_DATA.iter().filter(|e| p.b(e.0)) {
        let y = top0 + r as f64 * rowh;
        let key = e.0;
        v.push(Handle::zone((x0 - 50.0, y + sh / 2.0), (96.0, 26.0), "Click to hide this element's spectrum", move |q| q.set_b(key, false)));
        r += 1;
    }
    let custom = custom_lines(p.s("custom"));
    if !custom.is_empty() {
        let y = top0 + r as f64 * rowh + sh / 2.0;
        for (i, &(nm, _)) in custom.iter().enumerate() {
            if !(400.0..=700.0).contains(&nm) { continue; }
            v.push(Handle::knob((x0 + (nm - 400.0) / 300.0 * (x1 - x0), y), "Drag to change this line's wavelength", move |q, t| {
                let new = (400.0 + (t.0 - x0) / (x1 - x0) * 300.0).clamp(400.0, 700.0);
                let new = (new * 2.0).round() / 2.0;
                let mut toks: Vec<String> = q.s("custom").split(|c| c == ',' || c == ';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
                let Some(tok) = toks.iter_mut().filter(|t| t.split(':').next().and_then(|n| n.trim().parse::<f64>().ok()).is_some()).nth(i) else { return };
                let mut parts = tok.split(':');
                let old: f64 = parts.next().and_then(|n| n.trim().parse().ok()).unwrap_or(0.0);
                if (old - new).abs() < 0.26 { return; }
                let rest: Vec<&str> = parts.collect();
                *tok = if rest.is_empty() { format!("{new}") } else { format!("{new}:{}", rest.join(":")) };
                let joined = toks.join(", ");
                q.set_s("custom", &joined);
            }).with_cursor(Cursor::ResizeH));
        }
    }
    v
}

fn pes_handles(p: &Params) -> Vec<Handle> {
    let mut v = Vec::new();
    let peaks = pes_peaks(p);
    let (lo, hi, unit) = pes_frame(&peaks);
    let px = move |e: f64| PES_X0 + ((hi.ln() - e.ln()) / (hi.ln() - lo.ln())) * (PES_X1 - PES_X0);
    if p.s("source") == "Custom peaks" {
        let n_ok = p.s("custom").split(|c| c == ',' || c == ';').filter(|t| {
            let parts: Vec<&str> = t.trim().split(':').collect();
            parts.len() >= 2 && parts[0].trim().parse::<f64>().is_ok() && parts[1].trim().parse::<f64>().is_ok()
        }).count();
        if n_ok > 0 {
            for (i, (_, e, en)) in peaks.iter().enumerate() {
                v.push(Handle::knob((px(*en), PES_Y1 - *e as f64 * unit), "Drag: sideways = binding energy, up/down = electrons", move |q, t| {
                    let mut toks: Vec<String> = q.s("custom").split(|c| c == ',' || c == ';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
                    let peaks = pes_peaks(q);
                    let (lo, hi, unit) = pes_frame(&peaks);
                    let Some(&(_, e_old, en_old)) = peaks.get(i) else { return };
                    let en_new = (hi.ln() - (t.0 - PES_X0) / (PES_X1 - PES_X0) * (hi.ln() - lo.ln())).exp().clamp(0.05, 2000.0);
                    let e_new = ((PES_Y1 - t.1) / unit).round().clamp(1.0, 14.0) as u8;
                    if e_new == e_old && (en_new / en_old - 1.0).abs() < 0.01 { return; }
                    let Some(tok) = toks.get_mut(i) else { return };
                    let label = tok.split(':').nth(2).map(|l| l.trim().to_string());
                    *tok = match label { Some(l) => format!("{}:{e_new}:{l}", sig(en_new)), None => format!("{}:{e_new}", sig(en_new)) };
                    let joined = toks.join(", ");
                    q.set_s("custom", &joined);
                }));
            }
        }
    } else {
        v.push(
            Handle::zone(((PES_X0 + PES_X1) / 2.0, (PES_Y0 + PES_Y1) / 2.0), (PES_X1 - PES_X0, PES_Y1 - PES_Y0), "Scroll to change the element", |_| {}).with_scroll(|q, dy| {
                let z = z_of(q.s("element")) as i64;
                let nz = (z - 1 + if dy > 0.0 { 1 } else { -1 }).rem_euclid(18) + 1;
                q.set_s("element", ELEMENT_CHOICES[(nz - 1) as usize]);
            }),
        );
    }
    v
}

/// Three significant figures without trailing noise.
fn sig(x: f64) -> String {
    let s = if x >= 100.0 { format!("{x:.0}") } else if x >= 10.0 { format!("{x:.1}") } else { format!("{x:.2}") };
    s
}

fn ms_handles(p: &Params) -> Vec<Handle> {
    if p.b("molecule") { return Vec::new(); }
    let iso = ms_iso(p);
    if iso.is_empty() { return Vec::new(); }
    let bars = ms_bars(p, &iso);
    let (mlo, mhi) = (bars.first().unwrap().0 as f64 - 2.0, bars.last().unwrap().0 as f64 + 2.0);
    let span = mhi - mlo;
    bars.iter().enumerate().map(|(i, &(mass, h))| {
        let x = MS_X0 + (mass as f64 - mlo) / span * (MS_X1 - MS_X0);
        let y = MS_Y1 - h / 100.0 * (MS_Y1 - MS_Y0);
        Handle::knob((x, y), "Drag: sideways = mass number, up/down = abundance", move |q, t| {
            let iso = ms_iso(q);
            let bars = ms_bars(q, &iso);
            let (Some(&(m_old, h_old)), Some(&(m0, _))) = (bars.get(i), bars.first()) else { return };
            let _ = m0;
            let mlo = bars.first().unwrap().0 as f64 - 2.0;
            let mhi = bars.last().unwrap().0 as f64 + 2.0;
            let m_new = (mlo + (t.0 - MS_X0) / (MS_X1 - MS_X0) * (mhi - mlo)).round().clamp(1.0, 300.0) as u32;
            let h_new = ((MS_Y1 - t.1) / (MS_Y1 - MS_Y0) * 100.0).clamp(0.5, 100.0);
            let m_new = if iso.iter().enumerate().any(|(k, x)| k != i && x.0 == m_new) { m_old } else { m_new };
            if m_new == m_old && (h_new - h_old).abs() < 0.8 { return; }
            let others: Vec<f64> = iso.iter().enumerate().filter(|(k, _)| *k != i).map(|(_, x)| x.1).collect();
            let ab_old = iso[i].1;
            let ab_new = if others.is_empty() {
                ab_old
            } else if q.b("relative") {
                (h_new / 100.0 * others.iter().cloned().fold(0.0, f64::max)).max(0.01)
            } else {
                let s = (h_new / 100.0).min(0.95);
                (s / (1.0 - s) * others.iter().sum::<f64>()).max(0.01)
            };
            let mut list = iso.clone();
            list[i] = (m_new, ab_new);
            list.sort_by_key(|x| x.0);
            let text = list.iter().map(|(m, a)| format!("{m}:{}", sig(*a))).collect::<Vec<_>>().join(", ");
            q.set_s("source", "Custom isotopes");
            q.set_s("custom", &text);
        })
    }).collect()
}
