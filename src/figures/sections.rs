//! Section layout of each figure's side panel, mirroring the reference generators (generated from
//! the reference pages' section titles and the parity tables; edit by hand). Figures not listed here
//! fall back to the keyword grouping in `layout.rs`. `item: true` marks where the per-item sections
//! (Vector 1, Vector 2, ...) go.

pub struct RefSection {
    pub title: &'static str,
    pub item: bool,
    pub keys: &'static [&'static str],
    pub help: &'static str,
}

const fn s(title: &'static str, keys: &'static [&'static str], help: &'static str) -> RefSection {
    RefSection { title, item: false, keys, help }
}
const fn items(title: &'static str) -> RefSection {
    RefSection { title, item: true, keys: &[], help: "" }
}

static FIG_0: [RefSection; 6] = [
    s("Atom or ion", &["charge", "gained_lost"], "Picking an element sets its protons, its neutrons from its mass number (C: 12 − 6 = 6) and a neutral atom’s sh"),
    s("Protons, neutrons and electrons", &["empty", "caption", "neutrons", "shuffle", "shell_labels"], "Set any count by hand. Nothing is checked, so a wrong atom draws exactly as set."),
    s("Drawing", &["element", "nucleus", "placement"], "NUCLEUS"),
    s("Colors and symbols", &["proton_color", "proton_symbol", "neutron_color", "neutron_symbol", "electron_color", "electron_symbol"], "PROTON"),
    s("Labels", &["shells_custom", "key", "brackets"], ""),
    s("Chart title", &["title"], ""),
];

static FIG_1: [RefSection; 7] = [
    s("Circuit", &["layout", "n"], "Leave a name empty to number it automatically."),
    items("Resistor"),
    s("Cell or battery", &["cell", "cell_name_mode", "cell_name", "cell_val_mode", "cell_val", "polarity"], ""),
    s("Switch and meters", &["switch", "ammeter", "amm_read_mode", "amm_read", "voltmeter", "vm_read_mode", "vm_read"], ""),
    s("Symbols", &["style"], ""),
    s("Chart title", &["title_mode", "title"], ""),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_2: [RefSection; 3] = [
    s("Titles", &["title", "equations", "eq_labels", "title_mode", "xtitle_mode", "xtitle", "ytitle_mode", "ytitle"], ""),
    s("x-axis", &["xmin", "xmax", "xstep", "ymin", "ymax", "ystep", "numbers", "arrows", "square", "xlabel", "ylabel", "points", "pt_color", "xnum", "ynum", "xunits"], ""),
    s("Grid", &["minor", "grid", "line_width", "shading", "asymptotes", "endpoints", "pt_style", "pt_label"], ""),
];

static FIG_3: [RefSection; 4] = [
    s("Body", &["body"], ""),
    s("Forces", &["body_size", "n", "body_lab", "mv_lab_mode", "notation", "motion", "mdir"], "An arrow over every vector’s label, or every one in bold. For just one label, type \\vec{F} or \\mathbf{F} in it"),
    s("Motion", &["mv_lab", "ma_lab_mode", "ma_lab", "adir"], "Drawn beside the body, dashed, because velocity and acceleration aren't forces."),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_4: [RefSection; 3] = [
    s("Syringe", &["show", "size", "unit"], "Size"),
    s("Volume of gas", &["volume"], "From 0 to 100 cm³, to one decimal place."),
    s("Title and answer key", &["answer", "title"], "Chart title"),
];

static FIG_5: [RefSection; 3] = [
    s("Gel", &["setup", "lanes", "agarose", "look", "ruler"], "Agarose"),
    s("Labels", &["lane_names", "sizes", "electrodes", "answer", "lane_labels", "lane_numbers", "ladder_sizes", "size_units", "electrode_style", "answer_kind"], "Lane labels"),
    s("Chart title and answer key", &["title", "band_height", "run_length"], "Chart title"),
];

static FIG_6: [RefSection; 3] = [
    s("Labels", &["scale", "t_solid", "t_melt", "t_liquid", "t_boil", "t_gas", "labels", "ylabel", "phase_names", "seg_labels", "plateau_labels", "letters", "plateau_dashes", "temp_labels", "xtitle_style", "xtitle", "ytitle_style", "xaxis_fit"], ""),
    s("Curve", &["curve_color", "supercool"], ""),
    s("Titles", &["mode", "substance", "mp", "bp", "start", "end", "xaxis", "title"], ""),
];

static FIG_7: [RefSection; 5] = [
    s("Object", &["obj", "obj_lab_mode", "obj_lab", "size", "pos", "two", "tension"], ""),
    s("Ramp", &["angle", "show_angle", "ang_lab_mode", "ang_lab", "surface", "grav_lab_mode", "grav_lab", "norm_lab_mode", "norm_lab", "fric_lab_mode", "fric_lab", "app_lab_mode", "app_lab", "vel_lab_mode", "vel_lab", "acc_lab_mode", "acc_lab"], ""),
    s("Forces and motion", &["gravity", "normal", "friction", "applied", "velocity", "accel"], ""),
    s("Marks", &["mark_len", "len_lab", "mark_h", "h_lab"], ""),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_8: [RefSection; 4] = [
    s("Ruler", &["units", "ruler", "marks", "read_to"], "Length"),
    s("Length and position", &["length", "offset"], "From 0.50 to 15.00 cm, to 2 decimal places."),
    s("Object", &["object", "dashes"], ""),
    s("Title and answer key", &["answer", "title"], "Chart title"),
];

static FIG_9: [RefSection; 4] = [
    s("Structure", &["formula", "preset", "lone", "formal", "expand"], "Built around O as the central atom."),
    s("Look", &["shape", "bond_style"], "Shape"),
    s("Changes", &["central"], "Make a wrong structure for a “find the mistake” question. Click an atom or bond in the figure, or change one b"),
    s("Title and answer key", &["title", "tweak", "resonance", "resonance_pick", "question"], "Chart title"),
];

static FIG_10: [RefSection; 5] = [
    s("Strips", &["custom", "strips"], "1 line is outside 400–700 nm, so not drawn. Widen the axis to show it."),
    s("Look", &["style", "brightness"], "Bright colored lines on black, as a glowing gas gives off."),
    s("Labels", &["labels", "label_style"], "Each element strip’s name, e.g. Hydrogen."),
    s("Wavelength axis", &["nm_from", "nm_to", "tick_every", "num_every", "axis_pos", "axis_title_style", "axis_title"], "Visible light runs from about 380 to 780 nm; every strip is the same width whatever the range."),
    s("Chart title and answer key", &["title"], "Chart title"),
];

static FIG_11: [RefSection; 3] = [
    s("Balance", &["balance", "pan"], "Weighs up to 610 g, read to 0.01 g. The riders are placed for the mass you type."),
    s("Mass", &["mass", "places"], "From 0 to 610 g, to 2 decimal places."),
    s("Title and answer key", &["title"], "Chart title"),
];

static FIG_12: [RefSection; 4] = [
    s("Peaks", &["values"], "Peak heights"),
    s("For the student", &["element", "custom", "molecule", "relative", "custom_name", "show_name", "leave_out"], ""),
    s("Titles", &["title", "xtitle_style", "xtitle", "ytitle_style", "ytitle"], ""),
    s("x-axis", &["source", "xaxis_fit"], ""),
];

static FIG_13: [RefSection; 3] = [
    s("Micropipette", &["size", "tip", "size_label"], "Size"),
    s("Reading", &["volume"], "A 20 µL pipette is set from 2 to 20 µL, in steps of 0.1 µL."),
    s("Color", &["color", "decimal_line", "answer", "title", "answer_text"], "Red digits show where the decimal point goes. The line keeps that clear on a black and white copy."),
];

static FIG_14: [RefSection; 5] = [
    s("Motion", &["n", "x0", "show", "tangent_t"], "A to B (0–4 s): speeding up from 0 to 4 m/s, a = 1 m/s²; 0 m to 8 m"),
    s("Marks", &["letters", "styles", "tangent"], ""),
    s("Titles", &["title_mode", "title", "xt_mode", "xt", "yt_pos_mode", "yt_pos", "yt_vel_mode", "yt_vel", "yt_acc_mode", "yt_acc"], "Type ^ for a superscript, as in m/s^2, and _ for a subscript."),
    s("Axes and grid", &["numbers", "grid", "ranges", "t_min", "t_max", "t_step", "p_min", "p_max", "p_step", "v_min", "v_max", "v_step", "a_min", "a_max", "a_step"], ""),
    s("Figure", &["color"], ""),
];

static FIG_15: [RefSection; 6] = [
    s("Atom or ion", &["charge"], ""),
    s("Configuration", &["order", "exceptions"], "Order"),
    s("Look", &["element", "layout", "shorthand", "arrows", "orbitals", "edits"], "Arrows"),
    s("For the student", &["blank", "config", "caption", "config_blank", "symbol_style", "sublabels"], "Symbol"),
    s("Changes", &["extra_empty"], "Click an orbital to change its electrons, for a “which rule is broken?” question."),
    s("Title and answer key", &["title"], "Chart title"),
];

static FIG_16: [RefSection; 5] = [
    s("Layout", &["layout", "boxes", "state", "state2"], "Boxes"),
    s("Particles", &["d_count", "d_shape", "d_size", "d_shade", "shuffle"], ""),
    s("Key", &["key_show", "key_list", "note"], "Show"),
    s("Box", &["border"], "Border"),
    s("Chart title", &["title", "lab_before", "lab_after"], ""),
];

static FIG_17: [RefSection; 3] = [
    s("Symbols", &["carriers", "carrier_style"], "Carriers"),
    s("Labels", &["gens", "size", "seed", "changes", "gen_labels", "numbers", "key", "genotypes", "allele_letter", "answer"], "Genotypes under each person"),
    s("Chart title", &["inherit", "title", "symbol_size", "gen_prefix"], "Chart title"),
];

static FIG_18: [RefSection; 3] = [
    s("Instrument", &["instrument", "places"], "Decimal places"),
    s("pH", &["ph"], "From 0 to 14, to 2 decimal places."),
    s("Title and answer key", &["title"], "Chart title"),
];

static FIG_19: [RefSection; 4] = [
    s("Energy axis", &["units", "gridlines"], "Scale"),
    s("Peaks", &["element", "labels", "counts", "source", "custom", "drawn", "sublabel_style", "energies"], "Drawn as"),
    s("For the student", &["names", "numbered"], "The answer key always names the element and its configuration."),
    s("Title and answer key", &["title", "compare", "scale"], "Chart title"),
];

static FIG_20: [RefSection; 7] = [
    s("Graphs", &["growth", "n0", "r", "k", "tspan", "unit", "organism", "graph"], "The growth rate dN/dt is individuals added per year; the per capita growth rate, (dN/dt)/N, is per individual."),
    s("Curve", &["curve_color", "draw_curve"], ""),
    s("Census data", &["census", "census_auto", "census_every", "census_scatter", "census_seed", "census_table"], ""),
    s("Marked on the graph", &["mark_k", "mark_infl", "k_label", "tau", "k_label_style", "mark_phases"], ""),
    s("Titles", &["x_title", "y_title", "fit", "title", "curve_width", "title_style", "xtitle_style", "ytitle_style", "rate_title", "pc_title"], ""),
    s("x-axis", &["x_max", "x_min", "x_step", "x_numbers", "x_end", "x_end_text", "x_lo_end", "x_hi_end", "y_step"], ""),
    s("y-axis", &["y_max", "minor", "blank", "y_min", "y_numbers", "y_end", "y_end_text", "y_lo_end", "y_hi_end"], ""),
];

static FIG_21: [RefSection; 6] = [
    s("Graph", &["graph", "minor", "y_min", "y_max", "y_numbers"], ""),
    s("Data", &["data", "marks"], ""),
    s("Marked", &["pair", "prey", "pred", "source", "prey0", "pred0", "prey_avg", "pred_avg", "cycle", "alpha", "beta", "gamma", "delta", "show", "style", "y_axis", "census_every", "census_noise", "census_seed", "census_join", "mark_lag", "mark_period", "lag_labels"], ""),
    s("Titles", &["x_title", "y_title", "title", "line_width", "legend", "title_style", "xtitle_style", "ytitle_style", "x_min", "x_max", "x_lo_end", "x_hi_end", "y_lo_end", "y_hi_end"], ""),
    s("x-axis", &["tshow", "unit", "x_fit", "x_step", "x_numbers", "x_end", "x_end_text", "y_fit", "y_step"], ""),
    s("y-axis", &["y_end", "y_end_text"], ""),
];

static FIG_22: [RefSection; 6] = [
    s("Launch", &["from", "angle", "show_angle", "ang_lab_mode", "ang_lab", "vx_lab_mode", "vx_lab", "vy_lab_mode", "vy_lab"], ""),
    s("Object", &["object", "size"], ""),
    s("Path", &["path", "steps", "letters"], ""),
    s("Velocity and gravity", &["show_v", "v_lab_mode", "v_lab", "comps", "show_g", "g_lab_mode", "g_lab"], ""),
    s("Marks", &["cliff", "mark_h", "h_lab", "mark_r", "r_lab", "mark_cliff", "cliff_lab"], ""),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_23: [RefSection; 4] = [
    s("Setup", &["setup", "l_kind"], ""),
    s("Objects", &["l_lab_mode", "l_lab", "l_size", "r_lab_mode", "r_lab", "r_size", "lower", "third", "third_side", "t_lab_mode", "t_lab", "t_size", "second", "s_lab", "strands", "angle", "ang_lab_mode", "ang_lab", "surface", "gravity", "grav_lab_mode", "grav_lab", "normal", "norm_lab_mode", "norm_lab", "friction", "fric_lab_mode", "fric_lab"], ""),
    s("Forces and motion", &["tension", "tens_lab_mode", "tens_lab", "accel", "acc_lab_mode", "acc_lab"], ""),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_24: [RefSection; 3] = [
    s("Square", &["cross", "dominance", "parents", "phenotypes", "filled", "shading", "geno_ratio", "parent_labels", "blank_parents", "blank_gametes", "cells", "blank_cells", "cell_phenotype", "shade_style", "geno_ratio_style", "pheno_ratio_style"], "Parents"),
    s("Ratios", &["pheno_ratio", "ratio_form", "sex_split"], "Genotypes"),
    s("Title and question", &["title", "question", "shade_color", "outline_heavy"], "Chart title"),
];

static FIG_25: [RefSection; 4] = [
    s("Ruler", &["marks", "ruler", "read_to", "marbles", "magnify"], "Length"),
    s("Length and position", &["start", "length", "guides", "show_value", "dec", "ruler_color", "obj_color"], "From 0.50 to 15.00 cm, to 2 decimal places."),
    s("Magnifiers", &["object", "mag_marks"], "Numbered marks in the magnifier"),
    s("Title and answer key", &["title_mode", "title", "answer"], "Chart title"),
];

static FIG_26: [RefSection; 5] = [
    s("Spring scale", &["cap", "units", "coded"], "Capacity"),
    s("Reading", &["reading", "offset", "show_zero"], "From 0 to 10 N, to 2 decimal places."),
    s("On the hook", &["hook", "block_lab"], ""),
    s("Magnifier", &["masses", "view", "mag_marks"], "Numbered marks in the magnifier"),
    s("Title and answer key", &["title", "answer"], "Chart title"),
];

static FIG_27: [RefSection; 3] = [
    s("Thermometer", &["thermometer", "unit", "liquid"], "Unit"),
    s("Temperature", &["temperature", "places"], "From -10 to 110 °C, to one decimal place."),
    s("Title and answer key", &["title"], "Chart title"),
];

static FIG_28: [RefSection; 4] = [
    s("Marked points", &["equiv", "half", "eq_label", "half_label", "eq_label_style", "half_mark", "half_label_style"], ""),
    s("Curve", &["curve_color", "common", "source", "kp_start", "kp_end", "kp_veq", "kp_eqph"], ""),
    s("Titles", &["kind", "molarity", "volume", "titrant", "pk", "title", "xlabel", "xtitle_style", "ytitle_style", "ytitle"], ""),
    s("x-axis", &["xmax", "xaxis_fit"], ""),
];

static FIG_29: [RefSection; 5] = [
    s("Vectors", &["n"], "Drawn to scale, head to tail, in order. One grid square is a magnitude of 1."),
    items("Vector"),
    s("Resultant", &["resultant", "res_style", "res_lab_mode", "res_lab", "mark_angle", "ang_lab_mode", "ang_lab", "components"], "From the first tail to the last tip: 5 squares at 36.9°."),
    s("Grid and axes", &["grid", "axes"], ""),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_30: [RefSection; 6] = [
    s("Graduated cylinder", &["size", "color"], "Liquid"),
    s("Scale", &["marks", "numbered", "unit"], "Smallest marks"),
    s("Readings", &["before", "after", "dotted"], "From 0 to 10 mL, to 2 decimal places."),
    s("Object", &["object", "count"], ""),
    s("Captions", &["lab_before", "lab_after"], ""),
    s("Title and answer key", &["title"], "Chart title"),
];

static FIG_31: [RefSection; 4] = [
    s("Instrument", &["instrument", "size", "color"], "Size"),
    s("Scale", &["marks", "numbered", "unit"], "Smallest marks"),
    s("Reading", &["reading", "dotted"], "From 0 to 100 mL, to one decimal place."),
    s("Title and answer key", &["answer", "title"], "Chart title"),
];

static FIG_32: [RefSection; 6] = [
    s("Wave", &["kind", "against", "amp", "wl", "cycles"], "In the units of the axes."),
    s("Marks", &["mark_wl", "wl_lab_mode", "wl_lab", "mark_amp", "amp_lab_mode", "amp_lab", "mark_ct", "crest_mode", "trough_mode"], ""),
    s("Graph", &["numbers", "grid", "minor", "color"], ""),
    s("Titles", &["title_mode", "title", "xt_mode", "xt", "yt_mode", "yt"], ""),
    s("x-axis", &["xaxis_fit", "xmin", "xmax", "xstep", "xaxis_num", "xaxis_letter"], ""),
    s("y-axis", &["yaxis_fit", "ymin", "ymax", "ystep", "yaxis_num", "yaxis_letter"], ""),
];

pub fn reference(id: &str) -> Option<&'static [RefSection]> {
    Some(match id {
        "bohr-model" => &FIG_0,
        "circuit-diagram" => &FIG_1,
        "coordinate-grid" => &FIG_2,
        "free-body-diagram" => &FIG_3,
        "gas-syringe" => &FIG_4,
        "gel-electrophoresis" => &FIG_5,
        "heating-cooling-curve" => &FIG_6,
        "inclined-plane" => &FIG_7,
        "length-reading" => &FIG_8,
        "lewis-structures" => &FIG_9,
        "line-spectrum" => &FIG_10,
        "mass-reading" => &FIG_11,
        "mass-spectrum" => &FIG_12,
        "micropipette-reading" => &FIG_13,
        "motion-graphs" => &FIG_14,
        "orbital-diagram" => &FIG_15,
        "particle-diagram" => &FIG_16,
        "pedigree" => &FIG_17,
        "ph-reading" => &FIG_18,
        "photoelectron-spectrum" => &FIG_19,
        "population-growth" => &FIG_20,
        "predator-prey" => &FIG_21,
        "projectile-motion" => &FIG_22,
        "pulley" => &FIG_23,
        "punnett-square" => &FIG_24,
        "ruler-reading" => &FIG_25,
        "spring-scale" => &FIG_26,
        "temperature-reading" => &FIG_27,
        "titration-curve" => &FIG_28,
        "vector-diagram" => &FIG_29,
        "volume-by-displacement" => &FIG_30,
        "volume-reading" => &FIG_31,
        "waves" => &FIG_32,
        _ => return None,
    })
}
