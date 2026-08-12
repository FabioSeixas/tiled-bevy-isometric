//! Subtile collision/movement grid: each ground tile (64x32 on the isometric
//! grid) is logically subdivided into a 2x2 grid of subtiles, each a plain
//! walkable/blocked boolean — the *only* grid movement resolves against (see
//! the design brief this prototype validates: subtile collision replaces
//! whole-tile collision for movement purposes).
//!
//! The grid is auto-derived once, at map load, by sampling each subtile's
//! center against the existing hand-drawn Tile Collision Editor polygons
//! (`collision.rs`) — a tile whose polygon covers its full footprint ends up
//! with all 4 subtiles blocked, matching the old whole-tile behavior exactly
//! unless overridden below. `PARTIAL_WALL_TILE` and `CORNER_CUT_TILE` then
//! hand-override a couple of specific tiles to demonstrate a partial wall and
//! diagonal corner-cutting — hand-authored test data, not real per-tile
//! partial-collision authoring (out of scope for this prototype).

use std::collections::HashSet;

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::collision::{TileCollisionPolygons, is_point_blocked};
use crate::iso::{IsoGrid, tile_to_world};

/// Each tile is split into this many subtiles per axis.
pub const SUBTILES_PER_TILE: i32 = 2;
pub(crate) const SUBTILE_SIZE: f32 = 1.0 / SUBTILES_PER_TILE as f32;

/// An existing full-height wall tile (see `assets/map.tmx`, GID 92) whose art
/// is a full wall block but whose logical collision is overridden here to
/// only block its far/back row of subtiles (`local_y == 1`) — the near/front
/// row stays open. This is the concrete shape the captain asked for: half
/// the tile blocked, half open, not some smaller inset.
const PARTIAL_WALL_TILE: (i32, i32) = (8, 12);
const PARTIAL_WALL_OPEN_LOCAL: [(i32, i32); 2] = [(0, 0), (1, 0)];

/// A hand-authored checkerboard tile on open ground — two diagonally-opposite
/// subtiles blocked, the other two left open — purely to demonstrate
/// diagonal corner-cutting: moving between the two open corners must succeed
/// even though both subtiles flanking that diagonal are blocked.
const CORNER_CUT_TILE: (i32, i32) = (7, 12);
const CORNER_CUT_BLOCKED_LOCAL: [(i32, i32); 2] = [(1, 0), (0, 1)];

/// Per-subtile walkable/blocked flags, keyed by `(tx * SUBTILES_PER_TILE +
/// local_x, ty * SUBTILES_PER_TILE + local_y)`. Absent from the set = walkable
/// (the common case).
#[derive(Resource, Default)]
pub struct SubtileGrid {
    blocked: HashSet<(i32, i32)>,
}

impl SubtileGrid {
    /// Whether the subtile containing this fractional tile-space position is
    /// blocked.
    pub fn is_blocked(&self, tile_pos: Vec2) -> bool {
        self.blocked.contains(&subtile_coord(tile_pos))
    }

    pub fn is_coord_blocked(&self, coord: (i32, i32)) -> bool {
        self.blocked.contains(&coord)
    }
}

/// Which subtile a fractional tile-space position falls in.
pub fn subtile_coord(tile_pos: Vec2) -> (i32, i32) {
    (
        (tile_pos.x * SUBTILES_PER_TILE as f32).floor() as i32,
        (tile_pos.y * SUBTILES_PER_TILE as f32).floor() as i32,
    )
}

/// Tile-space centers of the two subtiles `PARTIAL_WALL_TILE` leaves open —
/// used to place a demo character/marker in each, showing two occupants
/// sharing what used to be a single tile.
pub fn partial_wall_open_centers() -> [Vec2; 2] {
    let (tx, ty) = PARTIAL_WALL_TILE;
    PARTIAL_WALL_OPEN_LOCAL.map(|(lx, ly)| {
        Vec2::new(
            tx as f32 + (lx as f32 + 0.5) * SUBTILE_SIZE,
            ty as f32 + (ly as f32 + 0.5) * SUBTILE_SIZE,
        )
    })
}

/// Tile-space centers of the two open (diagonal) corners of `CORNER_CUT_TILE`,
/// for driving a deterministic diagonal-move probe between them.
pub fn corner_cut_open_centers() -> [Vec2; 2] {
    let (tx, ty) = CORNER_CUT_TILE;
    [(0, 0), (1, 1)].map(|(lx, ly)| {
        Vec2::new(
            tx as f32 + (lx as f32 + 0.5) * SUBTILE_SIZE,
            ty as f32 + (ly as f32 + 0.5) * SUBTILE_SIZE,
        )
    })
}

pub struct SubtilePlugin;

impl Plugin for SubtilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SubtileGrid>().add_systems(
            Update,
            build_subtile_grid
                .after(crate::collision::build_collision_polygons)
                .after(crate::iso::update_iso_grid),
        );
    }
}

fn build_subtile_grid(
    mut events: MessageReader<TiledEvent<MapCreated>>,
    map_assets: Res<Assets<TiledMapAsset>>,
    grid: Res<IsoGrid>,
    polygons: Res<TileCollisionPolygons>,
    mut subtiles: ResMut<SubtileGrid>,
) {
    for event in events.read() {
        let Some(map_asset) = event.get_map_asset(&map_assets) else {
            continue;
        };

        let width = map_asset.map.width as i32;
        let height = map_asset.map.height as i32;

        let mut blocked = HashSet::new();
        for ty in 0..height {
            for tx in 0..width {
                for (lx, ly) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let center = Vec2::new(
                        tx as f32 + (lx as f32 + 0.5) * SUBTILE_SIZE,
                        ty as f32 + (ly as f32 + 0.5) * SUBTILE_SIZE,
                    );
                    let world = tile_to_world(center, &grid.grid, grid.offset);
                    if is_point_blocked(world, &polygons) {
                        blocked.insert((tx * SUBTILES_PER_TILE + lx, ty * SUBTILES_PER_TILE + ly));
                    }
                }
            }
        }

        let (px, py) = PARTIAL_WALL_TILE;
        for (lx, ly) in PARTIAL_WALL_OPEN_LOCAL {
            blocked.remove(&(px * SUBTILES_PER_TILE + lx, py * SUBTILES_PER_TILE + ly));
        }
        let (cx, cy) = CORNER_CUT_TILE;
        for (lx, ly) in CORNER_CUT_BLOCKED_LOCAL {
            blocked.insert((cx * SUBTILES_PER_TILE + lx, cy * SUBTILES_PER_TILE + ly));
        }

        info!("Built subtile grid: {} blocked subtile(s)", blocked.len());
        subtiles.blocked = blocked;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtile_coord_splits_each_tile_into_four() {
        assert_eq!(subtile_coord(Vec2::new(5.0, 7.0)), (10, 14));
        assert_eq!(subtile_coord(Vec2::new(5.49, 7.49)), (10, 14));
        assert_eq!(subtile_coord(Vec2::new(5.5, 7.0)), (11, 14));
        assert_eq!(subtile_coord(Vec2::new(5.0, 7.5)), (10, 15));
        assert_eq!(subtile_coord(Vec2::new(5.99, 7.99)), (11, 15));
    }

    #[test]
    fn negative_tile_positions_floor_towards_negative_infinity() {
        // Off this map, but the math must still behave like a normal grid
        // (no rounding-towards-zero surprise at the origin).
        assert_eq!(subtile_coord(Vec2::new(-0.1, -0.1)), (-1, -1));
    }

    #[test]
    fn corner_cut_open_centers_are_the_two_unblocked_diagonal_corners() {
        let centers = corner_cut_open_centers();
        let (tx, ty) = CORNER_CUT_TILE;
        assert_eq!(subtile_coord(centers[0]), (tx * 2, ty * 2));
        assert_eq!(subtile_coord(centers[1]), (tx * 2 + 1, ty * 2 + 1));
        for (lx, ly) in CORNER_CUT_BLOCKED_LOCAL {
            let blocked_coord = (tx * SUBTILES_PER_TILE + lx, ty * SUBTILES_PER_TILE + ly);
            assert_ne!(subtile_coord(centers[0]), blocked_coord);
            assert_ne!(subtile_coord(centers[1]), blocked_coord);
        }
    }
}
