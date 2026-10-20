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
