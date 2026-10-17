//! "Start from an example" entries for the physics figures (mirroring the reference sites'
//! example galleries where they could be seen).
use crate::figures::{Example, Value};

fn s(x: &str) -> Value { Value::S(x.into()) }
fn b(x: bool) -> Value { Value::B(x) }
fn i(x: i64) -> Value { Value::I(x) }
fn f(x: f64) -> Value { Value::F(x) }

fn ex(name: &'static str, params: Vec<(&'static str, Value)>) -> Example { Example { name, params } }

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let _ = (f(0.0), i(0));
    Some(match id {
        "circuit-diagram" => vec![
            ex("Series circuit with three resistors and an ammeter", vec![
                ("cell", s("One cell")), ("ammeter", b(true)), ("amm_read_mode", s("Blank line")),
            ]),
            ex("Parallel circuit with two resistors and a voltmeter", vec![
                ("layout", s("In parallel")), ("n", i(2)), ("r1_val", s("3 Ω")), ("r2_val", s("6 Ω")),
                ("cell", s("One cell")), ("cell_val", s("6 V")), ("ammeter", b(true)),
                ("voltmeter", s("Across component 2")), ("vm_read_mode", s("Blank line")),
            ]),
            ex("GCSE series circuit: a cell, a switch and two lamps", vec![
                ("n", i(2)), ("r1_kind", s("Bulb")), ("r2_kind", s("Bulb")), ("style", s("IEC (box resistor)")),
                ("r1_val_mode", s("None")), ("r2_val_mode", s("None")), ("cell", s("One cell")),
                ("cell_val_mode", s("None")), ("switch", s("Closed")), ("voltmeter", s("Across component 1")),
            ]),
            ex("Parallel circuit with three resistors: find the unknown resistance", vec![
                ("layout", s("In parallel")), ("r1_val", s("4 Ω")), ("r2_val", s("6 Ω")), ("r3_val", s("x")),
                ("ammeter", b(true)), ("amm_read_mode", s("Text")), ("amm_read", s("6 A")), ("polarity", b(true)),
            ]),
            ex("Series circuit with two bulbs and an open switch", vec![
                ("n", i(2)), ("r1_kind", s("Bulb")), ("r2_kind", s("Bulb")), ("r1_val_mode", s("None")),
                ("r2_val_mode", s("None")), ("cell", s("One cell")), ("cell_val", s("3 V")), ("switch", s("Open")),
            ]),
        ],
        "vector-diagram" => vec![
            ex("Adding two perpendicular vectors head to tail", vec![
                ("res_style", s("Dashed")), ("mark_angle", b(true)),
            ]),
            ex("Adding three vectors head to tail", vec![
                ("n", i(3)), ("v1_mag", f(5.0)), ("v1_ang", f(0.0)), ("v2_mag", f(3.5)), ("v2_ang", f(60.0)),
                ("v3_mag", f(2.5)), ("v3_ang", f(150.0)), ("res_style", s("Dashed")),
            ]),
            ex("Resolving a vector into x and y components", vec![
                ("n", i(1)), ("v1_mag", f(5.0)), ("v1_ang", f(37.0)), ("components", b(true)),
                ("axes", b(true)), ("mark_angle", b(true)),
            ]),
            ex("Vector addition worksheet: draw the resultant", vec![
                ("v1_mag", f(4.0)), ("v1_ang", f(30.0)), ("v2_mag", f(4.0)), ("v2_ang", f(120.0)), ("resultant", b(false)),
            ]),
            ex("Subtracting vectors: A − B as A + (−B)", vec![
                ("v1_mag", f(5.0)), ("v2_mag", f(3.0)), ("v2_ang", f(270.0)), ("v2_lab", s("−B")),
                ("res_lab", s("A − B")), ("res_style", s("Dashed")),
            ]),
        ],
        "waves" => vec![
            ex("Transverse wave with its wavelength and amplitude labeled", vec![
                ("cycles", f(2.5)), ("mark_ct", b(true)),
            ]),
            ex("Measure the wavelength and amplitude of a wave", vec![
                ("amp", f(2.0)), ("wl", f(6.0)), ("cycles", f(3.0)), ("mark_wl", b(false)), ("mark_amp", b(false)),
            ]),
            ex("Displacement–time graph of a wave: period and frequency", vec![
                ("against", s("Time")), ("amp", f(4.0)), ("wl", f(0.5)), ("cycles", f(4.0)), ("grid", b(false)),
                ("mark_amp", b(false)), ("xaxis_letter", s("None")), ("yaxis_letter", s("None")),
            ]),
            ex("Longitudinal wave above its matching transverse wave", vec![
                ("kind", s("Both")), ("cycles", f(2.5)), ("mark_ct", b(true)),
            ]),
            ex("Label the compression and rarefaction of a longitudinal wave", vec![
                ("kind", s("Longitudinal")), ("mark_ct", b(true)), ("crest_mode", s("Blank line")), ("trough_mode", s("Blank line")),
            ]),
            ex("Label the parts of a wave worksheet", vec![
                ("cycles", f(2.5)), ("numbers", b(false)), ("grid", b(false)), ("mark_ct", b(true)),
                ("wl_lab_mode", s("Blank line")), ("amp_lab_mode", s("Blank line")), ("crest_mode", s("Blank line")),
                ("trough_mode", s("Blank line")), ("xt_mode", s("None")), ("yt_mode", s("None")),
            ]),
        ],
        "motion-graphs" => vec![
            ex("Position–time graph: speeding up, constant velocity, slowing down, lettered", vec![
                ("letters", b(true)),
            ]),
            ex("Matching position–time, velocity–time and acceleration–time graphs", vec![
                ("n", i(5)), ("show", s("All three, stacked")), ("letters", b(true)), ("styles", b(true)),
                ("s1_kind", s("Speeding up")), ("s1_v", f(4.0)), ("s1_t", f(2.0)),
                ("s2_kind", s("Constant velocity forward")), ("s2_v", f(4.0)), ("s2_t", f(3.0)),
                ("s3_kind", s("Slowing down")), ("s3_v", f(0.0)), ("s3_t", f(2.0)),
                ("s4_kind", s("Speeding up")), ("s4_v", f(4.0)), ("s4_t", f(2.0)), ("s4_dir", s("Back")),
                ("s5_kind", s("Slowing down")), ("s5_v", f(0.0)), ("s5_t", f(2.0)),
            ]),
            ex("Velocity–time graph of a cart speeding up and slowing down", vec![
                ("n", i(5)), ("show", s("Velocity-time")), ("letters", b(true)),
                ("s1_kind", s("At rest")), ("s1_t", f(2.0)),
                ("s2_kind", s("Speeding up")), ("s2_v", f(2.0)), ("s2_t", f(3.0)),
                ("s3_kind", s("Speeding up")), ("s3_v", f(6.0)), ("s3_t", f(3.0)),
                ("s4_kind", s("Constant velocity forward")), ("s4_v", f(6.0)), ("s4_t", f(3.0)),
                ("s5_kind", s("Slowing down")), ("s5_v", f(0.0)), ("s5_t", f(3.0)),
            ]),
            ex("Position–time graph shapes without numbers", vec![
                ("n", i(5)), ("numbers", b(false)), ("grid", b(false)), ("letters", b(true)),
                ("s1_kind", s("Speeding up")), ("s1_v", f(4.0)), ("s1_t", f(3.0)),
                ("s2_kind", s("Constant velocity forward")), ("s2_v", f(4.0)), ("s2_t", f(2.0)),
                ("s3_kind", s("Slowing down")), ("s3_v", f(0.0)), ("s3_t", f(3.0)),
                ("s4_kind", s("At rest")), ("s4_t", f(2.0)),
                ("s5_kind", s("Constant velocity back")), ("s5_v", f(3.0)), ("s5_t", f(3.0)),
            ]),
            ex("Tangent to a position–time graph for instantaneous velocity", vec![
                ("n", i(1)), ("s1_kind", s("Speeding up")), ("s1_v", f(6.0)), ("s1_t", f(6.0)),
                ("tangent", b(true)), ("tangent_t", f(4.0)),
            ]),
        ],
        "free-body-diagram" => vec![
            ex("Free body diagram of a box pushed across a floor with friction", vec![
                ("body", s("Block")), ("n", i(4)), ("f3_ang", f(0.0)), ("f3_lab", s("F_A")),
                ("f4_ang", f(180.0)), ("f4_len", f(0.6)), ("f4_lab", s("F_f")), ("motion", s("Velocity")),
            ]),
            ex("Free body diagram of a block sliding down an incline with friction", vec![
                ("body", s("Block")), ("n", i(3)), ("f2_ang", f(120.0)), ("f2_len", f(0.85)), ("f2_mark", b(true)),
                ("f2_from", s("Vertical")), ("f3_ang", f(30.0)), ("f3_len", f(0.5)), ("f3_lab", s("F_f")),
                ("motion", s("Velocity")), ("mdir", f(210.0)),
            ]),
            ex("Free body diagram of a sign hanging from two cables", vec![
                ("n", i(3)), ("f1_ang", f(150.0)), ("f1_lab", s("T_1")), ("f1_mark", b(true)),
                ("f2_ang", f(30.0)), ("f2_lab", s("T_2")), ("f2_mark", b(true)), ("f3_ang", f(270.0)), ("f3_lab", s("F_g")),
            ]),
            ex("Free body diagram of a falling ball with air resistance", vec![
                ("body", s("Ball")), ("f2_len", f(0.5)), ("f2_lab", s("F_{air}")), ("motion", s("Both")), ("mdir", f(270.0)),
            ]),
            ex("Free body diagram of a sled pulled at an angle", vec![
                ("body", s("Block")), ("n", i(4)), ("f2_len", f(0.6)), ("f3_ang", f(180.0)), ("f3_len", f(0.5)),
                ("f3_lab", s("F_f")), ("f4_ang", f(35.0)), ("f4_lab", s("F_A")), ("f4_mark", b(true)),
            ]),
            ex("Free body diagram with the normal force and tension both pointing up, in vector notation", vec![
                ("body", s("Block")), ("n", i(3)), ("f2_len", f(0.6)), ("f3_ang", f(90.0)), ("f3_len", f(0.4)),
                ("f3_lab", s("T")), ("notation", s("Arrow")),
            ]),
        ],
        "spring-scale" => vec![
            ex("Spring scale reading 6.25 N", vec![("reading", f(6.25))]),
            ex("5 N spring scale with slotted masses", vec![
                ("cap", s("5 N")), ("reading", f(2.45)), ("hook", s("Slotted masses")), ("masses", i(4)),
            ]),
            ex("Spring scale reading in grams", vec![("cap", s("1 N")), ("units", s("Grams")), ("reading", f(0.455))]),
            ex("Spring scale with a zero error", vec![("reading", f(4.1)), ("offset", f(0.02)), ("show_zero", b(true))]),
            ex("50 N spring scale in newtons and grams", vec![
                ("cap", s("50 N")), ("units", s("Both")), ("reading", f(27.5)), ("coded", b(false)),
            ]),
        ],
        "inclined-plane" => vec![
            ex("Block on a rough inclined plane with gravity, normal force and friction", vec![
                ("surface", s("Rough")), ("gravity", b(true)), ("normal", b(true)), ("friction", s("Up the ramp")),
            ]),
            ex("Cart rolling down a 20° ramp", vec![
                ("obj", s("Cart")), ("angle", f(20.0)), ("ang_lab", s("20°")), ("velocity", s("Down the ramp")),
                ("accel", s("Down the ramp")),
            ]),
            ex("Ball on a ramp with its length, height and angle labeled", vec![
                ("obj", s("Ball")), ("angle", f(25.0)), ("mark_len", b(true)), ("mark_h", b(true)),
            ]),
            ex("Label the forces: 5 kg block on a 35° incline", vec![
                ("obj_lab", s("5 kg")), ("angle", f(35.0)), ("ang_lab", s("35°")), ("surface", s("Rough")),
                ("gravity", b(true)), ("grav_lab_mode", s("Blank line")), ("normal", b(true)), ("norm_lab_mode", s("Blank line")),
                ("friction", s("Up the ramp")), ("fric_lab_mode", s("Blank line")),
            ]),
            ex("Block pushed up a rough incline with an applied force", vec![
                ("surface", s("Rough")), ("gravity", b(true)), ("normal", b(true)), ("friction", s("Down the ramp")),
                ("applied", s("Up the ramp")), ("velocity", s("Up the ramp")),
            ]),
            ex("Two blocks tied by a string, pulled up an incline", vec![
                ("two", b(true)), ("obj_lab", s("m_1")), ("gravity", b(true)), ("normal", b(true)),
                ("applied", s("Up the ramp")), ("tension", b(true)),
            ]),
        ],
        "pulley" => vec![
            ex("Atwood machine with tension, weight and acceleration", vec![
                ("tension", b(true)), ("gravity", b(true)), ("accel", s("Right falls")),
            ]),
            ex("Block on a table pulled by a hanging mass over a pulley", vec![
                ("setup", s("Table")), ("surface", s("Rough")), ("tension", b(true)), ("gravity", b(true)),
                ("normal", b(true)), ("friction", s("Away from it")), ("accel", s("Right falls")),
            ]),
            ex("Block on an incline tied over a pulley to a hanging mass", vec![
                ("setup", s("Ramp")), ("angle", f(40.0)), ("tension", b(true)), ("gravity", b(true)), ("normal", b(true)),
            ]),
            ex("Block and tackle with 3 strands holding up the load", vec![
                ("setup", s("Block & tackle")), ("strands", i(3)), ("l_lab", s("m")), ("tension", b(true)), ("gravity", b(true)),
            ]),
            ex("Cart on a table and a hanging mass, masses left blank", vec![
                ("setup", s("Table")), ("l_kind", s("Cart")), ("l_lab_mode", s("Blank line")), ("r_lab_mode", s("Blank line")),
            ]),
            ex("Atwood machine with three masses", vec![
                ("third", b(true)), ("tension", b(true)), ("gravity", b(true)), ("accel", s("Right falls")),
            ]),
        ],
        "projectile-motion" => vec![
            ex("Projectile launched at 30° with its velocity components", vec![
                ("angle", f(30.0)), ("ang_lab", s("30°")), ("comps", b(true)),
            ]),
            ex("Ball launched horizontally off a cliff", vec![
                ("from", s("A cliff")), ("angle", f(0.0)), ("steps", i(4)), ("show_g", b(true)), ("mark_r", b(true)),
                ("mark_cliff", b(true)),
            ]),
            ex("Projectile path with the ball at equal time steps, lettered", vec![
                ("angle", f(60.0)), ("steps", i(6)), ("letters", b(true)), ("show_v", b(false)), ("show_angle", b(false)),
            ]),
            ex("Projectile with its maximum height and range labeled", vec![
                ("show_g", b(true)), ("mark_h", b(true)), ("mark_r", b(true)),
            ]),
            ex("Sketch the path: ball launched upward off a cliff", vec![
                ("from", s("A cliff")), ("angle", f(30.0)), ("path", s("None")), ("mark_cliff", b(true)),
            ]),
        ],
        _ => return None,
    })
}
