use shulkr::world::fluid::Fluid;

pub use shulkr::world::fluid::FluidState;

pub fn is_water_fluid(fluid: Fluid) -> bool {
    fluid == Fluid::WATER || fluid == Fluid::FLOWING_WATER
}

pub fn is_lava_fluid(fluid: Fluid) -> bool {
    fluid == Fluid::LAVA || fluid == Fluid::FLOWING_LAVA
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_and_lava_are_recognised() {
        assert!(is_water_fluid(Fluid::WATER));
        assert!(is_water_fluid(Fluid::FLOWING_WATER));
        assert!(!is_water_fluid(Fluid::LAVA));
        assert!(is_lava_fluid(Fluid::FLOWING_LAVA));
        assert!(!is_lava_fluid(Fluid::EMPTY));
    }
}
