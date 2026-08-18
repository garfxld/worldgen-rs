use crate::registry::REGISTRY;
use crate::util::ident::{BlockState, Identifier};
use crate::worldgen::backend::ir::json::{
    SurfaceConditionJson, SurfaceRuleJson, VerticalAnchorJson,
};
use crate::worldgen::surface::SurfaceRuleContext;

pub struct DatapackSurfaceRules {
    rule: Option<CompiledRule>,
    noise_ids: &'static [&'static str],
    gradient_ids: &'static [&'static str],
    block_states: &'static [BlockState],
    uses_biome: bool,
    uses_preliminary_surface: bool,
    uses_surface_secondary: bool,
    uses_steep: bool,
}

enum CompiledRule {
    Block(usize),
    Sequence(Vec<CompiledRule>),
    Condition(CompiledCondition, Box<CompiledRule>),
    Bandlands,
}

enum CompiledCondition {
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        is_floor: bool,
    },
    AbovePreliminarySurface,
    BiomeIs {
        ids: Vec<u16>,
        use_biome_provider: bool,
    },
    NoiseThreshold {
        index: usize,
        is_3d: bool,
        min: f64,
        max: f64,
    },
    VerticalGradient {
        index: usize,
        true_y: i32,
        false_y: i32,
    },
    YAbove {
        anchor_y: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    Temperature,
    Steep,
    Hole,
    Not(Box<CompiledCondition>),
}

struct Compiler {
    noise_ids: Vec<String>,
    gradient_ids: Vec<String>,
    block_states: Vec<BlockState>,
    uses_biome: bool,
    uses_preliminary_surface: bool,
    uses_surface_secondary: bool,
    uses_steep: bool,
    min_y: i32,
    height: i32,
}

impl Compiler {
    fn compile_rule(&mut self, rule: &SurfaceRuleJson) -> CompiledRule {
        match rule {
            SurfaceRuleJson::Block { result_state } => {
                let state = resolve_block_state(&result_state.name);
                let index = index_of_or_push(&mut self.block_states, state);
                CompiledRule::Block(index)
            }
            SurfaceRuleJson::Sequence { sequence } => {
                CompiledRule::Sequence(sequence.iter().map(|r| self.compile_rule(r)).collect())
            }
            SurfaceRuleJson::Condition { if_true, then_run } => {
                let cond = self.compile_condition(if_true);
                let body = self.compile_rule(then_run);
                CompiledRule::Condition(cond, Box::new(body))
            }
            SurfaceRuleJson::Bandlands {} => CompiledRule::Bandlands,
        }
    }

    fn compile_condition(&mut self, cond: &SurfaceConditionJson) -> CompiledCondition {
        match cond {
            SurfaceConditionJson::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                surface_type,
            } => {
                if *secondary_depth_range > 0 {
                    self.uses_surface_secondary = true;
                }
                CompiledCondition::StoneDepth {
                    offset: *offset,
                    add_surface_depth: *add_surface_depth,
                    secondary_depth_range: *secondary_depth_range,
                    is_floor: surface_type == "floor",
                }
            }
            SurfaceConditionJson::AbovePreliminarySurface {} => {
                self.uses_preliminary_surface = true;
                CompiledCondition::AbovePreliminarySurface
            }
            SurfaceConditionJson::BiomeIs { biome_is } => {
                self.uses_biome = true;
                let ids = biome_is
                    .as_slice()
                    .iter()
                    .filter_map(|b| {
                        let key: Identifier = b.as_str().parse().ok()?;
                        REGISTRY.biomes.by_key(&key)
                    })
                    .map(|biome| biome.id() as u16)
                    .collect();
                CompiledCondition::BiomeIs {
                    ids,
                    use_biome_provider: self.uses_preliminary_surface,
                }
            }
            SurfaceConditionJson::NoiseThreshold {
                noise,
                is_3d,
                min_threshold,
                max_threshold,
            } => {
                let index = index_of_or_push(&mut self.noise_ids, noise.clone());
                CompiledCondition::NoiseThreshold {
                    index,
                    is_3d: *is_3d,
                    min: *min_threshold,
                    max: *max_threshold,
                }
            }
            SurfaceConditionJson::VerticalGradient {
                random_name,
                true_at_and_below,
                false_at_and_above,
            } => {
                let index = index_of_or_push(&mut self.gradient_ids, random_name.clone());
                CompiledCondition::VerticalGradient {
                    index,
                    true_y: self.resolve_anchor(true_at_and_below),
                    false_y: self.resolve_anchor(false_at_and_above),
                }
            }
            SurfaceConditionJson::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => CompiledCondition::YAbove {
                anchor_y: self.resolve_anchor(anchor),
                surface_depth_multiplier: *surface_depth_multiplier,
                add_stone_depth: *add_stone_depth,
            },
            SurfaceConditionJson::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => CompiledCondition::Water {
                offset: *offset,
                surface_depth_multiplier: *surface_depth_multiplier,
                add_stone_depth: *add_stone_depth,
            },
            SurfaceConditionJson::Temperature {} => {
                self.uses_biome = true;
                CompiledCondition::Temperature
            }
            SurfaceConditionJson::Steep {} => {
                self.uses_steep = true;
                CompiledCondition::Steep
            }
            SurfaceConditionJson::Hole {} => CompiledCondition::Hole,
            SurfaceConditionJson::Not { invert } => {
                CompiledCondition::Not(Box::new(self.compile_condition(invert)))
            }
        }
    }

    fn resolve_anchor(&self, anchor: &VerticalAnchorJson) -> i32 {
        match anchor {
            VerticalAnchorJson::Absolute { absolute } => *absolute,
            VerticalAnchorJson::AboveBottom { above_bottom } => self.min_y + above_bottom,
            VerticalAnchorJson::BelowTop { below_top } => self.height - 1 + self.min_y - below_top,
        }
    }
}

fn index_of_or_push<T: PartialEq>(items: &mut Vec<T>, value: T) -> usize {
    if let Some(index) = items.iter().position(|item| *item == value) {
        return index;
    }
    items.push(value);
    items.len() - 1
}

fn rule_uses_preliminary_surface(rule: &SurfaceRuleJson) -> bool {
    match rule {
        SurfaceRuleJson::Block { .. } | SurfaceRuleJson::Bandlands {} => false,
        SurfaceRuleJson::Sequence { sequence } => {
            sequence.iter().any(rule_uses_preliminary_surface)
        }
        SurfaceRuleJson::Condition { if_true, then_run } => {
            condition_uses_preliminary_surface(if_true) || rule_uses_preliminary_surface(then_run)
        }
    }
}

fn condition_uses_preliminary_surface(condition: &SurfaceConditionJson) -> bool {
    match condition {
        SurfaceConditionJson::AbovePreliminarySurface {} => true,
        SurfaceConditionJson::Not { invert } => condition_uses_preliminary_surface(invert),
        SurfaceConditionJson::StoneDepth { .. }
        | SurfaceConditionJson::BiomeIs { .. }
        | SurfaceConditionJson::NoiseThreshold { .. }
        | SurfaceConditionJson::VerticalGradient { .. }
        | SurfaceConditionJson::YAbove { .. }
        | SurfaceConditionJson::Water { .. }
        | SurfaceConditionJson::Temperature {}
        | SurfaceConditionJson::Steep {}
        | SurfaceConditionJson::Hole {} => false,
    }
}

fn resolve_block_state(name: &str) -> BlockState {
    let key: Identifier = name
        .parse()
        .unwrap_or_else(|e| panic!("invalid surface rule block id {name}: {e}"));
    let block = REGISTRY
        .blocks
        .by_key(&key)
        .unwrap_or_else(|| panic!("surface rule references unknown block {name}"));
    REGISTRY.blocks.get_default_state_id(block)
}

impl DatapackSurfaceRules {
    pub fn new(rule: Option<&SurfaceRuleJson>, min_y: i32, height: i32) -> Self {
        let uses_preliminary_surface = rule.is_some_and(rule_uses_preliminary_surface);
        let mut compiler = Compiler {
            noise_ids: Vec::new(),
            gradient_ids: Vec::new(),
            block_states: Vec::new(),
            uses_biome: false,
            uses_preliminary_surface,
            uses_surface_secondary: false,
            uses_steep: false,
            min_y,
            height,
        };
        let compiled_rule = rule.map(|rule| compiler.compile_rule(rule));

        Self {
            rule: compiled_rule,
            noise_ids: leak_str_slice(&compiler.noise_ids),
            gradient_ids: leak_str_slice(&compiler.gradient_ids),
            block_states: Box::leak(compiler.block_states.into_boxed_slice()),
            uses_biome: compiler.uses_biome,
            uses_preliminary_surface: compiler.uses_preliminary_surface,
            uses_surface_secondary: compiler.uses_surface_secondary,
            uses_steep: compiler.uses_steep,
        }
    }

    pub const fn noise_ids(&self) -> &'static [&'static str] {
        self.noise_ids
    }

    pub const fn gradient_ids(&self) -> &'static [&'static str] {
        self.gradient_ids
    }

    pub const fn block_states(&self) -> &'static [BlockState] {
        self.block_states
    }

    pub const fn uses_biome(&self) -> bool {
        self.uses_biome
    }

    pub const fn uses_preliminary_surface(&self) -> bool {
        self.uses_preliminary_surface
    }

    pub const fn uses_surface_secondary(&self) -> bool {
        self.uses_surface_secondary
    }

    pub const fn uses_steep(&self) -> bool {
        self.uses_steep
    }

    pub fn try_apply(&self, ctx: &mut SurfaceRuleContext<'_>) -> Option<BlockState> {
        let rule = self.rule.as_ref()?;
        Self::eval_rule(rule, ctx)
    }

    fn eval_rule(rule: &CompiledRule, ctx: &mut SurfaceRuleContext<'_>) -> Option<BlockState> {
        match rule {
            CompiledRule::Block(index) => Some(ctx.block_state(*index)),
            CompiledRule::Sequence(rules) => {
                for rule in rules {
                    if let Some(result) = Self::eval_rule(rule, ctx) {
                        return Some(result);
                    }
                }
                None
            }
            CompiledRule::Condition(cond, body) => {
                if Self::eval_condition(cond, ctx) {
                    Self::eval_rule(body, ctx)
                } else {
                    None
                }
            }
            CompiledRule::Bandlands => {
                Some(ctx.system.get_band(ctx.block_x, ctx.block_y, ctx.block_z))
            }
        }
    }

    fn eval_condition(cond: &CompiledCondition, ctx: &mut SurfaceRuleContext<'_>) -> bool {
        match cond {
            CompiledCondition::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                is_floor,
            } => {
                let depth_field = if *is_floor {
                    ctx.stone_depth_above
                } else {
                    ctx.stone_depth_below
                };
                if *secondary_depth_range > 0 {
                    let extra = ((ctx.surface_secondary + 1.0) / 2.0
                        * f64::from(*secondary_depth_range)) as i32;
                    let base = if *add_surface_depth {
                        ctx.surface_depth
                    } else {
                        0
                    };
                    depth_field <= 1 + offset + base + extra
                } else if *add_surface_depth {
                    depth_field <= 1 + offset + ctx.surface_depth
                } else {
                    depth_field <= 1 + offset
                }
            }
            CompiledCondition::AbovePreliminarySurface => ctx.block_y >= ctx.min_surface_level,
            CompiledCondition::BiomeIs {
                ids,
                use_biome_provider,
            } => {
                let biome_id = if *use_biome_provider {
                    ctx.biome_id()
                } else {
                    ctx.known_biome_id()
                };
                biome_id.is_some_and(|id| ids.contains(&id))
            }
            CompiledCondition::NoiseThreshold {
                index,
                is_3d,
                min,
                max,
            } => {
                let value = if *is_3d {
                    ctx.condition_noise_3d(*index)
                } else {
                    ctx.condition_noise(*index)
                };
                value >= *min && value <= *max
            }
            CompiledCondition::VerticalGradient {
                index,
                true_y,
                false_y,
            } => ctx.system.vertical_gradient(
                *index,
                ctx.block_x,
                ctx.block_y,
                ctx.block_z,
                *true_y,
                *false_y,
            ),
            CompiledCondition::YAbove {
                anchor_y,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                let extra = if *add_stone_depth {
                    ctx.stone_depth_above
                } else {
                    0
                };
                ctx.block_y + extra >= anchor_y + ctx.surface_depth * surface_depth_multiplier
            }
            CompiledCondition::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                if ctx.water_height == i32::MIN {
                    return true;
                }
                let extra = if *add_stone_depth {
                    ctx.stone_depth_above
                } else {
                    0
                };
                ctx.block_y + extra
                    >= ctx.water_height + offset + ctx.surface_depth * surface_depth_multiplier
            }
            CompiledCondition::Temperature => ctx.cold_enough_to_snow(),
            CompiledCondition::Steep => ctx.steep,
            CompiledCondition::Hole => ctx.surface_depth <= 0,
            CompiledCondition::Not(inner) => !Self::eval_condition(inner, ctx),
        }
    }
}

fn leak_str_slice(strings: &[String]) -> &'static [&'static str] {
    let leaked: Vec<&'static str> = strings
        .iter()
        .map(|s| -> &'static str { Box::leak(s.clone().into_boxed_str()) })
        .collect();
    Box::leak(leaked.into_boxed_slice())
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test loads the checked-in vanilla datapack"
)]
mod tests {
    use std::cell::Cell;

    use std::sync::Once;

    use super::*;
    use crate::registry::{Registries, blocks::vanilla_blocks};
    use crate::util::rng::RandomSource;
    use crate::util::rng::xoroshiro::XoroshiroRandomSource;
    use crate::worldgen::backend::ir::loader::DatapackSource;
    use crate::worldgen::surface::{
        SurfaceConditionNoiseCache, SurfaceRuleContext, SurfaceSystem, TemperatureXzCache,
    };

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let registries = Registries::new_vanilla().expect("vanilla datapack should load");
            let _ = REGISTRY.init(registries);
        });
    }

    fn count_rule_nodes(rule: &SurfaceRuleJson) -> usize {
        1 + match rule {
            SurfaceRuleJson::Block { .. } | SurfaceRuleJson::Bandlands {} => 0,
            SurfaceRuleJson::Sequence { sequence } => sequence.iter().map(count_rule_nodes).sum(),
            SurfaceRuleJson::Condition { if_true, then_run } => {
                count_condition_nodes(if_true) + count_rule_nodes(then_run)
            }
        }
    }

    fn count_condition_nodes(cond: &SurfaceConditionJson) -> usize {
        1 + match cond {
            SurfaceConditionJson::Not { invert } => count_condition_nodes(invert),
            _ => 0,
        }
    }

    #[test]
    fn compiles_overworld_surface_rules_with_a_large_node_count() {
        init_global_registry();
        let source = DatapackSource::builtin();
        let settings = source
            .noise_settings(&Identifier::new(Identifier::VANILLA_NAMESPACE, "overworld"))
            .unwrap();
        let rule_json = settings
            .surface_rule
            .as_ref()
            .expect("overworld defines a surface_rule");

        let node_count = count_rule_nodes(rule_json);
        assert!(
            node_count > 100,
            "expected a large surface rule tree, got {node_count} nodes"
        );

        let rules =
            DatapackSurfaceRules::new(Some(rule_json), settings.noise.min_y, settings.noise.height);
        assert!(rules.uses_biome());
        assert!(rules.uses_preliminary_surface());
        assert!(!rules.block_states().is_empty());
        assert!(!rules.noise_ids().is_empty());
        assert!(!rules.gradient_ids().is_empty());
    }

    #[test]
    fn compiles_nether_and_end_surface_rules_without_panicking() {
        init_global_registry();
        let source = DatapackSource::builtin();

        for dimension in ["nether", "end"] {
            let settings = source
                .noise_settings(&Identifier::new(
                    Identifier::VANILLA_NAMESPACE,
                    dimension.to_owned(),
                ))
                .unwrap();
            let rule_json = settings
                .surface_rule
                .as_ref()
                .unwrap_or_else(|| panic!("{dimension} should define a surface_rule"));
            let rules = DatapackSurfaceRules::new(
                Some(rule_json),
                settings.noise.min_y,
                settings.noise.height,
            );

            let params = source.noise_parameters().unwrap();
            let mut root = RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(42));
            let splitter = root.next_positional();
            let default_block = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::STONE);
            let _system = SurfaceSystem::new(
                &splitter,
                &params,
                rules.noise_ids(),
                rules.gradient_ids(),
                default_block,
                settings.sea_level,
            );
        }
    }

    fn block_name(state: BlockState) -> String {
        REGISTRY
            .blocks
            .by_state_id(state)
            .map(|b| b.key.path.to_string())
            .unwrap_or_default()
    }

    #[test]
    fn resolves_grass_on_dry_plains_surface_and_none_deep_underground() {
        init_global_registry();
        let source = DatapackSource::builtin();
        let settings = source
            .noise_settings(&Identifier::new(Identifier::VANILLA_NAMESPACE, "overworld"))
            .unwrap();
        let rule_json = settings.surface_rule.as_ref().unwrap();
        let rules =
            DatapackSurfaceRules::new(Some(rule_json), settings.noise.min_y, settings.noise.height);

        let params = source.noise_parameters().unwrap();
        let mut root = RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(42));
        let splitter = root.next_positional();
        let default_block = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::STONE);
        let system = SurfaceSystem::new(
            &splitter,
            &params,
            rules.noise_ids(),
            rules.gradient_ids(),
            default_block,
            63,
        );

        let plains = REGISTRY
            .biomes
            .by_key(&"minecraft:plains".parse().unwrap())
            .unwrap();

        let values: Vec<Cell<f64>> = (0..rules.noise_ids().len())
            .map(|_| Cell::new(0.0))
            .collect();
        let computed_at: Vec<Cell<u64>> =
            (0..rules.noise_ids().len()).map(|_| Cell::new(0)).collect();

        let resolve =
            |block_y: i32, stone_depth_above: i32, stone_depth_below: i32| -> Option<BlockState> {
                let noise_cache = SurfaceConditionNoiseCache::new(&values, &computed_at);
                noise_cache.reset();
                let mut xz = TemperatureXzCache::new(0, 0);
                let mut ctx = SurfaceRuleContext::new(
                    0,
                    0,
                    4,
                    0.0,
                    0,
                    false,
                    block_y,
                    stone_depth_above,
                    stone_depth_below,
                    i32::MIN,
                    Some(plains.id() as u16),
                    None,
                    &system,
                    &noise_cache,
                    &mut xz,
                    rules.block_states(),
                );
                rules.try_apply(&mut ctx)
            };

        let surface = resolve(70, 1, 100).expect("topmost dry-land plains block should resolve");
        assert_eq!(block_name(surface), "grass_block");

        let deep = resolve(-40, 200, 5);
        assert!(
            deep.is_none() || block_name(deep.unwrap()) != "grass_block",
            "deep underground should not be grass"
        );
    }
}
