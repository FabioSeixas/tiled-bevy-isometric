//! Fractional-position isometric projection, mirroring the diamond-grid math
//! `bevy_ecs_tilemap` uses internally to place tiles (see
//! `bevy_ecs_tilemap::helpers::square_grid::diamond::DiamondPos::project`),
//! but usable with continuous (non-integer) tile coordinates for smooth
//! character movement.

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

/// Grid size and anchor offset of the currently loaded map. Defaults to
/// `assets/map.tmx`'s grid size with no offset, so the character can be
/// positioned before the map asset finishes loading; both fields are
/// overwritten once `TiledEvent<MapCreated>` fires.
///
/// The offset makes `tile_to_world` agree with `bevy_ecs_tiled`'s own
/// `tile_relative_position` (used for collision polygons in `collision.rs`):
/// it's the world position of tile (0,0) under our map's `TilemapAnchor`,
/// i.e. exactly the correction the raw diamond-grid formula is missing.
#[derive(Resource)]
pub struct IsoGrid {
    pub grid: TilemapGridSize,
    pub offset: Vec2,
}

impl Default for IsoGrid {
    fn default() -> Self {
        Self {
            grid: TilemapGridSize { x: 64.0, y: 32.0 },
            offset: Vec2::ZERO,
        }
    }
}

pub struct IsoPlugin;

impl Plugin for IsoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IsoGrid>()
            .add_systems(Update, update_iso_grid);
    }
}

fn update_iso_grid(
    mut events: MessageReader<TiledEvent<MapCreated>>,
    map_assets: Res<Assets<TiledMapAsset>>,
    mut grid: ResMut<IsoGrid>,
) {
    for event in events.read() {
        let Some(map_asset) = event.get_map_asset(&map_assets) else {
            continue;
        };
        grid.grid = grid_size_from_map(&map_asset.map);
        grid.offset = map_asset.tile_relative_position(
            &TilePos { x: 0, y: 0 },
            &map_asset.largest_tile_size,
            &crate::collision::MAP_ANCHOR,
        );
    }
}

/// Projects a fractional tile-space coordinate into world space.
pub fn tile_to_world(tile: Vec2, grid: &TilemapGridSize, offset: Vec2) -> Vec2 {
    Vec2::new(
        grid.x * 0.5 * (tile.x + tile.y),
        grid.y * 0.5 * (tile.y - tile.x),
    ) + offset
}
