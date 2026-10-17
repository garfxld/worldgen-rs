use super::climate::{Parameter, ParameterPoint, quantize_coord};

type Biome = &'static str;

pub struct OverworldBiomeBuilder {
    full_range: Parameter,
    temperatures: [Parameter; 5],
    humidities: [Parameter; 5],
    erosions: [Parameter; 7],
    frozen_range: Parameter,
    unfrozen_range: Parameter,
    mushroom_fields_continentalness: Parameter,
    deep_ocean_continentalness: Parameter,
    ocean_continentalness: Parameter,
    coast_continentalness: Parameter,
    inland_continentalness: Parameter,
    near_inland_continentalness: Parameter,
    mid_inland_continentalness: Parameter,
    far_inland_continentalness: Parameter,
    oceans: [[Biome; 5]; 2],
    middle_biomes: [[Biome; 5]; 5],
    middle_biomes_variant: [[Option<Biome>; 5]; 5],
    plateau_biomes: [[Biome; 5]; 5],
    plateau_biomes_variant: [[Option<Biome>; 5]; 5],
    shattered_biomes: [[Option<Biome>; 5]; 5],
}

impl Default for OverworldBiomeBuilder {
    fn default() -> Self {
        let temperatures = [
            Parameter::span(-1.0, -0.45),
            Parameter::span(-0.45, -0.15),
            Parameter::span(-0.15, 0.2),
            Parameter::span(0.2, 0.55),
            Parameter::span(0.55, 1.0),
        ];
        let humidities = [
            Parameter::span(-1.0, -0.35),
            Parameter::span(-0.35, -0.1),
            Parameter::span(-0.1, 0.1),
            Parameter::span(0.1, 0.3),
            Parameter::span(0.3, 1.0),
        ];
        let frozen_range = temperatures[0];
        let unfrozen_range = Parameter::span_params(temperatures[1], temperatures[4]);

        Self {
            full_range: Parameter::span(-1.0, 1.0),
            temperatures,
            humidities,
            erosions: [
                Parameter::span(-1.0, -0.78),
                Parameter::span(-0.78, -0.375),
                Parameter::span(-0.375, -0.2225),
                Parameter::span(-0.2225, 0.05),
                Parameter::span(0.05, 0.45),
                Parameter::span(0.45, 0.55),
                Parameter::span(0.55, 1.0),
            ],
            frozen_range,
            unfrozen_range,
            mushroom_fields_continentalness: Parameter::span(-1.2, -1.05),
            deep_ocean_continentalness: Parameter::span(-1.05, -0.455),
            ocean_continentalness: Parameter::span(-0.455, -0.19),
            coast_continentalness: Parameter::span(-0.19, -0.11),
            inland_continentalness: Parameter::span(-0.11, 0.55),
            near_inland_continentalness: Parameter::span(-0.11, 0.03),
            mid_inland_continentalness: Parameter::span(0.03, 0.3),
            far_inland_continentalness: Parameter::span(0.3, 1.0),
            oceans: [
                [
                    "minecraft:deep_frozen_ocean",
                    "minecraft:deep_cold_ocean",
                    "minecraft:deep_ocean",
                    "minecraft:deep_lukewarm_ocean",
                    "minecraft:warm_ocean",
                ],
                [
                    "minecraft:frozen_ocean",
                    "minecraft:cold_ocean",
                    "minecraft:ocean",
                    "minecraft:lukewarm_ocean",
                    "minecraft:warm_ocean",
                ],
            ],
            middle_biomes: [
                [
                    "minecraft:snowy_plains",
                    "minecraft:snowy_plains",
                    "minecraft:snowy_plains",
                    "minecraft:snowy_taiga",
                    "minecraft:taiga",
                ],
                [
                    "minecraft:plains",
                    "minecraft:plains",
                    "minecraft:forest",
                    "minecraft:taiga",
                    "minecraft:old_growth_spruce_taiga",
                ],
                [
                    "minecraft:flower_forest",
                    "minecraft:plains",
                    "minecraft:forest",
                    "minecraft:birch_forest",
                    "minecraft:dark_forest",
                ],
                [
                    "minecraft:savanna",
                    "minecraft:savanna",
                    "minecraft:forest",
                    "minecraft:jungle",
                    "minecraft:jungle",
                ],
                [
                    "minecraft:desert",
                    "minecraft:desert",
                    "minecraft:desert",
                    "minecraft:desert",
                    "minecraft:desert",
                ],
            ],
            middle_biomes_variant: [
                [
                    Some("minecraft:ice_spikes"),
                    None,
                    Some("minecraft:snowy_taiga"),
                    None,
                    None,
                ],
                [
                    Some("minecraft:dappled_forest"),
                    None,
                    None,
                    None,
                    Some("minecraft:old_growth_pine_taiga"),
                ],
                [
                    Some("minecraft:sunflower_plains"),
                    None,
                    None,
                    Some("minecraft:old_growth_birch_forest"),
                    None,
                ],
                [
                    None,
                    None,
                    Some("minecraft:plains"),
                    Some("minecraft:sparse_jungle"),
                    Some("minecraft:bamboo_jungle"),
                ],
                [None, None, None, None, None],
            ],
            plateau_biomes: [
                [
                    "minecraft:snowy_plains",
                    "minecraft:snowy_plains",
                    "minecraft:snowy_plains",
                    "minecraft:snowy_taiga",
                    "minecraft:snowy_taiga",
                ],
                [
                    "minecraft:meadow",
                    "minecraft:meadow",
                    "minecraft:forest",
                    "minecraft:taiga",
                    "minecraft:old_growth_spruce_taiga",
                ],
                [
                    "minecraft:meadow",
                    "minecraft:meadow",
                    "minecraft:meadow",
                    "minecraft:meadow",
                    "minecraft:pale_garden",
                ],
                [
                    "minecraft:savanna_plateau",
                    "minecraft:savanna_plateau",
                    "minecraft:forest",
                    "minecraft:forest",
                    "minecraft:jungle",
                ],
                [
                    "minecraft:badlands",
                    "minecraft:badlands",
                    "minecraft:badlands",
                    "minecraft:wooded_badlands",
                    "minecraft:wooded_badlands",
                ],
            ],
            plateau_biomes_variant: [
                [Some("minecraft:ice_spikes"), None, None, None, None],
                [
                    Some("minecraft:cherry_grove"),
                    None,
                    Some("minecraft:meadow"),
                    Some("minecraft:meadow"),
                    Some("minecraft:old_growth_pine_taiga"),
                ],
                [
                    Some("minecraft:cherry_grove"),
                    Some("minecraft:cherry_grove"),
                    Some("minecraft:forest"),
                    Some("minecraft:birch_forest"),
                    None,
                ],
                [None, None, None, None, None],
                [
                    Some("minecraft:eroded_badlands"),
                    Some("minecraft:eroded_badlands"),
                    None,
                    None,
                    None,
                ],
            ],
            shattered_biomes: [
                [
                    Some("minecraft:windswept_gravelly_hills"),
                    Some("minecraft:windswept_gravelly_hills"),
                    Some("minecraft:windswept_hills"),
                    Some("minecraft:windswept_forest"),
                    Some("minecraft:windswept_forest"),
                ],
                [
                    Some("minecraft:windswept_gravelly_hills"),
                    Some("minecraft:windswept_gravelly_hills"),
                    Some("minecraft:windswept_hills"),
                    Some("minecraft:windswept_forest"),
                    Some("minecraft:windswept_forest"),
                ],
                [
                    Some("minecraft:windswept_hills"),
                    Some("minecraft:windswept_hills"),
                    Some("minecraft:windswept_hills"),
                    Some("minecraft:windswept_forest"),
                    Some("minecraft:windswept_forest"),
                ],
                [None, None, None, None, None],
                [None, None, None, None, None],
            ],
        }
    }
}

impl OverworldBiomeBuilder {
    pub fn add_biomes(&self, out: &mut Vec<(ParameterPoint, Biome)>) {
        self.add_off_coast_biomes(out);
        self.add_inland_biomes(out);
        self.add_underground_biomes(out);
    }

    fn add_off_coast_biomes(&self, out: &mut Vec<(ParameterPoint, Biome)>) {
        self.add_surface_biome(
            out,
            self.full_range,
            self.full_range,
            self.mushroom_fields_continentalness,
            self.full_range,
            self.full_range,
            0.0,
            "minecraft:mushroom_fields",
        );

        for t in 0..self.temperatures.len() {
            let temperature = self.temperatures[t];
            self.add_surface_biome(
                out,
                temperature,
                self.full_range,
                self.deep_ocean_continentalness,
                self.full_range,
                self.full_range,
                0.0,
                self.oceans[0][t],
            );
            self.add_surface_biome(
                out,
                temperature,
                self.full_range,
                self.ocean_continentalness,
                self.full_range,
                self.full_range,
                0.0,
                self.oceans[1][t],
            );
        }
    }

    fn add_inland_biomes(&self, out: &mut Vec<(ParameterPoint, Biome)>) {
        self.add_mid_slice(out, Parameter::span(-1.0, -0.93333334));
        self.add_high_slice(out, Parameter::span(-0.93333334, -0.7666667));
        self.add_peaks(out, Parameter::span(-0.7666667, -0.56666666));
        self.add_high_slice(out, Parameter::span(-0.56666666, -0.4));
        self.add_mid_slice(out, Parameter::span(-0.4, -0.26666668));
        self.add_low_slice(out, Parameter::span(-0.26666668, -0.05));
        self.add_valleys(out, Parameter::span(-0.05, 0.05));
        self.add_low_slice(out, Parameter::span(0.05, 0.26666668));
        self.add_mid_slice(out, Parameter::span(0.26666668, 0.4));
        self.add_high_slice(out, Parameter::span(0.4, 0.56666666));
        self.add_peaks(out, Parameter::span(0.56666666, 0.7666667));
        self.add_high_slice(out, Parameter::span(0.7666667, 0.93333334));
        self.add_mid_slice(out, Parameter::span(0.93333334, 1.0));
    }

    fn add_peaks(&self, out: &mut Vec<(ParameterPoint, Biome)>, weirdness: Parameter) {
        let e = &self.erosions;
        for t in 0..self.temperatures.len() {
            let temperature = self.temperatures[t];
            for h in 0..self.humidities.len() {
                let humidity = self.humidities[h];
                let middle_biome = self.pick_middle_biome(t, h, weirdness);
                let middle_biome_or_badlands_if_hot =
                    self.pick_middle_biome_or_badlands_if_hot(t, h, weirdness);
                let middle_biome_or_badlands_if_hot_or_slope_if_cold =
                    self.pick_middle_biome_or_badlands_if_hot_or_slope_if_cold(t, h, weirdness);
                let plateau_biome = self.pick_plateau_biome(t, h, weirdness);
                let shattered_biome = self.pick_shattered_biome(t, h, weirdness);
                let shattered_biome_or_windswept_savanna =
                    self.maybe_pick_windswept_savanna_biome(t, h, weirdness, shattered_biome);
                let peak_biome = self.pick_peak_biome(t, h, weirdness);

                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[0],
                    weirdness,
                    0.0,
                    peak_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    e[1],
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot_or_slope_if_cold,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[1],
                    weirdness,
                    0.0,
                    peak_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    Parameter::span_params(e[2], e[3]),
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[2],
                    weirdness,
                    0.0,
                    plateau_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.mid_inland_continentalness,
                    e[3],
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.far_inland_continentalness,
                    e[3],
                    weirdness,
                    0.0,
                    plateau_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[4],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    e[5],
                    weirdness,
                    0.0,
                    shattered_biome_or_windswept_savanna,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[5],
                    weirdness,
                    0.0,
                    shattered_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[6],
                    weirdness,
                    0.0,
                    middle_biome,
                );
            }
        }
    }

    fn add_high_slice(&self, out: &mut Vec<(ParameterPoint, Biome)>, weirdness: Parameter) {
        let e = &self.erosions;
        for t in 0..self.temperatures.len() {
            let temperature = self.temperatures[t];
            for h in 0..self.humidities.len() {
                let humidity = self.humidities[h];
                let middle_biome = self.pick_middle_biome(t, h, weirdness);
                let middle_biome_or_badlands_if_hot =
                    self.pick_middle_biome_or_badlands_if_hot(t, h, weirdness);
                let middle_biome_or_badlands_if_hot_or_slope_if_cold =
                    self.pick_middle_biome_or_badlands_if_hot_or_slope_if_cold(t, h, weirdness);
                let plateau_biome = self.pick_plateau_biome(t, h, weirdness);
                let shattered_biome = self.pick_shattered_biome(t, h, weirdness);
                let middle_biome_or_windswept_savanna =
                    self.maybe_pick_windswept_savanna_biome(t, h, weirdness, middle_biome);
                let slope_biome = self.pick_slope_biome(t, h, weirdness);
                let peak_biome = self.pick_peak_biome(t, h, weirdness);

                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    Parameter::span_params(e[0], e[1]),
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    e[0],
                    weirdness,
                    0.0,
                    slope_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[0],
                    weirdness,
                    0.0,
                    peak_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    e[1],
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot_or_slope_if_cold,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[1],
                    weirdness,
                    0.0,
                    slope_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    Parameter::span_params(e[2], e[3]),
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[2],
                    weirdness,
                    0.0,
                    plateau_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.mid_inland_continentalness,
                    e[3],
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.far_inland_continentalness,
                    e[3],
                    weirdness,
                    0.0,
                    plateau_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[4],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    e[5],
                    weirdness,
                    0.0,
                    middle_biome_or_windswept_savanna,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[5],
                    weirdness,
                    0.0,
                    shattered_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[6],
                    weirdness,
                    0.0,
                    middle_biome,
                );
            }
        }
    }

    fn add_mid_slice(&self, out: &mut Vec<(ParameterPoint, Biome)>, weirdness: Parameter) {
        let e = &self.erosions;
        self.add_surface_biome(
            out,
            self.full_range,
            self.full_range,
            self.coast_continentalness,
            Parameter::span_params(e[0], e[2]),
            weirdness,
            0.0,
            "minecraft:stony_shore",
        );
        self.add_surface_biome(
            out,
            Parameter::span_params(self.temperatures[1], self.temperatures[2]),
            self.full_range,
            Parameter::span_params(
                self.near_inland_continentalness,
                self.far_inland_continentalness,
            ),
            e[6],
            weirdness,
            0.0,
            "minecraft:swamp",
        );
        self.add_surface_biome(
            out,
            Parameter::span_params(self.temperatures[3], self.temperatures[4]),
            self.full_range,
            Parameter::span_params(
                self.near_inland_continentalness,
                self.far_inland_continentalness,
            ),
            e[6],
            weirdness,
            0.0,
            "minecraft:mangrove_swamp",
        );

        for t in 0..self.temperatures.len() {
            let temperature = self.temperatures[t];
            for h in 0..self.humidities.len() {
                let humidity = self.humidities[h];
                let middle_biome = self.pick_middle_biome(t, h, weirdness);
                let middle_biome_or_badlands_if_hot =
                    self.pick_middle_biome_or_badlands_if_hot(t, h, weirdness);
                let middle_biome_or_badlands_if_hot_or_slope_if_cold =
                    self.pick_middle_biome_or_badlands_if_hot_or_slope_if_cold(t, h, weirdness);
                let shattered_biome = self.pick_shattered_biome(t, h, weirdness);
                let plateau_biome = self.pick_plateau_biome(t, h, weirdness);
                let beach_biome = self.pick_beach_biome(t, h);
                let middle_biome_or_windswept_savanna =
                    self.maybe_pick_windswept_savanna_biome(t, h, weirdness, middle_biome);
                let shattered_coast_biome = self.pick_shattered_coast_biome(t, h, weirdness);
                let slope_biome = self.pick_slope_biome(t, h, weirdness);

                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.near_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[0],
                    weirdness,
                    0.0,
                    slope_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.near_inland_continentalness,
                        self.mid_inland_continentalness,
                    ),
                    e[1],
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot_or_slope_if_cold,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.far_inland_continentalness,
                    e[1],
                    weirdness,
                    0.0,
                    if t == 0 { slope_biome } else { plateau_biome },
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    e[2],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.mid_inland_continentalness,
                    e[2],
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.far_inland_continentalness,
                    e[2],
                    weirdness,
                    0.0,
                    plateau_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.coast_continentalness,
                        self.near_inland_continentalness,
                    ),
                    e[3],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[3],
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot,
                );
                if weirdness.max < 0 {
                    self.add_surface_biome(
                        out,
                        temperature,
                        humidity,
                        self.coast_continentalness,
                        e[4],
                        weirdness,
                        0.0,
                        beach_biome,
                    );
                    self.add_surface_biome(
                        out,
                        temperature,
                        humidity,
                        Parameter::span_params(
                            self.near_inland_continentalness,
                            self.far_inland_continentalness,
                        ),
                        e[4],
                        weirdness,
                        0.0,
                        middle_biome,
                    );
                } else {
                    self.add_surface_biome(
                        out,
                        temperature,
                        humidity,
                        Parameter::span_params(
                            self.coast_continentalness,
                            self.far_inland_continentalness,
                        ),
                        e[4],
                        weirdness,
                        0.0,
                        middle_biome,
                    );
                }

                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    e[5],
                    weirdness,
                    0.0,
                    shattered_coast_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    e[5],
                    weirdness,
                    0.0,
                    middle_biome_or_windswept_savanna,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[5],
                    weirdness,
                    0.0,
                    shattered_biome,
                );
                if weirdness.max < 0 {
                    self.add_surface_biome(
                        out,
                        temperature,
                        humidity,
                        self.coast_continentalness,
                        e[6],
                        weirdness,
                        0.0,
                        beach_biome,
                    );
                } else {
                    self.add_surface_biome(
                        out,
                        temperature,
                        humidity,
                        self.coast_continentalness,
                        e[6],
                        weirdness,
                        0.0,
                        middle_biome,
                    );
                }

                if t == 0 {
                    self.add_surface_biome(
                        out,
                        temperature,
                        humidity,
                        Parameter::span_params(
                            self.near_inland_continentalness,
                            self.far_inland_continentalness,
                        ),
                        e[6],
                        weirdness,
                        0.0,
                        middle_biome,
                    );
                }
            }
        }
    }

    fn add_low_slice(&self, out: &mut Vec<(ParameterPoint, Biome)>, weirdness: Parameter) {
        let e = &self.erosions;
        self.add_surface_biome(
            out,
            self.full_range,
            self.full_range,
            self.coast_continentalness,
            Parameter::span_params(e[0], e[2]),
            weirdness,
            0.0,
            "minecraft:stony_shore",
        );
        self.add_surface_biome(
            out,
            Parameter::span_params(self.temperatures[1], self.temperatures[2]),
            self.full_range,
            Parameter::span_params(
                self.near_inland_continentalness,
                self.far_inland_continentalness,
            ),
            e[6],
            weirdness,
            0.0,
            "minecraft:swamp",
        );
        self.add_surface_biome(
            out,
            Parameter::span_params(self.temperatures[3], self.temperatures[4]),
            self.full_range,
            Parameter::span_params(
                self.near_inland_continentalness,
                self.far_inland_continentalness,
            ),
            e[6],
            weirdness,
            0.0,
            "minecraft:mangrove_swamp",
        );

        for t in 0..self.temperatures.len() {
            let temperature = self.temperatures[t];
            for h in 0..self.humidities.len() {
                let humidity = self.humidities[h];
                let middle_biome = self.pick_middle_biome(t, h, weirdness);
                let middle_biome_or_badlands_if_hot =
                    self.pick_middle_biome_or_badlands_if_hot(t, h, weirdness);
                let middle_biome_or_badlands_if_hot_or_slope_if_cold =
                    self.pick_middle_biome_or_badlands_if_hot_or_slope_if_cold(t, h, weirdness);
                let beach_biome = self.pick_beach_biome(t, h);
                let middle_biome_or_windswept_savanna =
                    self.maybe_pick_windswept_savanna_biome(t, h, weirdness, middle_biome);
                let shattered_coast_biome = self.pick_shattered_coast_biome(t, h, weirdness);

                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    Parameter::span_params(e[0], e[1]),
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    Parameter::span_params(e[0], e[1]),
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot_or_slope_if_cold,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    Parameter::span_params(e[2], e[3]),
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    Parameter::span_params(e[2], e[3]),
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    Parameter::span_params(e[3], e[4]),
                    weirdness,
                    0.0,
                    beach_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.near_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[4],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    e[5],
                    weirdness,
                    0.0,
                    shattered_coast_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.near_inland_continentalness,
                    e[5],
                    weirdness,
                    0.0,
                    middle_biome_or_windswept_savanna,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    e[5],
                    weirdness,
                    0.0,
                    middle_biome,
                );
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    self.coast_continentalness,
                    e[6],
                    weirdness,
                    0.0,
                    beach_biome,
                );
                if t == 0 {
                    self.add_surface_biome(
                        out,
                        temperature,
                        humidity,
                        Parameter::span_params(
                            self.near_inland_continentalness,
                            self.far_inland_continentalness,
                        ),
                        e[6],
                        weirdness,
                        0.0,
                        middle_biome,
                    );
                }
            }
        }
    }

    fn add_valleys(&self, out: &mut Vec<(ParameterPoint, Biome)>, weirdness: Parameter) {
        let e = &self.erosions;
        self.add_surface_biome(
            out,
            self.frozen_range,
            self.full_range,
            self.coast_continentalness,
            Parameter::span_params(e[0], e[1]),
            weirdness,
            0.0,
            if weirdness.max < 0 {
                "minecraft:stony_shore"
            } else {
                "minecraft:frozen_river"
            },
        );
        self.add_surface_biome(
            out,
            self.unfrozen_range,
            self.full_range,
            self.coast_continentalness,
            Parameter::span_params(e[0], e[1]),
            weirdness,
            0.0,
            if weirdness.max < 0 {
                "minecraft:stony_shore"
            } else {
                "minecraft:river"
            },
        );
        self.add_surface_biome(
            out,
            self.frozen_range,
            self.full_range,
            self.near_inland_continentalness,
            Parameter::span_params(e[0], e[1]),
            weirdness,
            0.0,
            "minecraft:frozen_river",
        );
        self.add_surface_biome(
            out,
            self.unfrozen_range,
            self.full_range,
            self.near_inland_continentalness,
            Parameter::span_params(e[0], e[1]),
            weirdness,
            0.0,
            "minecraft:river",
        );
        self.add_surface_biome(
            out,
            self.frozen_range,
            self.full_range,
            Parameter::span_params(self.coast_continentalness, self.far_inland_continentalness),
            Parameter::span_params(e[2], e[5]),
            weirdness,
            0.0,
            "minecraft:frozen_river",
        );
        self.add_surface_biome(
            out,
            self.unfrozen_range,
            self.full_range,
            Parameter::span_params(self.coast_continentalness, self.far_inland_continentalness),
            Parameter::span_params(e[2], e[5]),
            weirdness,
            0.0,
            "minecraft:river",
        );
        self.add_surface_biome(
            out,
            self.frozen_range,
            self.full_range,
            self.coast_continentalness,
            e[6],
            weirdness,
            0.0,
            "minecraft:frozen_river",
        );
        self.add_surface_biome(
            out,
            self.unfrozen_range,
            self.full_range,
            self.coast_continentalness,
            e[6],
            weirdness,
            0.0,
            "minecraft:river",
        );
        self.add_surface_biome(
            out,
            Parameter::span_params(self.temperatures[1], self.temperatures[2]),
            self.full_range,
            Parameter::span_params(self.inland_continentalness, self.far_inland_continentalness),
            e[6],
            weirdness,
            0.0,
            "minecraft:swamp",
        );
        self.add_surface_biome(
            out,
            Parameter::span_params(self.temperatures[3], self.temperatures[4]),
            self.full_range,
            Parameter::span_params(self.inland_continentalness, self.far_inland_continentalness),
            e[6],
            weirdness,
            0.0,
            "minecraft:mangrove_swamp",
        );
        self.add_surface_biome(
            out,
            self.frozen_range,
            self.full_range,
            Parameter::span_params(self.inland_continentalness, self.far_inland_continentalness),
            e[6],
            weirdness,
            0.0,
            "minecraft:frozen_river",
        );

        for t in 0..self.temperatures.len() {
            let temperature = self.temperatures[t];
            for h in 0..self.humidities.len() {
                let humidity = self.humidities[h];
                let middle_biome_or_badlands_if_hot =
                    self.pick_middle_biome_or_badlands_if_hot(t, h, weirdness);
                self.add_surface_biome(
                    out,
                    temperature,
                    humidity,
                    Parameter::span_params(
                        self.mid_inland_continentalness,
                        self.far_inland_continentalness,
                    ),
                    Parameter::span_params(e[0], e[1]),
                    weirdness,
                    0.0,
                    middle_biome_or_badlands_if_hot,
                );
            }
        }
    }

    fn add_underground_biomes(&self, out: &mut Vec<(ParameterPoint, Biome)>) {
        self.add_underground_biome(
            out,
            self.full_range,
            self.full_range,
            Parameter::span(0.8, 1.0),
            self.full_range,
            self.full_range,
            0.0,
            "minecraft:dripstone_caves",
        );
        self.add_underground_biome(
            out,
            self.full_range,
            Parameter::span(0.7, 1.0),
            self.full_range,
            self.full_range,
            self.full_range,
            0.0,
            "minecraft:lush_caves",
        );
        self.add_underground_biome(
            out,
            self.full_range,
            self.full_range,
            Parameter::span_params(self.coast_continentalness, self.inland_continentalness),
            Parameter::span_params(self.erosions[5], self.erosions[6]),
            Parameter::span(-1.1, -0.85),
            0.0,
            "minecraft:sulfur_caves",
        );
        self.add_bottom_biome(
            out,
            self.full_range,
            self.full_range,
            self.full_range,
            Parameter::span_params(self.erosions[0], self.erosions[1]),
            self.full_range,
            0.0,
            "minecraft:deep_dark",
        );
    }

    fn pick_middle_biome(&self, t: usize, h: usize, weirdness: Parameter) -> Biome {
        if weirdness.max < 0 {
            return self.middle_biomes[t][h];
        }
        self.middle_biomes_variant[t][h].unwrap_or(self.middle_biomes[t][h])
    }

    fn pick_middle_biome_or_badlands_if_hot(
        &self,
        t: usize,
        h: usize,
        weirdness: Parameter,
    ) -> Biome {
        if t == 4 {
            self.pick_badlands_biome(h, weirdness)
        } else {
            self.pick_middle_biome(t, h, weirdness)
        }
    }

    fn pick_middle_biome_or_badlands_if_hot_or_slope_if_cold(
        &self,
        t: usize,
        h: usize,
        weirdness: Parameter,
    ) -> Biome {
        if t == 0 {
            self.pick_slope_biome(t, h, weirdness)
        } else {
            self.pick_middle_biome_or_badlands_if_hot(t, h, weirdness)
        }
    }

    const fn maybe_pick_windswept_savanna_biome(
        &self,
        t: usize,
        h: usize,
        weirdness: Parameter,
        underlying_biome: Biome,
    ) -> Biome {
        if t > 1 && h < 4 && weirdness.max >= 0 {
            "minecraft:windswept_savanna"
        } else {
            underlying_biome
        }
    }

    fn pick_shattered_coast_biome(&self, t: usize, h: usize, weirdness: Parameter) -> Biome {
        let beach_or_middle_biome = if weirdness.max >= 0 {
            self.pick_middle_biome(t, h, weirdness)
        } else {
            self.pick_beach_biome(t, h)
        };
        self.maybe_pick_windswept_savanna_biome(t, h, weirdness, beach_or_middle_biome)
    }

    const fn pick_beach_biome(&self, t: usize, _h: usize) -> Biome {
        if t == 0 {
            "minecraft:snowy_beach"
        } else if t == 4 {
            "minecraft:desert"
        } else {
            "minecraft:beach"
        }
    }

    const fn pick_badlands_biome(&self, h: usize, weirdness: Parameter) -> Biome {
        if h < 2 {
            if weirdness.max < 0 {
                "minecraft:badlands"
            } else {
                "minecraft:eroded_badlands"
            }
        } else if h < 3 {
            "minecraft:badlands"
        } else {
            "minecraft:wooded_badlands"
        }
    }

    const fn pick_plateau_biome(&self, t: usize, h: usize, weirdness: Parameter) -> Biome {
        if weirdness.max >= 0
            && let Some(variant) = self.plateau_biomes_variant[t][h]
        {
            return variant;
        }
        self.plateau_biomes[t][h]
    }

    const fn pick_peak_biome(&self, t: usize, h: usize, weirdness: Parameter) -> Biome {
        if t <= 2 {
            if weirdness.max < 0 {
                "minecraft:jagged_peaks"
            } else {
                "minecraft:frozen_peaks"
            }
        } else if t == 3 {
            "minecraft:stony_peaks"
        } else {
            self.pick_badlands_biome(h, weirdness)
        }
    }

    const fn pick_slope_biome(&self, t: usize, h: usize, weirdness: Parameter) -> Biome {
        if t >= 3 {
            self.pick_plateau_biome(t, h, weirdness)
        } else if h <= 1 {
            "minecraft:snowy_slopes"
        } else {
            "minecraft:grove"
        }
    }

    fn pick_shattered_biome(&self, t: usize, h: usize, weirdness: Parameter) -> Biome {
        self.shattered_biomes[t][h].unwrap_or_else(|| self.pick_middle_biome(t, h, weirdness))
    }

    #[expect(clippy::too_many_arguments, reason = "mirrors OverworldBiomeBuilder")]
    fn add_surface_biome(
        &self,
        out: &mut Vec<(ParameterPoint, Biome)>,
        temperature: Parameter,
        humidity: Parameter,
        continentalness: Parameter,
        erosion: Parameter,
        weirdness: Parameter,
        offset: f32,
        biome: Biome,
    ) {
        out.push((
            ParameterPoint::new(
                temperature,
                humidity,
                continentalness,
                erosion,
                Parameter::point(0.0),
                weirdness,
                quantize_coord(f64::from(offset)),
            ),
            biome,
        ));
        out.push((
            ParameterPoint::new(
                temperature,
                humidity,
                continentalness,
                erosion,
                Parameter::point(1.0),
                weirdness,
                quantize_coord(f64::from(offset)),
            ),
            biome,
        ));
    }

    #[expect(clippy::too_many_arguments, reason = "mirrors OverworldBiomeBuilder")]
    fn add_underground_biome(
        &self,
        out: &mut Vec<(ParameterPoint, Biome)>,
        temperature: Parameter,
        humidity: Parameter,
        continentalness: Parameter,
        erosion: Parameter,
        weirdness: Parameter,
        offset: f32,
        biome: Biome,
    ) {
        out.push((
            ParameterPoint::new(
                temperature,
                humidity,
                continentalness,
                erosion,
                Parameter::span(0.2, 0.9),
                weirdness,
                quantize_coord(f64::from(offset)),
            ),
            biome,
        ));
    }

    #[expect(clippy::too_many_arguments, reason = "mirrors OverworldBiomeBuilder")]
    fn add_bottom_biome(
        &self,
        out: &mut Vec<(ParameterPoint, Biome)>,
        temperature: Parameter,
        humidity: Parameter,
        continentalness: Parameter,
        erosion: Parameter,
        weirdness: Parameter,
        offset: f32,
        biome: Biome,
    ) {
        out.push((
            ParameterPoint::new(
                temperature,
                humidity,
                continentalness,
                erosion,
                Parameter::point(1.1),
                weirdness,
                quantize_coord(f64::from(offset)),
            ),
            biome,
        ));
    }
}
