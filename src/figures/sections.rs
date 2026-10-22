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

static FIG_0: [RefSection; 2] = [
    s("Figure", &["around", "unknown", "hide", "sector_text", "right", "letters", "dec", "turn", "ray_ends", "shade", "label_size", "ray_count"], ""),
    s("More options", &["ray2", "ray3", "ray4", "ray5", "ray6"], ""),
];

static FIG_1: [RefSection; 7] = [
    s("Atom or ion", &["charge", "gained_lost"], "Picking an element sets its protons, its neutrons from the mass number (C: 12 − 6 = 6) and, for a neutral atom, its shells."),
    s("Protons, neutrons and electrons", &["empty", "caption", "neutrons", "shuffle", "shell_labels"], "Set any count by hand. Nothing is checked, so a wrong atom draws exactly as set."),
    s("Drawing", &["element", "nucleus", "placement"], ""),
    s("Colors and symbols", &["proton_color", "proton_symbol", "neutron_color", "neutron_symbol", "electron_color", "electron_symbol"], ""),
    s("Labels", &["shells_custom", "key", "brackets"], ""),
    s("Chart title", &["title"], ""),
    s("More options", &["title_style"], ""),
];

static FIG_2: [RefSection; 4] = [
    s("Box plot", &["caps"], "Labels write the five-number summary on the figure. Leave them off for students to find."),
    s("Titles", &["title", "axis_title", "fill", "data3", "summary3", "name3", "data4", "summary4", "name4", "read5", "min_lab", "min_label", "med_lab", "med_label", "q1_lab", "q1_label", "q3_lab", "q3_label", "max_lab", "max_label", "title_mode", "axis_title_mode"], ""),
    s("Number line", &["data", "summary", "name1", "data2", "summary2", "name2", "values", "outliers", "auto", "axis_min", "axis_max", "axis_step", "numbers"], "Leave these empty to fit the data."),
    s("More options", &["label_size"], ""),
];

static FIG_3: [RefSection; 8] = [
    s("Circuit", &["layout", "n"], "Leave a name empty to number it automatically."),
    items("Resistor"),
    s("Cell or battery", &["cell", "cell_name_mode", "cell_name", "cell_val_mode", "cell_val", "polarity"], ""),
    s("Switch and meters", &["switch", "ammeter", "amm_read_mode", "amm_read", "voltmeter", "vm_read_mode", "vm_read"], ""),
    s("Work out the circuit", &["solve", "show_answers"], "Type values such as 12 V for the battery and 4 Ω for each part. Meter readings are filled in from the circuit; turn off Show answers for a student copy with blank lines."),
    s("Symbols", &["style"], ""),
    s("Chart title", &["title_mode", "title"], ""),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_4: [RefSection; 1] = [
    s("Settings", &["preset", "unit", "dec", "angle_dec", "label_size", "shade", "fill_color", "rotate"], ""),
];

static FIG_5: [RefSection; 5] = [
    s("Lines", &["lean", "radius", "height", "show_height", "slant", "give_d"], ""),
    s("Position", &["stands"], ""),
    s("More options", &["view", "hidden", "right", "unit", "dec", "label_size"], ""),
    s("More options", &["diam"], ""),
    s("Labels", &["h_lab", "h_label", "l_lab", "l_label", "lean_lab", "lean_label", "r_lab", "r_label", "d_lab", "d_label", "label_offsets"], "Drag any label on the figure to move it."),
];

static FIG_6: [RefSection; 4] = [
    s("Titles", &["title", "equations", "eq_labels", "title_mode", "xtitle_mode", "xtitle", "ytitle_mode", "ytitle"], ""),
    s("x-axis", &["xmin", "xmax", "xstep", "numbers", "arrows", "square", "xlabel", "xnum", "xunits"], ""),
    s("y-axis", &["ymin", "ymax", "ystep", "ylabel", "ynum"], ""),
    s("Grid", &["minor", "grid", "line_width", "shading", "asymptotes", "endpoints", "points", "pt_color", "pt_style", "pt_label"], ""),
];

static FIG_7: [RefSection; 5] = [
    s("Lines", &["lean", "radius", "height", "show_height", "edge", "give_d"], ""),
    s("Position", &["stands"], ""),
    s("More options", &["view", "hidden", "right", "unit", "dec", "label_size"], ""),
    s("More options", &["diam"], ""),
    s("Labels", &["h_lab", "h_label", "lean_lab", "lean_label", "edge_lab", "edge_label", "r_lab", "r_label", "d_lab", "d_label", "label_offsets"], "Drag any label on the figure to move it."),
];

static FIG_8: [RefSection; 4] = [
    s("Body", &["body", "body_size", "body_lab"], ""),
    s("Forces", &["n", "notation"], ""),
    s("Motion", &["motion", "mv_lab_mode", "mv_lab", "ma_lab_mode", "ma_lab", "mdir", "adir"], "Drawn beside the body, dashed, because velocity and acceleration aren't forces."),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_9: [RefSection; 5] = [
    s("Syringe", &["show", "size", "unit"], ""),
    s("Volume of gas", &["volume"], "From 0 to 100 cm³, to one decimal place."),
    s("Title and answer key", &["answer", "title"], ""),
    s("More options", &["title_style"], ""),
    s("Magnifier", &["mag_show", "mag_marks"], ""),
];

static FIG_10: [RefSection; 4] = [
    s("Lanes", &["lanes"], ""),
    s("Gel", &["setup", "agarose", "look", "ruler"], ""),
    s("Labels", &["lane_names", "lane_labels", "lane_numbers", "sizes", "ladder_sizes", "size_units", "electrodes", "electrode_style"], ""),
    s("Chart title and answer key", &["title", "answer", "answer_kind", "band_height", "run_length"], ""),
];

static FIG_11: [RefSection; 7] = [
    s("Labels", &["scale", "t_solid", "t_melt", "t_liquid", "t_boil", "t_gas", "labels", "phase_names", "seg_labels", "plateau_labels", "letters", "plateau_dashes", "temp_labels"], ""),
    s("Curve", &["curve_color", "supercool"], ""),
    s("Titles", &["mode", "substance", "mp", "bp", "start", "end", "title"], ""),
    s("x-axis", &["xtitle_style", "xtitle", "xaxis_fit", "xaxis"], ""),
    s("y-axis", &["ylabel", "ytitle_style"], ""),
    s("More options", &["label_size", "title_style", "xaxis_from", "xaxis_to", "xaxis_by", "xaxis_num", "xaxis_lab", "xaxis_lab_text", "xaxis_lo", "xaxis_hi", "yaxis_from", "yaxis_to", "yaxis_by", "yaxis_num", "yaxis_lab", "yaxis_lab_text", "yaxis_lo", "yaxis_hi"], ""),
    s("Grid", &["minor_grid"], ""),
];

static FIG_12: [RefSection; 6] = [
    s("Object", &["obj", "obj_lab_mode", "obj_lab", "size", "pos", "two", "n", "joined", "spacing"], ""),
    items("Object"),
    s("Ramp", &["angle", "show_angle", "ang_lab_mode", "ang_lab", "surface"], ""),
    s("Forces and motion", &["gravity", "grav_lab_mode", "grav_lab", "normal", "norm_lab_mode", "norm_lab", "friction", "fric_lab_mode", "fric_lab", "applied", "app_lab_mode", "app_lab", "velocity", "vel_lab_mode", "vel_lab", "accel", "acc_lab_mode", "acc_lab", "tension", "ten_lab_mode", "ten_lab", "contact", "cont_lab_mode", "cont_lab"], ""),
    s("Marks", &["mark_len", "len_lab", "mark_h", "h_lab"], ""),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_13: [RefSection; 6] = [
    s("Heights", &["height", "height_label"], ""),
    s("Diagonals", &["diagonals"], ""),
    s("Labels", &["unit", "dec", "right", "ab_lab", "ab_label", "bc_lab", "bc_label", "cd_lab", "cd_label", "da_lab", "da_label", "a_lab", "a_label", "a_arcs", "b_lab", "b_label", "b_arcs", "c_lab", "c_label", "c_arcs", "d_lab", "d_label", "d_arcs", "ac_lab", "ac_label", "bd_lab", "bd_label", "arrow_marks", "label_offsets"], "Drag any label on the figure to move it."),
    s("Position", &["bottom", "flip", "rotate"], ""),
    s("More options", &["name_a", "name_b", "name_c", "name_d", "m_ab", "m_da", "m_a", "m_b", "m_d", "ab_ticks", "bc_ticks", "cd_ticks", "da_ticks", "vertices", "fill", "label_size", "line_style", "height_text"], ""),
    s("More options", &["ab_arrows", "bc_arrows", "cd_arrows", "da_arrows"], ""),
];

static FIG_14: [RefSection; 6] = [
    s("Ruler", &["units", "ruler", "marks", "read_to"], ""),
    s("Length and position", &["length", "offset"], "From 0.50 to 15.00 cm, to 2 decimal places."),
    s("Object", &["object", "dashes"], ""),
    s("Title and answer key", &["answer", "title"], ""),
    s("More options", &["title_style"], ""),
    s("Magnifiers", &["mag_show", "mag_marks"], ""),
];

static FIG_15: [RefSection; 6] = [
    s("Structure", &["formula", "preset"], "Built around O as the central atom."),
    s("Look", &["shape", "bond_style", "formal"], ""),
    s("Question", &["question"], ""),
    s("Changes", &["central", "tweak"], "Make a wrong structure for a \u{201c}find the mistake\u{201d} question. Click an atom or bond in the figure, or change one below."),
    s("Title and answer key", &["title_style", "title", "answer"], ""),
    s("More options", &["lone", "expand", "resonance", "resonance_pick"], ""),
];

static FIG_16: [RefSection; 6] = [
    s("Strips", &["custom", "strips"], ""),
    s("Look", &["style", "brightness"], "Bright colored lines on black, as a glowing gas gives off."),
    s("Labels", &["labels", "label_style"], "Each element strip’s name, e.g. Hydrogen."),
    s("Wavelength axis", &["nm_from", "nm_to", "tick_every", "num_every", "axis_pos", "axis_title_style", "axis_title"], "Visible light runs from about 380 to 780 nm; every strip is the same width whatever the range."),
    s("Chart title and answer key", &["title"], ""),
    s("More options", &["hydrogen", "helium", "neon", "sodium", "mercury", "lithium", "calcium", "title_style", "answer"], ""),
];

static FIG_17: [RefSection; 3] = [
    s("Or paste ordered pairs", &["paste"], "This replaces both lists and the arrows."),
    s("Titles", &["in_mode", "title_in", "out_mode", "title_out", "title_mode", "title"], ""),
    s("Sides", &["shape", "fill", "arrow_color", "label_size"], ""),
];

static FIG_18: [RefSection; 5] = [
    s("Balance", &["balance", "pan"], "Weighs up to 610 g, read to 0.01 g. The riders are placed for the mass you type."),
    s("Mass", &["mass", "places"], "From 0 to 610 g, to 2 decimal places."),
    s("Title and answer key", &["title"], ""),
    s("More options", &["answer", "title_style"], ""),
    s("Magnifier", &["mag_show", "mag_marks"], ""),
];

static FIG_19: [RefSection; 7] = [
    s("Isotopes", &["source", "element", "custom", "molecule"], ""),
    s("Peaks", &["relative", "values"], ""),
    s("For the student", &["show_name", "custom_name", "leave_out", "answer"], ""),
    s("Titles", &["title_style", "title", "xtitle_style", "xtitle", "ytitle_style", "ytitle"], ""),
    s("x-axis", &["xaxis_fit", "xaxis_from", "xaxis_to", "xaxis_by", "xaxis_num", "xaxis_lab", "xaxis_lab_text", "xaxis_lo", "xaxis_hi"], ""),
    s("y-axis", &["yaxis_from", "yaxis_to", "yaxis_by", "yaxis_num", "yaxis_lab", "yaxis_lab_text", "yaxis_lo", "yaxis_hi"], ""),
    s("Grid", &["minor_grid"], ""),
];

static FIG_20: [RefSection; 3] = [
    s("Micropipette", &["size", "tip", "size_label", "shape"], ""),
    s("Reading", &["volume"], "A 20 µL pipette is set from 2 to 20 µL, in steps of 0.1 µL."),
    s("Color", &["color", "decimal_line", "answer", "title", "answer_text"], "Red digits show where the decimal point goes. The line keeps that clear on a black and white copy."),
];

static FIG_21: [RefSection; 7] = [
    s("Motion", &["n", "x0", "tangent_t"], ""),
    items("Segment"),
    s("Graphs", &["show", "ranges"], ""),
    s("Marks", &["letters", "styles", "tangent"], ""),
    s("Titles", &["title_mode", "title", "xt_mode", "xt", "yt_pos_mode", "yt_pos", "yt_vel_mode", "yt_vel", "yt_acc_mode", "yt_acc", "label_size"], "Type ^ for a superscript, as in m/s^2, and _ for a subscript."),
    s("Axes and grid", &["numbers", "grid", "t_min", "t_max", "t_step", "p_min", "p_max", "p_step", "v_min", "v_max", "v_step", "a_min", "a_max", "a_step"], ""),
    s("Figure", &["color"], ""),
];

static FIG_22: [RefSection; 1] = [
    s("Line", &["min", "max", "step", "label_every", "minor", "numbers", "arrows", "inequality", "points", "label", "ineq_color", "pt_color", "step_text", "frac_style", "inequality2", "pt_values"], ""),
];

static FIG_23: [RefSection; 6] = [
    s("Atom or ion", &["element", "charge"], ""),
    s("Configuration", &["order", "exceptions"], ""),
    s("Look", &["layout", "shorthand", "arrows", "orbitals", "edits"], ""),
    s("For the student", &["blank", "config", "caption", "config_blank", "symbol_style", "sublabels"], ""),
    s("Changes", &["extra_empty"], "Click an orbital to change its electrons, for a “which rule is broken?” question."),
    s("Title and answer key", &["title", "title_style", "answer"], ""),
];

static FIG_24: [RefSection; 2] = [
    s("Figure", &["lines", "transversals", "name_m", "name_n", "name_p", "name_t", "name_u", "angle", "angle_b", "gap", "labels", "marked", "unknown", "angle_text", "names", "dec", "turn", "line_ends", "label_size"], ""),
    s("More options", &["m_style", "m_arrows", "n_style", "n_arrows", "p_style", "p_arrows", "t_style", "t_arrows", "u_style", "u_arrows"], ""),
];

static FIG_25: [RefSection; 6] = [
    s("Heights", &["kind", "height", "height_label"], ""),
    s("Diagonals", &["diagonals"], ""),
    s("Labels", &["unit", "dec", "right", "ab_lab", "ab_label", "bc_lab", "bc_label", "cd_lab", "cd_label", "da_lab", "da_label", "a_lab", "a_label", "a_arcs", "b_lab", "b_label", "b_arcs", "c_lab", "c_label", "c_arcs", "d_lab", "d_label", "d_arcs", "ac_lab", "ac_label", "bd_lab", "bd_label", "arrow_marks", "label_offsets"], "Drag any label on the figure to move it."),
    s("Position", &["bottom", "flip", "rotate"], ""),
    s("More options", &["name_a", "name_b", "name_c", "name_d", "m_ab", "m_da", "m_a", "m_b", "ab_ticks", "bc_ticks", "cd_ticks", "da_ticks", "vertices", "fill", "label_size", "line_style", "height_text"], ""),
    s("More options", &["ab_arrows", "bc_arrows", "cd_arrows", "da_arrows"], ""),
];

static FIG_26: [RefSection; 6] = [
    s("Layout", &["layout", "boxes", "state", "state2", "gaps"], ""),
    s("Particles", &["d_count", "d_shape", "d_size", "d_shade", "shuffle", "a2_count", "b2_count", "c2_count", "d2_count"], ""),
    s("Key", &["key_show", "key_list", "note"], ""),
    s("Box", &["border"], ""),
    s("Chart title", &["title", "lab_before", "lab_after"], ""),
    s("More options", &["a_count", "a_shape", "a_size", "a_shade", "b_count", "b_shape", "b_size", "b_shade", "c_count", "c_shape", "c_size", "c_shade", "a_charge", "a_osize", "a_oshade", "a_ocharge", "a_name", "a_oname", "b_charge", "b_osize", "b_oshade", "b_ocharge", "b_name", "b_oname", "c_charge", "c_osize", "c_oshade", "c_ocharge", "c_name", "c_oname", "d_charge", "d_osize", "d_oshade", "d_ocharge", "d_name", "d_oname", "title_style"], ""),
];

static FIG_27: [RefSection; 3] = [
    s("Symbols", &["carrier_style", "carriers", "symbol_size"], ""),
    s("Labels", &["genotypes", "allele_letter", "gen_labels", "numbers", "key", "gen_prefix"], ""),
    s("Chart title", &["title", "answer"], ""),
];

static FIG_28: [RefSection; 4] = [
    s("Instrument", &["instrument", "places", "meter"], ""),
    s("pH", &["ph"], "From 0 to 14, to 2 decimal places."),
    s("Title and answer key", &["title"], ""),
    s("More options", &["answer", "title_style"], ""),
];

static FIG_29: [RefSection; 5] = [
    s("Element", &["element", "compare"], ""),
    s("Energy axis", &["scale", "units", "gridlines"], ""),
    s("Peaks", &["drawn", "sublabel_style", "labels", "counts", "source", "custom", "energies"], ""),
    s("For the student", &["names", "numbered"], ""),
    s("Title and answer key", &["title", "title_style", "answer"], ""),
];

static FIG_30: [RefSection; 9] = [
    s("Population", &["growth", "n0", "r", "k", "tspan", "unit", "organism", "fit"], ""),
    s("Graphs", &["graph"], "The growth rate dN/dt is individuals added per year; the per capita growth rate, (dN/dt)/N, is per individual."),
    s("Curve", &["curve_color", "draw_curve"], ""),
    s("Census data", &["census", "census_auto", "census_every", "census_scatter", "census_seed", "census_table"], ""),
    s("Marked on the graph", &["mark_k", "mark_infl", "k_label", "tau", "k_label_style", "mark_phases"], ""),
    s("Titles", &["x_title", "y_title", "title", "curve_width", "title_style", "xtitle_style", "ytitle_style", "rate_title", "pc_title", "label_size"], ""),
    s("x-axis", &["x_max", "x_min", "x_step", "x_numbers", "x_end", "x_end_text", "x_lo_end", "x_hi_end", "y_step"], ""),
    s("y-axis", &["y_max", "blank", "y_min", "y_numbers", "y_end", "y_end_text", "y_lo_end", "y_hi_end", "y2_max", "y2_step"], ""),
    s("Grid", &["minor"], ""),
];

static FIG_31: [RefSection; 8] = [
    s("Predator and prey", &["pair", "prey", "pred", "source", "prey0", "pred0", "prey_avg", "pred_avg", "alpha", "beta", "gamma", "delta", "cycle", "tshow", "unit"], ""),
    s("Graph", &["graph", "show", "style", "y_axis"], ""),
    s("Data", &["data", "census_every", "census_noise", "census_seed", "census_join"], ""),
    s("Marked", &["marks", "mark_lag", "mark_period", "lag_labels"], ""),
    s("Titles", &["x_title", "y_title", "title", "line_width", "legend", "title_style", "xtitle_style", "ytitle_style", "label_size"], ""),
    s("x-axis", &["x_min", "x_max", "x_step", "x_fit", "x_numbers", "x_end", "x_end_text", "x_lo_end", "x_hi_end"], ""),
    s("y-axis", &["y_min", "y_max", "y_step", "y_fit", "y_numbers", "y_end", "y_end_text", "y_lo_end", "y_hi_end"], ""),
    s("Grid", &["minor"], ""),
];

static FIG_32: [RefSection; 3] = [
    s("Labels", &["base", "lean", "length", "width", "n", "side", "height", "names", "length_lab", "length_label", "width_lab", "width_label", "height_lab", "height_label", "side_lab", "side_label", "hyp_lab", "hyp_label", "apothem_lab", "apothem_label", "lean_lab", "lean_label", "edge", "edge_lab", "edge_label", "label_offsets"], "Drag any label on the figure to move it."),
    s("Position", &["stands", "sits", "depth"], ""),
    s("More options", &["hidden", "right", "unit", "dec", "label_size"], ""),
];

static FIG_33: [RefSection; 6] = [
    s("Launch", &["from", "angle", "show_angle", "ang_lab_mode", "ang_lab", "vx_lab_mode", "vx_lab", "vy_lab_mode", "vy_lab"], ""),
    s("Object", &["object", "size", "ball_at"], ""),
    s("Path", &["path", "steps", "letters"], ""),
    s("Velocity and gravity", &["show_v", "v_lab_mode", "v_lab", "comps", "show_g", "g_lab_mode", "g_lab"], ""),
    s("Marks", &["cliff", "mark_h", "h_lab", "mark_r", "r_lab", "mark_cliff", "cliff_lab"], ""),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_34: [RefSection; 4] = [
    s("Setup", &["setup", "l_kind"], ""),
    s("Objects", &["l_lab_mode", "l_lab", "l_size", "r_lab_mode", "r_lab", "r_size", "lower", "third", "third_side", "t_lab_mode", "t_lab", "t_size", "second", "s_lab", "strands", "angle", "ang_lab_mode", "ang_lab", "surface"], ""),
    s("Forces and motion", &["tension", "tens_lab_mode", "tens_lab", "gravity", "grav_lab_mode", "grav_lab", "normal", "norm_lab_mode", "norm_lab", "friction", "fric_lab_mode", "fric_lab", "accel", "acc_lab_mode", "acc_lab"], ""),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_35: [RefSection; 5] = [
    s("Phenotypes", &["phenotypes"], "Names for what each allele looks like, for the summary, key and cells."),
    s("Square", &["cross", "dominance", "parents", "filled", "parent_labels", "blank_parents", "blank_gametes", "cells", "blank_cells", "cell_phenotype"], ""),
    s("Shading", &["shading", "shade_style", "shade_color"], ""),
    s("Ratios", &["geno_ratio", "geno_ratio_style", "pheno_ratio", "pheno_ratio_style", "ratio_form", "sex_split"], "Genotypes and phenotypes under the square."),
    s("Title and question", &["title", "question", "outline_heavy", "label_size"], ""),
];

static FIG_36: [RefSection; 4] = [
    s("Lines", &["base", "lean", "length", "width", "n", "side", "height", "show_height", "slant"], ""),
    s("Labels", &["names", "length_lab", "length_label", "width_lab", "width_label", "side_lab", "side_label", "height_lab", "height_label", "slant_lab", "slant_label", "apothem_lab", "apothem_label", "lean_lab", "lean_label", "label_offsets"], "Drag any label on the figure to move it."),
    s("Position", &["stands", "depth"], ""),
    s("More options", &["hidden", "right", "unit", "dec", "label_size"], ""),
];

static FIG_37: [RefSection; 5] = [
    s("Diagonals", &["diagonals"], ""),
    s("Labels", &["unit", "dec", "right", "ab_lab", "ab_label", "bc_lab", "bc_label", "cd_lab", "cd_label", "da_lab", "da_label", "a_lab", "a_label", "a_arcs", "b_lab", "b_label", "b_arcs", "c_lab", "c_label", "c_arcs", "d_lab", "d_label", "d_arcs", "ac_lab", "ac_label", "bd_lab", "bd_label", "arrow_marks", "label_offsets"], "Drag any label on the figure to move it."),
    s("Position", &["kind", "bottom", "flip", "rotate"], ""),
    s("More options", &["name_a", "name_b", "name_c", "name_d", "m_ab", "m_bc", "ab_ticks", "bc_ticks", "cd_ticks", "da_ticks", "vertices", "fill", "label_size", "line_style"], ""),
    s("More options", &["ab_arrows", "bc_arrows", "cd_arrows", "da_arrows"], ""),
];

static FIG_38: [RefSection; 3] = [
    s("Center and lines", &["n", "from", "value", "side_lab", "side_label", "side_ticks", "ang_lab", "ang_label", "ang_arcs", "center_dot", "center_name", "apothem", "apothem_label", "apothem_text", "radius", "radius_label", "radius_text", "all_radii", "dec"], ""),
    s("Labels", &["vertices", "vnames", "unit", "right", "label_offsets"], "Drag any label on the figure to move it."),
    s("Position", &["rotate", "fill", "flip", "label_size", "line_style"], ""),
];

static FIG_39: [RefSection; 5] = [
    s("Ruler", &["marks", "ruler", "read_to", "marbles", "magnify"], ""),
    s("Length and position", &["start", "length", "guides", "show_value", "dec", "ruler_color"], "From 0.50 to 15.00 cm, to 2 decimal places."),
    s("Magnifiers", &["mag_marks"], ""),
    s("Title and answer key", &["title_mode", "title", "answer"], ""),
    s("Object", &["obj_color", "object"], ""),
];

static FIG_40: [RefSection; 4] = [
    s("Lines", &["draw", "radius", "face", "give_d"], ""),
    s("More options", &["view", "hidden", "right", "unit", "dec", "label_size"], ""),
    s("More options", &["diam"], ""),
    s("Labels", &["r_lab", "r_label", "d_lab", "d_label", "label_offsets"], "Drag any label on the figure to move it."),
];

static FIG_41: [RefSection; 5] = [
    s("Spring scale", &["cap", "units", "coded", "minor"], ""),
    s("Reading", &["reading", "offset", "show_zero"], "From 0 to 10 N, to 2 decimal places."),
    s("On the hook", &["hook", "block_lab"], ""),
    s("Magnifier", &["masses", "view", "mag_marks"], ""),
    s("Title and answer key", &["title", "answer"], ""),
];

static FIG_42: [RefSection; 5] = [
    s("Thermometer", &["thermometer", "unit", "liquid"], ""),
    s("Temperature", &["temperature", "places"], "From -10 to 110 °C, to one decimal place."),
    s("Title and answer key", &["title"], ""),
    s("More options", &["answer", "title_style"], ""),
    s("Magnifier", &["mag_show", "mag_marks"], ""),
];

static FIG_43: [RefSection; 8] = [
    s("Titration", &["kind", "common", "source", "molarity", "volume", "titrant", "pk", "kp_start", "kp_end", "kp_veq", "kp_eqph"], ""),
    s("Marked points", &["equiv", "eq_label_style", "eq_label", "half_mark", "half_label_style", "half_label", "half"], ""),
    s("Curve", &["curve_color"], ""),
    s("Titles", &["title_style", "title", "xtitle_style", "xlabel", "ytitle_style", "ytitle"], ""),
    s("x-axis", &["xaxis_from", "xaxis_to", "xaxis_by", "xaxis_num", "xaxis_lab", "xaxis_lab_text", "xaxis_lo", "xaxis_hi"], ""),
    s("y-axis", &["yaxis_from", "yaxis_to", "yaxis_by", "yaxis_num", "yaxis_lab", "yaxis_lab_text", "yaxis_lo", "yaxis_hi"], ""),
    s("Grid", &["minor_grid"], ""),
    s("More options", &["answer", "label_size", "xaxis_fit", "xmax"], ""),
];

static FIG_44: [RefSection; 6] = [
    s("Heights", &["kind", "m_h", "show_height", "hfrom", "height_label"], ""),
    s("Diagonals", &["diagonals"], ""),
    s("Labels", &["unit", "dec", "right", "ab_lab", "ab_label", "bc_lab", "bc_label", "cd_lab", "cd_label", "da_lab", "da_label", "a_lab", "a_label", "a_arcs", "b_lab", "b_label", "b_arcs", "c_lab", "c_label", "c_arcs", "d_lab", "d_label", "d_arcs", "ac_lab", "ac_label", "bd_lab", "bd_label", "arrow_marks", "label_offsets"], "Drag any label on the figure to move it."),
    s("Position", &["bottom", "flip", "rotate"], ""),
    s("More options", &["name_a", "name_b", "name_c", "name_d", "m_ab", "m_bc", "m_cd", "m_da", "m_a", "m_b", "m_c", "m_d", "ab_ticks", "bc_ticks", "cd_ticks", "da_ticks", "vertices", "fill", "label_size", "line_style", "height_text"], ""),
    s("More options", &["ab_arrows", "bc_arrows", "cd_arrows", "da_arrows"], ""),
];

static FIG_45: [RefSection; 4] = [
    s("Heights", &["name_a", "name_b", "name_c", "m_a", "m_b", "m_c", "m_ab", "m_bc", "m_ca", "other", "height", "height_label"], ""),
    s("Labels", &["unit", "dec", "right", "a_lab", "a_label", "a_arcs", "b_lab", "b_label", "b_arcs", "c_lab", "c_label", "c_arcs", "ab_lab", "ab_label", "bc_lab", "bc_label", "ca_lab", "ca_label", "label_offsets"], "Drag any label on the figure to move it."),
    s("Position", &["bottom", "flip", "rotate"], ""),
    s("More options", &["ab_ticks", "bc_ticks", "ca_ticks", "vertices", "fill", "label_size", "line_style", "height_text"], ""),
];

static FIG_46: [RefSection; 4] = [
    s("Vectors", &["n"], "Drawn to scale, head to tail, in order. One grid square is a magnitude of 1."),
    s("Resultant", &["resultant", "res_style", "res_lab_mode", "res_lab", "mark_angle", "ang_lab_mode", "ang_lab", "components"], "From the first tail to the last tip: 5 squares at 36.9°."),
    s("Grid and axes", &["grid", "axes", "bold"], ""),
    s("Figure", &["mirror", "color"], ""),
];

static FIG_47: [RefSection; 8] = [
    s("Graduated cylinder", &["size", "color"], ""),
    s("Scale", &["marks", "numbered", "unit"], ""),
    s("Readings", &["before", "after", "dotted"], "From 0 to 10 mL, to 2 decimal places."),
    s("Object", &["object", "count", "marble_color"], ""),
    s("Captions", &["lab_before", "lab_after", "arrow"], ""),
    s("Title and answer key", &["title"], ""),
    s("More options", &["answer", "title_style"], ""),
    s("Magnifiers", &["mag_show", "mag_marks"], ""),
];

static FIG_48: [RefSection; 6] = [
    s("Instrument", &["instrument", "size", "color"], ""),
    s("Scale", &["marks", "numbered", "unit"], ""),
    s("Reading", &["reading", "dotted"], "From 0 to 100 mL, to one decimal place."),
    s("Title and answer key", &["answer", "title"], ""),
    s("More options", &["title_style"], ""),
    s("Magnifier", &["mag_show", "mag_marks"], ""),
];

static FIG_49: [RefSection; 7] = [
    s("Wave", &["kind", "against", "amp", "wl", "cycles"], "In the units of the axes."),
    s("Marks", &["mark_wl", "wl_lab_mode", "wl_lab", "mark_amp", "amp_lab_mode", "amp_lab", "mark_ct", "crest_mode", "trough_mode"], ""),
    s("Graph", &["numbers", "color"], ""),
    s("Titles", &["title_mode", "title", "xt_mode", "xt", "yt_mode", "yt", "label_size"], ""),
    s("x-axis", &["xaxis_fit", "xmin", "xmax", "xstep", "xaxis_num", "xaxis_letter"], ""),
    s("y-axis", &["yaxis_fit", "ymin", "ymax", "ystep", "yaxis_num", "yaxis_letter"], ""),
    s("Grid", &["grid", "minor"], ""),
];

/// Statistics figures (original designs, not mirrored from a reference page).
static HISTOGRAM: [RefSection; 5] = [
    s("Data", &["data"], "Type or paste numbers, open a CSV, or generate sample data from a seed."),
    s("Bins", &["auto_bins", "bin_width", "bin_start", "closed"], ""),
    s("Bars", &["measure", "auto_y", "y_max", "values", "show_answers"], "Turn off Show answers for a student copy: the bars and values are hidden and the axes stay."),
    s("Labels and text", &["title", "x_title", "y_title"], ""),
    s("Appearance", &["style", "label_size", "grid"], ""),
];

static SCATTERPLOT: [RefSection; 4] = [
    s("Data", &["pts"], "Type or paste x, y pairs (two columns from a spreadsheet work), open a CSV, or generate sample data from a seed."),
    s("Line of best fit", &["fit", "eq", "rval", "show_answers"], "Turn off Show answers for a student copy: the line, equation and r are hidden and the points stay."),
    s("Labels and text", &["title", "x_title", "y_title"], ""),
    s("Appearance", &["point_size", "style", "label_size", "grid"], ""),
];

static NORMAL_CURVE: [RefSection; 4] = [
    s("Distribution", &["mean", "sd"], ""),
    s("Shaded region", &["region", "by", "a", "b", "show_prob", "show_answers"], "Turn off Show answers for a student copy: the shaded region stays and the probability is hidden."),
    s("Labels and text", &["title", "x_title", "ticks"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static DOT_PLOT: [RefSection; 5] = [
    s("Data", &["data", "group"], "Type or paste numbers, open a CSV, or generate sample data from a seed. Grouping rounds values so close ones share a stack."),
    s("Number line", &["axis_auto", "axis_min", "axis_max", "axis_step"], ""),
    s("Dots and marks", &["dot_size", "marks", "show_answers"], "Turn off Show answers for a student copy: the dots and marks are hidden and the number line stays."),
    s("Labels and text", &["title", "x_title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static STEM_AND_LEAF: [RefSection; 4] = [
    s("Data", &["data", "leaf_unit", "split", "order"], "Type or paste numbers, open a CSV, or generate sample data from a seed. Each number is rounded to the leaf unit."),
    s("Key and answers", &["key", "show_answers"], "Turn off Show answers for a student copy: the leaves are hidden and the stems and key stay."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["label_size"], ""),
];

static TWO_WAY_TABLE: [RefSection; 4] = [
    s("Table", &["row_labels", "col_labels", "cells", "row_title", "col_title"], "Name the rows and columns, then type the counts one line per row. The totals are worked out for you."),
    s("Student copy", &["blank", "blank_count", "show_answers"], "Turn off Show answers for a student copy: the chosen boxes are left empty. Show answers fills them in, in bold."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static LOGIC_CIRCUIT: [RefSection; 5] = [
    s("Circuit", &["source", "spec", "expr", "symbols"], "List the inputs, then the gates, then the outputs, one per line. Or type a boolean expression and the gates are drawn for you."),
    s("Truth table and answers", &["table", "show_expr", "show_answers"], "Turn off Show answers for a student copy: the output column, the expression and the values inside the circuit are hidden."),
    s("Try the inputs", &["try"], "Type 0s and 1s, one per input, to light up the wires and mark that row of the table."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static GRAPH_DIAGRAM: [RefSection; 4] = [
    s("Graph", &["spec", "layout", "weights"], "List the nodes, then the edges, one per line. Directed edges use ->, weights follow a colon."),
    s("Work out", &["answer", "start", "end", "mark", "show_answers"], "Turn off Show answers for a student copy: the answer and the marks on the graph are hidden and the heading stays."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static TREE_DIAGRAM: [RefSection; 4] = [
    s("Tree", &["source", "values", "spec"], "Grow a binary search tree from values, or write any tree as parent lines."),
    s("Work out", &["answer", "mark", "show_answers"], "Turn off Show answers for a student copy: the answer and the numbers on the tree are hidden and the heading stays."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static NUMBER_BASE: [RefSection; 3] = [
    s("Table", &["values", "given", "width", "place_values", "show_answers"], "Give one column; turn off Show answers for empty boxes in the others."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static DATA_STRUCTURE: [RefSection; 3] = [
    s("Structure", &["kind", "values", "ops", "indices", "show_answers"], "List the values, then optional operations; turn off Show answers for an empty box where the result goes."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static FLOWCHART: [RefSection; 3] = [
    s("Flowchart", &["spec", "blank", "show_answers"], "One box per line, then the arrows; name boxes to leave empty in the student copy."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static BOOLEAN_TABLE: [RefSection; 4] = [
    s("Function", &["source", "expr", "vars", "minterms", "dontcares", "output"], "Give an expression, or the rows where the output is 1."),
    s("Display", &["show", "row_numbers", "groups", "simplify", "show_answers"], "Turn off Show answers for a student copy: outputs, groups and the simplified form are left empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static STATE_MACHINE: [RefSection; 4] = [
    s("Machine", &["spec", "direction"], "Name the start and accepting states, then the transitions."),
    s("Work out", &["answer", "input", "show_answers"], "Turn off Show answers for a student copy: the table cells and the trace are left empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static TRACE_TABLE: [RefSection; 4] = [
    s("Program", &["program", "rows", "numbers"], "A few lines of pseudocode with at most one loop."),
    s("Student copy", &["given", "show_answers"], "Turn off Show answers to leave the later rows empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static SORTING_PASSES: [RefSection; 4] = [
    s("Sort", &["algorithm", "values", "descending", "counts"], "Choose the algorithm and the values to sort."),
    s("Student copy", &["given", "show_answers"], "Turn off Show answers to leave the passes empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static RECURSION_TREE: [RefSection; 4] = [
    s("Function", &["func", "a", "b"], "Choose the function and its starting values."),
    s("Display", &["number", "returns", "show_answers"], "Turn off Show answers to leave the return values and numbers empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static ER_DIAGRAM: [RefSection; 4] = [
    s("Diagram", &["spec", "layout", "attributes"], "Entities with their attributes, then the relationships between them."),
    s("Student copy", &["hide_attrs", "show_answers"], "Turn off Show answers to leave empty boxes where the cardinalities go."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static NETWORK_TOPOLOGY: [RefSection; 4] = [
    s("Network", &["kind", "devices", "names", "centre"], "Choose the layout and how many computers."),
    s("Notes", &["facts", "show_answers"], "Turn off Show answers to leave the name and the notes empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static PROBABILITY_TREE: [RefSection; 4] = [
    s("Branches", &["first", "second", "third"], "Outcomes with their probabilities, stage by stage."),
    s("Work out", &["products", "blank_probs", "show_answers"], "Turn off Show answers to leave the path probabilities blank."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static VENN_DIAGRAM: [RefSection; 4] = [
    s("Sets", &["sets", "a_name", "b_name", "c_name", "counts"], "How many sets, their names and the value in each region."),
    s("Work out", &["shade", "totals", "show_answers"], "Turn off Show answers to leave the regions and totals empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static DISCRETE_DISTRIBUTION: [RefSection; 4] = [
    s("Distribution", &["dist", "n", "p", "lambda"], "Binomial or Poisson, with its parameters."),
    s("Shading", &["region", "a", "b", "show_prob", "values", "show_answers"], "Turn off Show answers to hide the probability and keep the shaded bars."),
    s("Labels and text", &["title", "x_title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static CONFIDENCE_INTERVAL: [RefSection; 4] = [
    s("Sample", &["kind", "estimate", "sd", "n", "level"], "What was measured and the confidence level."),
    s("Marking", &["mark", "show_answers"], "Turn off Show answers to hide the interval and the working."),
    s("Labels and text", &["title", "x_title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static RESIDUAL_PLOT: [RefSection; 4] = [
    s("Data and view", &["pts", "view", "table", "sse", "point_size", "grid"], "The pairs, which panels to draw and what to work out."),
    s("Marking", &["show_answers"], "Turn off Show answers to leave the line and the residuals for students."),
    s("Labels and text", &["title", "x_title", "y_title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static RC_CURVE: [RefSection; 4] = [
    s("Circuit", &["mode", "quantity", "v", "r", "c"], "The capacitor, the resistor and the battery."),
    s("Working", &["marks", "readout", "grid", "show_answers"], "Turn off Show answers to hide the curve and the working."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static KIRCHHOFF_LOOPS: [RefSection; 4] = [
    s("Circuit", &["v1", "v2", "r1", "r2", "r3", "loops"], "Two batteries and three resistors."),
    s("Working", &["working", "show_answers"], "Turn off Show answers to leave the equations and currents blank."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static T_ACCOUNT: [RefSection; 4] = [
    s("Accounts", &["spec", "descriptions", "balance"], "The accounts and their debits and credits."),
    s("Money", &["currency", "cents", "show_answers"], "Turn off Show answers to leave the balancing figure and totals empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static JOURNAL_ENTRY: [RefSection; 4] = [
    s("Entries", &["spec", "totals"], "Dates, debits, credits and narrations."),
    s("Money", &["currency", "cents", "show_answers"], "Turn off Show answers to leave the accounts and amounts empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static TRIAL_BALANCE: [RefSection; 4] = [
    s("Balances", &["spec", "totals"], "Each account with the side its balance is on."),
    s("Money", &["currency", "cents", "show_answers"], "Turn off Show answers to leave the columns and totals empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static FINANCIAL_STATEMENT: [RefSection; 3] = [
    s("Statement", &["kind", "is_spec", "bs_spec"], "Which statement, and the accounts under each heading."),
    s("Labels and money", &["business", "period", "currency", "cents", "show_answers"], "Turn off Show answers to leave the totals empty."),
    s("Appearance", &["style", "label_size"], ""),
];

static ACCOUNTING_EQUATION: [RefSection; 4] = [
    s("Transactions", &["spec", "totals"], "What happens, and its effect on assets, liabilities and equity."),
    s("Money", &["currency", "cents", "show_answers"], "Turn off Show answers to leave the effects empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static DEPRECIATION_SCHEDULE: [RefSection; 4] = [
    s("Asset", &["asset", "cost", "salvage", "life", "method"], "What is depreciated, and how."),
    s("Money", &["currency", "cents", "show_answers"], "Turn off Show answers to leave the table empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static BANK_RECONCILIATION: [RefSection; 4] = [
    s("Reconciliation", &["spec", "check"], "The two balances and the adjustments to each."),
    s("Money", &["currency", "cents", "show_answers"], "Turn off Show answers to leave the adjusted balances empty."),
    s("Labels and text", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static CASH_FLOW_STATEMENT: [RefSection; 3] = [
    s("Activities", &["spec", "opening", "closing_given"], "The cash flows by activity, and the opening cash."),
    s("Labels and money", &["business", "period", "currency", "cents", "show_answers"], "Turn off Show answers to leave the totals empty."),
    s("Appearance", &["style", "label_size"], ""),
];

static SUPPLY_DEMAND: [RefSection; 4] = [
    s("Curves", &["d_int", "d_slope", "s_int", "s_slope"], "Each curve from its price at quantity 0 and its slope."),
    s("Shifts and price controls", &["d_shift", "d_shift_by", "s_shift", "s_shift_by", "control", "control_at"], "Move a curve, or add a ceiling or floor."),
    s("Display", &["shading", "numbers", "title", "show_answers"], "Turn off Show answers to draw only the curves."),
    s("Appearance", &["style", "label_size"], ""),
];

static ELASTICITY: [RefSection; 4] = [
    s("Two points", &["p1", "q1", "p2", "q2"], "A price and the quantity demanded at it, twice."),
    s("Working", &["method", "show_answers"], "Turn off Show answers to leave the working blank."),
    s("Display", &["numbers", "title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static PPF: [RefSection; 4] = [
    s("Frontier", &["x_name", "y_name", "x_max", "y_max", "shape"], "The goods and the most of each."),
    s("Points", &["points", "from", "to"], "Named points to class, and a move to cost."),
    s("Display", &["numbers", "title", "show_answers"], "Turn off Show answers to leave the classes blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static COST_CURVES: [RefSection; 4] = [
    s("Costs", &["fc", "a", "b", "c"], "Fixed cost and the variable cost function."),
    s("Price and axis", &["price", "qmax"], "The market price, and how far the graph runs."),
    s("Display", &["numbers", "title", "show_answers"], "Turn off Show answers to draw only the curves."),
    s("Appearance", &["style", "label_size"], ""),
];

static MONOPOLY: [RefSection; 4] = [
    s("Demand", &["d_int", "d_slope"], "The demand curve; marginal revenue is worked out."),
    s("Costs", &["mc_int", "mc_slope", "fixed"], "Marginal cost and the fixed cost."),
    s("Display", &["dwl", "numbers", "title", "show_answers"], "Turn off Show answers to draw only the curves."),
    s("Appearance", &["style", "label_size"], ""),
];

static CIRCULAR_FLOW: [RefSection; 4] = [
    s("Model", &["model", "real", "money"], "Which sectors, and which flows to draw."),
    s("Student copy", &["hide", "show_answers"], "Turn off Show answers to blank the labels or the names."),
    s("Labels", &["title"], ""),
    s("Appearance", &["style", "label_size"], ""),
];

static AD_AS: [RefSection; 4] = [
    s("Curves", &["ad_int", "ad_slope", "as_int", "as_slope", "full"], "AD, short-run AS and the full-employment output."),
    s("Shifts", &["ad_shift", "ad_by", "sras_shift", "sras_by"], "Move AD or SRAS."),
    s("Display", &["numbers", "title", "show_answers"], "Turn off Show answers to draw only the curves."),
    s("Appearance", &["style", "label_size"], ""),
];

static LORENZ: [RefSection; 3] = [
    s("Incomes", &["incomes", "group"], "The income of each equal-sized group."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the cumulative columns and the curve for students."),
    s("Appearance", &["style", "label_size"], ""),
];

static EARTH_CYCLES: [RefSection; 3] = [
    s("Cycle", &["cycle", "extras"], "Rock or water cycle, and whether to add the shortcuts or transpiration."),
    s("Display", &["hide", "title", "show_answers"], "Turn off Show answers to leave labels as empty boxes."),
    s("Appearance", &["style", "label_size"], ""),
];

static EARTH_LAYERS: [RefSection; 3] = [
    s("Layout", &["view", "widen"], "Whole circle or cutaway, and a widened crust."),
    s("Display", &["hide", "title", "show_answers"], "Turn off Show answers to blank the chosen column."),
    s("Appearance", &["style", "label_size"], ""),
];

static PLATE_BOUNDARIES: [RefSection; 3] = [
    s("Boundary", &["kind"], "Which boundary to draw."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave each name blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static STRATA: [RefSection; 3] = [
    s("Layers", &["layers", "structure", "dyke"], "The rocks, how they are folded or faulted, and an optional dyke."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the order of formation blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static MOON_SEASONS: [RefSection; 3] = [
    s("Diagram", &["mode", "hemisphere"], "Moon phases or seasons."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave names as empty boxes."),
    s("Appearance", &["style", "label_size"], ""),
];

static TOPO_CONTOURS: [RefSection; 3] = [
    s("Heights", &["grid", "interval", "unit"], "The grid of heights and the contour interval."),
    s("Display", &["labels", "title", "show_answers"], "Turn off Show answers to leave the contours for students to draw."),
    s("Appearance", &["style", "label_size"], ""),
];

static HR_DIAGRAM: [RefSection; 3] = [
    s("Stars", &["stars"], "Name, temperature and luminosity for each star."),
    s("Display", &["table", "title", "show_answers"], "Turn off Show answers to blank the regions and the Class column."),
    s("Appearance", &["style", "label_size"], ""),
];

static CLIMATE_GRAPH: [RefSection; 3] = [
    s("Data", &["data", "place"], "Temperature then rainfall for each month, January first."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the graph and summary for students."),
    s("Appearance", &["style", "label_size"], ""),
];

static POPULATION_PYRAMID: [RefSection; 3] = [
    s("Data", &["data", "width", "unit"], "Males then females for each age band, youngest first."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the bars and summary for students."),
    s("Appearance", &["style", "label_size"], ""),
];

static GRID_REFERENCE: [RefSection; 4] = [
    s("Map", &["cols", "rows", "origin"], "The size of the grid and the number of its first line."),
    s("Features", &["features", "bearings"], "What is on the map and the bearings to work out."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the references and bearings empty."),
    s("Appearance", &["style", "label_size"], ""),
];

static MAP_SCALE: [RefSection; 3] = [
    s("Scale", &["ratio", "lengths"], "The scale and the measured map distances."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the workings empty."),
    s("Appearance", &["style", "label_size"], ""),
];

static MUSIC_STAFF: [RefSection; 3] = [
    s("Music", &["notes", "clef", "key", "time"], "The notes, clef, key signature and time signature."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the note names and beats blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static CIRCLE_OF_FIFTHS: [RefSection; 3] = [
    s("Circle", &["key"], "A key to shade with its neighbours."),
    s("Display", &["hide", "title", "show_answers"], "Turn off Show answers to blank the names or signatures."),
    s("Appearance", &["style", "label_size"], ""),
];

static PIANO_KEYBOARD: [RefSection; 4] = [
    s("Keys", &["task", "mode", "root", "keys"], "What to mark and which worksheet to make."),
    s("Keyboard", &["octaves", "start"], "How many octaves and where the lowest C is."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave out the answer half."),
    s("Appearance", &["style", "label_size"], ""),
];

static GUITAR_CHORDS: [RefSection; 3] = [
    s("Chords", &["chords", "task", "fret"], "The chords and which worksheet to make."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave out the answer half."),
    s("Appearance", &["style", "label_size"], ""),
];

static RHYTHM_COUNTING: [RefSection; 3] = [
    s("Rhythm", &["rhythm", "time"], "The rhythm and its time signature."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the counts and beat totals blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static BREAK_EVEN: [RefSection; 3] = [
    s("Costs and price", &["fixed", "price", "variable", "expected", "currency", "cents"], "The costs, the price and optional expected sales."),
    s("Display", &["title", "show_answers"], "Turn off Show answers for empty axes, a blank table and blank readouts."),
    s("Appearance", &["style", "label_size"], ""),
];

static TIME_VALUE: [RefSection; 4] = [
    s("Cash flows", &["flows", "rate", "mode", "unit"], "Period and amount per row, the rate and what to work out."),
    s("Money", &["currency", "cents"], ""),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the factors, values and total empty."),
    s("Appearance", &["style", "label_size"], ""),
];

static LOAN_AMORTISATION: [RefSection; 4] = [
    s("The loan", &["principal", "rate", "years", "per_year"], "The amount, the yearly rate, the term and how often it is repaid."),
    s("Table", &["rows", "currency"], "How many rows to show."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the payment and every cell blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static ORG_CHART: [RefSection; 3] = [
    s("Roles", &["roles"], "Indent a role under who it reports to."),
    s("Display", &["title", "show_answers", "hide"], "Turn off Show answers to blank the questions; choose Role titles to empty the boxes too."),
    s("Appearance", &["style", "label_size"], ""),
];

static SWOT: [RefSection; 3] = [
    s("Points", &["strengths", "weaknesses", "opportunities", "threats"], "One point per line."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the boxes empty."),
    s("Appearance", &["style", "label_size"], ""),
];

static MARKETING_FUNNEL: [RefSection; 3] = [
    s("Funnel", &["stages", "spend", "currency", "cents"], "A name and a count per stage, widest first."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the rates and cost blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static PLOT_ARC: [RefSection; 3] = [
    s("Story", &["events", "story"], "One line per stage, in story order."),
    s("Display", &["show_answers"], "Turn off Show answers to leave the stage names blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static ORGANISER: [RefSection; 4] = [
    s("Organiser", &["kind", "title_a", "title_b"], "Which organiser, and the headings of the first two boxes."),
    s("Items", &["items_a", "items_b", "items_c"], "One item per line. The third list is the overlap for a Venn diagram, or Learned for KWL."),
    s("Display", &["title", "show_answers"], "Turn off Show answers for empty boxes and a mixed-up list."),
    s("Appearance", &["style", "label_size"], ""),
];

static TIMELINE: [RefSection; 3] = [
    s("Events", &["events", "unit"], "A date then the event, one per line."),
    s("Display", &["title", "show_answers", "hide"], "Turn off Show answers to hide the dates or the event names."),
    s("Appearance", &["style", "label_size"], ""),
];

static CAUSE_EFFECT: [RefSection; 3] = [
    s("Web", &["event", "causes", "effects"], "The central event, and its causes and effects."),
    s("Display", &["title", "show_answers"], "Turn off Show answers for empty boxes and a mixed-up list."),
    s("Appearance", &["style", "label_size"], ""),
];

static FAMILY_TREE: [RefSection; 3] = [
    s("People", &["people"], "Indent children under their parents; join a couple with a plus."),
    s("Display", &["title", "show_answers", "hide"], "Turn off Show answers to blank the counts, and the names too if chosen."),
    s("Appearance", &["style", "label_size"], ""),
];

static SENTENCE_DIAGRAM: [RefSection; 4] = [
    s("Sentence", &["sentence", "subject", "verb", "object"], "The sentence and its subject, verb and object."),
    s("Describing words", &["subject_mods", "verb_mods", "object_mods"], "One word per line, up to 3 each."),
    s("Display", &["show_answers"], "Turn off Show answers for an empty frame with no words in it."),
    s("Appearance", &["style", "label_size"], ""),
];

static STRUCTURE_CHART: [RefSection; 3] = [
    s("Links", &["links"], "Box A -> Box B : what the arrow means."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the arrow labels blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static FOOD_PLATE: [RefSection; 4] = [
    s("Plate", &["veg", "grains", "protein", "fruit", "dairy"], "Shares are relative; they need not add to 100."),
    s("Foods", &["foods"], "name: group, one per line."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the table empty."),
    s("Appearance", &["style", "label_size"], ""),
];

static HEART_ZONES: [RefSection; 3] = [
    s("Person", &["age", "resting"], "A resting rate of 0 uses the simple percentage method."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the ranges blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static COURT_LAYOUT: [RefSection; 3] = [
    s("Court", &["sport", "formation"], "The formation only applies to soccer."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the position names blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static BODY_MAP: [RefSection; 3] = [
    s("Organs", &["show"], ""),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the names blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static BLOCK_DIAGRAM: [RefSection; 3] = [
    s("System", &["description", "blocks", "signals", "feedback"], "One more signal than blocks at most."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave names and labels blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static FREE_BODY: [RefSection; 4] = [
    s("Object", &["object", "mass"], "A mass above 0 adds the acceleration."),
    s("Forces", &["forces"], "name size direction, one per line."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the arrows and results out."),
    s("Appearance", &["style", "label_size"], ""),
];

static GEAR_TRAIN: [RefSection; 3] = [
    s("Gears", &["teeth", "rpm", "clockwise"], "The first gear is the one that is turned."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the results blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static ORTHOGRAPHIC: [RefSection; 3] = [
    s("Stack", &["heights"], "Back row first, one row per line."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the views empty."),
    s("Appearance", &["style", "label_size"], ""),
];

static COLOUR_WHEEL: [RefSection; 3] = [
    s("Scheme", &["scheme", "base"], "Counts the colours from yellow, clockwise."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the names and scheme blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static PERSPECTIVE_GRID: [RefSection; 3] = [
    s("Grid", &["kind", "horizon", "rays"], "The horizon is the eye level."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the box off."),
    s("Appearance", &["style", "label_size"], ""),
];

static MASLOW: [RefSection; 3] = [
    s("Levels", &["levels"], "name: example, bottom first."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the level names blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static BRAIN_REGIONS: [RefSection; 3] = [
    s("Regions", &["show"], ""),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the names blank."),
    s("Appearance", &["style", "label_size"], ""),
];

static EXPERIMENT_FLOW: [RefSection; 3] = [
    s("Experiment", &["question", "sample", "treatment", "control", "measure"], "What each group gets and what is measured."),
    s("Display", &["title", "show_answers"], "Turn off Show answers to leave the role labels blank."),
    s("Appearance", &["style", "label_size"], ""),
];

pub fn reference(id: &str) -> Option<&'static [RefSection]> {
    Some(match id {
        "angles" => &FIG_0,
        "histogram" => &HISTOGRAM,
        "logic-circuit" => &LOGIC_CIRCUIT,
        "graph-diagram" => &GRAPH_DIAGRAM,
        "tree-diagram" => &TREE_DIAGRAM,
        "number-base" => &NUMBER_BASE,
        "data-structure" => &DATA_STRUCTURE,
        "flowchart" => &FLOWCHART,
        "boolean-table" => &BOOLEAN_TABLE,
        "probability-tree" => &PROBABILITY_TREE,
        "venn-diagram" => &VENN_DIAGRAM,
        "discrete-distribution" => &DISCRETE_DISTRIBUTION,
        "confidence-interval" => &CONFIDENCE_INTERVAL,
        "residual-plot" => &RESIDUAL_PLOT,
        "rc-curve" => &RC_CURVE,
        "t-account" => &T_ACCOUNT,
        "journal-entry" => &JOURNAL_ENTRY,
        "trial-balance" => &TRIAL_BALANCE,
        "financial-statement" => &FINANCIAL_STATEMENT,
        "accounting-equation" => &ACCOUNTING_EQUATION,
        "depreciation-schedule" => &DEPRECIATION_SCHEDULE,
        "bank-reconciliation" => &BANK_RECONCILIATION,
        "cash-flow-statement" => &CASH_FLOW_STATEMENT,
        "supply-demand" => &SUPPLY_DEMAND,
        "elasticity" => &ELASTICITY,
        "ppf" => &PPF,
        "cost-curves" => &COST_CURVES,
        "monopoly" => &MONOPOLY,
        "circular-flow" => &CIRCULAR_FLOW,
        "ad-as" => &AD_AS,
        "lorenz" => &LORENZ,
        "earth-cycles" => &EARTH_CYCLES,
        "earth-layers" => &EARTH_LAYERS,
        "plate-boundaries" => &PLATE_BOUNDARIES,
        "strata" => &STRATA,
        "moon-seasons" => &MOON_SEASONS,
        "topo-contours" => &TOPO_CONTOURS,
        "hr-diagram" => &HR_DIAGRAM,
        "climate-graph" => &CLIMATE_GRAPH,
        "population-pyramid" => &POPULATION_PYRAMID,
        "grid-reference" => &GRID_REFERENCE,
        "map-scale" => &MAP_SCALE,
        "music-staff" => &MUSIC_STAFF,
        "circle-of-fifths" => &CIRCLE_OF_FIFTHS,
        "piano-keyboard" => &PIANO_KEYBOARD,
        "guitar-chords" => &GUITAR_CHORDS,
        "rhythm-counting" => &RHYTHM_COUNTING,
        "break-even" => &BREAK_EVEN,
        "time-value" => &TIME_VALUE,
        "loan-amortisation" => &LOAN_AMORTISATION,
        "org-chart" => &ORG_CHART,
        "swot" => &SWOT,
        "marketing-funnel" => &MARKETING_FUNNEL,
        "plot-arc" => &PLOT_ARC,
        "organiser" => &ORGANISER,
        "timeline" => &TIMELINE,
        "cause-effect" => &CAUSE_EFFECT,
        "family-tree" => &FAMILY_TREE,
        "sentence-diagram" => &SENTENCE_DIAGRAM,
        "structure-chart" => &STRUCTURE_CHART,
        "food-plate" => &FOOD_PLATE,
        "heart-zones" => &HEART_ZONES,
        "court-layout" => &COURT_LAYOUT,
        "body-map" => &BODY_MAP,
        "block-diagram" => &BLOCK_DIAGRAM,
        "free-body" => &FREE_BODY,
        "gear-train" => &GEAR_TRAIN,
        "orthographic" => &ORTHOGRAPHIC,
        "colour-wheel" => &COLOUR_WHEEL,
        "perspective-grid" => &PERSPECTIVE_GRID,
        "maslow" => &MASLOW,
        "brain-regions" => &BRAIN_REGIONS,
        "experiment-flow" => &EXPERIMENT_FLOW,
        "kirchhoff-loops" => &KIRCHHOFF_LOOPS,
        "state-machine" => &STATE_MACHINE,
        "trace-table" => &TRACE_TABLE,
        "sorting-passes" => &SORTING_PASSES,
        "recursion-tree" => &RECURSION_TREE,
        "er-diagram" => &ER_DIAGRAM,
        "network-topology" => &NETWORK_TOPOLOGY,
        "two-way-table" => &TWO_WAY_TABLE,
        "stem-and-leaf" => &STEM_AND_LEAF,
        "dot-plot" => &DOT_PLOT,
        "normal-curve" => &NORMAL_CURVE,
        "scatterplot" => &SCATTERPLOT,
        "bohr-model" => &FIG_1,
        "box-plot" => &FIG_2,
        "circuit-diagram" => &FIG_3,
        "composite-shape" => &FIG_4,
        "cone" => &FIG_5,
        "coordinate-grid" => &FIG_6,
        "cylinder" => &FIG_7,
        "free-body-diagram" => &FIG_8,
        "gas-syringe" => &FIG_9,
        "gel-electrophoresis" => &FIG_10,
        "heating-cooling-curve" => &FIG_11,
        "inclined-plane" => &FIG_12,
        "kite" => &FIG_13,
        "length-reading" => &FIG_14,
        "lewis-structures" => &FIG_15,
        "line-spectrum" => &FIG_16,
        "mapping-diagram" => &FIG_17,
        "mass-reading" => &FIG_18,
        "mass-spectrum" => &FIG_19,
        "micropipette-reading" => &FIG_20,
        "motion-graphs" => &FIG_21,
        "number-line" => &FIG_22,
        "orbital-diagram" => &FIG_23,
        "parallel-lines" => &FIG_24,
        "parallelogram" => &FIG_25,
        "particle-diagram" => &FIG_26,
        "pedigree" => &FIG_27,
        "ph-reading" => &FIG_28,
        "photoelectron-spectrum" => &FIG_29,
        "population-growth" => &FIG_30,
        "predator-prey" => &FIG_31,
        "prism" => &FIG_32,
        "projectile-motion" => &FIG_33,
        "pulley" => &FIG_34,
        "punnett-square" => &FIG_35,
        "pyramid" => &FIG_36,
        "rectangle" => &FIG_37,
        "regular-polygon" => &FIG_38,
        "ruler-reading" => &FIG_39,
        "sphere" => &FIG_40,
        "spring-scale" => &FIG_41,
        "temperature-reading" => &FIG_42,
        "titration-curve" => &FIG_43,
        "trapezoid" => &FIG_44,
        "triangle" => &FIG_45,
        "vector-diagram" => &FIG_46,
        "volume-by-displacement" => &FIG_47,
        "volume-reading" => &FIG_48,
        "waves" => &FIG_49,
        _ => return None,
    })
}

/// The help line under a section header for the figure's current state. Most are fixed text
/// (`help`); a few follow the figure: the Lewis structure's central atom and the reading ranges,
/// which change with the instrument's size, unit or decimal places.
pub fn live_help(fig: &str, title: &str, help: &str, p: &super::Params) -> String {
    if let Some(h) = super::chemistry::live_help(fig, title, p) {
        return h;
    }
    match (fig, title) {
        ("micropipette-reading", "Micropipette") => super::biology::pipette::notes(p).0,
        ("micropipette-reading", "Reading") => super::biology::pipette::notes(p).1,
        ("spring-scale", "Reading") => {
            let cap = p.s("cap").trim_end_matches(" N").trim().parse::<f64>().unwrap_or(10.0);
            format!("From 0 to {} N, to 2 decimal places.", super::chemistry::num(cap, if cap.fract() == 0.0 { 0 } else { 1 }))
        }
        ("ruler-reading", "Length and position") => {
            let imperial = p.s("marks").ends_with(" in");
            format!("From 0.10 to {}.00 {}, to 2 decimal places.", p.i("ruler"), if imperial { "in" } else { "cm" })
        }
        _ => help.to_string(),
    }
}
