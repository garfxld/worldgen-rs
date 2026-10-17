use shulkr::world::block::{Block, BlockState, Properties};

use crate::util::rng::RandomSource;
use crate::world::chunk::heightmap::HeightmapType;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::pools::jigsaw_junction::JigsawJunction;
use crate::world::worldgen::structure::pools::structure_pool_element::StructurePoolElement;
use crate::world::worldgen::structure::structures::buried_treasure_pieces::BuriedTreasurePiece;
use crate::world::worldgen::structure::structures::desert_pyramid_piece::DesertPyramidPiece;
use crate::world::worldgen::structure::structures::end_city_pieces::EndCityPiece;
use crate::world::worldgen::structure::structures::igloo_pieces::IglooPiece;
use crate::world::worldgen::structure::structures::jungle_temple_piece::JungleTemplePiece;
use crate::world::worldgen::structure::structures::mineshaft_pieces::{
    MineShaftPiece, MineshaftType,
};
use crate::world::worldgen::structure::structures::nether_fortress_pieces::NetherBridgePiece;
use crate::world::worldgen::structure::structures::nether_fossil_pieces::NetherFossilPiece;
use crate::world::worldgen::structure::structures::ocean_monument_pieces::MonumentBuilding;
use crate::world::worldgen::structure::structures::ocean_ruin_pieces::OceanRuinPiece;
use crate::world::worldgen::structure::structures::ruined_portal_piece::RuinedPortalPiece;
use crate::world::worldgen::structure::structures::shipwreck_pieces::ShipwreckPiece;
use crate::world::worldgen::structure::structures::stronghold_pieces::StrongholdPiece;
use crate::world::worldgen::structure::structures::swamp_hut_piece::SwampHutPiece;
use crate::world::worldgen::structure::structures::woodland_mansion_pieces::WoodlandMansionPiece;
use crate::world::worldgen::structure::templatesystem::liquid_settings::LiquidSettings;
use shulkr::util::Axis;
use shulkr::util::BlockPosition;
use shulkr::util::BoundingBox;
use shulkr::util::Direction;
use shulkr::world::block::Mirror;
use shulkr::world::block::Rotation;

const SHAPE_CHECK_BLOCKS: &[Block] = &[
    Block::NETHER_BRICK_FENCE,
    Block::TORCH,
    Block::WALL_TORCH,
    Block::OAK_FENCE,
    Block::SPRUCE_FENCE,
    Block::DARK_OAK_FENCE,
    Block::PALE_OAK_FENCE,
    Block::ACACIA_FENCE,
    Block::BIRCH_FENCE,
    Block::JUNGLE_FENCE,
    Block::LADDER,
    Block::IRON_BARS,
];

pub const fn air() -> BlockState {
    Block::AIR.default_state()
}

pub const fn cave_air() -> BlockState {
    Block::CAVE_AIR.default_state()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CanBeReplaced {
    #[default]
    Always,
    MineshaftWood(MineshaftType),
}

#[derive(Debug)]
pub struct PieceBase {
    pub bounding_box: BoundingBox,
    orientation: Option<Direction>,
    mirror: Mirror,
    rotation: Rotation,
    pub gen_depth: i32,
    pub can_be_replaced: CanBeReplaced,
}

impl PieceBase {
    pub const fn new(gen_depth: i32, bounding_box: BoundingBox) -> Self {
        Self {
            bounding_box,
            orientation: None,
            mirror: Mirror::None,
            rotation: Rotation::None,
            gen_depth,
            can_be_replaced: CanBeReplaced::Always,
        }
    }

    pub const fn make_bounding_box(
        x: i32,
        y: i32,
        z: i32,
        direction: Direction,
        width: i32,
        height: i32,
        depth: i32,
    ) -> BoundingBox {
        match direction.get_axis() {
            Axis::Z => BoundingBox::new(x, y, z, x + width - 1, y + height - 1, z + depth - 1),
            _ => BoundingBox::new(x, y, z, x + depth - 1, y + height - 1, z + width - 1),
        }
    }

    pub const fn orientation(&self) -> Option<Direction> {
        self.orientation
    }

    pub const fn rotation(&self) -> Rotation {
        self.rotation
    }

    pub const fn mirror(&self) -> Mirror {
        self.mirror
    }

    pub const fn set_orientation(&mut self, orientation: Option<Direction>) {
        self.orientation = orientation;
        match orientation {
            None => {
                self.rotation = Rotation::None;
                self.mirror = Mirror::None;
            }
            Some(Direction::South) => {
                self.mirror = Mirror::LeftRight;
                self.rotation = Rotation::None;
            }
            Some(Direction::West) => {
                self.mirror = Mirror::LeftRight;
                self.rotation = Rotation::Clockwise90;
            }
            Some(Direction::East) => {
                self.mirror = Mirror::None;
                self.rotation = Rotation::Clockwise90;
            }
            Some(_) => {
                self.mirror = Mirror::None;
                self.rotation = Rotation::None;
            }
        }
    }

    pub const fn world_pos(&self, x: i32, y: i32, z: i32) -> BlockPosition {
        BlockPosition::new(self.world_x(x, z), self.world_y(y), self.world_z(x, z))
    }

    pub const fn world_x(&self, x: i32, z: i32) -> i32 {
        match self.orientation {
            None => x,
            Some(Direction::North | Direction::South) => self.bounding_box.min_x() + x,
            Some(Direction::West) => self.bounding_box.max_x() - z,
            Some(Direction::East) => self.bounding_box.min_x() + z,
            Some(_) => x,
        }
    }

    pub const fn world_y(&self, y: i32) -> i32 {
        match self.orientation {
            None => y,
            Some(_) => y + self.bounding_box.min_y(),
        }
    }

    pub const fn world_z(&self, x: i32, z: i32) -> i32 {
        match self.orientation {
            None => z,
            Some(Direction::North) => self.bounding_box.max_z() - z,
            Some(Direction::South) => self.bounding_box.min_z() + z,
            Some(Direction::West | Direction::East) => self.bounding_box.min_z() + x,
            Some(_) => z,
        }
    }

    fn is_replacement_allowed(
        &self,
        region: &WorldGenRegion<'_>,
        x: i32,
        y: i32,
        z: i32,
        chunk_bb: BoundingBox,
    ) -> bool {
        match self.can_be_replaced {
            CanBeReplaced::Always => true,
            CanBeReplaced::MineshaftWood(kind) => {
                let block = self.get_block(region, x, y, z, chunk_bb).as_block();
                block != kind.planks_state().as_block()
                    && block != kind.wood_state().as_block()
                    && block != kind.fence_state().as_block()
                    && block != Block::IRON_CHAIN
            }
        }
    }

    pub fn place_block(
        &self,
        region: &WorldGenRegion<'_>,
        state: BlockState,
        x: i32,
        y: i32,
        z: i32,
        chunk_bb: BoundingBox,
    ) {
        let pos = self.world_pos(x, y, z);
        if !chunk_bb.is_inside(pos) || !self.is_replacement_allowed(region, x, y, z, chunk_bb) {
            return;
        }

        let mut state = state;
        if self.mirror != Mirror::None {
            state = state.mirror(self.mirror);
        }
        if self.rotation != Rotation::None {
            state = state.rotate(self.rotation);
        }

        region.set_block(pos, state);

        if SHAPE_CHECK_BLOCKS.contains(&state.as_block()) {
            region.mark_pos_for_postprocessing(pos);
        }
    }

    pub fn get_block(
        &self,
        region: &WorldGenRegion<'_>,
        x: i32,
        y: i32,
        z: i32,
        chunk_bb: BoundingBox,
    ) -> BlockState {
        let pos = self.world_pos(x, y, z);
        if chunk_bb.is_inside(pos) {
            region.get_block(pos)
        } else {
            air()
        }
    }

    pub fn is_interior(
        &self,
        region: &WorldGenRegion<'_>,
        x: i32,
        y: i32,
        z: i32,
        chunk_bb: BoundingBox,
    ) -> bool {
        let pos = self.world_pos(x, y + 1, z);
        chunk_bb.is_inside(pos)
            && pos.y() < region.get_height(HeightmapType::OceanFloorWg, pos.x(), pos.z())
    }

    #[expect(clippy::too_many_arguments, reason = "mirrors vanilla generateAirBox")]
    pub fn generate_air_box(
        &self,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
    ) {
        let air = air();
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    self.place_block(region, air, x, y, z, chunk_bb);
                }
            }
        }
    }

    #[expect(clippy::too_many_arguments, reason = "mirrors vanilla generateBox")]
    pub fn generate_box(
        &self,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        edge_block: BlockState,
        fill_block: BlockState,
        skip_air: bool,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if skip_air && self.get_block(region, x, y, z, chunk_bb).is_air() {
                        continue;
                    }
                    if y != y0 && y != y1 && x != x0 && x != x1 && z != z0 && z != z1 {
                        self.place_block(region, fill_block, x, y, z, chunk_bb);
                    } else {
                        self.place_block(region, edge_block, x, y, z, chunk_bb);
                    }
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla generateBox with a BlockSelector"
    )]
    pub fn generate_box_selected(
        &self,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        skip_air: bool,
        random: &mut RandomSource,
        mut selector: impl FnMut(&mut RandomSource, i32, i32, i32, bool) -> BlockState,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if skip_air && self.get_block(region, x, y, z, chunk_bb).is_air() {
                        continue;
                    }
                    let is_edge = y == y0 || y == y1 || x == x0 || x == x1 || z == z0 || z == z1;
                    let state = selector(random, x, y, z, is_edge);
                    self.place_block(region, state, x, y, z, chunk_bb);
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla generateMaybeBox"
    )]
    pub fn generate_maybe_box(
        &self,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        random: &mut RandomSource,
        probability: f32,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        edge_block: BlockState,
        fill_block: BlockState,
        skip_air: bool,
        has_to_be_inside: bool,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if random.next_f32() > probability {
                        continue;
                    }
                    if skip_air && self.get_block(region, x, y, z, chunk_bb).is_air() {
                        continue;
                    }
                    if has_to_be_inside && !self.is_interior(region, x, y, z, chunk_bb) {
                        continue;
                    }
                    if y != y0 && y != y1 && x != x0 && x != x1 && z != z0 && z != z1 {
                        self.place_block(region, fill_block, x, y, z, chunk_bb);
                    } else {
                        self.place_block(region, edge_block, x, y, z, chunk_bb);
                    }
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla maybeGenerateBlock"
    )]
    pub fn maybe_generate_block(
        &self,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        random: &mut RandomSource,
        probability: f32,
        x: i32,
        y: i32,
        z: i32,
        state: BlockState,
    ) {
        if random.next_f32() < probability {
            self.place_block(region, state, x, y, z, chunk_bb);
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla generateUpperHalfSphere"
    )]
    pub fn generate_upper_half_sphere(
        &self,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        fill_block: BlockState,
        skip_air: bool,
    ) {
        let diag_x = (x1 - x0 + 1) as f32;
        let diag_y = (y1 - y0 + 1) as f32;
        let diag_z = (z1 - z0 + 1) as f32;
        let cx = x0 as f32 + diag_x / 2.0;
        let cz = z0 as f32 + diag_z / 2.0;

        for y in y0..=y1 {
            let ny = (y - y0) as f32 / diag_y;
            for x in x0..=x1 {
                let nx = (x as f32 - cx) / (diag_x * 0.5);
                for z in z0..=z1 {
                    let nz = (z as f32 - cz) / (diag_z * 0.5);
                    if skip_air && self.get_block(region, x, y, z, chunk_bb).is_air() {
                        continue;
                    }
                    if nx.mul_add(nx, ny.mul_add(ny, nz * nz)) <= 1.05 {
                        self.place_block(region, fill_block, x, y, z, chunk_bb);
                    }
                }
            }
        }
    }

    pub fn fill_column_down(
        &self,
        region: &WorldGenRegion<'_>,
        state: BlockState,
        x: i32,
        start_y: i32,
        z: i32,
        chunk_bb: BoundingBox,
    ) {
        let mut pos = self.world_pos(x, start_y, z);
        if !chunk_bb.is_inside(pos) {
            return;
        }
        while is_replaceable_by_structures(region.get_block(pos)) && pos.y() > region.min_y() + 1 {
            region.set_block(pos, state);
            pos = pos.below();
        }
    }

    pub fn create_chest(
        &self,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        random: &mut RandomSource,
        x: i32,
        y: i32,
        z: i32,
    ) -> bool {
        self.create_chest_at(region, chunk_bb, random, self.world_pos(x, y, z), None)
    }

    pub fn create_chest_at(
        &self,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        random: &mut RandomSource,
        pos: BlockPosition,
        state: Option<BlockState>,
    ) -> bool {
        create_chest_at(region, chunk_bb, random, pos, state)
    }

    #[expect(clippy::too_many_arguments, reason = "mirrors vanilla createDispenser")]
    pub fn create_dispenser(
        &self,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        random: &mut RandomSource,
        x: i32,
        y: i32,
        z: i32,
        facing: Direction,
    ) -> bool {
        let pos = self.world_pos(x, y, z);
        if !chunk_bb.is_inside(pos) || region.get_block(pos).is(Block::DISPENSER) {
            return false;
        }
        let state = Block::DISPENSER
            .default_state()
            .with(Properties::FACING, facing);
        self.place_block(region, state, x, y, z, chunk_bb);
        random.next_i64();
        true
    }

    pub const fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        self.bounding_box = self.bounding_box.moved(dx, dy, dz);
    }
}

pub fn create_chest_at(
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    random: &mut RandomSource,
    pos: BlockPosition,
    state: Option<BlockState>,
) -> bool {
    if !chunk_bb.is_inside(pos) || region.get_block(pos).is(Block::CHEST) {
        return false;
    }
    let state = state.unwrap_or_else(|| reorient(region, pos, Block::CHEST.default_state()));
    region.set_block(pos, state);
    random.next_i64();
    true
}

pub fn is_replaceable_by_structures(state: BlockState) -> bool {
    let block = state.as_block();
    state.is_air()
        || state.has_fluid()
        || block == Block::GLOW_LICHEN
        || block == Block::SEAGRASS
        || block == Block::TALL_SEAGRASS
}

pub fn reorient(region: &WorldGenRegion<'_>, pos: BlockPosition, state: BlockState) -> BlockState {
    let mut solid_neighbor = None;

    for direction in Direction::HORIZONTAL {
        let neighbor = region.get_block(pos.relative(direction));
        if neighbor.is(Block::CHEST) {
            return state;
        }
        if neighbor.is_solid_render() {
            if solid_neighbor.is_some() {
                solid_neighbor = None;
                break;
            }
            solid_neighbor = Some(direction);
        }
    }

    if let Some(direction) = solid_neighbor {
        return state.with(Properties::HORIZONTAL_FACING, direction.opposite());
    }

    let mut lock_dir = state.try_get(Properties::HORIZONTAL_FACING);
    if region.get_block(pos.relative(lock_dir)).is_solid_render() {
        lock_dir = lock_dir.opposite();
    }
    if region.get_block(pos.relative(lock_dir)).is_solid_render() {
        lock_dir = lock_dir.rotate_y_clockwise();
    }
    if region.get_block(pos.relative(lock_dir)).is_solid_render() {
        lock_dir = lock_dir.opposite();
    }
    state.with(Properties::HORIZONTAL_FACING, lock_dir)
}

#[derive(Debug)]
pub struct PoolElementStructurePiece {
    pub element: &'static StructurePoolElement,
    pub position: BlockPosition,
    pub ground_level_delta: i32,
    pub rotation: Rotation,
    pub liquid_settings: LiquidSettings,
    pub junctions: Vec<JigsawJunction>,
}

impl PoolElementStructurePiece {
    pub const fn new(
        element: &'static StructurePoolElement,
        position: BlockPosition,
        ground_level_delta: i32,
        rotation: Rotation,
        liquid_settings: LiquidSettings,
    ) -> Self {
        Self {
            element,
            position,
            ground_level_delta,
            rotation,
            liquid_settings,
            junctions: Vec::new(),
        }
    }

    pub fn add_junction(&mut self, junction: JigsawJunction) {
        self.junctions.push(junction);
    }
}

#[derive(Debug)]
pub enum PieceKind {
    PoolElement(PoolElementStructurePiece),
    SwampHut(SwampHutPiece),
    DesertPyramid(DesertPyramidPiece),
    Igloo(IglooPiece),
    JungleTemple(JungleTemplePiece),
    BuriedTreasure(BuriedTreasurePiece),
    Shipwreck(ShipwreckPiece),
    OceanRuin(OceanRuinPiece),
    RuinedPortal(RuinedPortalPiece),
    Mineshaft(MineShaftPiece),
    NetherFossil(NetherFossilPiece),
    EndCity(EndCityPiece),
    WoodlandMansion(WoodlandMansionPiece),
    NetherBridge(NetherBridgePiece),
    Stronghold(StrongholdPiece),
    OceanMonument(MonumentBuilding),
}

#[derive(Debug)]
pub struct StructurePiece {
    pub base: PieceBase,
    pub kind: PieceKind,
}

impl StructurePiece {
    pub const fn new(base: PieceBase, kind: PieceKind) -> Self {
        Self { base, kind }
    }

    pub const fn pool_element(piece: PoolElementStructurePiece, bounding_box: BoundingBox) -> Self {
        Self::new(
            PieceBase::new(0, bounding_box),
            PieceKind::PoolElement(piece),
        )
    }

    pub const fn bounding_box(&self) -> BoundingBox {
        self.base.bounding_box
    }

    pub const fn as_pool_element(&self) -> &PoolElementStructurePiece {
        match &self.kind {
            PieceKind::PoolElement(piece) => piece,
            _ => panic!("not a pool element piece"),
        }
    }

    pub const fn as_pool_element_mut(&mut self) -> &mut PoolElementStructurePiece {
        match &mut self.kind {
            PieceKind::PoolElement(piece) => piece,
            _ => panic!("not a pool element piece"),
        }
    }

    pub fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        self.base.move_by(dx, dy, dz);
        match &mut self.kind {
            PieceKind::PoolElement(piece) => {
                piece.position = piece.position.offset(dx, dy, dz);
            }
            PieceKind::Igloo(piece) => piece.template.move_by(dx, dy, dz),
            PieceKind::Shipwreck(piece) => piece.template.move_by(dx, dy, dz),
            PieceKind::OceanRuin(piece) => piece.template.move_by(dx, dy, dz),
            PieceKind::RuinedPortal(piece) => piece.template.move_by(dx, dy, dz),
            PieceKind::Mineshaft(piece) => piece.move_by(dx, dy, dz),
            PieceKind::NetherFossil(piece) => piece.template.move_by(dx, dy, dz),
            PieceKind::EndCity(piece) => piece.template.move_by(dx, dy, dz),
            PieceKind::WoodlandMansion(piece) => piece.template.move_by(dx, dy, dz),
            PieceKind::SwampHut(_)
            | PieceKind::DesertPyramid(_)
            | PieceKind::JungleTemple(_)
            | PieceKind::BuriedTreasure(_)
            | PieceKind::NetherBridge(_)
            | PieceKind::Stronghold(_)
            | PieceKind::OceanMonument(_) => {}
        }
    }

    pub fn post_process(
        &mut self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
        reference_pos: BlockPosition,
    ) {
        let Self { base, kind } = self;
        match kind {
            PieceKind::PoolElement(piece) => {
                piece.element.place(
                    region,
                    piece.position,
                    reference_pos,
                    piece.rotation,
                    chunk_bb,
                    random,
                    piece.liquid_settings,
                    false,
                );
            }
            PieceKind::SwampHut(piece) => piece.post_process(base, region, chunk_bb),
            PieceKind::DesertPyramid(piece) => {
                piece.post_process(base, region, random, chunk_bb);
            }
            PieceKind::Igloo(piece) => {
                piece.post_process(base, region, random, chunk_bb, reference_pos);
            }
            PieceKind::JungleTemple(piece) => {
                piece.post_process(base, region, random, chunk_bb);
            }
            PieceKind::BuriedTreasure(_) => {
                BuriedTreasurePiece::post_process(base, region, random, chunk_bb);
            }
            PieceKind::Shipwreck(piece) => {
                piece.post_process(base, region, random, chunk_bb, reference_pos);
            }
            PieceKind::OceanRuin(piece) => {
                piece.post_process(base, region, random, chunk_bb, reference_pos);
            }
            PieceKind::RuinedPortal(piece) => {
                piece.post_process(base, region, random, chunk_bb, reference_pos);
            }
            PieceKind::Mineshaft(piece) => {
                piece.post_process(base, region, random, chunk_bb);
            }
            PieceKind::NetherFossil(piece) => {
                piece.post_process(base, region, random, chunk_bb, reference_pos);
            }
            PieceKind::EndCity(piece) => {
                piece.post_process(base, region, random, chunk_bb, reference_pos);
            }
            PieceKind::WoodlandMansion(piece) => {
                piece.post_process(base, region, random, chunk_bb, reference_pos);
            }
            PieceKind::NetherBridge(piece) => {
                piece.post_process(base, region, random, chunk_bb);
            }
            PieceKind::Stronghold(piece) => {
                piece.post_process(base, region, random, chunk_bb);
            }
            PieceKind::OceanMonument(piece) => {
                piece.post_process(base, region, random, chunk_bb);
            }
        }
    }
}

pub fn create_bounding_box<'a>(
    pieces: impl IntoIterator<Item = &'a StructurePiece>,
) -> Option<BoundingBox> {
    pieces
        .into_iter()
        .map(StructurePiece::bounding_box)
        .reduce(BoundingBox::encapsulate_box)
}
