//! Hand-built collision, sourced directly from Tiled's Tile Collision Editor data.
//!
//! No physics engine is used here on purpose (see AGENTS.md). We read each placed
//! tile's `<objectgroup>` polygon straight from the loaded `tiled::Map` (exposed by
//! `bevy_ecs_tiled` without enabling its optional `physics` feature), convert every
//! polygon to world-space once when the map finishes loading, and then do a plain
//! point-in-polygon test against the character's feet position every frame.

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

/// World-space collision polygons, one per placed tile that has a Tile Collision
/// Editor shape. Populated once from `TiledEvent<MapCreated>`.
#[derive(Resource, Default)]
pub struct TileCollisionPolygons(pub Vec<Vec<Vec2>>);

pub struct CollisionPlugin;

impl Plugin for CollisionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileCollisionPolygons>()
            .add_systems(Update, build_collision_polygons);
    }
}

/// Anchor used both for spawning the map and for all our own world-space math.
/// Must match the `TilemapAnchor` inserted alongside `TiledMap` in `main.rs`.
pub const MAP_ANCHOR: TilemapAnchor = TilemapAnchor::Center;

fn build_collision_polygons(
    mut events: MessageReader<TiledEvent<MapCreated>>,
    map_assets: Res<Assets<TiledMapAsset>>,
    mut polygons: ResMut<TileCollisionPolygons>,
) {
    for event in events.read() {
        let Some(map_asset) = event.get_map_asset(&map_assets) else {
            continue;
        };

        let mut out = Vec::new();
        for layer in map_asset.map.layers() {
            let tiled::LayerType::Tiles(tile_layer) = layer.layer_type() else {
                continue;
            };
            map_asset.for_each_tile(&tile_layer, |layer_tile, _data, tile_pos, _pos| {
                let Some(tile) = layer_tile.get_tile() else {
                    return;
                };
                let Some(collision) = &tile.collision else {
                    return;
                };

                let size = tile_size(&tile);
                let center = map_asset.tile_relative_position(&tile_pos, &size, &MAP_ANCHOR);
                let bbox_min = Vec2::new(center.x - size.x / 2.0, center.y - size.y / 2.0);

                for object in collision.object_data() {
                    let tiled::ObjectShape::Polygon { points } = &object.shape else {
                        continue;
                    };
                    let world_points = points
                        .iter()
                        .map(|(px, py)| {
                            Vec2::new(
                                bbox_min.x + object.x + px,
                                bbox_min.y + size.y - (object.y + py),
                            )
                        })
                        .collect();
                    out.push(world_points);
                }
            });
        }

        info!("Built {} tile collision polygon(s) from Tiled data", out.len());
        polygons.0 = out;
    }
}

/// Standard ray-casting point-in-polygon test.
fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut inside = false;
    let n = polygon.len();
    for i in 0..n {
        let a = polygon[i];
        let b = polygon[(i + 1) % n];
        let crosses_y = (a.y > point.y) != (b.y > point.y);
        if crosses_y {
            let x_at_y = a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if point.x < x_at_y {
                inside = !inside;
            }
        }
    }
    inside
}

/// Whether `point` (in world space) falls inside any hand-drawn collision shape.
pub fn is_point_blocked(point: Vec2, polygons: &TileCollisionPolygons) -> bool {
    polygons.0.iter().any(|poly| point_in_polygon(point, poly))
}
