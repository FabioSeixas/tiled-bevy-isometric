//! Runtime instrumentation for the character/wall z-order (draw order) bug.
//!
//! Disabled by default (no behavior/systems added unless `ZSORT_DEBUG` is set
//! in the environment) so it ships harmlessly but stays available for the
//! next investigation. When enabled it:
//! - samples every non-floor tile's world position at map load, using the
//!   exact same `bevy_ecs_tiled::tile_relative_position` API the collision
//!   system trusts (`world_tiled`), *and* this crate's own `tile_to_world`
//!   (`world_ours`), so any drift between the two is visible instead of
//!   assumed away;
//! - every frame, finds the wall tiles nearest the character and logs (via
//!   `info!`) both entities' world Y and the y_sort Z `bevy_ecs_tilemap`
//!   would compute for each, plus a frame counter for correlating against a
//!   screenshot taken at a known frame;
//! - mirrors the same numbers into an always-on-top UI `Text` node (renders
//!   in a separate phase from the tilemap, so it stays legible regardless of
//!   whatever the z-order bug does to the tilemap/character layer), so a
//!   screenshot shows the numbers next to the visual result.
//!
//! Env vars:
//! - `ZSORT_DEBUG` = set (to anything) to enable every system in this module.

use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::character::Player;
use crate::collision::MAP_ANCHOR;
use crate::iso::{IsoGrid, tile_to_world};

/// One non-floor tile's position, sampled once at map load, in both this
/// crate's own projection and `bevy_ecs_tiled`'s authoritative one.
struct WallTileSample {
    tile_pos: (u32, u32),
    world_tiled: Vec2,
    world_ours: Vec2,
}

#[derive(Resource, Default)]
pub(crate) struct WallTileSamples(Vec<WallTileSample>);

#[derive(Component)]
pub(crate) struct ZsortDebugText;

pub struct ZsortDebugPlugin;

impl Plugin for ZsortDebugPlugin {
    fn build(&self, app: &mut App) {
        // Always present (empty by default) so `zsort_debug_update`, which is
        // unconditionally registered in `main.rs` and self-checks the same
        // env var every frame, always has a valid `Res<WallTileSamples>` to
        // depend on regardless of whether the flag is set.
        app.init_resource::<WallTileSamples>();

        if std::env::var("ZSORT_DEBUG").is_err() {
            return;
        }
        app.add_systems(Startup, spawn_debug_text)
            .add_systems(Update, (build_wall_tile_samples, report_real_tilemap_sort_inputs));

        // Separate flag: the gizmos answer a different question from the
        // numbers (where a tile's sort anchor sits *relative to its own art*),
        // and they draw over the scene, so they must not be on by default when
        // the overlay is.
        if std::env::var("ZSORT_GIZMOS").is_ok() {
            app.add_systems(Update, draw_sort_anchor_gizmos);
        }
    }
}

/// Draws each nearby wall tile's *sort anchor* — the exact world point whose Y
/// becomes that tile's `y_sort` key — as its 64x32 diamond footprint, plus the
/// character's own sort anchor as a cross.
///
/// Every prior investigation compared the character's sort Y against the
/// tiles' sort Y and found them consistent. That check cannot see the failure
/// this draws: whether a tile's sort anchor coincides with where that tile's
/// *art* actually lands on screen. These tiles are 64x64 drawn on a 64x32
/// grid, so the art extends well above its own footprint; if the anchor and
/// the art's visual base disagree, depth can be computed perfectly and still
/// look wrong, because the player judges "in front of" against the art.
fn draw_sort_anchor_gizmos(
    mut gizmos: Gizmos,
    samples: Res<WallTileSamples>,
    player: Query<&Transform, With<Player>>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    let p = player_transform.translation.xy();

    // The character's sort anchor: where `sync_player_transform` places it,
    // i.e. the sprite's feet.
    gizmos.line_2d(p - Vec2::X * 40.0, p + Vec2::X * 40.0, Color::srgb(0.0, 1.0, 0.0));
    gizmos.line_2d(p - Vec2::Y * 24.0, p + Vec2::Y * 24.0, Color::srgb(0.0, 1.0, 0.0));

    for sample in samples.0.iter() {
        let c = sample.world_tiled;
        if c.distance(p) > 220.0 {
            continue;
        }
        // Colour by the depth relation the renderer will actually use, so the
        // predicted ordering is visible per-tile instead of inferred: red =
        // this tile sorts behind the character (character should draw over
        // it), blue = in front (it should draw over the character).
        let color = if c.y > p.y {
            Color::srgb(1.0, 0.0, 0.0)
        } else {
            Color::srgb(0.2, 0.4, 1.0)
        };
        gizmos.linestrip_2d(
            [
                c + Vec2::new(-32.0, 0.0),
                c + Vec2::new(0.0, 16.0),
                c + Vec2::new(32.0, 0.0),
                c + Vec2::new(0.0, -16.0),
                c + Vec2::new(-32.0, 0.0),
            ],
            color,
        );
    }
}

/// Logs, per spawned tilemap entity, the *actual* component values
/// `bevy_ecs_tilemap` will feed into its `y_sort` key — rather than
/// re-deriving them from the map asset, which is what `IsoGrid` (and every
/// prior investigation) does.
///
/// This is the one input to the depth comparison that reading the formula
/// cannot verify: `render/material.rs` divides by `chunk.map_size.y *
/// chunk.tile_size.y` and adds the chunk's *inherited* Z, where `map_size` /
/// `tile_size` are the `TilemapSize` / `TilemapTileSize` components
/// `bevy_ecs_tiled` put on the tilemap entity (`map/spawn.rs` sets
/// `tile_size` from the *tileset's* `tile_width`/`tile_height`, which need not
/// equal the map asset's `largest_tile_size` that `IsoGrid::y_sort_extent`
/// uses), and the Z baseline comes from the parent *layer* entity's transform,
/// not the tilemap entity's own. If either disagrees with what
/// `sync_player_transform` assumes, the character's Z is on a different scale
/// or baseline from the tiles' and no amount of checking the formula itself
/// will show it.
///
/// Emits `ZSORT_INPUTS ... MATCH` / `MISMATCH` per tilemap so the check is a
/// pass/fail assertion, not a table to eyeball.
fn report_real_tilemap_sort_inputs(
    grid: Res<IsoGrid>,
    tilemaps: Query<(
        Entity,
        &TilemapSize,
        &TilemapTileSize,
        &TilemapGridSize,
        &GlobalTransform,
    )>,
    mut done: Local<bool>,
) {
    // Wait until both the tilemaps exist and `IsoGrid` has been overwritten
    // from the loaded map (its default `y_sort_extent` is 1.0).
    if *done || tilemaps.is_empty() || grid.y_sort_extent == 1.0 {
        return;
    }
    *done = true;

    for (entity, size, tile_size, grid_size, global) in &tilemaps {
        let real_extent = size.y as f32 * tile_size.y;
        // The character assumes every wall tile's layer sits at exactly Z=0
        // (see `sync_player_transform`); the renderer instead adds this
        // tilemap's inherited global Z to its y_sort key.
        let real_baseline_z = global.translation().z;
        let extent_matches = (real_extent - grid.y_sort_extent).abs() < 1e-3;
        // Only the topmost layer sits at Z=0, and only that layer's tiles are
        // ever the occluder side of a character/wall comparison. A lower layer
        // reporting a negative baseline is this map's floor doing exactly what
        // it should, not a defect — so don't label it as one.
        let verdict = match (extent_matches, real_baseline_z == 0.0) {
            (true, true) => "OK (topmost layer: character Z is directly comparable)",
            (true, false) => "OK (below topmost layer: always behind the character, by design)",
            (false, _) => "EXTENT MISMATCH: character Z is on a different scale from these tiles",
        };
        info!(
            "ZSORT_INPUTS tilemap={entity} TilemapSize=({},{}) TilemapTileSize=({},{}) \
             TilemapGridSize=({},{}) global_z={:.4} real_extent={:.2} \
             iso_grid_y_sort_extent={:.2} {}",
            size.x,
            size.y,
            tile_size.x,
            tile_size.y,
            grid_size.x,
            grid_size.y,
            real_baseline_z,
            real_extent,
            grid.y_sort_extent,
            verdict,
        );
    }
}

fn spawn_debug_text(mut commands: Commands) {
    commands.spawn((
        ZsortDebugText,
        Text::new("zsort debug: waiting for map..."),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        TextBackgroundColor(Color::BLACK.with_alpha(0.6)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
    ));
}

/// Samples every tile outside the bottom (floor) layer, in both projections,
/// once per `MapCreated` event. Mirrors `for_each_tile`'s use in
/// `collision.rs` exactly, so `tile_pos` here is guaranteed to be the same
/// value (and Y convention) the real renderer assigned that tile — see
/// AGENTS.md "Tiled CSV tile coordinates are Y-flipped from TilePos".
fn build_wall_tile_samples(
    mut events: MessageReader<TiledEvent<MapCreated>>,
    map_assets: Res<Assets<TiledMapAsset>>,
    mut samples: ResMut<WallTileSamples>,
) {
    for event in events.read() {
        let Some(map_asset) = event.get_map_asset(&map_assets) else {
            continue;
        };

        let grid = grid_size_from_map(&map_asset.map);
        let offset = map_asset.tile_relative_position(
            &TilePos { x: 0, y: 0 },
            &map_asset.largest_tile_size,
            &MAP_ANCHOR,
        );

        let mut out = Vec::new();
        for (layer_index, layer) in map_asset.map.layers().enumerate() {
            // Layer 0 is the floor (see assets/map.tmx) and is never the
            // occluder side of a character/wall z-order comparison.
            if layer_index == 0 {
                continue;
            }
            let tiled::LayerType::Tiles(tile_layer) = layer.layer_type() else {
                continue;
            };
            map_asset.for_each_tile(&tile_layer, |_layer_tile, _data, tile_pos, _pos| {
                let world_tiled = map_asset.tile_relative_position(
                    &tile_pos,
                    &map_asset.largest_tile_size,
                    &MAP_ANCHOR,
                );
                let world_ours = tile_to_world(
                    Vec2::new(tile_pos.x as f32, tile_pos.y as f32),
                    &grid,
                    offset,
                );
                out.push(WallTileSample {
                    tile_pos: (tile_pos.x, tile_pos.y),
                    world_tiled,
                    world_ours,
                });
            });
        }

        info!("ZSORT_DEBUG sampled {} non-floor tile(s)", out.len());
        samples.0 = out;
    }
}

/// `1.0 - world_y / y_sort_extent`, `bevy_ecs_tilemap`'s exact `y_sort` key
/// (see `render/material.rs` in that crate and `character.rs`'s
/// `sync_player_transform`), assuming the tile's layer lands at the same
/// Z=0 baseline the character assumes (true for the last-spawned layer,
/// which is every wall tile in this map — see AGENTS.md).
fn predicted_z(world_y: f32, extent: f32) -> f32 {
    1.0 - world_y / extent
}

pub(crate) fn zsort_debug_update(
    grid: Res<IsoGrid>,
    samples: Res<WallTileSamples>,
    player: Query<(&Player, &Transform)>,
    mut text: Query<&mut Text, With<ZsortDebugText>>,
    mut frame: Local<u32>,
) {
    if std::env::var("ZSORT_DEBUG").is_err() {
        return;
    }
    *frame += 1;

    let Ok((player, player_transform)) = player.single() else {
        return;
    };
    let Ok(mut text) = text.single_mut() else {
        return;
    };

    if samples.0.is_empty() {
        return;
    }

    let p = player_transform.translation;
    let mut nearest: Vec<&WallTileSample> = samples.0.iter().collect();
    nearest.sort_by(|a, b| {
        a.world_ours
            .distance_squared(p.xy())
            .partial_cmp(&b.world_ours.distance_squared(p.xy()))
            .unwrap()
    });

    let mut lines = vec![format!(
        "frame={} player tile=({:.2},{:.2}) world=({:.2},{:.2}) z={:.5}",
        *frame, player.tile_pos.x, player.tile_pos.y, p.x, p.y, p.z
    )];

    for sample in nearest.iter().take(3) {
        let z_ours = predicted_z(sample.world_ours.y, grid.y_sort_extent);
        let z_tiled = predicted_z(sample.world_tiled.y, grid.y_sort_extent);
        // `player_in_front` is *predicted from the same formula* as `z_ours`
        // (both are monotonic in world Y against the same extent), so it can
        // never disagree with `p.z > z_ours` on its own — that comparison
        // would be tautological, not a check. The only real check is
        // comparing this predicted ordering against what the screenshot
        // taken this same frame actually shows on screen.
        let player_in_front = p.y < sample.world_ours.y;
        lines.push(format!(
            "wall tile=({},{}) world_ours=({:.2},{:.2}) z_ours={:.5} world_tiled=({:.2},{:.2}) z_tiled={:.5} predicted_player_in_front={}",
            sample.tile_pos.0,
            sample.tile_pos.1,
            sample.world_ours.x,
            sample.world_ours.y,
            z_ours,
            sample.world_tiled.x,
            sample.world_tiled.y,
            z_tiled,
            player_in_front,
        ));
    }

    let joined = lines.join("\n");
    info!("ZSORT_DEBUG {}", joined.replace('\n', " | "));
    text.0 = joined;
}
