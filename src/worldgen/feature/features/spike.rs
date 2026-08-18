use serde::Deserialize;

use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::block_predicate::BlockPredicate;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
pub struct SpikeConfig {
    pub state: BlockState,
    pub can_place_on: BlockPredicate,
    pub can_replace: BlockPredicate,
}

#[expect(
    clippy::neg_cmp_op_on_partial_ord,
    reason = "vanilla writes these comparisons negated"
)]
pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &SpikeConfig,
) -> bool {
    let mut origin = origin;
    while region.get_block(origin).is_air() && origin.y() > region.min_y() + 2 {
        origin = origin.below();
    }

    if !config.can_place_on.test(region, origin) {
        return false;
    }

    origin = origin.above_n(random.next_i32_bounded(4));
    let height = random.next_i32_bounded(4) + 7;
    let mut width = height / 4 + random.next_i32_bounded(2);
    if width > 1 && random.next_i32_bounded(60) == 0 {
        origin = origin.above_n(10 + random.next_i32_bounded(30));
    }

    for y_off in 0..height {
        let scale = (1.0 - y_off as f32 / height as f32) * width as f32;
        let new_width = scale.ceil() as i32;

        for xo in -new_width..=new_width {
            let dx = xo.abs() as f32 - 0.25;

            for zo in -new_width..=new_width {
                let dz = zo.abs() as f32 - 0.25;
                let inside_cone = (xo == 0 && zo == 0) || !(dx * dx + dz * dz > scale * scale);
                let allow_corner_gap =
                    (xo != -new_width && xo != new_width && zo != -new_width && zo != new_width)
                        || !(random.next_f32() > 0.75);
                if inside_cone && allow_corner_gap {
                    let positive_offset = origin.offset(xo, y_off, zo);
                    if region.get_block(positive_offset).is_air()
                        || config.can_replace.test(region, positive_offset)
                    {
                        region.set_block(positive_offset, config.state);
                    }

                    if y_off != 0 && new_width > 1 {
                        let negative_offset = origin.offset(xo, -y_off, zo);
                        if region.get_block(negative_offset).is_air()
                            || config.can_replace.test(region, negative_offset)
                        {
                            region.set_block(negative_offset, config.state);
                        }
                    }
                }
            }
        }
    }

    width = (width - 1).clamp(0, 1);

    for xo in -width..=width {
        for zo in -width..=width {
            let mut cursor = origin.offset(xo, -1, zo);
            let mut run_length = 50;
            if xo.abs() == 1 && zo.abs() == 1 {
                run_length = random.next_i32_bounded(5);
            }

            while cursor.y() > 50 {
                let state = region.get_block(cursor);
                if !state.is_air()
                    && !config.can_replace.test(region, cursor)
                    && state != config.state
                {
                    break;
                }

                region.set_block(cursor, config.state);
                cursor = cursor.below();
                run_length -= 1;
                if run_length <= 0 {
                    cursor = cursor.below_n(random.next_i32_bounded(5) + 1);
                    run_length = random.next_i32_bounded(5);
                }
            }
        }
    }

    true
}
