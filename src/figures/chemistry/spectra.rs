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
    ];
    let labels: [&'static str; 7] = ["Hydrogen", "Helium", "Neon", "Sodium", "Mercury", "Lithium", "Calcium"];
    for (i, (k, _, _)) in LINE_DATA.iter().enumerate() { v.push(ParamDef::bool(k, labels[i], *k == "hydrogen")); }
    v
}

fn line_render(p: &Params) -> String {
    let sel: Vec<&(&str, &str, Lines)> = LINE_DATA.iter().filter(|e| p.b(e.0)).collect();
    let style = p.s("style");
    let rel = p.s("brightness") == "Relative";
    let (x0, x1) = (110.0, 650.0);
    let rowh = 52.0;
    let n = sel.len().max(1);
    let w = 700.0;
    let h = 40.0 + rowh * n as f64 + 60.0;
    let mut d = Doc::new(w, h);
    let px = |nm: f64| x0 + (nm - 400.0) / 300.0 * (x1 - x0);
    let top0 = 30.0;
    for (r, e) in sel.iter().enumerate() {
        let y = top0 + r as f64 * rowh;
        let sh = 38.0;
        tx(&mut d, x0 - 12.0, y + sh / 2.0 + 5.0, 14.0, "end", INK, false, e.1);
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
        for &(nm, inten) in e.2.iter() {
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
    ]
}

fn pes_render(p: &Params) -> String {
    let z = z_of(p.s("element")).min(18);
    let peaks = pes_energies(z);
    let emax = peaks.iter().map(|x| x.2).fold(0.0, f64::max);
    let emin = peaks.iter().map(|x| x.2).fold(f64::MAX, f64::min);
    let (lo, hi) = ((emin / 2.2).min(0.3), emax * 2.2);
    let (x0, x1, y0, y1) = (70.0, 640.0, 40.0, 320.0);
    let px = |e: f64| x0 + ((hi.ln() - e.ln()) / (hi.ln() - lo.ln())) * (x1 - x0);
    let maxe = peaks.iter().map(|x| x.1).max().unwrap_or(1) as f64;
    let unit = (y1 - y0 - 30.0) / maxe.max(4.0);
    let mut d = Doc::new(700.0, 420.0);
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
        for &(_, e, en) in &peaks {
            let c = px(en);
            yv += e as f64 * unit * (-((x - c).powi(2)) / (2.0 * sigma * sigma)).exp();
        }
        pts.push((x, y1 - yv));
    }
    d.polyline(&pts, "#1f5fa8", 2.4);
    for &(code, e, en) in &peaks {
        let c = px(en);
        let top = y1 - e as f64 * unit;
        if p.b("labels") {
            let l = format!("{}{}", code / 10, ['s', 'p', 'd', 'f'][(code % 10) as usize]);
            tx(&mut d, c, top - 8.0 - if p.b("counts") { 14.0 } else { 0.0 }, 13.0, "middle", INK, true, &l);
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
    ]
}

fn ms_render(p: &Params) -> String {
    let mut iso: Vec<(u32, f64)> = Vec::new();
    let mut title = String::new();
    if p.s("source") == "Custom isotopes" {
        for tok in p.s("custom").split(|c| c == ',' || c == ';' || c == '\n') {
            let mut it = tok.split(|c| c == ':' || c == '=');
            if let (Some(a), Some(b)) = (it.next(), it.next()) {
                if let (Ok(m), Ok(ab)) = (a.trim().parse::<u32>(), b.trim().trim_end_matches('%').parse::<f64>()) { if ab > 0.0 { iso.push((m, ab)); } }
            }
        }
    } else if let Some(e) = ISOTOPES.iter().find(|e| e.0 == p.s("element")) {
        iso = e.1.to_vec();
        title = e.0.to_string();
    }
    if iso.is_empty() {
        let mut d = Doc::new(420.0, 120.0);
        tx(&mut d, 210.0, 66.0, 15.0, "middle", "#b03030", false, "Enter isotopes as mass:abundance, e.g. 35:75.8, 37:24.2");
        return d.finish();
    }
    let _ = title;
    iso.sort_by_key(|x| x.0);
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
    let maxv = bars.iter().map(|x| x.1).fold(0.0, f64::max);
    let relative = p.b("relative");
    if relative { for b in bars.iter_mut() { b.1 = b.1 / maxv * 100.0; } }
    let (mlo, mhi) = (bars.first().unwrap().0 as f64 - 2.0, bars.last().unwrap().0 as f64 + 2.0);
    let (x0, x1, y0, y1) = (80.0, 650.0, 40.0, 340.0);
    let px = |m: f64| x0 + (m - mlo) / (mhi - mlo) * (x1 - x0);
    let py = |v: f64| y1 - v / 100.0 * (y1 - y0);
    let mut d = Doc::new(700.0, 430.0);
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
