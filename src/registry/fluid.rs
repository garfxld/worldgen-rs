use crate::registry::{REGISTRY, Registry};
use crate::util::ident::Identifier;

#[derive(Debug, PartialEq)]
pub struct Fluid {
    pub key: Identifier,
    pub is_empty: bool,
    pub is_source: bool,
    pub is_randomly_ticking: bool,
    pub block: Identifier,
    pub source_fluid: Option<Identifier>,
    pub flowing_fluid: Option<Identifier>,
    pub tick_delay: u32,
    pub explosion_resistance: f32,
}

impl Fluid {
    #[must_use]
    pub fn has_tag(&self, tag: &Identifier) -> bool {
        REGISTRY.fluids.is_in_tag(&self.key, tag)
    }

    #[must_use]
    pub fn source_variant(&'static self) -> FluidRef {
        let Some(source_key) = &self.source_fluid else {
            return self;
        };

        match REGISTRY.fluids.by_key(source_key) {
            Some(fluid) => fluid,
            None => panic!(
                "fluid `{}` references missing source fluid `{source_key}`",
                self.key
            ),
        }
    }

    #[must_use]
    pub fn flowing_variant(&'static self) -> FluidRef {
        let Some(flowing_key) = &self.flowing_fluid else {
            return self;
        };

        match REGISTRY.fluids.by_key(flowing_key) {
            Some(fluid) => fluid,
            None => panic!(
                "fluid `{}` references missing flowing fluid `{flowing_key}`",
                self.key
            ),
        }
    }
}

pub type FluidRef = &'static Fluid;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FluidState {
    pub fluid_id: FluidRef,
    pub amount: u8,
    pub falling: bool,
}

impl FluidState {
    pub const EMPTY: Self = Self {
        fluid_id: &vanilla_fluids::EMPTY,
        amount: 0,
        falling: false,
    };

    #[must_use]
    pub const fn new(fluid: FluidRef, amount: u8, falling: bool) -> Self {
        Self {
            fluid_id: fluid,
            amount,
            falling,
        }
    }

    #[must_use]
    pub const fn source(fluid: FluidRef) -> Self {
        Self {
            fluid_id: fluid,
            amount: 8,
            falling: false,
        }
    }

    #[must_use]
    pub const fn flowing(fluid: FluidRef, amount: u8, falling: bool) -> Self {
        Self {
            fluid_id: fluid,
            amount,
            falling,
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.fluid_id.is_empty || self.amount == 0
    }

    #[must_use]
    pub const fn is_source(&self) -> bool {
        self.fluid_id.is_source
    }

    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.amount == 8
    }

    #[must_use]
    pub const fn is_randomly_ticking(&self) -> bool {
        self.fluid_id.is_randomly_ticking
    }

    #[must_use]
    pub fn own_height(&self) -> f32 {
        if self.is_empty() {
            0.0
        } else {
            f32::from(self.amount) / 9.0
        }
    }

    #[must_use]
    pub const fn to_block_level(self) -> u8 {
        if self.is_source() {
            0
        } else if self.falling {
            8
        } else {
            8 - self.amount
        }
    }

    #[must_use]
    pub fn is_water(&self) -> bool {
        is_water_fluid(self.fluid_id)
    }

    #[must_use]
    pub fn is_lava(&self) -> bool {
        is_lava_fluid(self.fluid_id)
    }
}

pub type FluidRegistry = Registry<Fluid>;

#[must_use]
pub fn is_water_fluid(fluid: FluidRef) -> bool {
    !fluid.is_empty
        && (fluid.key == vanilla_fluids::WATER.key
            || fluid.key == vanilla_fluids::FLOWING_WATER.key)
}

#[must_use]
pub fn is_lava_fluid(fluid: FluidRef) -> bool {
    !fluid.is_empty
        && (fluid.key == vanilla_fluids::LAVA.key || fluid.key == vanilla_fluids::FLOWING_LAVA.key)
}

pub mod vanilla_fluids {
    use super::Fluid;
    use crate::util::ident::Identifier;

    pub static EMPTY: Fluid = Fluid {
        key: Identifier::const_vanilla("empty"),
        is_empty: true,
        is_source: false,
        is_randomly_ticking: false,
        block: Identifier::const_vanilla("air"),
        source_fluid: None,
        flowing_fluid: None,
        tick_delay: 0,
        explosion_resistance: 0.0,
    };

    pub static FLOWING_WATER: Fluid = Fluid {
        key: Identifier::const_vanilla("flowing_water"),
        is_empty: false,
        is_source: false,
        is_randomly_ticking: false,
        block: Identifier::const_vanilla("water"),
        source_fluid: Some(Identifier::const_vanilla("water")),
        flowing_fluid: Some(Identifier::const_vanilla("flowing_water")),
        tick_delay: 5,
        explosion_resistance: 100.0,
    };

    pub static WATER: Fluid = Fluid {
        key: Identifier::const_vanilla("water"),
        is_empty: false,
        is_source: true,
        is_randomly_ticking: false,
        block: Identifier::const_vanilla("water"),
        source_fluid: Some(Identifier::const_vanilla("water")),
        flowing_fluid: Some(Identifier::const_vanilla("flowing_water")),
        tick_delay: 5,
        explosion_resistance: 100.0,
    };

    pub static FLOWING_LAVA: Fluid = Fluid {
        key: Identifier::const_vanilla("flowing_lava"),
        is_empty: false,
        is_source: false,
        is_randomly_ticking: true,
        block: Identifier::const_vanilla("lava"),
        source_fluid: Some(Identifier::const_vanilla("lava")),
        flowing_fluid: Some(Identifier::const_vanilla("flowing_lava")),
        tick_delay: 30,
        explosion_resistance: 100.0,
    };

    pub static LAVA: Fluid = Fluid {
        key: Identifier::const_vanilla("lava"),
        is_empty: false,
        is_source: true,
        is_randomly_ticking: true,
        block: Identifier::const_vanilla("lava"),
        source_fluid: Some(Identifier::const_vanilla("lava")),
        flowing_fluid: Some(Identifier::const_vanilla("flowing_lava")),
        tick_delay: 30,
        explosion_resistance: 100.0,
    };

    pub fn register_fluids(registry: &mut super::FluidRegistry) {
        registry.register(EMPTY.key.clone(), &EMPTY);
        registry.register(FLOWING_WATER.key.clone(), &FLOWING_WATER);
        registry.register(WATER.key.clone(), &WATER);
        registry.register(FLOWING_LAVA.key.clone(), &FLOWING_LAVA);
        registry.register(LAVA.key.clone(), &LAVA);
    }
}

#[cfg(test)]
mod tests {
    use crate::registry::{REGISTRY, Registries};

    use super::*;

    fn init_test_registry() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let mut registries = Registries::new_vanilla();
            registries.freeze();
            let _ = REGISTRY.init(registries);
        });
    }

    #[test]
    fn source_variant_resolves_flowing_to_source() {
        init_test_registry();

        assert_eq!(
            vanilla_fluids::FLOWING_WATER.source_variant(),
            &vanilla_fluids::WATER
        );
        assert_eq!(
            vanilla_fluids::FLOWING_LAVA.source_variant(),
            &vanilla_fluids::LAVA
        );
    }

    #[test]
    fn flowing_variant_resolves_source_to_flowing() {
        init_test_registry();

        assert_eq!(
            vanilla_fluids::WATER.flowing_variant(),
            &vanilla_fluids::FLOWING_WATER
        );
        assert_eq!(
            vanilla_fluids::LAVA.flowing_variant(),
            &vanilla_fluids::FLOWING_LAVA
        );
    }

    #[test]
    fn source_fluid_type_is_source_even_when_falling() {
        init_test_registry();

        let falling_source = FluidState::new(&vanilla_fluids::WATER, 8, true);

        assert!(falling_source.is_source());
        assert!(falling_source.is_full());
    }

    #[test]
    fn by_key_matches_registration_order() {
        let mut registry = FluidRegistry::new();
        vanilla_fluids::register_fluids(&mut registry);
        registry.freeze();

        assert_eq!(registry.len(), 5);
        assert_eq!(
            registry
                .by_key(&Identifier::const_vanilla("water"))
                .map(|f| &f.key),
            Some(&vanilla_fluids::WATER.key)
        );
    }
}
