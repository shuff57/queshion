//! The control for `Kind::Data` params: a text box that takes typed numbers or a spreadsheet
//! paste (tidied into the canonical text when it loses focus), a count of what was read, and
//! buttons to open a CSV file, generate sample data from a seed, or clear.
//!
//! Reading, tidying and generating live in `figures::data` (unit tested there); this file is
//! input handling and layout only.
use egui::{Button, DragValue, Id, TextEdit, Ui};

use crate::figures::data::{self, Dist};
use crate::figures::{ParamDef, Params, Value};
use crate::theme;

/// What the generator popover remembers between frames (per figure and param).
#[derive(Clone)]
struct Gen {
    dist: usize,
    n: i64,
    mean: f64,
    sd: f64,
    seed: u64,
    decimals: u32,
    slope: f64,
    intercept: f64,
    noise: f64,
}

impl Default for Gen {
    fn default() -> Self {
        Gen { dist: 0, n: 30, mean: 50.0, sd: 10.0, seed: 1, decimals: 1, slope: 2.0, intercept: 5.0, noise: 6.0 }
    }
}

/// A seed from the clock the egui context keeps (works on the web build too).
fn fresh_seed(ui: &Ui) -> u64 {
    let t = ui.input(|i| i.time).to_bits();
    t ^ (t >> 17).wrapping_mul(0x9E37_79B9)
}

pub fn data_control(ui: &mut Ui, fig: &str, pd: &ParamDef, columns: usize, label: &str, p: &mut Params) {
    super::param_label(ui, label, pd.hint);
    let ctx = ui.ctx().clone();
    let id = Id::new(("data_entry", fig, pd.key));
    let mut text = p.s(pd.key).to_string();
    let mut set: Option<String> = None;

    let hint = if columns <= 1 { "Type or paste numbers" } else { "Paste two columns (x, y)" };
    let r = ui.add(TextEdit::multiline(&mut text).id(id).desired_width(f32::INFINITY).desired_rows(4).hint_text(hint).font(egui::TextStyle::Monospace));
    if r.changed() {
        set = Some(text.clone());
    }
    // a spreadsheet paste is tidied once the box is left, so typing is never rewritten under the cursor
    if r.lost_focus()
        && let Some(c) = data::canonical(&text, columns)
        && c != text
    {
        set = Some(c);
    }

    let note_id = id.with("note");
    let note: String = ctx.data(|d| d.get_temp(note_id)).unwrap_or_default();
    let count = data::summary(set.as_deref().unwrap_or(&text), columns);
    ui.label(theme::muted(if note.is_empty() { count } else { format!("{count} · {note}") }).size(11.5));

    ui.horizontal_wrapped(|ui| {
        if ui.add(Button::new("Open CSV…").small()).on_hover_text("Read numbers from a CSV or text file").clicked() {
            let msg = match crate::platform::pick_file("CSV or text", &["csv", "tsv", "txt"]) {
                None => String::new(),
                Some(path) => match crate::platform::read(&path) {
                    Ok(b) if b.len() > 2_000_000 => "That file is too large".to_string(),
                    Ok(b) => match data::canonical(&String::from_utf8_lossy(&b), columns) {
                        Some(c) => {
                            set = Some(c);
                            format!("read {}", path.file_name().and_then(|n| n.to_str()).unwrap_or("file"))
                        }
                        None => "No numbers found in that file".to_string(),
                    },
                    Err(e) => format!("Couldn't read the file: {e}"),
                },
            };
            ctx.data_mut(|d| d.insert_temp(note_id, msg));
        }
        let gen_btn = ui.add(Button::new("Generate…").small()).on_hover_text("Make sample data from a seed");
        egui::Popup::menu(&gen_btn).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_min_width(230.0);
            let gid = id.with("gen");
            let mut g: Gen = ui.ctx().data(|d| d.get_temp(gid)).unwrap_or_default();
            if g.seed == 1 && !ui.ctx().data(|d| d.get_temp::<bool>(gid.with("seeded"))).unwrap_or(false) {
                g.seed = fresh_seed(ui) % 1_000_000;
                ui.ctx().data_mut(|d| d.insert_temp(gid.with("seeded"), true));
            }
            ui.label(theme::strong("Sample data").size(12.0));
            if columns <= 1 {
                egui::ComboBox::from_id_salt(gid.with("dist")).selected_text(Dist::ALL[g.dist].name()).show_ui(ui, |ui| {
                    for (i, d) in Dist::ALL.iter().enumerate() {
                        ui.selectable_value(&mut g.dist, i, d.name());
                    }
                });
            }
            egui::Grid::new(gid.with("grid")).num_columns(2).show(ui, |ui| {
                ui.label("How many");
                ui.add(DragValue::new(&mut g.n).range(2..=500));
                ui.end_row();
                ui.label(if columns <= 1 { "Mean" } else { "x mean" });
                ui.add(DragValue::new(&mut g.mean).speed(0.5));
                ui.end_row();
                ui.label(if columns <= 1 { "Spread (sd)" } else { "x spread (sd)" });
                ui.add(DragValue::new(&mut g.sd).speed(0.2).range(0.0..=1e6));
                ui.end_row();
                if columns > 1 {
                    ui.label("Slope");
                    ui.add(DragValue::new(&mut g.slope).speed(0.1));
                    ui.end_row();
                    ui.label("Intercept");
                    ui.add(DragValue::new(&mut g.intercept).speed(0.5));
                    ui.end_row();
                    ui.label("Scatter (sd)");
                    ui.add(DragValue::new(&mut g.noise).speed(0.2).range(0.0..=1e6));
                    ui.end_row();
                }
                ui.label("Decimals");
                ui.add(DragValue::new(&mut g.decimals).range(0..=3));
                ui.end_row();
                ui.label("Seed");
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut g.seed).range(0..=999_999_999));
                    if ui.small_button("New").on_hover_text("Pick another seed; the same seed always makes the same numbers").clicked() {
                        g.seed = fresh_seed(ui) % 1_000_000;
                    }
                });
                ui.end_row();
            });
            if ui.add(Button::new("Make data")).clicked() {
                set = Some(if columns <= 1 {
                    data::column_text(&data::generate(Dist::ALL[g.dist], g.n as usize, g.seed, g.mean, g.sd, g.decimals))
                } else {
                    data::pairs_text(&data::generate_pairs(g.n as usize, g.seed, g.mean, g.sd, g.slope, g.intercept, g.noise, g.decimals))
                });
                ctx.data_mut(|d| d.insert_temp(note_id, format!("seed {}", g.seed)));
                ui.close();
            }
            ui.ctx().data_mut(|d| d.insert_temp(gid, g));
        });
        if ui.add_enabled(!text.is_empty(), Button::new("Clear").small()).clicked() {
            set = Some(String::new());
            ctx.data_mut(|d| d.insert_temp(note_id, String::new()));
        }
    });

    if let Some(s) = set {
        p.0.insert(pd.key.to_string(), Value::S(s));
    }
}
