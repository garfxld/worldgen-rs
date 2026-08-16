use crate::util::pos::BlockPos;

pub fn between_closed(from: BlockPos, to: BlockPos) -> impl Iterator<Item = BlockPos> {
    let min_x = from.x().min(to.x());
    let min_y = from.y().min(to.y());
    let min_z = from.z().min(to.z());
    let max_x = from.x().max(to.x());
    let max_y = from.y().max(to.y());
    let max_z = from.z().max(to.z());

    (min_z..=max_z).flat_map(move |z| {
        (min_y..=max_y).flat_map(move |y| (min_x..=max_x).map(move |x| BlockPos::new(x, y, z)))
    })
}
