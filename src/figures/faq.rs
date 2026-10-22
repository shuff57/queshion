//! Per-figure FAQ: short questions and answers shown in a collapsible "FAQ" at the bottom of a
//! figure's panel. Written for verSHion (paraphrased topics, not copied text).

/// One question and its answer.
pub struct Faq {
    pub q: &'static str,
    pub a: &'static str,
}

/// The FAQ for a figure id (empty for unknown ids).
pub fn faq_for(id: &str) -> &'static [Faq] {
    match id {
        // physics
        "free-body-diagram" => FREE_BODY_DIAGRAM,
        "vector-diagram" => VECTOR_DIAGRAM,
        "inclined-plane" => INCLINED_PLANE,
        "pulley" => PULLEY,
        "projectile-motion" => PROJECTILE_MOTION,
        "motion-graphs" => MOTION_GRAPHS,
        "spring-scale" => SPRING_SCALE,
        "waves" => WAVES,
        "circuit-diagram" => CIRCUIT_DIAGRAM,
        // biology
        "population-growth" => POPULATION_GROWTH,
        "predator-prey" => PREDATOR_PREY,
        "punnett-square" => PUNNETT_SQUARE,
        "pedigree" => PEDIGREE,
        "gel-electrophoresis" => GEL_ELECTROPHORESIS,
        "micropipette-reading" => MICROPIPETTE_READING,
        // chemistry
        "bohr-model" => BOHR_MODEL,
        "gas-syringe" => GAS_SYRINGE,
        "heating-cooling-curve" => HEATING_COOLING_CURVE,
        "length-reading" => LENGTH_READING,
        "lewis-structures" => LEWIS_STRUCTURES,
        "line-spectrum" => LINE_SPECTRUM,
        "mass-reading" => MASS_READING,
        "mass-spectrum" => MASS_SPECTRUM,
        "orbital-diagram" => ORBITAL_DIAGRAM,
        "particle-diagram" => PARTICLE_DIAGRAM,
        "ph-reading" => PH_READING,
        "photoelectron-spectrum" => PHOTOELECTRON_SPECTRUM,
        "temperature-reading" => TEMPERATURE_READING,
        "titration-curve" => TITRATION_CURVE,
        "volume-by-displacement" => VOLUME_BY_DISPLACEMENT,
        "volume-reading" => VOLUME_READING,
        // math
        "coordinate-grid" => COORDINATE_GRID,
        "composite-shape" => COMPOSITE_SHAPE,
        "number-line" => NUMBER_LINE,
        "triangle" => TRIANGLE,
        "rectangle" => RECTANGLE,
        "parallelogram" => PARALLELOGRAM,
        "trapezoid" => TRAPEZOID,
        "kite" => KITE,
        "regular-polygon" => REGULAR_POLYGON,
        "angles" => ANGLES,
        "parallel-lines" => PARALLEL_LINES,
        "prism" => PRISM,
        "cylinder" => CYLINDER,
        "pyramid" => PYRAMID,
        "cone" => CONE,
        "sphere" => SPHERE,
        "box-plot" => BOX_PLOT,
        "histogram" => HISTOGRAM,
        "two-way-table" => TWO_WAY_TABLE,
        "stem-and-leaf" => STEM_AND_LEAF,
        "logic-circuit" => LOGIC_CIRCUIT,
        "graph-diagram" => GRAPH_DIAGRAM,
        "tree-diagram" => TREE_DIAGRAM,
        "number-base" => NUMBER_BASE,
        "data-structure" => DATA_STRUCTURE,
        "flowchart" => FLOWCHART,
        "boolean-table" => BOOLEAN_TABLE,
        "probability-tree" => PROBABILITY_TREE,
        "venn-diagram" => VENN_DIAGRAM,
        "discrete-distribution" => DISCRETE_DISTRIBUTION,
        "confidence-interval" => CONFIDENCE_INTERVAL,
        "residual-plot" => RESIDUAL_PLOT,
        "rc-curve" => RC_CURVE,
        "t-account" => T_ACCOUNT,
        "journal-entry" => JOURNAL_ENTRY,
        "trial-balance" => TRIAL_BALANCE,
        "financial-statement" => FINANCIAL_STATEMENT,
        "accounting-equation" => ACCOUNTING_EQUATION,
        "depreciation-schedule" => DEPRECIATION_SCHEDULE,
        "bank-reconciliation" => BANK_RECONCILIATION,
        "cash-flow-statement" => CASH_FLOW_STATEMENT,
        "supply-demand" => SUPPLY_DEMAND,
        "elasticity" => ELASTICITY,
        "ppf" => PPF,
        "cost-curves" => COST_CURVES,
        "monopoly" => MONOPOLY,
        "circular-flow" => CIRCULAR_FLOW,
        "ad-as" => AD_AS,
        "lorenz" => LORENZ,
        "earth-cycles" => EARTH_CYCLES,
        "earth-layers" => EARTH_LAYERS,
        "plate-boundaries" => PLATE_BOUNDARIES,
        "strata" => STRATA,
        "moon-seasons" => MOON_SEASONS,
        "topo-contours" => TOPO_CONTOURS,
        "hr-diagram" => HR_DIAGRAM,
        "climate-graph" => CLIMATE_GRAPH,
        "population-pyramid" => POPULATION_PYRAMID,
        "grid-reference" => GRID_REFERENCE,
        "map-scale" => MAP_SCALE,
        "music-staff" => MUSIC_STAFF,
        "circle-of-fifths" => CIRCLE_OF_FIFTHS,
        "piano-keyboard" => PIANO_KEYBOARD,
        "guitar-chords" => GUITAR_CHORDS,
        "rhythm-counting" => RHYTHM_COUNTING,
        "break-even" => BREAK_EVEN,
        "time-value" => TIME_VALUE,
        "loan-amortisation" => LOAN_AMORTISATION,
        "org-chart" => ORG_CHART,
        "swot" => SWOT,
        "marketing-funnel" => MARKETING_FUNNEL,
        "plot-arc" => PLOT_ARC,
        "organiser" => ORGANISER,
        "timeline" => TIMELINE,
        "cause-effect" => CAUSE_EFFECT,
        "family-tree" => FAMILY_TREE,
        "sentence-diagram" => SENTENCE_DIAGRAM,
        "structure-chart" => STRUCTURE_CHART,
        "food-plate" => FOOD_PLATE,
        "heart-zones" => HEART_ZONES,
        "court-layout" => COURT_LAYOUT,
        "body-map" => BODY_MAP,
        "block-diagram" => BLOCK_DIAGRAM,
        "free-body" => FREE_BODY,
        "gear-train" => GEAR_TRAIN,
        "orthographic" => ORTHOGRAPHIC,
        "colour-wheel" => COLOUR_WHEEL,
        "perspective-grid" => PERSPECTIVE_GRID,
        "maslow" => MASLOW,
        "brain-regions" => BRAIN_REGIONS,
        "experiment-flow" => EXPERIMENT_FLOW,
        "kirchhoff-loops" => KIRCHHOFF_LOOPS,
        "state-machine" => STATE_MACHINE,
        "trace-table" => TRACE_TABLE,
        "sorting-passes" => SORTING_PASSES,
        "recursion-tree" => RECURSION_TREE,
        "er-diagram" => ER_DIAGRAM,
        "network-topology" => NETWORK_TOPOLOGY,
        "dot-plot" => DOT_PLOT,
        "normal-curve" => NORMAL_CURVE,
        "scatterplot" => SCATTERPLOT,
        "mapping-diagram" => MAPPING_DIAGRAM,
        "ruler-reading" => RULER_READING,
        _ => &[],
    }
}

// ----- physics -----

const FREE_BODY_DIAGRAM: &[Faq] = &[
    Faq {
        q: "What belongs on a free body diagram?",
        a: "Just the object and the forces other things exert on it, each drawn as an arrow in the direction it acts. Leave out surfaces, ropes and forces the object exerts on something else. Velocity and acceleration are not forces, so keep them separate from the force arrows.",
    },
    Faq {
        q: "Should I draw the object as a dot or as a box?",
        a: "Both are common. AP-style answers use a single dot with every arrow starting there, while a block or cart can help younger students picture the situation. Either way, the arrows begin at the center.",
    },
    Faq {
        q: "How are force directions measured?",
        a: "As an angle measured counterclockwise from the positive x direction: 0° is right, 90° is up, 180° is left and 270° is down. Any angle in between tilts the arrow accordingly.",
    },
    Faq {
        q: "Do the arrow lengths have to be exact?",
        a: "No. Arrow lengths only need to compare the forces: equal forces get equal arrows and a larger force gets a longer one. They show relative size, not a precise scale.",
    },
    Faq {
        q: "On a slope, what directions do the normal force and friction take?",
        a: "The normal force is perpendicular to the surface, pushing out of it. Friction runs along the surface, opposite to the motion or the motion that would happen. On a slope of angle θ, the normal force leans θ away from vertical.",
    },
    Faq {
        q: "How can I ask about components without giving them away?",
        a: "Draw the force by itself and let students resolve it, or show the horizontal and vertical component arrows with their labels left blank. A force F at angle θ has components F cos θ and F sin θ.",
    },
];

const VECTOR_DIAGRAM: &[Faq] = &[
    Faq {
        q: "What does adding vectors tip to tail mean?",
        a: "Place each new vector so it starts where the previous one ends. The resultant is the arrow from the very first tail to the very last tip. For example, 4 units right followed by 3 units up gives a resultant of 5 units at about 36.9° above horizontal.",
    },
    Faq {
        q: "How are a vector's x and y components found?",
        a: "For a vector of size A at angle θ above the horizontal, the x-component is A cos θ and the y-component is A sin θ. The two components and the vector form a right triangle.",
    },
    Faq {
        q: "How do I show subtracting one vector from another?",
        a: "Subtracting B is the same as adding −B, which has the same length as B but points the opposite way (turned 180°). Draw A, then −B from its tip, and the resultant is A − B.",
    },
    Faq {
        q: "Can students be asked to draw the resultant themselves?",
        a: "Yes. Leave the resultant arrow off the figure so students construct it, and keep its magnitude and direction for your answer key. You can also hide one vector in the chain to make a find-the-missing-vector question.",
    },
    Faq {
        q: "What is one grid square worth?",
        a: "You choose the scale, because the grid itself carries no numbers. A square might stand for 2 m, 5 m/s or 10 N; state it in the question.",
    },
];

const INCLINED_PLANE: &[Faq] = &[
    Faq {
        q: "Why doesn't the normal force point straight up on a ramp?",
        a: "The normal force is always perpendicular to the surface. On a slope tilted θ, it leans θ from vertical, and if nothing else pushes into the ramp its size is mg cos θ.",
    },
    Faq {
        q: "Which direction does friction act on a ramp?",
        a: "Parallel to the surface, against the motion or the motion that would happen without it. If the block moves down or sits still on the slope, friction acts up the slope; if it is pushed upward, friction acts down the slope.",
    },
    Faq {
        q: "How is gravity split into parts on a slope?",
        a: "Gravity points straight down with size mg. Along the slope its part is mg sin θ (pulling down the ramp), and into the slope it is mg cos θ. The angle between gravity and the perpendicular to the ramp equals the ramp angle θ.",
    },
    Faq {
        q: "Does a heavier object have to be drawn bigger?",
        a: "No. The drawing size of an object is independent of its mass, so a 5 kg block can look the same as a 2 kg one. The mass belongs in the label or the question text.",
    },
    Faq {
        q: "How are forces labeled when there are several objects on the ramp?",
        a: "Number each object's forces so they can be told apart, such as F_g1 and F_g2. Objects tied by strings each get tension forces (T₁, T₂), and objects pressed together get a pair of equal and opposite contact forces.",
    },
];

const PULLEY: &[Faq] = &[
    Faq {
        q: "What is an Atwood machine?",
        a: "Two hanging masses joined by one string over a fixed pulley. If the string is light and the pulley frictionless, the heavier mass accelerates down and the lighter one up, with a = (m₂ − m₁)g / (m₁ + m₂).",
    },
    Faq {
        q: "When do tension arrows share one label, and when do they get numbers?",
        a: "One light string over an ideal pulley has the same tension everywhere, so all its arrows can be labeled T. Separate strings can have different tensions, so they are labeled T₁, T₂ and so on.",
    },
    Faq {
        q: "How does a block and tackle reduce the effort needed?",
        a: "The load is supported by several strands of the same rope, each carrying the same tension. With n supporting strands, the ideal effort is the load's weight divided by n, but the rope must be pulled n times as far.",
    },
    Faq {
        q: "Which way does friction act on a block on the table?",
        a: "Opposite to the way the block moves or would move. If a hanging mass drags the block toward the pulley, friction on the block points away from the pulley.",
    },
    Faq {
        q: "How do I show only the forces on one of the masses?",
        a: "Use a free body diagram for that single object, drawing only the forces acting on it, such as the tension pulling up and its weight pulling down on a hanging mass.",
    },
];

const PROJECTILE_MOTION: &[Faq] = &[
    Faq {
        q: "Why does a projectile follow a parabola?",
        a: "Ignoring air resistance, the horizontal speed never changes while gravity steadily increases the downward speed. Horizontal distance grows in proportion to time and vertical drop in proportion to time squared, which traces a parabola.",
    },
    Faq {
        q: "What do the positions at equal time intervals show?",
        a: "Horizontally they are evenly spaced, because horizontal velocity is constant. Vertically the gaps grow or shrink, since gravity alters the vertical velocity every moment.",
    },
    Faq {
        q: "Which launch angle travels farthest?",
        a: "With no air resistance and a launch and landing at the same height, 45°. Two angles the same distance from 45°, for instance 30° and 60°, reach equal range, though the steeper one climbs higher and spends more time in the air.",
    },
    Faq {
        q: "Are the velocity arrows drawn to scale?",
        a: "Treat them as approximate: they show direction and rough size. Velocity arrows are not forces, so they are commonly drawn dashed to keep the two apart.",
    },
    Faq {
        q: "What changes when the ball is launched from a cliff?",
        a: "It falls for longer before landing, so it travels farther horizontally than the same launch on level ground. The horizontal velocity still stays constant the whole time.",
    },
];

const MOTION_GRAPHS: &[Faq] = &[
    Faq {
        q: "How do position, velocity and acceleration graphs relate?",
        a: "The slope of the position–time graph is velocity, and the slope of the velocity–time graph is acceleration. Working backward, the change in position equals the area between the velocity–time line and the time axis.",
    },
    Faq {
        q: "What do different shapes on a position–time graph mean?",
        a: "A flat line means at rest, and a straight sloped line means constant velocity, steeper meaning faster. A curve that gets steeper means speeding up, and one that flattens out means slowing down.",
    },
    Faq {
        q: "Why can a velocity–time graph show a sudden jump?",
        a: "When motion switches straight from one constant velocity to another, the idealized model treats the change as instant. Real objects take a short time to change speed, but textbook straight-line graphs ignore that.",
    },
    Faq {
        q: "What does a tangent line on a position–time graph tell you?",
        a: "Its slope gives the instantaneous velocity at that moment. Students find it by taking rise over run along the tangent.",
    },
    Faq {
        q: "How can I make graphs that focus on shape only?",
        a: "Hide the axis numbers and gridlines so students describe the motion from the shape alone, then keep the actual values for your answer key.",
    },
];

const SPRING_SCALE: &[Faq] = &[
    Faq {
        q: "How should a spring scale be read?",
        a: "Keep it vertical and look at the pointer. Find what one small division is worth, count to the mark just below the pointer, and add an estimated digit. A scale with 0.2 N divisions, for instance, is recorded to the nearest 0.02 N.",
    },
    Faq {
        q: "Why do some spring scales also have gram markings?",
        a: "A spring scale measures force in newtons, but a gram scale lets it double as a rough balance. With g taken as 10 N/kg, 1 N lines up with 100 g. Only the newton reading is actually a force.",
    },
    Faq {
        q: "What is a zero error and how do you correct for it?",
        a: "It is a reading other than zero with nothing attached. Subtract that unloaded reading from every loaded one: a scale resting at 0.20 N that reads 4.30 N under load is really measuring 4.10 N.",
    },
    Faq {
        q: "How does the reading relate to the mass hanging on it?",
        a: "An object hanging still pulls on the hook with a force equal to its weight, mg. So 400 g shows about 3.9 N when g is 9.8 N/kg, or 4 N when g is 10 N/kg.",
    },
    Faq {
        q: "Why are spring scales different colors?",
        a: "School spring scales are often color-coded by their maximum load so the right one can be grabbed quickly. The color is only a convenience; the scale markings are what matter.",
    },
];

const WAVES: &[Faq] = &[
    Faq {
        q: "How are wavelength and amplitude measured?",
        a: "Wavelength is the distance for one full cycle, such as crest to next crest. Amplitude is measured from the rest line (the middle) to a crest, so the crest-to-trough height is twice the amplitude.",
    },
    Faq {
        q: "How do period and frequency show up on a graph?",
        a: "When displacement is plotted against time, one full cycle along the time axis is the period T. Frequency is 1/T, so a period of 0.5 s means 2 Hz.",
    },
    Faq {
        q: "How does a longitudinal wave match up with a transverse wave picture?",
        a: "Compressions, where particles bunch together, line up with the crests, and rarefactions, where they spread out, line up with the troughs. One compression plus one rarefaction make one wavelength.",
    },
    Faq {
        q: "What is the difference between a transverse and a longitudinal wave?",
        a: "In a transverse wave the medium moves perpendicular to the direction the wave travels, like a wave on a rope. In a longitudinal wave it moves back and forth along that direction, like sound in air.",
    },
    Faq {
        q: "How are wave speed, frequency and wavelength connected?",
        a: "Wave speed equals frequency times wavelength (v = fλ). A wave with a 2 m wavelength at 3 Hz travels at 6 m/s.",
    },
];

const CIRCUIT_DIAGRAM: &[Faq] = &[
    Faq {
        q: "Can the figure work out the current and voltages, and make a worksheet from it?",
        a: "Yes. Type numbers for the battery and each part (12 V, 4 Ω, 4.7 kΩ) and set Work out the circuit to Fill meter readings or Readings and results. It handles series, parallel and an open switch; a part typed as a symbol such as R cannot be worked out, so that reading keeps your text. Turn off Show answers for a student copy: the worked-out values become blank lines, and Student + key saves both.",
    },
    Faq {
        q: "How can I turn a circuit into a problem for students?",
        a: "Give some values, like the battery voltage and resistances, and leave others unknown: a blank ammeter reading, a resistor labeled x, or a voltmeter whose reading students must find.",
    },
    Faq {
        q: "Why do resistor symbols look different in different books?",
        a: "US texts usually draw a resistor as a zigzag, while IEC symbols used in the UK and Europe draw it as a plain rectangle. Lamps differ too, but cells, switches and meters look much the same in both styles.",
    },
    Faq {
        q: "What is the difference between a cell and a battery symbol?",
        a: "A single cell is drawn as two parallel plates of different lengths, and the longer one marks the + terminal. Join two or more cells end to end and you have a battery.",
    },
    Faq {
        q: "How are ammeters and voltmeters connected?",
        a: "An ammeter goes in series, in the path of the current it measures. A voltmeter goes in parallel, with its leads on either side of the component it measures.",
    },
    Faq {
        q: "How do series and parallel circuits behave differently?",
        a: "In series, the same current flows through every part and the voltages add up to the battery voltage. In parallel, each branch gets the full battery voltage and the branch currents add up to the total.",
    },
];

// ----- biology -----

const POPULATION_GROWTH: &[Faq] = &[
    Faq {
        q: "How does exponential growth differ from logistic growth?",
        a: "With unlimited resources, a population grows exponentially, dN/dt = rN, curving upward ever more steeply (a J shape). In the logistic model, dN/dt = rN(K − N)/K, growth brakes as numbers approach the carrying capacity K, so the curve flattens into an S shape that levels at K.",
    },
    Faq {
        q: "At what population size does logistic growth happen fastest?",
        a: "When the population is half of K (N = K/2). That point, the S curve's inflection point, has the fastest growth; below it growth accelerates and above it growth slows.",
    },
    Faq {
        q: "What does per capita growth rate mean?",
        a: "It is dN/dt shared out per individual, that is dN/dt ÷ N. Exponential growth keeps it fixed at r, while logistic growth makes it drop linearly from r to zero as N reaches K.",
    },
    Faq {
        q: "How is the doubling time of an exponentially growing population found?",
        a: "Doubling time is ln 2 ÷ r, roughly 0.693 ÷ r. At r = 0.5 per year, numbers double about every 1.39 years.",
    },
    Faq {
        q: "What do the lag, exponential and stationary phases describe?",
        a: "They describe a culture's growth curve: a slow start while numbers are small (lag), a period of rapid growth (exponential), then a plateau close to the carrying capacity (stationary).",
    },
    Faq {
        q: "Why might a population overshoot its carrying capacity?",
        a: "If the population responds to crowding with a delay, it can keep growing past K before births and deaths catch up, then swing back. Small delays settle down; longer ones can cause lasting oscillations.",
    },
];

const PREDATOR_PREY: &[Faq] = &[
    Faq {
        q: "Why do predator numbers peak after prey numbers?",
        a: "Predators need time to turn abundant food into more offspring, so their numbers keep climbing after the prey peak. Once there are many predators, prey decline, food runs short, predators fall and prey recover. The gap between the two peaks is the lag.",
    },
    Faq {
        q: "What is the Lotka–Volterra model?",
        a: "Two coupled equations: dx/dt = αx − βxy for prey x and dy/dt = δxy − γy for predators y. Prey multiply on their own and are lost when they meet predators; predators gain from eating prey and die off otherwise, so the two populations cycle.",
    },
    Faq {
        q: "What is the classic real-world example of these cycles?",
        a: "Snowshoe hares and Canada lynx, whose numbers rose and fell roughly every ten years with the lynx trailing the hares. Real cycles also depend on food supply and other predators, so the model is a simplification.",
    },
    Faq {
        q: "What does a phase plane graph of predators versus prey show?",
        a: "Instead of plotting each population against time, it plots one against the other. Because the cycle repeats, the points form a closed loop, and one trip around is one full cycle.",
    },
    Faq {
        q: "How do students find the period and lag from the graph?",
        a: "The period is the time between two successive peaks of the same population. The lag is the time from a prey peak to the predator peak that follows it.",
    },
];

const PUNNETT_SQUARE: &[Faq] = &[
    Faq {
        q: "How is a Punnett square filled in?",
        a: "Write the possible gametes of one parent across the top and of the other parent down the side, each gamete carrying one allele per gene. Each box gets the alleles from its column plus its row; Tt × Tt gives TT, Tt, Tt and tt.",
    },
    Faq {
        q: "What ratios come from crossing two heterozygotes (Tt × Tt)?",
        a: "The genotypes split 1 TT : 2 Tt : 1 tt. Since TT and Tt look alike under complete dominance, the phenotypes split 3 dominant : 1 recessive.",
    },
    Faq {
        q: "Where does the 9:3:3:1 ratio come from?",
        a: "A parent heterozygous for two independently assorting genes, like RrYy, makes four gamete types equally often. A 4 × 4 square gives 16 boxes: 9 show both dominant traits, 3 and 3 show one each, and 1 shows both recessive traits.",
    },
    Faq {
        q: "How are sex-linked crosses written?",
        a: "Write the alleles as superscripts on the X chromosomes, such as X^B X^b for a carrier mother and X^B Y for a father; the Y carries no allele. Report results separately for daughters and sons, because a son's single X always comes from his mother.",
    },
    Faq {
        q: "How do incomplete dominance and codominance differ?",
        a: "With incomplete dominance the heterozygote is a blend, like pink flowers from red and white parents. With codominance both traits appear fully, like type AB blood.",
    },
];

const PEDIGREE: &[Faq] = &[
    Faq {
        q: "How can a pedigree show whether a trait is dominant or recessive?",
        a: "Look for parents who match each other but have a child who differs. Two unaffected parents with an affected child means recessive; two affected parents with an unaffected child means dominant. Recessive traits often skip generations, while dominant ones usually appear in each one.",
    },
    Faq {
        q: "What pattern suggests an X-linked recessive trait?",
        a: "Mostly males are affected: an affected man hands the allele to his daughters, who carry it, and they may pass it to their sons. Fathers never pass it to sons. An affected daughter of an unaffected father rules it out.",
    },
    Faq {
        q: "What does Y-linked inheritance look like?",
        a: "The trait appears only in males and is handed from every affected father to all of his sons, straight down the male line. Finding an affected female, or an affected boy whose father is unaffected, rules it out.",
    },
    Faq {
        q: "What do the shapes and shading in a pedigree mean?",
        a: "Squares are males and circles are females. A filled symbol is affected, and a half-filled or dotted one is a carrier. Horizontal lines join partners, children hang below, and generations are numbered with Roman numerals.",
    },
    Faq {
        q: "Why are some genotypes written with a blank, like A_?",
        a: "Sometimes the pedigree only proves one allele. An unaffected person in a recessive family might be AA or Aa, so the second allele is left as a blank.",
    },
];

const GEL_ELECTROPHORESIS: &[Faq] = &[
    Faq {
        q: "Why do short DNA fragments move farther through the gel?",
        a: "DNA carries a negative charge, so it is pulled from the wells toward the positive electrode. The gel acts like a sieve, and shorter pieces squeeze through it faster, so they travel farther in the same time.",
    },
    Faq {
        q: "How is a DNA ladder used to estimate band sizes?",
        a: "The ladder lane contains fragments whose lengths are known. Line a sample band up against the nearest ladder bands to estimate its size; for more accuracy, graph log of fragment size versus migration distance and read the unknown off that standard curve.",
    },
    Faq {
        q: "How do you decide which suspect matches in a DNA fingerprint?",
        a: "The match is the lane whose bands line up exactly with every band in the evidence lane, with no extras or missing bands. In a paternity test, each of the child's bands must come from either the mother or the father.",
    },
    Faq {
        q: "How does gel percentage affect what you can separate?",
        a: "Lower-percentage gels have larger pores and separate big fragments well. Higher percentages have smaller pores and resolve small fragments, such as short PCR products.",
    },
    Faq {
        q: "Why are some bands darker or thicker than others?",
        a: "A band containing more DNA stains more strongly. Two fragments too close in size to separate can also merge into one thick band.",
    },
];

const MICROPIPETTE_READING: &[Faq] = &[
    Faq {
        q: "How do you read the display on a micropipette?",
        a: "Take the digits in order from the top, then put in the decimal point based on the pipette size. On a P20 the digits are tens, units and tenths of a µL, and 1-6-2 means 16.2 µL; a P200 shows hundreds, tens and units, so 1-5-4 means 154 µL.",
    },
    Faq {
        q: "Why are some digits shown in red?",
        a: "On many pipettes, red digits mark the places after the decimal point. Different brands mark the decimal differently, so check the models in your own lab.",
    },
    Faq {
        q: "How do you choose the right micropipette for a volume?",
        a: "Pick the smallest pipette whose range includes the volume, because accuracy drops near the bottom of a range. Each typically works from about a tenth of its maximum up to the maximum, so 15 µL suits a P20 and 150 µL a P200.",
    },
    Faq {
        q: "What does the number in P20 or P200 tell you?",
        a: "It is the top volume the pipette can deliver, in µL. Because the two displays look the same, identical digits stand for different volumes on each, so always check the pipette size first.",
    },
    Faq {
        q: "Why can't a micropipette be set to any volume?",
        a: "A pipette can only be set inside its working range and in increments of its last digit. On a P20, 25 µL is too high and 12.53 µL is finer than the 0.1 µL steps.",
    },
];

// ----- chemistry -----

const BOHR_MODEL: &[Faq] = &[
    Faq {
        q: "How many electrons fit in each shell?",
        a: "Among the first 20 elements shell capacities run 2, 8, 8, then 2 (potassium, for example, has 2, 8, 8, 1). After calcium the third shell keeps filling even once the fourth has started, so iron ends up as 2, 8, 14, 2.",
    },
    Faq {
        q: "How do you find the number of neutrons?",
        a: "Subtract the atomic number from the mass number. Chlorine's atomic mass rounds to 35 and its atomic number is 17, so the common isotope has 18 neutrons.",
    },
    Faq {
        q: "How does a Bohr model change for an ion?",
        a: "A positive ion has lost electrons from its outer shell, and a negative ion has gained some. The nucleus stays the same; ions are often written in square brackets with the charge outside.",
    },
    Faq {
        q: "What are valence electrons in a Bohr model?",
        a: "They are the electrons in the outermost shell. For main-group elements their number matches the group's last digit and largely decides how the atom bonds.",
    },
    Faq {
        q: "How can a Bohr model be used as a question?",
        a: "Leave the nucleus blank and ask how many protons and neutrons it has, draw empty shells for students to fill with electrons, or leave the element name off and ask students to identify it.",
    },
];

const GAS_SYRINGE: &[Faq] = &[
    Faq {
        q: "Where do you read the volume on a gas syringe?",
        a: "At the flat inner face of the plunger, the side closest to the nozzle, not at the handle end. Read to the last marked line and estimate one more digit.",
    },
    Faq {
        q: "Is cm³ the same as mL?",
        a: "Yes. One cubic centimeter equals one milliliter, so changing the unit only changes the label, not the reading.",
    },
    Faq {
        q: "What is a gas syringe used for in experiments?",
        a: "It collects and measures the gas made in a reaction, such as a metal with acid. Reading it at regular times gives volume against time data for a rate of reaction graph.",
    },
    Faq {
        q: "How precise is a gas syringe reading?",
        a: "Read one digit past the smallest marking. On a syringe marked every 1 cm³, that means recording to 0.1 cm³, like 37.4 cm³.",
    },
];

const HEATING_COOLING_CURVE: &[Faq] = &[
    Faq {
        q: "Why does the temperature stay level during melting or boiling?",
        a: "The energy added goes into separating particles (raising potential energy) rather than speeding them up, so the temperature holds until the phase change finishes. During freezing or condensing the same energy is released.",
    },
    Faq {
        q: "Why is the boiling flat section longer than the melting one?",
        a: "Turning a liquid into a gas separates the particles completely, which needs much more energy than melting, where they only loosen. Water's heat of vaporization is about 6.8 times its heat of fusion.",
    },
    Faq {
        q: "Which calculations go with each part of the curve?",
        a: "Sloped parts, where temperature changes, use q = m·c·ΔT. Flat parts, where the phase changes, use q = n·ΔH (or m times the latent heat).",
    },
    Faq {
        q: "What is supercooling?",
        a: "A liquid cooled below its freezing point without solidifying yet. Once freezing starts, the temperature rises back up to the freezing point and stays there until it has all frozen.",
    },
    Faq {
        q: "What does the slope of a sloped section tell you?",
        a: "With steady heating, a steeper slope means a lower specific heat for that phase, since less energy is needed to change its temperature.",
    },
];

const LENGTH_READING: &[Faq] = &[
    Faq {
        q: "How many digits should a ruler measurement have?",
        a: "Go to the finest marking and then guess one further digit. With millimeter marks that means recording to hundredths of a centimeter, such as 6.82 cm; with only centimeter marks, to tenths, such as 6.8 cm.",
    },
    Faq {
        q: "What if the object doesn't start at zero?",
        a: "Read the positions of both ends and subtract the smaller from the larger. This also avoids errors from a worn ruler end.",
    },
    Faq {
        q: "How do you measure a round object like a marble?",
        a: "Its widest points sit above the ruler, so line each edge straight down to the scale before reading. Guide lines dropped from the edges make this easier.",
    },
    Faq {
        q: "How do you read an inch ruler?",
        a: "Count whole inches, then the fraction from the smaller marks: halves, quarters, eighths and sixteenths, depending on the ruler. Reduce the fraction, so 4/16 in is written 1/4 in.",
    },
];

const LEWIS_STRUCTURES: &[Faq] = &[
    Faq {
        q: "How do you draw a Lewis structure?",
        a: "Count all valence electrons (adjusting for charge), connect the atoms with single bonds, then place lone pairs to give outer atoms octets. If the central atom is short, turn lone pairs into double or triple bonds.",
    },
    Faq {
        q: "How do you draw the Lewis symbol of a single atom or ion?",
        a: "Place the valence electrons around the symbol, one per side before pairing. For an ion, add electrons for a negative charge or remove them for a positive one, and enclose the ion in brackets with the charge written outside.",
    },
    Faq {
        q: "Why do textbooks show different structures for ions like sulfate?",
        a: "Some follow a strict octet rule, accepting formal charges. Others let atoms in period 3 and beyond exceed eight electrons to minimize formal charges, adding double bonds. Both appear in courses, so use your class's convention.",
    },
    Faq {
        q: "What are resonance structures?",
        a: "Two or more valid Lewis structures that differ only in where the electrons sit, like the double bond in O₃ or NO₃⁻. The real molecule is a blend of them, and they are shown joined by a double-headed arrow.",
    },
    Faq {
        q: "How is formal charge calculated?",
        a: "Formal charge = valence electrons − lone pair electrons − half the bonding electrons. The best structure usually has formal charges as close to zero as possible.",
    },
    Faq {
        q: "Why isn't NaCl drawn as one Lewis structure?",
        a: "It is ionic, not a molecule with shared bonds. Each ion, Na⁺ and Cl⁻, is drawn separately in brackets instead.",
    },
];

const LINE_SPECTRUM: &[Faq] = &[
    Faq {
        q: "Why does every element have its own set of lines?",
        a: "Electrons can only occupy certain energy levels. When one drops to a lower level, the atom emits light whose energy, and so wavelength, equals the gap. Each element's levels are different, so its lines act like a fingerprint.",
    },
    Faq {
        q: "How can you tell which elements made an unknown spectrum?",
        a: "Check it against reference spectra. An element counts as present only when every one of its lines shows up in the unknown, since a mixture produces all the lines of each element it contains.",
    },
    Faq {
        q: "How do emission and absorption spectra differ?",
        a: "An emission spectrum is bright colored lines on a dark background, from a hot glowing gas. An absorption spectrum is a continuous rainbow with dark gaps, made when white light passes through a cooler gas. The gaps fall at the same wavelengths as the bright lines.",
    },
    Faq {
        q: "Why are only a few lines shown for each element?",
        a: "Real elements have many lines, many of them faint. Worksheets usually show only the strongest visible ones so the spectra stay readable.",
    },
    Faq {
        q: "How does wavelength relate to the energy of the light?",
        a: "Shorter wavelengths carry more energy (E = hc/λ). Violet lines come from bigger energy drops than red lines.",
    },
];

const MASS_READING: &[Faq] = &[
    Faq {
        q: "How is a triple beam balance read?",
        a: "Add up the three rider positions: the hundreds beam, the tens beam, and the sliding front beam that reads 0 to 10 g in small steps. Estimate one extra digit on the front beam, giving a reading like 347.26 g.",
    },
    Faq {
        q: "Do you estimate a digit on a digital balance?",
        a: "No. Record exactly what the display shows, including any trailing zeros, since those zeros are significant.",
    },
    Faq {
        q: "What is an analytical balance?",
        a: "A high-precision balance that reads to the milligram or tenth of a milligram (three or four decimal places in grams). A glass draft shield surrounds the pan so moving air doesn't upset the reading.",
    },
    Faq {
        q: "How can a mass reading be used for a density question?",
        a: "Measure the object's mass on a balance and its volume by water displacement, then divide mass by volume.",
    },
    Faq {
        q: "Why should a balance read zero before you weigh something?",
        a: "Any reading with an empty pan adds to every measurement. Zero (tare) the balance first, or subtract the empty reading from the result.",
    },
];

const MASS_SPECTRUM: &[Faq] = &[
    Faq {
        q: "How is the average atomic mass worked out from a mass spectrum?",
        a: "Multiply each isotope's mass by its percent abundance, add the results, and divide by 100. For magnesium this gives about 24.3, matching the periodic table.",
    },
    Faq {
        q: "Why do the peaks sit at whole numbers?",
        a: "School spectra plot each isotope at its mass number on the mass-to-charge axis. The isotopes' exact masses differ slightly from whole numbers and are used for precise averages.",
    },
    Faq {
        q: "What does the height of each peak show?",
        a: "How common that isotope is, either as a percent abundance or relative to the tallest peak.",
    },
    Faq {
        q: "What does m/z on the x-axis mean?",
        a: "Mass divided by charge. Most ions in these spectra carry a charge of +1, so m/z is simply the isotope's mass.",
    },
    Faq {
        q: "What if the abundances don't add to 100%?",
        a: "Divide by the total of the abundances instead of 100. This also works when peak heights are given relative to the tallest peak.",
    },
];

const ORBITAL_DIAGRAM: &[Faq] = &[
    Faq {
        q: "What rules are used to fill an orbital diagram?",
        a: "Start with the lowest-energy sublevel and work upward: 1s, 2s, 2p, 3s, then 3p, 4s, 3d and so on. No orbital may hold more than two electrons, and a pair must have opposite spins (the Pauli principle). In a sublevel, put one electron in every orbital before doubling any up (Hund's rule).",
    },
    Faq {
        q: "Why are chromium and copper exceptions?",
        a: "In their real ground states one 4s electron moves to 3d, giving a half-full or full d sublevel. Chromium is written [Ar] 3d⁵ 4s¹ and copper [Ar] 3d¹⁰ 4s¹. Some courses teach the predicted order instead.",
    },
    Faq {
        q: "Which electrons come off first when a transition metal forms an ion?",
        a: "The 4s electrons go before any 3d ones. Fe²⁺ is therefore [Ar] 3d⁶, not [Ar] 4s² 3d⁴.",
    },
    Faq {
        q: "What mistakes do students often make in orbital diagrams?",
        a: "Pairing electrons before every orbital in the sublevel has one, putting two same-spin arrows in one orbital, or using the wrong total number of electrons. A diagram with an electron in a higher level than needed is an excited state rather than wrong.",
    },
    Faq {
        q: "Should 4s be written before or after 3d?",
        a: "Both appear. Listing by filling order puts 4s first; listing by shell groups 3d alongside 3s and 3p.",
    },
];

const PARTICLE_DIAGRAM: &[Faq] = &[
    Faq {
        q: "What is a particle diagram?",
        a: "A picture of the atoms, molecules or ions in a sample, showing how many of each and how they are arranged. They are used to show states of matter, mixtures and reactions.",
    },
    Faq {
        q: "How do solids, liquids and gases look in a particle diagram?",
        a: "Solid particles are packed tightly in a regular arrangement, liquid particles are close together but disordered, and gas particles are far apart and spread throughout the container.",
    },
    Faq {
        q: "How can before-and-after boxes show a reaction?",
        a: "Draw the reactant particles in one box and the products in another. Every atom in the first box must also appear in the second, which shows conservation of mass; leftover reactant molecules point to the excess reactant.",
    },
    Faq {
        q: "How do element, compound and mixture diagrams differ?",
        a: "An element has only one kind of atom, a compound has identical particles made of different atoms bonded together, and a mixture has two or more different kinds of particles mixed but not bonded.",
    },
    Faq {
        q: "How are ionic solids and alloys shown?",
        a: "An ionic solid alternates positive and negative ions in a lattice. A substitutional alloy swaps some metal atoms for atoms of another metal, and an interstitial alloy fits small atoms into the gaps.",
    },
];

const PH_READING: &[Faq] = &[
    Faq {
        q: "Does litmus paper do the same job as pH paper?",
        a: "No. Litmus only tells acid from base. Universal indicator paper changes through a range of colors, so you can read an approximate pH by matching it to the color chart.",
    },
    Faq {
        q: "What is the right way to read a pH meter with a needle?",
        a: "Work out what each small division is worth, find the mark just before the needle, and estimate the rest of the way to the next one. On a dial with 0.2 divisions, that gives a value like 6.47.",
    },
    Faq {
        q: "How precise are the different pH tools?",
        a: "Indicator paper gives about a whole number. An analog meter is read with one estimated digit past its marks, and a digital meter shows its digits directly.",
    },
    Faq {
        q: "What do pH values mean?",
        a: "Below 7 is acidic, 7 is neutral and above 7 is basic. Each step of 1 is a tenfold change in hydrogen ion concentration.",
    },
];

const PHOTOELECTRON_SPECTRUM: &[Faq] = &[
    Faq {
        q: "How is a photoelectron spectrum read?",
        a: "Each peak stands for one sublevel. Its position on the axis gives the energy needed to pull an electron out of that sublevel, and its height gives the number of electrons there.",
    },
    Faq {
        q: "Why does binding energy increase to the left?",
        a: "It's the standard used by AP Chemistry and most textbooks: energy decreases from left to right along the axis. That places the 1s peak farthest to the left.",
    },
    Faq {
        q: "Why are one element's peaks shifted compared with another's?",
        a: "More protons pull electrons in more tightly, so core peaks move to higher binding energy (further left) across the table. A valence peak can break the pattern, as when an electron pairs up in an orbital and is repelled by its partner.",
    },
    Faq {
        q: "Why is the energy axis sometimes broken or logarithmic?",
        a: "Core electrons may be bound thousands of times more strongly than valence ones. On an ordinary axis the valence peaks would crowd together, so a break or log scale keeps every peak visible.",
    },
    Faq {
        q: "How do you identify an element from its spectrum?",
        a: "Add up the peak heights to get the total number of electrons, which equals the atomic number of a neutral atom. The number of peaks and their heights also give the electron configuration.",
    },
];

const TEMPERATURE_READING: &[Faq] = &[
    Faq {
        q: "How is a liquid thermometer read?",
        a: "Look across at the top of the liquid. Note the mark just below it and estimate one more digit; with 1 °C marks, a reading looks like 18.6 °C.",
    },
    Faq {
        q: "Why might a Fahrenheit scale be marked every 2 degrees?",
        a: "Its range is wider, so there may not be room for a mark every degree. You still estimate one digit past the marks.",
    },
    Faq {
        q: "Why is kelvin written K rather than °K?",
        a: "The kelvin is an absolute scale starting at absolute zero, and its unit is written simply as K, with no degree sign.",
    },
    Faq {
        q: "How do you convert between Celsius, Kelvin and Fahrenheit?",
        a: "K = °C + 273.15 and °F = (9/5)°C + 32. A temperature is the same physical state whichever unit it is written in.",
    },
];

const TITRATION_CURVE: &[Faq] = &[
    Faq {
        q: "What is special about the half-equivalence point?",
        a: "It falls at half the titrant volume needed for equivalence. At that point a weak acid's pKa is equal to the pH, and for a weak base the pOH matches the pKb.",
    },
    Faq {
        q: "Why isn't the equivalence pH 7 for a weak acid?",
        a: "At equivalence the flask holds the weak acid's conjugate base, which makes the solution slightly basic, so it ends up above pH 7. A weak base with strong acid gives a pH below 7. Only strong acid with strong base lands at 7.",
    },
    Faq {
        q: "How do the equivalence point and end point differ?",
        a: "The equivalence point is reached when the added titrant has exactly used up the analyte; it sits midway up the steep rise. The end point is when the indicator changes color, and a good indicator makes the two nearly the same.",
    },
    Faq {
        q: "How do you choose an indicator for a titration?",
        a: "Pick one whose color change range falls on the steep part of the curve, ideally near the equivalence pH. Phenolphthalein suits weak acid with strong base; methyl orange suits strong acid with weak base.",
    },
    Faq {
        q: "Where is the buffer region on the curve?",
        a: "In the fairly flat section before equivalence, centered on the half-equivalence point, where both the weak acid and its conjugate base are present and the pH changes slowly.",
    },
];

const VOLUME_BY_DISPLACEMENT: &[Faq] = &[
    Faq {
        q: "How does water displacement give an object's volume?",
        a: "Read the water level before and after adding the object; the difference is the volume of the object. For example, 32.0 mL rising to 38.5 mL means 6.5 mL, which is 6.5 cm³.",
    },
    Faq {
        q: "Does the object have to be fully under water?",
        a: "Yes. Only the part that is submerged pushes water up, so a floating or partly covered object gives too small a volume.",
    },
    Faq {
        q: "How is this used to find density?",
        a: "Find the mass on a balance and the volume by displacement, then divide mass by volume to get density in g/mL or g/cm³.",
    },
    Faq {
        q: "Why use displacement instead of a ruler?",
        a: "Irregular objects like rocks have no simple formula for volume. Displacement works for any shape that sinks and doesn't absorb water.",
    },
    Faq {
        q: "Why use a smaller graduated cylinder when possible?",
        a: "Smaller cylinders have finer markings, so the before and after readings, and their difference, are more precise.",
    },
];

const VOLUME_READING: &[Faq] = &[
    Faq {
        q: "How should a graduated cylinder be read?",
        a: "With your eye level with the liquid, read the bottom of the curved meniscus. Decide the value of one small division, then add one estimated digit; with 1 mL divisions, record to 0.1 mL.",
    },
    Faq {
        q: "Why does a buret's scale run from the top down?",
        a: "A buret records how much liquid has drained out, so its scale begins at 0 near the top and counts up as you go down. With 0.1 mL markings, it is read to 0.01 mL.",
    },
    Faq {
        q: "Is a beaker as precise as a graduated cylinder?",
        a: "No. Beaker markings are far apart and only approximate, so a beaker is useful for rough volumes. Graduated cylinders and burets are used for measurement.",
    },
    Faq {
        q: "Is a reading in cm³ different from one in mL?",
        a: "No. One cubic centimeter is exactly one milliliter, so only the unit label changes.",
    },
    Faq {
        q: "What is parallax error?",
        a: "Reading from above or below the liquid level makes the meniscus look higher or lower than it is. Keeping your eye level with it avoids the error.",
    },
];

// ----- math -----

const COORDINATE_GRID: &[Faq] = &[
    Faq {
        q: "How do I get an empty coordinate plane?",
        a: "Leave out all equations and points and set each axis's range, such as −10 to 10 for all four quadrants or 0 to 12 for quadrant I alone.",
    },
    Faq {
        q: "How is an inequality shown on a coordinate grid?",
        a: "Graph its boundary line, dashed for < or > and solid for ≤ or ≥, then shade the side that makes the inequality true. For a system, the solution is where all the shaded regions overlap.",
    },
    Faq {
        q: "How do you plot a point like (3, −2)?",
        a: "Start at the origin, move 3 units along the x-axis (right), then 2 units parallel to the y-axis (down). The first number is always x and the second is y.",
    },
    Faq {
        q: "How are holes and asymptotes shown on a graph?",
        a: "A hole is a single missing point, drawn as an open circle. An asymptote is a line the curve approaches but never reaches, usually drawn dotted or dashed.",
    },
    Faq {
        q: "How is a piecewise function graphed?",
        a: "Graph each piece only over its own interval. A closed dot marks an endpoint that is included (≤ or ≥) and an open dot one that is not (< or >).",
    },
    Faq {
        q: "How should the x-axis be scaled for trig graphs?",
        a: "Use multiples of π, such as steps of π/2 or π/4, when working in radians, or steps of 90° or 30° in degrees. That puts key points like peaks and zeros on gridlines.",
    },
];

const COMPOSITE_SHAPE: &[Faq] = &[
    Faq {
        q: "What is a composite shape?",
        a: "A figure made by combining simpler shapes such as rectangles, triangles, circles and semicircles. Its area is found by working with those simpler pieces.",
    },
    Faq {
        q: "How do you find the area of a composite shape?",
        a: "Split it into basic shapes, find each area, then add them. If a piece is cut out, like a hole, subtract its area instead.",
    },
    Faq {
        q: "How do you find the perimeter of a composite shape?",
        a: "Add only the lengths around the outside edge. Edges where two pieces join are inside the figure and do not count, and a curved edge uses part of a circle's circumference.",
    },
    Faq {
        q: "How do you find a missing side length?",
        a: "Use the sides you know on the same line: opposite sides of a rectangular outline add up to the same total, so a missing piece is the whole length minus the other parts.",
    },
    Faq {
        q: "How are semicircles handled in area problems?",
        a: "A semicircle's area is half of πr². Make sure to use the radius, which is half the diameter shown on the shape it sits against.",
    },
];

const NUMBER_LINE: &[Faq] = &[
    Faq {
        q: "How is an inequality shown on a number line?",
        a: "Put a closed dot on the number if it is included (≤ or ≥) and an open dot if it is not (< or >). Then shade the line over every number that makes the inequality true.",
    },
    Faq {
        q: "How are compound inequalities graphed?",
        a: "An \"and\" inequality like −3 < x ≤ 4 is the overlap, a single shaded segment between the two values. An \"or\" inequality like x < −1 or x ≥ 3 shades two separate pieces heading in opposite directions.",
    },
    Faq {
        q: "How do you make a number line for fractions?",
        a: "Divide each whole into equal parts, such as fourths, so each tick mark is 1/4. Labeling only the whole numbers lets students name the fractions themselves.",
    },
    Faq {
        q: "How can points be marked for students to identify?",
        a: "Label each point with a letter only, like P or Q, and ask students for its value. Writing the value next to the dot would give the answer away.",
    },
    Faq {
        q: "How does a number line show negative numbers?",
        a: "Numbers to the left of 0 are negative and those to the right are positive. A number further left is always smaller, so −5 is less than −2.",
    },
];

const TRIANGLE: &[Faq] = &[
    Faq {
        q: "How many measures are needed to fix a triangle?",
        a: "Three, including at least one side: three sides (SSS), two sides with the angle between them (SAS), a side with two angles (ASA or AAS), or two sides plus a non-included angle (SSA). Knowing only the three angles settles the shape, not the size.",
    },
    Faq {
        q: "How can a side be labeled x for students to solve?",
        a: "Give enough other measures to determine the triangle, then label the unknown side with a variable instead of its value. The triangle is still drawn with the correct shape.",
    },
    Faq {
        q: "Is the triangle drawn in proportion?",
        a: "Yes. Side lengths and angles are proportional to what you enter, so sides of 3, 4 and 5 really show a right angle. The figure is scaled to fit, not printed at true size.",
    },
    Faq {
        q: "What is the ambiguous case?",
        a: "Given two sides and an angle that is not between them (SSA), sometimes two different triangles fit. Usually one has an acute unknown angle and the other an obtuse one.",
    },
    Faq {
        q: "What do the tick marks on the sides mean?",
        a: "Sides with the same number of tick marks are equal in length, and angles with the same number of arcs are equal in measure.",
    },
];

const RECTANGLE: &[Faq] = &[
    Faq {
        q: "Is the rectangle drawn to scale?",
        a: "Its proportions follow your lengths: a 12 by 5 rectangle, for instance, comes out well over double its width in length. The drawing is resized to fit the figure rather than shown at actual size.",
    },
    Faq {
        q: "Can a side be labeled with a variable or expression?",
        a: "Yes. Use the Labels section to show text such as x or 2x + 3 instead of the measure. The rectangle keeps the shape of the lengths you entered.",
    },
    Faq {
        q: "How is a diagonal used in a Pythagorean theorem problem?",
        a: "A diagonal splits the rectangle into two right triangles, with the length and width as legs. So diagonal² = length² + width².",
    },
    Faq {
        q: "What makes a square a special rectangle?",
        a: "All four sides are equal as well as all four angles being 90°. Tick marks on each side show that the sides are equal.",
    },
    Faq {
        q: "How are a rectangle's area and perimeter found?",
        a: "Area is length times width. Perimeter is twice the length plus twice the width.",
    },
];

const PARALLELOGRAM: &[Faq] = &[
    Faq {
        q: "Why do area questions need the height, not the slanted side?",
        a: "Area is base times height, where the height is the perpendicular distance between the parallel sides. It is usually shown as a dashed line with a right-angle mark.",
    },
    Faq {
        q: "What do the arrow marks on the sides mean?",
        a: "Sides with matching arrows are parallel: one arrow on one pair, two arrows on the other. Tick marks, by contrast, show equal lengths.",
    },
    Faq {
        q: "How is a rhombus different from other parallelograms?",
        a: "A rhombus has all four sides equal. Its diagonals bisect each other at right angles.",
    },
    Faq {
        q: "What angle facts hold in a parallelogram?",
        a: "Opposite angles are equal, and neighboring angles add to 180°. So if one angle is 70°, the others are 110°, 70° and 110°.",
    },
    Faq {
        q: "Can the angles be labeled with variables?",
        a: "Yes. Each angle can show its measure, a text label like x, or nothing, which works well for angle-relationship questions.",
    },
];

const TRAPEZOID: &[Faq] = &[
    Faq {
        q: "Is a trapezoid the same as a trapezium?",
        a: "Yes, in the usual school sense: a quadrilateral having just one pair of parallel sides, called the bases. \"Trapezoid\" is the US term and \"trapezium\" the UK term.",
    },
    Faq {
        q: "How is the area of a trapezoid found?",
        a: "Average the two bases and multiply by the height: A = (b₁ + b₂)/2 × h. The height is the perpendicular distance between the bases, not the slanted leg.",
    },
    Faq {
        q: "What makes a trapezoid isosceles?",
        a: "Its two legs are equal in length. Its base angles are equal in pairs, and its diagonals are the same length.",
    },
    Faq {
        q: "How is a right trapezoid used with the Pythagorean theorem?",
        a: "Dropping the height from the top base makes a right triangle with the slanted leg as its hypotenuse. The difference between the bases is one leg and the height is the other.",
    },
    Faq {
        q: "Can sides be given as radicals or fractions?",
        a: "Yes. Values like 3√2 or 7/2 can be entered, and the side is drawn at that length and labeled the way it was typed.",
    },
];

const KITE: &[Faq] = &[
    Faq {
        q: "What is a kite in geometry?",
        a: "A quadrilateral with two pairs of equal adjacent sides. It has one line of symmetry, along the diagonal joining the angles between the pairs of equal sides.",
    },
    Faq {
        q: "How do you find the area of a kite?",
        a: "Multiply the two diagonals and halve the result: A = (d₁ × d₂)/2. This works because the diagonals cross at right angles.",
    },
    Faq {
        q: "What do the tick marks on a kite show?",
        a: "Matching tick counts mean matching lengths. A kite typically shows a single tick on both shorter sides and a double tick on both longer sides.",
    },
    Faq {
        q: "Which angles of a kite are equal?",
        a: "The pair of angles between unequal sides are equal. The other two angles, on the line of symmetry, are generally different from each other.",
    },
    Faq {
        q: "Can a side be entered as a radical like 3√2?",
        a: "Yes. The side is drawn at that exact length and labeled as typed, which keeps answers exact.",
    },
];

const REGULAR_POLYGON: &[Faq] = &[
    Faq {
        q: "What is the apothem?",
        a: "The perpendicular segment from the center of a regular polygon to the middle of a side, or its length. The area equals half the apothem times the perimeter.",
    },
    Faq {
        q: "How are side, radius and apothem related?",
        a: "The radius runs from the center to a vertex. The apothem, half a side and the radius form a right triangle, so knowing any one measure fixes the others.",
    },
    Faq {
        q: "How do you find the interior angle of a regular polygon?",
        a: "Each interior angle is (n − 2) × 180° ÷ n for n sides. A regular hexagon's angles are each 120°.",
    },
    Faq {
        q: "Why do all the sides and angles carry the same marks?",
        a: "In a regular polygon every side is equal and every angle is equal, so matching tick marks and arcs show that.",
    },
    Faq {
        q: "What are polygons with more than 12 sides called?",
        a: "They are usually named by their number of sides, such as a 15-gon or 20-gon, rather than with a Greek prefix.",
    },
];

const ANGLES: &[Faq] = &[
    Faq {
        q: "How are ray directions measured?",
        a: "Like a protractor: from the baseline that points right, counterclockwise. A ray at 90° points straight up and one at 180° points left.",
    },
    Faq {
        q: "What angle facts apply to angles on a straight line and around a point?",
        a: "Angles that make up a straight line add to 180°, and angles all the way around a point add to 360°.",
    },
    Faq {
        q: "What are vertical angles?",
        a: "When two lines cross, the angles directly opposite each other are vertical angles, and they are always equal. Neighboring angles at the crossing add to 180°.",
    },
    Faq {
        q: "What is the difference between complementary and supplementary angles?",
        a: "Complementary angles add to 90°, and supplementary angles add to 180°.",
    },
    Faq {
        q: "How do I set up a find-x angle question?",
        a: "Label one angle with a variable or an expression like 2x + 20 instead of its measure and give the others as numbers. Students then use the angle sum to solve.",
    },
    Faq {
        q: "How are angles classified by size?",
        a: "Acute is less than 90°, right is exactly 90°, obtuse is between 90° and 180°, straight is 180°, and reflex is between 180° and 360°.",
    },
];

const PARALLEL_LINES: &[Faq] = &[
    Faq {
        q: "What is a transversal?",
        a: "A line that crosses two or more other lines. When it crosses parallel lines, it creates pairs of angles with special relationships.",
    },
    Faq {
        q: "Which angle pairs are equal when a transversal cuts parallel lines?",
        a: "Corresponding angles (same position at each crossing), alternate interior angles and alternate exterior angles are all equal, as are vertical angles.",
    },
    Faq {
        q: "Which angle pairs add up to 180°?",
        a: "Same-side (consecutive) interior angles add to 180°, as do any two angles that form a straight line along the transversal.",
    },
    Faq {
        q: "What do the arrow marks on the lines mean?",
        a: "Lines carrying matching arrows are parallel. Without the marks, students shouldn't assume the lines are parallel just because they look it.",
    },
    Faq {
        q: "How can a figure be used to prove lines are parallel?",
        a: "If a pair of corresponding or alternate interior angles are equal, or same-side interior angles add to 180°, then the lines must be parallel.",
    },
];

const PRISM: &[Faq] = &[
    Faq {
        q: "How can the height be labeled with a variable?",
        a: "Enter a number for the height so the prism has its shape, then change its label to a letter like h or x. The figure shows the letter while keeping the shape.",
    },
    Faq {
        q: "How is a prism's volume found?",
        a: "Multiply the area of the base by the height: V = B × h. This works for any base shape, rectangle, triangle or polygon.",
    },
    Faq {
        q: "What makes a prism oblique?",
        a: "Its top is shifted sideways from its base, so the side edges slant. The height is still the perpendicular distance between the two bases, not the slanted edge length, and the volume formula is unchanged.",
    },
    Faq {
        q: "What do the dashed edges show?",
        a: "Edges hidden behind the solid when viewed from the front. They help students see the full three-dimensional shape and can be hidden for a cleaner picture.",
    },
    Faq {
        q: "How do you find a prism's surface area?",
        a: "Add the areas of all faces: two bases plus the side faces. The sides together equal the base perimeter times the height.",
    },
];

const CYLINDER: &[Faq] = &[
    Faq {
        q: "Can the diameter be shown instead of the radius?",
        a: "Yes. Remember that the radius is half the diameter, so students need to halve it before using r in the formulas.",
    },
    Faq {
        q: "How is a cylinder's volume found?",
        a: "Multiply the area of the circular base by the height: V = πr²h.",
    },
    Faq {
        q: "What is a cylinder's surface area?",
        a: "Two circles plus the curved side: SA = 2πr² + 2πrh. The curved side unrolls into a rectangle whose width is the circumference 2πr.",
    },
    Faq {
        q: "How is an oblique cylinder different?",
        a: "Its top is shifted sideways from its base, so its sides slant. The height is measured perpendicular to the base, and the volume is still πr²h.",
    },
    Faq {
        q: "How can the diagram show just r and h instead of numbers?",
        a: "Use the Labels section to replace the measures with text labels r and h, and remove the unit so only the letters show.",
    },
];

const PYRAMID: &[Faq] = &[
    Faq {
        q: "How are the height and slant height different?",
        a: "The height drops vertically from the apex to the center of the base. The slant height runs along the center line of a triangular face, from the apex to the base edge. These two and half the base width form a right triangle.",
    },
    Faq {
        q: "How do you find the slant height from the height?",
        a: "Use the Pythagorean theorem: slant height² = height² + (half the base width)².",
    },
    Faq {
        q: "How is a pyramid's volume found?",
        a: "One third of the base area times the height: V = (1/3)Bh. It is exactly a third of a prism with the same base and height.",
    },
    Faq {
        q: "Why does a rectangular pyramid have two slant heights?",
        a: "Its front and side faces slope at different angles because the base edges are different lengths, so each pair of faces has its own slant height.",
    },
    Faq {
        q: "When is the slant height used?",
        a: "For surface area: each triangular face has area ½ × base edge × slant height. Volume uses the regular height instead.",
    },
];

const CONE: &[Faq] = &[
    Faq {
        q: "How are a cone's radius, height and slant height related?",
        a: "They form a right triangle with the slant height as the hypotenuse: l² = r² + h². So the slant height is always longer than both the radius and the height.",
    },
    Faq {
        q: "How is a cone's volume found?",
        a: "One third of the base area times the height: V = (1/3)πr²h, a third of a cylinder with the same base and height.",
    },
    Faq {
        q: "What is a cone's surface area?",
        a: "The base circle plus the curved side: SA = πr² + πrl, where l is the slant height.",
    },
    Faq {
        q: "What is an oblique cone?",
        a: "A cone whose tip is not directly above the center of its base. Its height is the perpendicular distance from the tip to the plane of the base, and it has no single slant height.",
    },
    Faq {
        q: "Can the slant height be labeled with a variable?",
        a: "Yes. Give the radius and height so the cone is drawn correctly, then label the slant height with text such as x for students to calculate.",
    },
];

const SPHERE: &[Faq] = &[
    Faq {
        q: "How is a sphere's volume found?",
        a: "V = (4/3)πr³. Doubling the radius makes the volume eight times as large.",
    },
    Faq {
        q: "What is a sphere's surface area?",
        a: "SA = 4πr², which is four times the area of a circle with the same radius.",
    },
    Faq {
        q: "What is a hemisphere and how do its formulas change?",
        a: "Half a sphere. Its volume is (2/3)πr³; its total surface area is the curved half, 2πr², plus the flat circle, πr², for 3πr².",
    },
    Faq {
        q: "Why is half of the circle around the middle dashed?",
        a: "The circle around the middle is seen at a slight angle, and its back half would be hidden behind the sphere, so it is drawn dashed like other hidden edges.",
    },
    Faq {
        q: "Should students use the radius or the diameter?",
        a: "The formulas use the radius. If the figure shows the diameter, halve it first.",
    },
];

const BOX_PLOT: &[Faq] = &[
    Faq {
        q: "How are the quartiles found?",
        a: "Split the ordered data into a lower and upper half: Q1 is the middle value of the lower half, Q3 the middle value of the upper half. If there is an odd count, leave the median out of both halves; this is the convention of most US textbooks and graphing calculators.",
    },
    Faq {
        q: "What is the five-number summary?",
        a: "It lists the smallest value, Q1, the median, Q3 and the largest value: everything needed to draw a box plot.",
    },
    Faq {
        q: "How do you compare two box plots?",
        a: "Draw them on the same number line, one above the other. Compare their medians for center and their box widths (IQR) and whisker lengths for spread.",
    },
    Faq {
        q: "How are outliers identified?",
        a: "Compute IQR = Q3 − Q1. Anything farther than 1.5 × IQR beyond Q1 or Q3 counts as an outlier and is plotted as its own dot; the whisker then ends at the most extreme value that is not an outlier.",
    },
    Faq {
        q: "What does the length of the box tell you?",
        a: "The box covers the middle 50% of the data. A longer box means the middle values are more spread out.",
    },
];

const MAPPING_DIAGRAM: &[Faq] = &[
    Faq {
        q: "How can a mapping diagram show whether a relation is a function?",
        a: "It is a function if every input has exactly one arrow leaving it. An input with two arrows, or with none, means it is not a function.",
    },
    Faq {
        q: "Can two inputs share the same output?",
        a: "Yes. That is still a function, though it is not one-to-one.",
    },
    Faq {
        q: "How do ordered pairs turn into a mapping diagram?",
        a: "Each pair (x, y) becomes an arrow from x in the input list to y in the output list. Each value appears only once in its list, even if it occurs in several pairs.",
    },
    Faq {
        q: "What are the domain and range in a mapping diagram?",
        a: "The domain is the set of inputs on the left, and the range is the set of outputs that arrows actually point to on the right.",
    },
    Faq {
        q: "Do inputs and outputs have to be numbers?",
        a: "No. They can be expressions or words, such as students and their favorite sports, as long as each arrow pairs one input with one output.",
    },
];

const RULER_READING: &[Faq] = &[
    Faq {
        q: "How do you measure to the nearest quarter inch?",
        a: "Use a ruler marked every 1/4 inch, count whole inches, then the quarter marks past the last whole inch. An object ending on the first quarter mark after 3 is 3 1/4 inches long.",
    },
    Faq {
        q: "What if the object doesn't start at zero?",
        a: "Read where each end lines up and subtract the smaller reading from the larger one.",
    },
    Faq {
        q: "How do centimeters and millimeters relate on a metric ruler?",
        a: "Each centimeter is divided into 10 millimeters. A length of 8.4 cm is the same as 84 mm.",
    },
    Faq {
        q: "How do you measure an object with rounded ends?",
        a: "Line up each widest point straight down to the ruler before reading, since a rounded object touches the ruler below its edges.",
    },
    Faq {
        q: "How should fractional inch readings be written?",
        a: "In simplest form: 2/4 in is 1/2 in and 6/8 in is 3/4 in. Finer rulers go down to eighths or sixteenths of an inch.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_figure_has_a_short_faq() {
        for d in crate::figures::registry() {
            let f = faq_for(d.id);
            assert!((4..=6).contains(&f.len()), "{}: {} entries", d.id, f.len());
            for e in f {
                assert!(e.q.ends_with('?') && !e.a.is_empty(), "{}: {}", d.id, e.q);
            }
        }
        assert!(faq_for("nope").is_empty());
    }
}

const HISTOGRAM: &[Faq] = &[
    Faq {
        q: "How do I get a student copy to draw on?",
        a: "Turn off Show answers. The bars and values disappear and the axes, gridlines and scale stay, so students build the histogram from the data. Student + key saves both versions together.",
    },
    Faq {
        q: "Which bin does a value on an edge go in?",
        a: "Choose with \"A value on a bin edge goes\". By default 20 in the bins 10-20 and 20-30 goes in 20-30 (the bin to its right). Pick the left option to put it in 10-20 instead.",
    },
    Faq {
        q: "Should I use frequency, relative frequency or density?",
        a: "Frequency counts the values in each bin. Relative frequency shows each bin's share of all the values. Density divides that share by the bin width, so the bars' areas add to 1; use it whenever the bins are not all the same width.",
    },
    Faq {
        q: "How do I paste data from a spreadsheet?",
        a: "Copy a column, click in the Data box and paste. When you click away the numbers are tidied into a list. Open CSV reads a file, and Generate makes sample data from a seed.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Set Style to Print. The bars are filled with hatching instead of colour, so they still read on a black-and-white copy.",
    },
];

const SCATTERPLOT: &[Faq] = &[
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The points stay and the line of best fit, its equation and r are hidden, so students draw their own line and find r. Student + key saves both versions.",
    },
    Faq {
        q: "How do I enter the pairs?",
        a: "Type one x, y pair per line, or copy two columns from a spreadsheet and paste them in. A header row is skipped. Open CSV reads a file, and Generate makes pairs around a line you choose, from a seed.",
    },
    Faq {
        q: "What does the line of best fit mean?",
        a: "It is the least-squares line: the straight line that makes the squared vertical gaps from the points as small as possible. Its slope says how much y changes for each 1 in x.",
    },
    Faq {
        q: "What does r tell me?",
        a: "The correlation r runs from \u{2212}1 to 1. Near 1 the points rise along a line, near \u{2212}1 they fall along one, and near 0 there is no straight-line pattern. It is not shown when every y is the same.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Set Style to Print. The line is drawn dashed in black so it still stands out on a black-and-white copy.",
    },
];

const NORMAL_CURVE: &[Faq] = &[
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The curve and the shaded region stay and the probability is hidden, so students find the area. Student + key saves both versions.",
    },
    Faq {
        q: "What is the 68-95-99.7 rule?",
        a: "About 68% of a normal distribution lies within 1 standard deviation of the mean, about 95% within 2 and about 99.7% within 3. Shade Between with bounds at the mean plus and minus 1, 2 or 3 standard deviations to see each.",
    },
    Faq {
        q: "Should I give the bounds as values or z-scores?",
        a: "Use x values for a real measurement such as an exam score, and z-scores for a standard normal question. A z-score is the number of standard deviations a value is above (positive) or below (negative) the mean.",
    },
    Faq {
        q: "What do Below, Above, Between and Outside shade?",
        a: "Below shades everything left of the upper bound, Above everything right of the lower bound, Between the part between the two bounds, and Outside both tails beyond them.",
    },
    Faq {
        q: "How is the area found?",
        a: "The area under the whole curve is 1, so the shaded area is the probability. It is computed exactly from the normal distribution function, not read off a table, so it can differ from a table value in the last digit.",
    },
];

const GRAPH_DIAGRAM: &[Faq] = &[
    Faq {
        q: "How do I write a graph?",
        a: "One item per line. List nodes with nodes A, B, C (a node also appears when an edge uses it). A - B is an edge, A -> B is a directed one, and A - B : 4 gives it a weight. To place a node yourself add a line such as node A @ 2,1; the rest stay where the layout puts them.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Choose what to Work out (adjacency list or matrix, degrees, a traversal or the shortest path) and turn off Show answers. The graph stays, the heading stays and the answer is left blank for students to fill in; the adjacency matrix keeps its empty grid. Student + key saves both versions.",
    },
    Faq {
        q: "In what order are neighbours visited?",
        a: "In the order the nodes were listed, so a traversal is the same every time. List the nodes yourself (nodes A, B, C ...) if you want a particular order rather than the order the edges mention them.",
    },
    Faq {
        q: "What does Layers from the first node do?",
        a: "It draws the graph like a tree: the first node on top, the nodes one step away on the next row, and so on, with any node that cannot be reached starting a new tree to the right. Circle spaces the nodes round a ring instead. Either way a node you placed with @ stays put.",
    },
    Faq {
        q: "How is the shortest path decided?",
        a: "By the total of the edge weights (every edge counts 1 if no weights are given), following arrows on directed edges. It is not available if any weight is negative. Ties are broken by node order.",
    },
];

const NETWORK_TOPOLOGY: &[Faq] = &[
    Faq {
        q: "Which topologies are there?",
        a: "Star (every device to a central switch), ring (each device to its two neighbours, round a loop), bus (every device off one backbone cable with a terminator at each end), mesh (every device to every other) and tree (a switch at the top with switches below it and computers below those).",
    },
    Faq {
        q: "How is the number of cables worked out?",
        a: "Star and ring need one cable per device. A bus has one cable (a drop) per device plus the backbone. A full mesh needs n(n - 1) / 2 cables for n devices, so 5 devices need 10 and 8 need 28, which is why meshes are limited to 8 here. A tree needs one cable per device plus the cables between the switches.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The diagram stays and the topology name, the cable count and the strength and weakness lines are left as empty lines to write on. Student + key saves both copies.",
    },
    Faq {
        q: "Can I name the devices?",
        a: "Yes: type the names separated by commas (Anna, Ben, Cara). Any you leave out are called PC 1, PC 2 and so on. Names are cut at 10 characters. The central device of a star or tree can be renamed too, for example Hub or Server.",
    },
    Faq {
        q: "How many devices can there be?",
        a: "From 2 to 12. A ring or mesh needs at least 3, and a mesh is limited to 8 so the diagram stays readable.",
    },
];

const ER_DIAGRAM: &[Faq] = &[
    Faq {
        q: "How do I write an ER diagram?",
        a: "Entities are one line each: entity Student: id*, name, year, where a * marks the key (underlined) and a ^ marks a foreign key (tagged FK). Relationships are A 1:N B : label, the two cardinalities being how many of each side take part: 1, N, M, 0..1, 0..N or 1..N.",
    },
    Faq {
        q: "Which end does each cardinality belong to?",
        a: "The first sits next to the first entity and the second next to the second. In Teacher 1:N Course, one teacher is joined to many courses, so the 1 is by Teacher and the N by Course.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. Each cardinality becomes an empty box to fill in, and the entities, attributes and relationship labels stay. Tick Hide the attributes in the student copy as well to ask for the attributes too. Student + key saves both copies.",
    },
    Faq {
        q: "How is the diagram laid out?",
        a: "In layers from the first entity, with the entities joined to it on the next row; Circle spaces them round a ring instead. Two relationships between the same pair of entities are drawn side by side.",
    },
    Faq {
        q: "What are the limits?",
        a: "Up to 10 entities with 8 attributes each and 14 relationships.",
    },
];

const RECURSION_TREE: &[Faq] = &[
    Faq {
        q: "What do the boxes mean?",
        a: "Each box is one call of the function, with the call on top and the value it returns underneath. A call's children are the calls it makes, left to right. Shaded boxes are base cases, which return without calling anything.",
    },
    Faq {
        q: "Which functions are there?",
        a: "fib(n) (fib(n-1) + fib(n-2)), factorial(n), sum(n) (the sum of 1 to n), gcd(a, b) (Euclid's algorithm with a mod b) and power(x, n), which squares the result of power(x, n div 2) and multiplies by x when n is odd. Set the starting numbers with the n / a / x box and the b box.",
    },
    Faq {
        q: "What do the numbers on the boxes show?",
        a: "Order the calls start numbers the boxes in the order the calls are made (a call starts before its children). Order the calls return numbers them in the order the calls finish, children before their parent, so the first call returns last.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The calls and the boxes stay and the return values and numbers are left empty to fill in. Student + key saves both copies.",
    },
    Faq {
        q: "Why does it say there are too many calls?",
        a: "A tree is limited to 80 calls so it stays readable; fib(8) is the largest Fibonacci that fits. Choose a smaller number. Results too big to show (a huge power) are refused the same way.",
    },
];

const SORTING_PASSES: &[Faq] = &[
    Faq {
        q: "What does each row show?",
        a: "The start is the array as typed. Each Pass row is the array when that pass has finished. The cells that changed in that pass are highlighted, and the sorted part is lightly shaded. The last column counts the comparisons made and the swaps (or, for insertion sort, shifts) done in that pass.",
    },
    Faq {
        q: "How does each sort work here?",
        a: "Bubble sort sweeps from the left swapping neighbours that are out of order, so the largest settles at the end each pass, and stops after a pass with no swaps. Selection sort finds the smallest of what is left and swaps it into the next place. Insertion sort takes the next value and shifts the sorted values along to open a gap for it.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The start row and the headings stay and the other rows are empty boxes of the same size. Rows filled in for students keeps more than the start row if you want to give a worked pass. Student + key saves both copies.",
    },
    Faq {
        q: "Can it sort largest first?",
        a: "Yes, tick Largest first. Equal values keep their original order in selection and insertion sort and in bubble sort.",
    },
    Faq {
        q: "How many values can it take?",
        a: "Up to 12 numbers, separated by commas or spaces or one per line. Values that are not numbers are ignored.",
    },
];

const TRACE_TABLE: &[Faq] = &[
    Faq {
        q: "What can the pseudocode do?",
        a: "Assignments (total = total + i), print, and one loop: while condition ... end, or for i = 1 to 5 ... end (add step 2 or step -1). Expressions use + - * /, div and mod (or // and %), brackets, comparisons, and, or and not. Lines after the loop's end run once afterwards. Start a line with # for a comment.",
    },
    Faq {
        q: "What does one row mean?",
        a: "With One row per Loop pass, the first row (start) is the variables after the lines before the loop, and each later row is the values at the end of a pass round the loop. With Every statement there is a row after each statement, labelled by its line number.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The program, the headings and the number of rows stay; the first rows (set by Rows filled in for students, one by default) keep their values and the rest of the cells are empty. Student + key saves both copies.",
    },
    Faq {
        q: "Why does it stop with a note?",
        a: "A line it cannot run (dividing by zero, using a variable before it has a value) ends the trace at that line and says so under the table. A loop that never finishes stops after 40 rows. The figure is limited to 24 lines and 8 variables.",
    },
    Faq {
        q: "Why is i one more than the last value in the loop?",
        a: "A for loop adds the step after each pass, so when the loop ends i has gone past the end value, as it does in most languages. The table shows that value in the last row.",
    },
];

const STATE_MACHINE: &[Faq] = &[
    Faq {
        q: "How do I write a state machine?",
        a: "One item per line. start q0 marks the start state and accept q2, q3 marks accepting states (drawn as a double circle). A transition is q0 -> q1 : 0, 1, which moves on either symbol. A state appears as soon as a line mentions it; a line states A, B, C fixes the order they are listed in.",
    },
    Faq {
        q: "What does Trace an input string do?",
        a: "It runs the machine on the input and shows the state after each symbol, then Accepted or Rejected. A symbol with no transition leaves the empty set and rejects. If a state has two transitions on the same symbol (a nondeterministic machine) the trace follows every branch and shows the set of states.",
    },
    Faq {
        q: "How do I write the input?",
        a: "Together when each symbol is one character (0110) or separated by spaces when symbols are longer words (coin push coin). Up to 24 symbols are used.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Choose a transition table or a trace and turn off Show answers. The diagram stays, the table keeps its states and symbols with empty cells, and the trace keeps the input with blank lines to write on. Student + key saves both copies.",
    },
    Faq {
        q: "Why are two arrows drawn side by side?",
        a: "A transition each way between the same two states is drawn as two arrows next to each other so both stay visible. Labels for the same pair of states in the same direction share one arrow.",
    },
];

const SUPPLY_DEMAND: &[Faq] = &[
    Faq {
        q: "How are the curves defined?",
        a: "Each is a straight line from its price at quantity 0 and its slope: demand falls by the slope for each extra unit, supply rises by it. The axes are chosen so both curves and the equilibrium fit.",
    },
    Faq {
        q: "How do I shift a curve?",
        a: "Pick Right (increase) or Left (decrease) for demand or supply and say how many units of quantity to move it. The original curve stays as a dashed grey line labelled D1 or S1, the new one is D2 or S2, and the readout gives the new and the old equilibrium.",
    },
    Faq {
        q: "How do price controls work?",
        a: "Choose a ceiling or a floor and a price. A ceiling above the equilibrium price, or a floor below it, does nothing and the readout says it is not binding. Otherwise the figure marks the shortage or the surplus between the quantity supplied and demanded.",
    },
    Faq {
        q: "What are the shaded areas?",
        a: "Consumer surplus is the triangle between the demand curve and the equilibrium price, producer surplus the triangle between that price and the supply curve. Their sizes are in the readout. Choose which to shade, or none.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The curves, their labels and any price control stay; the equilibrium, shading, guide lines, shortage and readout go. Student + key saves both copies at the same size.",
    },
];

const ELASTICITY: &[Faq] = &[
    Faq {
        q: "How is elasticity worked out?",
        a: "By the midpoint formula: the change in quantity over the average quantity, divided by the change in price over the average price, taken as a positive number. The starting-point formula divides by the first point instead. The working is shown line by line.",
    },
    Faq {
        q: "What do the verdicts mean?",
        a: "Above 1 is elastic (quantity responds more than price), below 1 is inelastic, and within half a percent of 1 is unit elastic. The last line compares total revenue (price times quantity) at the two points, which rises when price falls only if demand is elastic.",
    },
    Faq {
        q: "Why must the two points differ?",
        a: "Elasticity compares a change in price with a change in quantity, so both must change; the figure says so if they do not. A starting-point formula also needs a first price and quantity above 0.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The two points and the demand line stay, with a blank line after each step of the working for the student to fill in. Student + key saves both copies.",
    },
    Faq {
        q: "Can I hide the axis numbers?",
        a: "Yes: turn off Number the axes for a plain sketch with the points and guide lines only.",
    },
];

const PPF: &[Faq] = &[
    Faq {
        q: "How is the frontier drawn?",
        a: "From the most of each good that could be made if all resources went to it. Bowed out is a quarter ellipse (rising opportunity cost); a straight line has a constant opportunity cost.",
    },
    Faq {
        q: "How are the points classed?",
        a: "Each point is compared with the frontier: inside means attainable but not efficient, on it means efficient, outside means unattainable with current resources. A point within one percent of the curve counts as on it.",
    },
    Faq {
        q: "How do I work out an opportunity cost?",
        a: "Give a from point and a to point by name. The readout is what is given up of one good per unit gained of the other. If the move gains or loses both goods it is not a trade-off along the frontier and the readout says so.",
    },
    Faq {
        q: "How do I add points?",
        a: "One per line in the Points box: a name (up to 8 characters) and the two quantities, like A 40 60. Up to six.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The frontier and the points stay; each point's class and the opportunity cost are left as blank lines. Student + key saves both copies.",
    },
];

const COST_CURVES: &[Faq] = &[
    Faq {
        q: "What cost function does it use?",
        a: "Fixed cost plus variable cost = a q − b q² + c q³. From it the figure works out marginal cost (the slope of variable cost), average variable cost and average total cost. It checks that marginal cost never goes below zero.",
    },
    Faq {
        q: "Where is the profit-maximising quantity?",
        a: "Where marginal cost, on its rising part, equals the price. Below the lowest average variable cost the firm shuts down and produces nothing, losing its fixed cost.",
    },
    Faq {
        q: "What is the rectangle?",
        a: "The profit or loss: the gap between the price and average total cost at that quantity, times the quantity. Green is a profit, red a loss.",
    },
    Faq {
        q: "What are the small circles?",
        a: "The lowest points of the average total cost and average variable cost curves, where marginal cost crosses them. They are part of the answer, so a student copy leaves them out.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The three curves and the price line stay; the quantity, rectangle, markers and readout go. Student + key saves both copies.",
    },
];

const MONOPOLY: &[Faq] = &[
    Faq {
        q: "How are the curves worked out?",
        a: "Demand is a straight line; marginal revenue starts at the same price but falls twice as fast. Marginal cost is a line from its intercept, and average total cost comes from the fixed cost plus the cost of the units.",
    },
    Faq {
        q: "Where does the monopolist produce?",
        a: "Where marginal revenue equals marginal cost, with the price read up to the demand curve from that output. The profit rectangle is price minus average total cost, times the output.",
    },
    Faq {
        q: "What is the deadweight loss?",
        a: "The triangle between demand and marginal cost, from the monopoly output to the output a competitive market would produce, where price equals marginal cost. It can be turned off.",
    },
    Faq {
        q: "What if marginal cost starts above demand?",
        a: "Then no output is worth making; the figure says so instead of drawing.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The four curves stay; the output, price, rectangle, triangle and readout go. Student + key saves both copies.",
    },
];

const CIRCULAR_FLOW: &[Faq] = &[
    Faq {
        q: "Which models are there?",
        a: "Two sectors with markets: households and firms joined by the product market and the factor market, with real flows (goods, services, factors) as solid blue arrows and money flows (spending, revenue, payments, income) as dashed orange ones. Three sectors adds government and shows the money flows between households, firms and government.",
    },
    Faq {
        q: "Can I show only one loop?",
        a: "Yes, in the two-sector model turn the real flows or the money flows off.",
    },
    Faq {
        q: "What can I leave blank?",
        a: "Choose Flow labels or Sector names under Leave blank in the student copy. With Show answers off that part becomes empty boxes of the same size, for students to label.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The arrows stay, with the part you chose left blank. Student + key saves both copies at the same size.",
    },
    Faq {
        q: "What does Print style change?",
        a: "Everything becomes black and grey; the real and money flows are told apart by solid and dashed lines.",
    },
];

const AD_AS: &[Faq] = &[
    Faq {
        q: "What curves are drawn?",
        a: "A downward aggregate demand curve, an upward short-run aggregate supply curve, and a vertical long-run aggregate supply at the full-employment output.",
    },
    Faq {
        q: "How do shifts work?",
        a: "Move AD or SRAS right or left by a number of units of output. The old curve stays dashed and grey as AD1 or SRAS1, the new one is AD2 or SRAS2, and the readout says whether output and the price level rise or fall.",
    },
    Faq {
        q: "What is the gap?",
        a: "The difference between the equilibrium output and full employment. Below it is a recessionary gap, above it an inflationary gap; within half a percent is no gap.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The curves and their labels stay; the equilibrium, guide lines, gap and readout go. Student + key saves both copies.",
    },
    Faq {
        q: "Why can the figure refuse a shift?",
        a: "If a shift would push a curve below a price level of 0, or the curves would not cross, it says so instead of drawing a misleading graph.",
    },
];

const LORENZ: &[Faq] = &[
    Faq {
        q: "What do I enter?",
        a: "The income of each equal-sized group of the population, from 2 to 10 numbers separated by spaces, commas or new lines. They are put in order from lowest to highest.",
    },
    Faq {
        q: "How is the Gini coefficient worked out?",
        a: "From the area between the line of equality and the Lorenz curve, as a share of the area under the line of equality. 0 is perfect equality; the nearer to 1, the more one group has.",
    },
    Faq {
        q: "What does the table show?",
        a: "Each group's income and share of the total, the cumulative population share and the cumulative income share, which are the points of the Lorenz curve.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The incomes and the line of equality stay; the share and cumulative columns, the curve, the shading and the Gini are left for the student to work out and plot. Student + key saves both copies.",
    },
    Faq {
        q: "What are the limits?",
        a: "Up to ten groups, incomes up to a trillion, and at least some income somewhere.",
    },
];

const BANK_RECONCILIATION: &[Faq] = &[
    Faq {
        q: "How do I enter the balances and adjustments?",
        a: "One line for each balance, bank 5200 for the bank statement and book 5250 for the cash book, then adjustment lines: bank + Deposits in transit 800 adds to the bank side and book - Bank charges 35 deducts from the cash book side. Each side has up to six adjustments.",
    },
    Faq {
        q: "Which items go on which side?",
        a: "Timing differences adjust the bank statement: deposits in transit are added and outstanding cheques deducted. Items the bank has recorded that the business has not adjust the cash book: interest and direct credits are added, charges and dishonoured cheques deducted. The figure draws whatever you type; it does not decide the side for you.",
    },
    Faq {
        q: "What does the check say?",
        a: "When the two adjusted balances are equal it says Reconciled and shows the balance. When they are not it says how far apart they are and which side is higher, which is the cue to look for a missed item.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. Both opening balances and every adjustment stay; the two adjusted balances are empty boxes and the check is a blank line. Student + key saves both copies.",
    },
    Faq {
        q: "Can I change the labels?",
        a: "The two panels are always headed Bank statement and Cash book. The adjustment names are your own, cut at 26 characters, and the whole statement can have a title.",
    },
];

const CASH_FLOW_STATEMENT: &[Faq] = &[
    Faq {
        q: "How do I write the activities?",
        a: "A heading such as Operating activities, then one Description amount line for each cash flow. A plain or + amount is cash in; a - amount, or an amount in (brackets), is cash out and is shown in brackets. Up to four sections of eight lines. Investing and financing activities follow the same way.",
    },
    Faq {
        q: "What does it work out?",
        a: "The net cash from each section, the net increase (or decrease) in cash, and the closing cash, which is the opening cash plus the net change. The opening cash is a separate setting, given on the statement.",
    },
    Faq {
        q: "What is the closing cash check?",
        a: "If you type the closing cash from the balance sheet, the statement checks that it agrees with the closing cash worked out and says so, or by how much it differs. Leave it empty for no check.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The cash flows and the opening cash stay; every net total, the net change, the closing cash and the check become empty boxes. Student + key saves both copies.",
    },
    Faq {
        q: "Does it show the direct or the indirect method?",
        a: "It lays out whatever lines you type, so the operating section can list cash received and paid (the direct method) or start from net income with adjustments (the indirect method): in either case the totals work the same way.",
    },
];

const FINANCIAL_STATEMENT: &[Faq] = &[
    Faq {
        q: "How do I write the accounts?",
        a: "A heading line such as Revenue, then one Account amount line for each account under it, then the next heading. For an income statement the first section is revenue and every later section is an expense. For a balance sheet the first is assets and the later ones are liabilities and equity. Up to six sections of ten accounts.",
    },
    Faq {
        q: "What does it work out?",
        a: "A total for each section, then the net income (or net loss, written in brackets) for an income statement, or the total of each side of the balance sheet with a check that assets equal liabilities plus equity. If they do not, it says by how much and which side is larger.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The accounts and amounts stay; every total, the net income and the check become empty boxes and a blank line. Student + key saves both copies.",
    },
    Faq {
        q: "Can I keep two statements for the same business?",
        a: "Yes: the income statement and the balance sheet have separate account boxes, and the business name and period are shared, so switch the Statement choice to move between them.",
    },
    Faq {
        q: "How are the amounts laid out?",
        a: "Account amounts sit in the inner column and totals in the outer column with a rule above, the totals carrying the currency symbol and a double rule under the final figure, as is usual on printed statements.",
    },
];

const ACCOUNTING_EQUATION: &[Faq] = &[
    Faq {
        q: "How do I enter the transactions?",
        a: "One line each: a description, a colon, then the effects as letters and amounts, for example Buy equipment on credit: A +3000, L +3000. A is assets, L liabilities and E equity; a minus sign reduces it. Two effects on the same letter, such as A +400, A -400, are added, which is how an asset swap shows as no change.",
    },
    Faq {
        q: "What is checked?",
        a: "Each transaction must keep the equation balanced: the change in assets equals the change in liabilities plus the change in equity. A row that does not is named under the table. The totals then show assets = liabilities + equity.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The numbered descriptions stay and the effect cells, the totals and the check are empty for students to fill. Student + key saves both copies.",
    },
    Faq {
        q: "How many transactions can there be?",
        a: "Up to ten, with descriptions cut at 40 characters.",
    },
    Faq {
        q: "Why are the amounts signed?",
        a: "Because the table shows the change each transaction makes, a plus for an increase and a minus for a decrease. The totals row shows the net result for each part.",
    },
];

const DEPRECIATION_SCHEDULE: &[Faq] = &[
    Faq {
        q: "Which methods are there?",
        a: "Straight-line (the same expense each year), double-declining balance (twice the straight-line rate applied to the opening book value, never taking the asset below its salvage value) and sum of the years' digits (a falling fraction of cost less salvage: for 5 years 5/15, 4/15, and so on).",
    },
    Faq {
        q: "What do the columns mean?",
        a: "Expense is the depreciation charged that year, accumulated depreciation is the total so far, and book value is cost less accumulated depreciation. Year 0 shows the starting book value, which is the cost.",
    },
    Faq {
        q: "How are rounding and the last year handled?",
        a: "Each year's expense is rounded to the cent, and the last year takes whatever brings the book value exactly to the salvage value, so the total expense is always cost less salvage.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The asset, cost, salvage value, life and method stay, along with the year-0 line; every expense, accumulated depreciation and book value cell and the total are left empty. Student + key saves both copies.",
    },
    Faq {
        q: "What are the limits?",
        a: "A life of 1 to 15 years, a cost above 0 and a salvage value of at least 0 and below the cost, up to a trillion.",
    },
];

const T_ACCOUNT: &[Faq] = &[
    Faq {
        q: "How do I write the accounts?",
        a: "Start each account with a line such as account Cash, then one line per entry: Dr 5000 Capital for a debit (left side) or Cr 1200 Rent for a credit (right side), with an optional short description. Up to four accounts, each with up to 12 entries a side, are laid out two to a row.",
    },
    Faq {
        q: "How is an account balanced off?",
        a: "The two sides are added. The smaller side gets a balancing figure (Balance c/d) so both totals match, the totals are ruled off with a double line, and the balance is carried down below as Balance b/d, a debit balance if the debits were larger. An account that already balances shows Balance nil.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The entries stay as the question; the balancing figure, the two totals and the balance carried down are left as empty boxes and a blank line. Student + key saves both copies.",
    },
    Faq {
        q: "Can I leave the accounts unbalanced?",
        a: "Yes: untick Balance off each account and each side shows just its own total, which is useful for a first lesson on debits and credits.",
    },
    Faq {
        q: "What amounts can I use?",
        a: "Positive amounts up to a trillion, with or without commas and a currency symbol (1,200.50, $300). A debit or credit is never negative, so put an amount on the other side instead. Cents are shown automatically when an amount has them, or always if you tick Always show cents.",
    },
];

const JOURNAL_ENTRY: &[Faq] = &[
    Faq {
        q: "How do I write a journal entry?",
        a: "A date line, then Dr Account amount for each account debited and Cr Account amount for each credited, then the narration in brackets, for example (Paid wages and rent). A blank line, or a new date, starts the next entry. An entry can have up to six lines, so compound entries are fine.",
    },
    Faq {
        q: "What does the check show?",
        a: "The two totals are written under the table with Debits equal credits when they match, or Out of balance by an amount and which side is larger when they do not, so a typing slip in a worked example shows up.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The dates and narrations stay as the description of each transaction; the accounts, amounts and totals are left empty, and a Do debits equal credits? line is left to fill in. Student + key saves both copies.",
    },
    Faq {
        q: "How is the table laid out?",
        a: "Date, Account, Debit and Credit columns with the debit accounts at the left of the account column and the credit accounts indented, the narration in brackets beneath, and a ruled line between entries. Up to eight entries fit.",
    },
    Faq {
        q: "Can I use another currency?",
        a: "Yes: choose $, \u{a3}, \u{20ac} or None. The symbol is written on the totals line; the table columns show plain amounts as is usual.",
    },
];

const TRIAL_BALANCE: &[Faq] = &[
    Faq {
        q: "How do I enter the balances?",
        a: "One line per account: the name, then Dr or Cr for the side its balance is on, then the amount, for example Accounts payable Cr 900. Up to 20 accounts. The Balance column shows each as given.",
    },
    Faq {
        q: "What does the figure work out?",
        a: "Each balance goes into the Debit or Credit column, both columns are added, and the line underneath says either The totals agree or Out of balance by an amount with the side that is larger. Try the Out of balance example to see a difference reported.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The accounts and balances stay; the Debit and Credit columns and both totals are left empty and the difference is a blank line. Student + key saves both copies.",
    },
    Faq {
        q: "Does the order matter?",
        a: "Accounts are listed in the order you type them, so use the usual order of assets, liabilities, capital, income and expenses if you want a conventional trial balance.",
    },
    Faq {
        q: "What are the limits?",
        a: "Up to 20 accounts, names cut at 28 characters, and positive amounts up to a trillion.",
    },
];

const RC_CURVE: &[Faq] = &[
    Faq {
        q: "What is the time constant?",
        a: "\u{3c4} = R \u{d7} C, the time a capacitor takes to reach about 63% of its final voltage when charging, or to fall to about 37% when discharging. The axis is in time constants so every circuit has the same shape; the working under the graph gives \u{3c4} in seconds, milliseconds or microseconds.",
    },
    Faq {
        q: "What do the percentages on the curve mean?",
        a: "With Mark each time constant on, the curve is marked at 1\u{3c4} to 5\u{3c4} with how far it has got: 63.2%, 86.5%, 95.0%, 98.2% and 99.3% when charging up, or the amounts left, 36.8%, 13.5%, 5.0%, 1.8% and 0.7%, when falling. After about 5\u{3c4} a capacitor is treated as fully charged or discharged.",
    },
    Faq {
        q: "Which quantities can I graph?",
        a: "Capacitor voltage, current (in mA) or charge (in \u{b5}C). While charging, the voltage and charge rise and the current falls; while discharging all three fall. Current is drawn as its size, whichever way it flows.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The axes, the given values of R, C and the battery stay; the curve, the marks and the working are removed, with a blank line for \u{3c4}. Student + key saves both copies.",
    },
    Faq {
        q: "What units do I enter?",
        a: "Resistance in kilohms, capacitance in microfarads and the battery in volts, so the defaults of 10 k\u{3a9} and 100 \u{b5}F give \u{3c4} = 1 s.",
    },
];

const KIRCHHOFF_LOOPS: &[Faq] = &[
    Faq {
        q: "What circuit is this?",
        a: "Three branches joined at a top and bottom junction: a battery and R1 on the left, R2 in the middle, and R3 with a second battery on the right. It is the standard two-loop problem for Kirchhoff's laws.",
    },
    Faq {
        q: "Which equations are used?",
        a: "The junction rule, I\u{2081} + I\u{2083} = I\u{2082}, and the loop rule round each loop, V1 \u{2212} R1 I\u{2081} \u{2212} R2 I\u{2082} = 0 and V2 \u{2212} R3 I\u{2083} \u{2212} R2 I\u{2082} = 0. The three are solved together for the currents.",
    },
    Faq {
        q: "Why is a current negative?",
        a: "The arrows are only assumed directions: I\u{2081} and I\u{2083} up their branches and I\u{2082} down the middle. A negative answer means the real current runs against its arrow, which happens when one battery is much stronger than the other.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The circuit, every value and the assumed directions stay, and the junction equation, both loop equations and the currents become blank lines. Student + key saves both copies.",
    },
    Faq {
        q: "Can I change the values?",
        a: "Yes: both batteries and all three resistors. They must be above 0, and the answers are exact to three decimals.",
    },
];

const RESIDUAL_PLOT: &[Faq] = &[
    Faq {
        q: "What is a residual?",
        a: "The vertical distance from a point to the line of best fit: the actual y minus the predicted y. A point above the line has a positive residual and a point below it a negative one, and the residuals always add up to 0.",
    },
    Faq {
        q: "What does each view show?",
        a: "Scatter with residuals draws the line and a dashed segment from every point to it. Residual plot draws each residual against x around a dashed zero line, where a random scatter means a line fits well and a curve means it does not. Both stacks the two with the same x axis.",
    },
    Faq {
        q: "How do I get the table and the sum of squares?",
        a: "Tick Table of predicted values and residuals (up to 15 points) for x, y, predicted y and residual, and Write the sum of squared residuals for the total the line minimises.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The scatterplot keeps its points and loses the line and segments, the residual plot keeps empty axes with the zero line, and the predicted y, residual and sum of squares are blank. Student + key saves both copies.",
    },
    Faq {
        q: "Does it pair with the scatterplot?",
        a: "Yes. It reads pairs the same way, so paste the same data into both. Enter at least two points with different x values.",
    },
];

const CONFIDENCE_INTERVAL: &[Faq] = &[
    Faq {
        q: "Which intervals can I draw?",
        a: "A mean when the population standard deviation is known (uses z*), a mean when only the sample standard deviation is known (uses t* with n - 1 degrees of freedom), and a proportion (uses z*). The level can be 90%, 95% or 99%.",
    },
    Faq {
        q: "How is it worked out?",
        a: "The margin of error is the critical value times the standard error: z* or t* times the standard deviation over the square root of n, or times the square root of p(1 - p)/n for a proportion. The interval is the estimate plus and minus the margin. The working is written under the number line.",
    },
    Faq {
        q: "What does Mark a value do?",
        a: "Type a claimed mean or proportion to draw a dashed line at it. The working then says whether it lies inside the interval, which is the usual test of a claim. Leave it empty for no mark.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The number line, the estimate and the facts about the sample stay; the interval, its ends, the margin of error and the working become blank lines. Student + key saves both copies.",
    },
    Faq {
        q: "What are the limits?",
        a: "n from 2 up, a standard deviation above 0, and a proportion between 0 and 1. The t values are found numerically and agree with the printed tables to about four significant figures.",
    },
];

const DISCRETE_DISTRIBUTION: &[Faq] = &[
    Faq {
        q: "Which distributions are there?",
        a: "Binomial (n trials, each with probability p of success, counting the successes) and Poisson (events at an average rate, counting how many happen). A binomial has bars from 0 to n; a Poisson has bars up to about four standard deviations above its mean, and any probability beyond the last bar is still included in the answer.",
    },
    Faq {
        q: "What do the shading options mean?",
        a: "Exactly k shades one bar. At most k shades k and everything below it. At least k shades k and everything above it. Between shades both values and all those between. If you give the bounds the wrong way round they are swapped.",
    },
    Faq {
        q: "How is the probability worked out?",
        a: "By adding the exact probability of each shaded bar, so it is exact rather than a normal approximation. It is shown to four decimals, and as < 0.0001 or > 0.9999 when it is that close to 0 or 1.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The bars and the shading stay and the probability is hidden for students to work out. Student + key saves both copies. Write the probability above each bar gives a table of values on the key.",
    },
    Faq {
        q: "What are the limits?",
        a: "Up to 60 trials, a Poisson mean up to 30, and p between 0 and 1. The values above the bars are shown only when there are 24 bars or fewer.",
    },
];

const VENN_DIAGRAM: &[Faq] = &[
    Faq {
        q: "In what order do I enter the region values?",
        a: "Two sets: A only, B only, both, neither. Three sets: A only, B only, C only, A and B only, A and C only, B and C only, all three, none. Separate them with commas. A value can be a count or short text such as 2 4 6 to list elements.",
    },
    Faq {
        q: "How are the totals worked out?",
        a: "From the region values when they are all numbers: n(A) adds every region inside A, so with 12 in A only and 5 in both, n(A) = 17. If a value is text, the totals are left out.",
    },
    Faq {
        q: "How do I shade a region?",
        a: "Choose one under Shade: a set, both sets, either set, one set only, outside a set, or everything outside all the circles. The shading stays in the student copy so it can be the question.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The circles, names and shading stay and the region values and totals are left empty. Student + key saves both copies.",
    },
    Faq {
        q: "Can I name the sets?",
        a: "Yes, up to 12 characters each. Set C is only used with three sets.",
    },
];

const PROBABILITY_TREE: &[Faq] = &[
    Faq {
        q: "How do I enter the branches?",
        a: "Each stage is a list of outcomes with their probabilities, separated by commas: Rain 0.3, Dry 0.7. Probabilities can be decimals (0.3), percentages (30%) or fractions (3/10). The second stage is one line used after every first-stage outcome, or one line per first-stage outcome, in order, when the probabilities change (such as drawing without replacement). The third stage is one line used after every second-stage outcome.",
    },
    Faq {
        q: "What does Work out each path show?",
        a: "On the right, each whole path from left to right is listed with its probability, found by multiplying the probabilities along its branches: Rain, Late = 0.3 x 0.4 = 0.12. The paths always add to 1 when every set of branches does.",
    },
    Faq {
        q: "Why does it say the probabilities do not add to 1?",
        a: "The branches leaving any one point must add to 1. The figure still draws what you typed and notes the first place it found, so you can fix the numbers or leave it for a student to spot.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The tree stays and the path probabilities are left as blank lines. Tick Leave the branch probabilities blank in the student copy as well to have students fill in the branches. Student + key saves both copies.",
    },
    Faq {
        q: "How big can the tree be?",
        a: "Up to five outcomes per stage, three stages, and 30 paths in all.",
    },
];

const BOOLEAN_TABLE: &[Faq] = &[
    Faq {
        q: "How do I give the function?",
        a: "Either as an expression (Q = (A AND B) OR (NOT A AND C), one per line, words or the symbols & | ^ !) or as a list of minterms: the row numbers where the output is 1, counting the first variable as the top bit, so with A, B, C the row A=1, B=0, C=1 is number 5. Don't-cares are rows where the output does not matter and are shown as X.",
    },
    Faq {
        q: "How is the simplified expression found?",
        a: "By the Quine-McCluskey method: all the biggest groups of rows are found, the essential ones are taken and then the group that covers the most that is left, so the result is a small sum of products (a minimal one for the usual textbook sizes). A prime means NOT, so A'C + AB reads (NOT A AND C) OR (A AND B). Don't-cares are used when they make a group bigger and are never required.",
    },
    Faq {
        q: "How do I read the Karnaugh map?",
        a: "The side and top labels run in Gray code (00, 01, 11, 10) so neighbouring cells differ in one variable. The map is for 2 to 4 variables. The coloured loops are the groups of 1s (and X) that make the terms of the simplified expression; a group that reaches the edge wraps round and is drawn as two loops.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The inputs, the empty table cells, the empty map and the heading stay and the outputs, the groups and the simplified expression are left for students to fill in. Student + key saves both copies.",
    },
    Faq {
        q: "What if the expression has several outputs?",
        a: "The truth table shows a column for each. The Karnaugh map shows one: pick which under Output for the map.",
    },
];

const FLOWCHART: &[Faq] = &[
    Faq {
        q: "How do I write a flowchart?",
        a: "One box per line as id: kind text, for example B: decision n > 0?. The kinds are start, end, process, input, output and decision. Arrows are A -> B, which can chain (A -> B -> C), and A -> B : yes puts a label on the arrow. Lines starting after a # are ignored.",
    },
    Faq {
        q: "How is the chart laid out?",
        a: "Top to bottom in layers from the first box, with the boxes that follow a decision side by side. An arrow that goes back up to an earlier box (a loop) is drawn round the right-hand side so it does not cross the boxes.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "List the ids of the boxes to hide under Blank in student copy (for example D, P) and turn off Show answers. Those boxes stay, at full size, with nothing inside, and the arrows and labels stay. Student + key saves both copies.",
    },
    Faq {
        q: "What are the shapes?",
        a: "Start and end are rounded, process is a rectangle, input and output are slanted boxes and a decision is a diamond. Long text wraps onto two lines, and text is cut at 40 characters.",
    },
    Faq {
        q: "Why does it say a box is missing?",
        a: "An arrow names a box id that has no line of its own. Define every box (A: process ...) before or after the arrows; the ids are case-sensitive and up to 8 letters, digits or underscores. A chart is limited to 30 boxes and 60 arrows.",
    },
];

const DATA_STRUCTURE: &[Faq] = &[
    Faq {
        q: "How do I show what push and pop do?",
        a: "Choose Stack (or Queue, Linked list), list the starting values and type the operations separated by commas, such as push 5, push 9, pop. The starting structure is drawn first and the result below it.",
    },
    Faq {
        q: "Which operations does each structure understand?",
        a: "Stack: push and pop. Queue: enqueue and dequeue. Linked list: addfirst, addlast, removefirst and removelast. An array has no operations; it shows its values and indices. Anything else is skipped and listed under the result.",
    },
    Faq {
        q: "Which end is which?",
        a: "Values run bottom to top for a stack (the top is marked), front to back for a queue and head to tail for a linked list (ending in null). The first value of an array is index 0.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The starting structure and the operations stay and the result is replaced by an empty dashed box of the same size, for students to draw in. Student + key saves both copies.",
    },
    Faq {
        q: "What happens when an operation cannot be done?",
        a: "Popping an empty stack, or pushing past 14 items, is skipped and listed under the result as skipped, so you can see why the answer is what it is. At most 14 values and 24 operations are used.",
    },
];

const NUMBER_BASE: &[Faq] = &[
    Faq {
        q: "How do I choose which column is given?",
        a: "Set Given in to decimal, binary, hexadecimal or octal and type the values in that base, separated by commas or spaces. That column is filled in for every row and the other three are the answers.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The given column stays and the other columns become empty boxes of the same size. Student + key saves both copies.",
    },
    Faq {
        q: "What does Binary width do?",
        a: "It pads binary answers with zeros to a whole number of bytes (or 4, 12 or 16 bits) and puts a space every four digits, so 13 reads 0000 1101 with 8 bits and 1101 with 4. Shortest shows no padding.",
    },
    Faq {
        q: "What is the strip of numbers above the table?",
        a: "Show binary place values adds a row of 128, 64, 32 ... 1 (as many as the grouping, or 8) with an empty row under it, for students to write the bits of a number in.",
    },
    Faq {
        q: "What happens with a value that is not a number in that base?",
        a: "Its row stays, with a question mark after what was typed and dashes in the other columns, so you can see which entry to fix. Values above 4,294,967,295 are treated the same way, and at most 24 rows are drawn.",
    },
];

const TREE_DIAGRAM: &[Faq] = &[
    Faq {
        q: "How is a binary search tree built from the values?",
        a: "The values are inserted one at a time in the order given. Each goes left if it is smaller than the node it meets and right if it is larger, until it finds an empty place. A repeated value is ignored. Change the order and the tree changes shape: sorted values make a lopsided chain.",
    },
    Faq {
        q: "How do I draw a tree that is not a search tree?",
        a: "Choose Parent lines and write one line per parent, such as A: B, C. The root is the one node nobody lists as a child. Use _ for an empty slot (B: D, _) to make a binary tree with a gap, which is drawn with the single child on its own side.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Choose what to Work out and turn off Show answers. The tree and the heading stay and the answer is left blank, with the node numbers hidden. Student + key saves both versions.",
    },
    Faq {
        q: "Why does In-order say it needs a binary tree?",
        a: "In-order means left, node, right, which only makes sense with at most two children each. Pre-order, post-order and level-order work for any tree.",
    },
    Faq {
        q: "What are height and depth?",
        a: "Depth is how many edges a node is below the root (the root is 0). Height is the largest depth, so a single node has height 0 and the default tree has height 3.",
    },
];

const LOGIC_CIRCUIT: &[Faq] = &[
    Faq {
        q: "How do I write a circuit?",
        a: "One item per line. List the inputs with in A, B. Write each gate as a name, an equals sign and the gate with what it reads, such as g1 = AND(A, B). Mark an output with out Q = g1. Gates can read inputs or other gates, in any order. Add @x,y to a line to place that part yourself. Or build it on the canvas: pick a part and click to place it, drag from an output pin onto an input pin to wire, click an input to flip it, double-click a name to rename, and the text is written for you.",
    },
    Faq {
        q: "Can I type a boolean expression instead?",
        a: "Yes. Choose Boolean expression under Build the circuit from and type Q = (A OR B) AND NOT C. Use the words AND, OR, XOR, NOT or the symbols & | ^ !. Names that are never on the left of an equals sign become the inputs, and a line without an equals sign becomes an output named Q.",
    },
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The circuit and the table's inputs stay; the output column, the expression and the values inside the circuit are hidden. Student + key saves both versions.",
    },
    Faq {
        q: "How do the wires light up?",
        a: "Type 0s and 1s under Light up the inputs, one for each input in order, such as 101. Wires carrying 1 turn green (thick in Print style) and wires carrying 0 turn grey (thin and dashed in Print). The matching row of the table is marked. With answers off only the input wires show, so students trace the rest.",
    },
    Faq {
        q: "Why does it say my circuit has a loop?",
        a: "A gate cannot read its own output, directly or through other gates. This figure draws combinational circuits, where every output depends only on the inputs. Check that no gate is listed as an input to something it feeds.",
    },
    Faq {
        q: "Why is the truth table missing?",
        a: "A table has one row for every combination of inputs, so it is limited to 5 inputs (32 rows). A circuit with more inputs still draws; turn the table off to hide the note.",
    },
];

const DOT_PLOT: &[Faq] = &[
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The dots and the mean and median marks are hidden and the labelled number line stays, so students plot the data themselves. Student + key saves both versions.",
    },
    Faq {
        q: "My values are all different, so nothing stacks. What do I do?",
        a: "Set Group size, for example 2 to put values within 1 of each other in one stack. Leave it at 0 for exact values, which suits whole-number data such as counts or shoe sizes.",
    },
    Faq {
        q: "Mean or median?",
        a: "Mark either or both. The mean is the balance point of the dots and is pulled toward a few very high or low values; the median is the middle value and is not. Seeing them apart is a good way to show skew.",
    },
    Faq {
        q: "How do I paste data from a spreadsheet?",
        a: "Copy a column, click in the Numbers box and paste. When you click away the numbers are tidied into a list. Open CSV reads a file, and Generate makes sample data from a seed.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Set Style to Print. The dots are filled grey with a black outline so they read on a black-and-white copy.",
    },
];

const STEM_AND_LEAF: &[Faq] = &[
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. The stems, the bar and the key stay and the leaves are hidden, so students fill them in. Student + key saves both versions.",
    },
    Faq {
        q: "What is the leaf unit?",
        a: "It is the place value of a leaf. With a leaf unit of 1, the number 63 is stem 6 and leaf 3. With 0.1, the number 6.3 is stem 6 and leaf 3. Auto picks the smallest unit that gives about a dozen stems. Values are rounded to the leaf unit.",
    },
    Faq {
        q: "When should I split the stems?",
        a: "When one stem has so many leaves that the plot looks like a single long row. Splitting puts leaves 0 to 4 and leaves 5 to 9 of each stem on two rows, which shows the shape more clearly.",
    },
    Faq {
        q: "Why is there a stem with no leaves?",
        a: "An empty stem is a gap in the data. It is kept on purpose, because the gap is part of the shape, just as a gap would be in a dot plot or histogram.",
    },
    Faq {
        q: "How are negative numbers shown?",
        a: "Negative stems have a minus sign, and the stems run from the most negative up through -0 and 0. A leaf belongs to its stem's number, so -3 | 2 means -32, and the leaves on a negative stem go from the lowest value to the highest.",
    },
];

const TWO_WAY_TABLE: &[Faq] = &[
    Faq {
        q: "How do I make a student copy?",
        a: "Turn off Show answers. By default the row totals, column totals and grand total are left as empty boxes. Use Blank for students to leave some counts empty as well, or instead. Student + key saves both versions, with the answers in bold on the key.",
    },
    Faq {
        q: "How do I enter the counts?",
        a: "Type the row names and column names separated by commas, then one line of counts per row. The table is as big as the names you give. Counts left out are 0.",
    },
    Faq {
        q: "Which cells are left blank?",
        a: "The blanks are taken along the table's diagonals, so they do not all land in one row or column. Change How many cells to leave more or fewer.",
    },
    Faq {
        q: "Can I show percentages?",
        a: "Not yet. The table shows counts and their totals. Type percentages as the counts if you want a table of percentages; the totals add them up.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Set Style to Print. The shaded header, side and total cells become light grey.",
    },
];

const EARTH_CYCLES: &[Faq] = &[
    Faq {
        q: "Which cycles are there?",
        a: "The rock cycle (igneous, sedimentary and metamorphic rock, magma and sediment, with the processes between them) and the water cycle. The extras tick adds the rock cycle's shortcuts or the water cycle's transpiration.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The stages stay and the process labels are left as empty boxes. Use Hide to blank the stage names instead. Student + key saves both copies.",
    },
    Faq {
        q: "Why is it drawn as a loop?",
        a: "Rock and water are never used up, they move from one stage to the next, and the arrows show which way.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Yes. The Print style uses greys only, with the same labels and arrows.",
    },
];

const EARTH_LAYERS: &[Faq] = &[
    Faq {
        q: "Which layers are drawn?",
        a: "Crust, mantle, outer core and inner core, with their depths in kilometres and whether each is solid or liquid.",
    },
    Faq {
        q: "Why is the crust so thin?",
        a: "It is only about 35 km of 6371 km. Widen the crust makes it thick enough to see; the table keeps the real figures.",
    },
    Faq {
        q: "How do I blank part of the table?",
        a: "Turn off Show answers and pick in Hide whether the names, the thicknesses or the states are left empty.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Yes. The Print style uses greys only, and the table stays readable.",
    },
];

const PLATE_BOUNDARIES: &[Faq] = &[
    Faq {
        q: "Which boundaries are there?",
        a: "A divergent boundary (mid-ocean ridge), three convergent ones (ocean and continent, two continents, two oceans) and a transform boundary seen from above.",
    },
    Faq {
        q: "What do the numbers mean?",
        a: "Each numbered circle marks a feature on the drawing and the same number names it in the list below.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The drawing, arrows and numbers stay and each name is left as a blank line. Student + key saves both copies.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Yes. The Print style swaps the colours for greys; the arrows and numbers are unchanged.",
    },
];

const STRATA: &[Faq] = &[
    Faq {
        q: "How are the layers dated?",
        a: "The lowest layer formed first, so the order runs from the bottom up (the law of superposition). A fault or dyke that cuts layers is younger than every layer it cuts.",
    },
    Faq {
        q: "What do I enter?",
        a: "One rock name per line, from 2 to 6 layers, top to bottom.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The layers and list stay and the order of formation is left as a blank line. Student + key saves both copies.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Yes. The Print style fills each layer with a different grey pattern so the layers can still be told apart.",
    },
];

const MOON_SEASONS: &[Faq] = &[
    Faq {
        q: "What does it show?",
        a: "Either the Moon at eight places round the Earth, with the lit half facing the Sun and how each phase looks from Earth, or the Earth at four places round the Sun with its tilted axis.",
    },
    Faq {
        q: "Why does the tilt cause seasons?",
        a: "The axis points the same way all year, so each hemisphere leans toward the Sun for part of the orbit and away for the rest. Choose which hemisphere the seasons are named for.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The phase names, or the dates and seasons, are left as empty boxes. Student + key saves both copies.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Yes. The Print style shades the dark and lit halves in greys.",
    },
];

const TOPO_CONTOURS: &[Faq] = &[
    Faq {
        q: "What do I enter?",
        a: "A grid of heights, rows of numbers separated by spaces or commas, from 2 by 2 up to 12 by 12 with the same number of values in each row.",
    },
    Faq {
        q: "How are the lines found?",
        a: "A contour is drawn every interval, joining points of equal height, by interpolating between grid values.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The grid numbers stay and the contour lines are left for students to draw. Student + key saves both copies.",
    },
    Faq {
        q: "What is marked as the highest point?",
        a: "A triangle on the grid cell with the largest height, named by its row and column in the caption below the map.",
    },
];

const HR_DIAGRAM: &[Faq] = &[
    Faq {
        q: "What do I enter?",
        a: "One star per line as name, temperature in kelvin and luminosity in times the Sun's, up to 10 stars.",
    },
    Faq {
        q: "Why is the temperature axis backwards?",
        a: "By convention hot stars are on the left, so temperature falls from left to right.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The stars stay and the shaded regions and the Class column are left blank. Student + key saves both copies.",
    },
    Faq {
        q: "How is a star classed?",
        a: "By its place on the diagram: main sequence along the diagonal band, giants and supergiants above it, white dwarfs below it and to the left.",
    },
];

const CLIMATE_GRAPH: &[Faq] = &[
    Faq {
        q: "What do I enter?",
        a: "Twelve rows, January first, each with the month's mean temperature in degrees Celsius then its rainfall in millimetres.",
    },
    Faq {
        q: "How is it drawn?",
        a: "Rainfall is bars read from the left axis and temperature is a line read from the right axis; the two axes share their gridlines.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The table stays and the axes are left empty to plot on, with the summary left as blank lines. Student + key saves both copies.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Yes. The Print style uses a grey pattern for the bars and a black line.",
    },
];

const POPULATION_PYRAMID: &[Faq] = &[
    Faq {
        q: "What do I enter?",
        a: "One row per age band, youngest first: the number of males then the number of females, from 2 to 21 bands. Choose 5 or 10 years for the band width.",
    },
    Faq {
        q: "What does the shape tell you?",
        a: "A wide base means many children, a young and growing population. A narrow base means few children, an ageing or shrinking one. The summary says which.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The table stays and the axes are left empty for students to draw the bars, with the summary left blank. Student + key saves both copies.",
    },
    Faq {
        q: "Can I print it in black and white?",
        a: "Yes. The Print style uses two grey patterns for males and females.",
    },
];

const GRID_REFERENCE: &[Faq] = &[
    Faq {
        q: "How are the references read?",
        a: "A four-figure reference gives the line to the left of the square, then the line below it. A six-figure reference adds tenths along and up inside the square.",
    },
    Faq {
        q: "What are the position numbers?",
        a: "Counts in grid squares from the bottom-left corner, so 3.4 is four tenths of the way across the fourth square.",
    },
    Faq {
        q: "How are bearings measured?",
        a: "Clockwise from north in three digits, so due east is 090° and due south is 180°. List pairs as feature numbers, such as 1 3.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The map and positions stay and the reference and bearing boxes are left empty. Student + key saves both copies.",
    },
];

const MAP_SCALE: &[Faq] = &[
    Faq {
        q: "What is the scale?",
        a: "1 : 50 000 means 1 cm on the map is 50 000 cm, which is 500 m, on the ground.",
    },
    Faq {
        q: "How is the scale bar drawn?",
        a: "To the scale, with the longest round length that fits in 10 cm, and the lines are drawn at their true length so a ruler agrees with the numbers when the page is printed at full size.",
    },
    Faq {
        q: "How are the workings set out?",
        a: "Map distance times the scale gives the ground distance in cm, shown in metres or kilometres.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The scale, the bar and the lines stay and the workings and ground distances are left empty. Student + key saves both copies.",
    },
];

const MUSIC_STAFF: &[Faq] = &[
    Faq {
        q: "How do I write the notes?",
        a: "A pitch then a length, separated by commas: C4 q, F#4 e, Bb3 h. Middle C is C4. Lengths are w (whole), h (half), q (quarter), e (eighth) and s (sixteenth); a dot after, as in q., makes a note half as long again. r is a rest, as in r q.",
    },
    Faq {
        q: "How are the bars made?",
        a: "A bar is closed when it holds as many beats as the time signature says, or where you write a | . The beats in each bar are listed below, and a bar that is too short or long is pointed out in the key.",
    },
    Faq {
        q: "When does an accidental appear?",
        a: "Only when a note differs from the key signature, and it carries on for the same pitch until the bar line. Write the pitch you want to hear: in G major write F#4 for the note in the signature and F4 for the natural.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The staff stays and the note names and the beats in each bar are left as blank lines. Student + key saves both copies.",
    },
];

const CIRCLE_OF_FIFTHS: &[Faq] = &[
    Faq {
        q: "How is the circle read?",
        a: "Each step clockwise goes up a fifth and adds a sharp, from C at the top; each step anticlockwise adds a flat. The outer ring has the major keys, the middle ring their relative minors and the inner ring the key signatures.",
    },
    Faq {
        q: "What does choosing a key do?",
        a: "It shades that key, the key a fifth above (V) and the key a fifth below (IV), and its relative minor (vi), and writes them out in the middle.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers and pick in Leave blank whether the key names, the key signatures or both are left empty. Student + key saves both copies.",
    },
    Faq {
        q: "Why are some sectors split?",
        a: "Six sharps and six flats name the same key, so F# major and Gb major share a sector.",
    },
];

const PIANO_KEYBOARD: &[Faq] = &[
    Faq {
        q: "What can be marked?",
        a: "The notes of a major or natural minor scale, a major or minor triad or a dominant 7th chord from any root, or any keys you list such as C4 E4 G4.",
    },
    Faq {
        q: "What are the two worksheets?",
        a: "Name the keys marks the keys and leaves their names as blank lines. Mark the keys names the scale or chord and leaves the keyboard empty.",
    },
    Faq {
        q: "Why are notes spelled with flats or sharps?",
        a: "By scale degree: the third of D major is F#, and the seventh of C7 is Bb, as it would be written on a staff.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. Whichever half is the answer is left out. Student + key saves both copies.",
    },
];

const GUITAR_CHORDS: &[Faq] = &[
    Faq {
        q: "How do I write a chord?",
        a: "A name then six characters, low E string first: C x32010 means: low E not played, A string at fret 3, D at fret 2, G open, B at fret 1, high E open. x is not played, 0 is open and 1 to 9 is the fret.",
    },
    Faq {
        q: "What are the two worksheets?",
        a: "Name the chord shows the dots and leaves the name as a blank line. Mark the chord shows the name and leaves the box empty for students to fill.",
    },
    Faq {
        q: "What if the chord is high up the neck?",
        a: "The box starts at the lowest fret used and is labelled, such as 7fr. You can also set the first fret yourself.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. Whichever half is the answer is left out. Student + key saves both copies.",
    },
];

const RHYTHM_COUNTING: &[Faq] = &[
    Faq {
        q: "How do I write a rhythm?",
        a: "Lengths separated by spaces: w h q e s, with a dot for longer and r before one for a rest, such as q. e r q. A | ends a bar.",
    },
    Faq {
        q: "How is the count worked out?",
        a: "A beat is the note the bottom number names, so in 6/8 an eighth note is one beat. Notes on the beat are counted 1 2 3, halves of a beat are &, and quarters of a beat are e and a.",
    },
    Faq {
        q: "What if a bar does not add up?",
        a: "The key shows how many beats it has and how many it needs. A bar also ends at the | you write, so you can set a trap on purpose.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn off Show answers. The notes and the grid stay and the counts and the beats in each bar are left as blank lines. Student + key saves both copies.",
    },
];

const BREAK_EVEN: &[Faq] = &[
    Faq {
        q: "What does the chart show?",
        a: "Revenue and total cost against the units sold. Where the two lines cross is the break-even point: below it the business makes a loss, above it a profit.",
    },
    Faq {
        q: "How is the break-even point worked out?",
        a: "Fixed costs divided by the contribution per unit, which is the price minus the variable cost. Whole units are needed, so a fractional answer is rounded up.",
    },
    Faq {
        q: "What if it never breaks even?",
        a: "If the price is not above the variable cost per unit, each sale loses money, so the figure says so instead of drawing lines.",
    },
    Faq {
        q: "What does expected sales add?",
        a: "The margin of safety (how far sales can fall before a loss, in units and as a share of expected sales) and the profit at that level.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off for a copy with empty axes, a blank table and blank lines, and keep it on for the key. Both are the same size.",
    },
];

const TIME_VALUE: &[Faq] = &[
    Faq {
        q: "How do I enter cash flows?",
        a: "One row per flow: the period, then the amount. Period 0 is now, and money paid out is negative, such as 0, -1000.",
    },
    Faq {
        q: "How is a present value worked out?",
        a: "Each flow is multiplied by the discount factor 1 divided by (1 + rate) to the power of the period. The values added up are the net present value.",
    },
    Faq {
        q: "What does Future value do?",
        a: "It compounds each flow forward to the last period using (1 + rate) to the power of the periods remaining, and adds them up.",
    },
    Faq {
        q: "What does a positive NPV mean?",
        a: "The flows are worth more than the money put in at that rate, so the project earns more than the rate asked for.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off to leave the factors, values and total empty. The given cash flows and the timeline stay.",
    },
];

const LOAN_AMORTISATION: &[Faq] = &[
    Faq {
        q: "How is the repayment worked out?",
        a: "The same payment each period clears the loan: amount times the periodic rate, divided by 1 minus (1 + rate) to the power of minus the number of periods. It is rounded to the cent.",
    },
    Faq {
        q: "What are interest and principal?",
        a: "Interest is the balance times the periodic rate. The rest of the repayment is principal, which lowers the balance.",
    },
    Faq {
        q: "Why does the last payment differ a little?",
        a: "Rounding to the cent each period leaves a few cents over, so the last payment is adjusted to bring the balance to exactly zero.",
    },
    Faq {
        q: "How many rows are shown?",
        a: "Up to the number you choose, with a note of how many repayments are left out. Long loans show only the first rows.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off to leave the payment, the totals and every cell of the table empty. The opening balance stays.",
    },
];

const ORG_CHART: &[Faq] = &[
    Faq {
        q: "How do I write the roles?",
        a: "One role per line, indented under the role it reports to. Use spaces or tabs, and a dash at the start of a line is fine.",
    },
    Faq {
        q: "What are the limits?",
        a: "One role at the top, up to 30 roles, 6 levels, and 10 roles at the ends of branches so the chart stays readable.",
    },
    Faq {
        q: "What is span of control?",
        a: "The number of people reporting directly to one manager. The figure finds the role with the most.",
    },
    Faq {
        q: "What are levels?",
        a: "The rows of the hierarchy from the top role down. A chart with a director, managers and staff has three.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off to leave the questions blank, and choose Role titles to empty the boxes too, so students can write the roles in.",
    },
];

const SWOT: &[Faq] = &[
    Faq {
        q: "What do the four boxes mean?",
        a: "Strengths and weaknesses are inside the business; opportunities and threats come from outside it. Strengths and opportunities help, weaknesses and threats harm.",
    },
    Faq {
        q: "How do I enter the points?",
        a: "One point per line in each box, up to 6 each. Long points are shortened to fit.",
    },
    Faq {
        q: "Why are the points numbered in a different order?",
        a: "The list below the grid is mixed up on purpose, so a student has to decide which box each point belongs in.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off for a copy with empty boxes and the mixed-up list. The key places each numbered point in its box.",
    },
];

const MARKETING_FUNNEL: &[Faq] = &[
    Faq {
        q: "How do I enter the stages?",
        a: "One stage per line: a name then a count, such as Visited the website 4000. Give 2 to 7 stages, the widest first.",
    },
    Faq {
        q: "What is conversion?",
        a: "The share of one stage that reached the next. 800 of 4000 is 20%.",
    },
    Faq {
        q: "What is overall conversion?",
        a: "The last stage as a share of the first, so 120 of 20000 is 0.6%.",
    },
    Faq {
        q: "What does advertising spend add?",
        a: "The cost per customer, the spend divided by the count at the last stage.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off to leave the conversion rates, the overall rate and the cost per customer blank.",
    },
];

const PLOT_ARC: &[Faq] = &[
    Faq {
        q: "What are the five stages?",
        a: "Exposition sets the scene, rising action builds the problem, the climax is the turning point, falling action follows its results, and the resolution ties things up.",
    },
    Faq {
        q: "How do I fill it in?",
        a: "One line per stage in story order. Leave a line empty to skip a stage, and its box stays blank.",
    },
    Faq {
        q: "Why is the climax the highest point?",
        a: "It is where the tension peaks. The arc rises to it and falls away after it.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off for a copy with the events in their boxes and blank lines where the stage names go. The key names every stage.",
    },
];

const ORGANISER: &[Faq] = &[
    Faq {
        q: "Which organisers are there?",
        a: "A T-chart (two columns), a KWL chart (Know, Want to know, Learned) and a Venn diagram (two circles that overlap).",
    },
    Faq {
        q: "Where does each item go?",
        a: "The key places every numbered item in its box. In the Venn diagram the third list goes in the overlap, the part both circles share.",
    },
    Faq {
        q: "Why are the items numbered in a mixed order?",
        a: "The list under the organiser is mixed up on purpose, so students must decide where each item belongs.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off for empty columns or circles with the mixed-up list. Each box holds up to 8 items.",
    },
];

const TIMELINE: &[Faq] = &[
    Faq {
        q: "How do I enter events?",
        a: "One per line: a number then the event, such as 1066 Battle of Hastings. Use a negative number for BC dates. Events can be in any order.",
    },
    Faq {
        q: "Is it drawn to scale?",
        a: "Yes. The gap between two events on the line follows the gap between their dates, and events too close together are lifted to different levels.",
    },
    Faq {
        q: "What are the readouts?",
        a: "The time from the first event to the last, and the longest gap between neighbouring events.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off and choose to hide the dates or the event names, and the readouts are left blank too.",
    },
];

const CAUSE_EFFECT: &[Faq] = &[
    Faq {
        q: "What does the web show?",
        a: "Causes arrow into the central event, and the event arrows out to its effects.",
    },
    Faq {
        q: "How do I enter them?",
        a: "One cause or effect per line, up to 5 of each. Arrows show which side an item belongs on.",
    },
    Faq {
        q: "Why are the events numbered in a mixed order?",
        a: "The list below is mixed up, so students sort each event into the cause or effect boxes.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off for empty boxes and the mixed-up list. The key writes each numbered event in its box.",
    },
];

const FAMILY_TREE: &[Faq] = &[
    Faq {
        q: "How do I write a family?",
        a: "One person per line, or a couple joined by a plus, such as Anna + Ben. Indent their children underneath.",
    },
    Faq {
        q: "What is counted?",
        a: "Generations (rows), people (every name) and descendants of the first couple (everyone in a box below them, counting a child and their partner as one line).",
    },
    Faq {
        q: "What are the limits?",
        a: "One person or couple at the top, up to 24 lines, 5 generations and 8 ends of branches.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off to leave the counts blank, and choose Names to empty the boxes so students can write the family in.",
    },
];

const SENTENCE_DIAGRAM: &[Faq] = &[
    Faq {
        q: "How is a sentence diagram drawn?",
        a: "The subject, verb and object sit on a base line, split by upright lines. Words that describe them hang below on slanted lines.",
    },
    Faq {
        q: "How do I enter the parts?",
        a: "Type the sentence, then the subject, the verb and the object, and the describing words for each, one per line, up to 3.",
    },
    Faq {
        q: "Why does the object have a short divider?",
        a: "A line that stops at the base line separates the verb from its object. The line between subject and verb goes right through.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off to show the sentence and an empty frame of the same shape, with no words in it.",
    },
];

const STRUCTURE_CHART: &[Faq] = &[
    Faq {
        q: "How do I enter the links?",
        a: "One per line: Box A -> Box B : what the arrow means. The text after the colon is optional. Boxes with the same name are one box.",
    },
    Faq {
        q: "What are the limits?",
        a: "Up to 10 boxes and 14 links. Arrows that go both ways curve apart so you can read each one.",
    },
    Faq {
        q: "How are the boxes arranged?",
        a: "Boxes are in rows following the arrows, so what comes first is at the top. An arrow that would loop back is still drawn.",
    },
    Faq {
        q: "How do I make a worksheet?",
        a: "Turn Show answers off to keep the boxes and arrows and leave the label boxes blank for students to write.",
    },
];

const FOOD_PLATE: &[Faq] = &[
    Faq {
        q: "How do I list my foods?",
        a: "One per line as name: group, for example Apple: Fruits. The groups are Vegetables, Grains, Protein, Fruits and Dairy, up to 14 foods.",
    },
    Faq {
        q: "What do the shares control?",
        a: "The size of each slice of the plate. They are relative, so 2, 2, 1, 1 gives the same plate as 40, 40, 20, 20.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, the table shows the food numbers in the right group. Off, the table is empty and the foods are listed mixed up, for students to sort.",
    },
    Faq {
        q: "Is the dairy circle optional?",
        a: "Yes. Turn Dairy off to draw the plate without the side circle.",
    },
];

const HEART_ZONES: &[Faq] = &[
    Faq {
        q: "How is the maximum heart rate found?",
        a: "As 220 minus age. It is an estimate; real maximums vary from person to person.",
    },
    Faq {
        q: "What does the resting rate do?",
        a: "At 0 the zones are plain percentages of the maximum. Give a resting rate and they use the heart rate reserve method, which suits fit people better.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, each zone shows its beats-per-minute range. Off, those ranges are blank for students to work out.",
    },
    Faq {
        q: "Why do I get a message about the resting rate?",
        a: "The resting rate must be below the maximum for the age given.",
    },
];

const COURT_LAYOUT: &[Faq] = &[
    Faq {
        q: "Which sports are there?",
        a: "Basketball, soccer and volleyball. Each shows its court or field markings and numbered positions.",
    },
    Faq {
        q: "What is the formation for?",
        a: "It only applies to soccer: 4-4-2, 4-3-3 or 3-5-2 move the players into those lines.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, the list names each numbered position. Off, the numbers stay on the court and the names are blank lines.",
    },
    Faq {
        q: "Is the court to scale?",
        a: "It is a simplified diagram with the main markings, not an exact scale drawing.",
    },
];

const BODY_MAP: &[Faq] = &[
    Faq {
        q: "What can I show?",
        a: "All organs, only the digestive system, or only breathing and circulation.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, the numbered organs are named in the list. Off, the list is blank lines for students to fill in.",
    },
    Faq {
        q: "Is the outline accurate?",
        a: "It is simplified so every organ can be seen. Sizes and positions are approximate.",
    },
    Faq {
        q: "Can I add organs?",
        a: "No, the set is fixed. Choose a view to show the ones you want.",
    },
];

const BLOCK_DIAGRAM: &[Faq] = &[
    Faq {
        q: "How do I enter blocks and signals?",
        a: "Blocks go one per line, left to right, up to 6. Signals go one per line: the input first, then one for each arrow after a block.",
    },
    Faq {
        q: "How many signals can I give?",
        a: "At most one more than the number of blocks. Missing ones are left blank.",
    },
    Faq {
        q: "How do I add feedback?",
        a: "Type a feedback signal. It is drawn from the output back to a summing junction at the start. Leave it empty for an open loop.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "Off, the block names and signal labels are blank for students. The description stays.",
    },
];

const FREE_BODY: &[Faq] = &[
    Faq {
        q: "How do I enter forces?",
        a: "One per line as name, size in newtons, direction: Weight 49 270. Up to 6 forces.",
    },
    Faq {
        q: "How are directions measured?",
        a: "In degrees anticlockwise from pointing right: 0 right, 90 up, 180 left, 270 down.",
    },
    Faq {
        q: "When is the acceleration shown?",
        a: "When you give a mass above 0. It is the net force divided by the mass.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, the arrows, net force and acceleration are drawn and worked out. Off, only the object and the list remain.",
    },
];

const GEAR_TRAIN: &[Faq] = &[
    Faq {
        q: "How do I enter the gears?",
        a: "List the tooth counts in order, from the gear that is turned to the last one, 2 to 4 gears with 8 to 60 teeth each.",
    },
    Faq {
        q: "How is the speed worked out?",
        a: "Speed times the first gear's teeth divided by the gear's own teeth. Meshing gears turn in opposite directions.",
    },
    Faq {
        q: "What is an idler gear?",
        a: "A gear between two others. It reverses nothing overall but changes the direction of the last gear.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "Off, the ratio, speeds and directions are blank lines for students. The first gear's speed and turn are given.",
    },
];

const ORTHOGRAPHIC: &[Faq] = &[
    Faq {
        q: "How do I describe the stack?",
        a: "A grid of numbers, one row per line, the back row first. Each number is how many cubes are stacked there, 0 to 5, up to 5 by 5.",
    },
    Faq {
        q: "What are the three views?",
        a: "Top, front and right: what you would see looking straight at the stack from each side.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, the views are filled in. Off, empty grids of the right size are left for students to draw in.",
    },
    Faq {
        q: "Why are some rows shorter than others?",
        a: "Short rows are padded with zeros, so a missing number means no cubes.",
    },
];

const COLOUR_WHEEL: &[Faq] = &[
    Faq {
        q: "Which colour wheel is this?",
        a: "The artist's red-yellow-blue wheel: yellow, red and blue are primary, orange, violet and green are secondary, and the six between are tertiary.",
    },
    Faq {
        q: "How do I mark a colour scheme?",
        a: "Choose a scheme and a starting colour. Complementary marks two opposite colours, triadic three evenly spaced, analogous three neighbours, split complementary one and the two beside its opposite.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, the colours are named and the scheme is drawn. Off, the wheel is left with blank lines to fill in.",
    },
    Faq {
        q: "Why does the Print style look different?",
        a: "It uses greys, so colours become lightness steps and the names matter more than the shades.",
    },
];

const PERSPECTIVE_GRID: &[Faq] = &[
    Faq {
        q: "What is the difference between one-point and two-point?",
        a: "One-point has a single vanishing point and a box facing you. Two-point has two, and the box is seen corner-on.",
    },
    Faq {
        q: "What does the horizon height do?",
        a: "It is the eye level. A box above it is seen from below, a box below it from above.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, a box is drawn on the guides. Off, only the horizon, the vanishing points and the guide lines are shown, for students to draw on.",
    },
    Faq {
        q: "Can I move and resize the box?",
        a: "Yes: size, position across, position down from the horizon and depth. Only the faces you could really see are drawn, so a box that straddles the horizon shows no top or bottom.",
    },
];

const MASLOW: &[Faq] = &[
    Faq {
        q: "How do I enter the levels?",
        a: "One per line from the bottom up as name: example. The example is optional. Two to seven levels.",
    },
    Faq {
        q: "Can I use my own pyramid?",
        a: "Yes. Any list of levels works, such as stages of learning or of a food chain.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, each band is named. Off, the bands are empty and the examples beside them remain, for students to match.",
    },
    Faq {
        q: "Where do names go when a band is too narrow?",
        a: "Beside the pyramid, on the left, with a short line to the band. The bands near the top are narrow.",
    },
];

const BRAIN_REGIONS: &[Faq] = &[
    Faq {
        q: "Is this an anatomical drawing?",
        a: "It is an original side-view drawing, simplified: the outline, the lateral fissure, the central sulcus and the parieto-occipital line are drawn where they sit, front on the left, but folds and fine detail are left out.",
    },
    Faq {
        q: "What can I show?",
        a: "The four lobes with the cerebellum and brain stem, or the four lobes only.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, the numbered regions are named. Off, the names are blank and what each region does is listed to match.",
    },
    Faq {
        q: "Which regions are named?",
        a: "Frontal, parietal, occipital and temporal lobes, the cerebellum and the brain stem.",
    },
];

const EXPERIMENT_FLOW: &[Faq] = &[
    Faq {
        q: "What does the diagram show?",
        a: "A controlled experiment: a sample is randomly split into treatment groups and a control group, all are measured and the results compared.",
    },
    Faq {
        q: "What goes in the boxes?",
        a: "Your sample, what each group gets (2 to 4 groups, the control group last) and what is measured. Your text fills the boxes; the roles are the answers.",
    },
    Faq {
        q: "What does Show answers do?",
        a: "On, the roles are labelled: random assignment, treatment and control group, independent and dependent variable. Off, those are blank lines.",
    },
    Faq {
        q: "Why a control group?",
        a: "It shows what happens without the treatment, so any difference can be put down to the treatment.",
    },
];
