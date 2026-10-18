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

static FIG_3: [RefSection; 7] = [
    s("Circuit", &["layout", "n"], "Leave a name empty to number it automatically."),
    items("Resistor"),
    s("Cell or battery", &["cell", "cell_name_mode", "cell_name", "cell_val_mode", "cell_val", "polarity"], ""),
    s("Switch and meters", &["switch", "ammeter", "amm_read_mode", "amm_read", "voltmeter", "vm_read_mode", "vm_read"], ""),
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

static FIG_6: [RefSection; 3] = [
    s("Titles", &["title", "equations", "eq_labels", "title_mode", "xtitle_mode", "xtitle", "ytitle_mode", "ytitle"], ""),
    s("x-axis", &["xmin", "xmax", "xstep", "ymin", "ymax", "ystep", "numbers", "arrows", "square", "xlabel", "ylabel", "points", "pt_color", "xnum", "ynum", "xunits"], ""),
    s("Grid", &["minor", "grid", "line_width", "shading", "asymptotes", "endpoints", "pt_style", "pt_label"], ""),
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
    s("Lanes", &["lanes", "lane_names", "lane_labels", "lane_numbers"], ""),
    s("Gel", &["setup", "agarose", "look", "ruler"], ""),
    s("Labels", &["sizes", "electrodes", "answer", "ladder_sizes", "size_units", "electrode_style", "answer_kind"], ""),
    s("Chart title and answer key", &["title", "band_height", "run_length"], ""),
];

static FIG_11: [RefSection; 7] = [
    s("Labels", &["scale", "t_solid", "t_melt", "t_liquid", "t_boil", "t_gas", "labels", "phase_names", "seg_labels", "plateau_labels", "letters", "plateau_dashes", "temp_labels"], ""),
    s("Curve", &["curve_color", "supercool"], ""),
    s("Titles", &["mode", "substance", "mp", "bp", "start", "end", "title"], ""),
    s("x-axis", &["xtitle_style", "xtitle", "xaxis_fit", "xaxis"], ""),
    s("y-axis", &["ylabel", "ytitle_style"], ""),
    s("More options", &["title_style", "xaxis_from", "xaxis_to", "xaxis_by", "xaxis_num", "xaxis_lab", "xaxis_lab_text", "xaxis_lo", "xaxis_hi", "yaxis_from", "yaxis_to", "yaxis_by", "yaxis_num", "yaxis_lab", "yaxis_lab_text", "yaxis_lo", "yaxis_hi"], ""),
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
    s("Heights", &["height"], ""),
    s("Position", &["bottom", "rotate"], ""),
    s("More options", &["name_a", "name_b", "name_c", "name_d", "m_ab", "m_da", "m_a", "m_b", "m_d", "ab_ticks", "bc_ticks", "cd_ticks", "da_ticks", "right", "vertices", "unit", "dec", "fill", "flip", "label_size", "line_style", "height_text"], ""),
    s("Diagonals", &["diagonals"], ""),
    s("Labels", &["ab_lab", "ab_label", "bc_lab", "bc_label", "cd_lab", "cd_label", "da_lab", "da_label", "a_lab", "a_label", "a_arcs", "b_lab", "b_label", "b_arcs", "c_lab", "c_label", "c_arcs", "d_lab", "d_label", "d_arcs", "ac_lab", "ac_label", "bd_lab", "bd_label", "arrow_marks", "height_label", "label_offsets"], "Drag any label on the figure to move it."),
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
    s("Structure", &["formula", "preset", "lone", "formal", "expand"], "Built around O as the central atom."),
    s("Look", &["shape", "bond_style"], ""),
    s("Changes", &["central"], "Make a wrong structure for a “find the mistake” question. Click an atom or bond in the figure, or type the change below."),
    s("Title and answer key", &["title", "tweak", "resonance", "resonance_pick"], ""),
    s("Question", &["question"], ""),
    s("More options", &["title_style", "answer"], ""),
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
    s("Peaks", &["values"], ""),
    s("For the student", &["element", "custom", "custom_name", "show_name", "leave_out"], ""),
    s("Titles", &["title", "xtitle_style", "xtitle", "ytitle_style", "ytitle"], ""),
    s("x-axis", &["source", "xaxis_fit"], ""),
    s("Isotopes", &["molecule", "relative"], ""),
    s("More options", &["answer", "title_style", "xaxis_from", "xaxis_to", "xaxis_by", "xaxis_num", "xaxis_lab", "xaxis_lab_text", "xaxis_lo", "xaxis_hi", "yaxis_from", "yaxis_to", "yaxis_by", "yaxis_num", "yaxis_lab", "yaxis_lab_text", "yaxis_lo", "yaxis_hi"], ""),
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

static FIG_23: [RefSection; 7] = [
    s("Atom or ion", &["charge"], ""),
    s("Configuration", &["order", "exceptions"], ""),
    s("Look", &["element", "layout", "shorthand", "arrows", "orbitals", "edits"], ""),
    s("For the student", &["blank", "config", "caption", "config_blank", "symbol_style", "sublabels"], ""),
    s("Changes", &["extra_empty"], "Click an orbital to change its electrons, for a “which rule is broken?” question."),
    s("Title and answer key", &["title"], ""),
    s("More options", &["title_style", "answer"], ""),
];

static FIG_24: [RefSection; 2] = [
    s("Figure", &["lines", "transversals", "name_m", "name_n", "name_p", "name_t", "name_u", "angle", "angle_b", "gap", "labels", "marked", "unknown", "angle_text", "names", "dec", "turn", "line_ends", "label_size"], ""),
    s("More options", &["m_style", "m_arrows", "n_style", "n_arrows", "p_style", "p_arrows", "t_style", "t_arrows", "u_style", "u_arrows"], ""),
];

static FIG_25: [RefSection; 6] = [
    s("Heights", &["kind", "height"], ""),
    s("Position", &["bottom", "rotate"], ""),
    s("More options", &["name_a", "name_b", "name_c", "name_d", "m_ab", "m_da", "m_a", "m_b", "ab_ticks", "bc_ticks", "cd_ticks", "da_ticks", "right", "vertices", "unit", "dec", "fill", "flip", "label_size", "line_style", "height_text"], ""),
    s("Diagonals", &["diagonals"], ""),
    s("Labels", &["ab_lab", "ab_label", "bc_lab", "bc_label", "cd_lab", "cd_label", "da_lab", "da_label", "a_lab", "a_label", "a_arcs", "b_lab", "b_label", "b_arcs", "c_lab", "c_label", "c_arcs", "d_lab", "d_label", "d_arcs", "ac_lab", "ac_label", "bd_lab", "bd_label", "arrow_marks", "height_label", "label_offsets"], "Drag any label on the figure to move it."),
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

static FIG_29: [RefSection; 6] = [
    s("Energy axis", &["units", "gridlines"], ""),
    s("Peaks", &["labels", "counts", "source", "custom", "drawn", "sublabel_style", "energies"], ""),
    s("For the student", &["names", "numbered"], "The answer key always names the element and its configuration."),
    s("Title and answer key", &["title", "compare", "scale"], ""),
    s("Element", &["element"], ""),
    s("More options", &["title_style", "answer"], ""),
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
    s("Phenotypes", &["phenotypes"], ""),
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
    s("Position", &["kind", "bottom", "rotate"], ""),
    s("More options", &["name_a", "name_b", "name_c", "name_d", "m_ab", "m_bc", "ab_ticks", "bc_ticks", "cd_ticks", "da_ticks", "right", "vertices", "unit", "dec", "fill", "flip", "label_size", "line_style"], ""),
    s("Diagonals", &["diagonals"], ""),
    s("Labels", &["ab_lab", "ab_label", "bc_lab", "bc_label", "cd_lab", "cd_label", "da_lab", "da_label", "a_lab", "a_label", "a_arcs", "b_lab", "b_label", "b_arcs", "c_lab", "c_label", "c_arcs", "d_lab", "d_label", "d_arcs", "ac_lab", "ac_label", "bd_lab", "bd_label", "arrow_marks", "label_offsets"], "Drag any label on the figure to move it."),
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

static FIG_43: [RefSection; 7] = [
    s("Marked points", &["equiv", "half", "eq_label", "half_label", "eq_label_style", "half_mark", "half_label_style"], ""),
    s("Curve", &["curve_color", "common", "source", "kp_start", "kp_end", "kp_veq", "kp_eqph"], ""),
    s("Titles", &["title", "xlabel", "xtitle_style", "ytitle_style", "ytitle"], ""),
    s("x-axis", &["xmax", "xaxis_fit"], ""),
    s("Titration", &["kind", "molarity", "volume", "titrant", "pk"], ""),
    s("More options", &["answer", "title_style", "xaxis_from", "xaxis_to", "xaxis_by", "xaxis_num", "xaxis_lab", "xaxis_lab_text", "xaxis_lo", "xaxis_hi", "yaxis_from", "yaxis_to", "yaxis_by", "yaxis_num", "yaxis_lab", "yaxis_lab_text", "yaxis_lo", "yaxis_hi"], ""),
    s("Grid", &["minor_grid"], ""),
];

static FIG_44: [RefSection; 6] = [
    s("Heights", &["kind", "m_h", "show_height", "hfrom"], ""),
    s("Position", &["bottom", "rotate"], ""),
    s("More options", &["name_a", "name_b", "name_c", "name_d", "m_ab", "m_bc", "m_cd", "m_da", "m_a", "m_b", "m_c", "m_d", "ab_ticks", "bc_ticks", "cd_ticks", "da_ticks", "right", "vertices", "unit", "dec", "fill", "flip", "label_size", "line_style", "height_text"], ""),
    s("Diagonals", &["diagonals"], ""),
    s("Labels", &["ab_lab", "ab_label", "bc_lab", "bc_label", "cd_lab", "cd_label", "da_lab", "da_label", "a_lab", "a_label", "a_arcs", "b_lab", "b_label", "b_arcs", "c_lab", "c_label", "c_arcs", "d_lab", "d_label", "d_arcs", "ac_lab", "ac_label", "bd_lab", "bd_label", "arrow_marks", "height_label", "label_offsets"], "Drag any label on the figure to move it."),
    s("More options", &["ab_arrows", "bc_arrows", "cd_arrows", "da_arrows"], ""),
];

static FIG_45: [RefSection; 4] = [
    s("Heights", &["name_a", "name_b", "name_c", "m_a", "m_b", "m_c", "m_ab", "m_bc", "m_ca", "other", "height"], ""),
    s("Position", &["bottom", "rotate"], ""),
    s("More options", &["ab_ticks", "bc_ticks", "ca_ticks", "right", "vertices", "unit", "dec", "fill", "flip", "label_size", "line_style", "height_text"], ""),
    s("Labels", &["a_lab", "a_label", "a_arcs", "b_lab", "b_label", "b_arcs", "c_lab", "c_label", "c_arcs", "ab_lab", "ab_label", "bc_lab", "bc_label", "ca_lab", "ca_label", "height_label", "label_offsets"], "Drag any label on the figure to move it."),
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

pub fn reference(id: &str) -> Option<&'static [RefSection]> {
    Some(match id {
        "angles" => &FIG_0,
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
