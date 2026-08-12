//! Toggleable debug overlay for the subtile collision grid — draws every
//! nearby subtile's walkable/blocked state as a small diamond, and spawns a
//! stationary "buddy" marker in the other half of the partial-wall test tile
//! to demonstrate two occupants sharing what used to be a single tile.
//!
//! Off by default; press `g` at any time to toggle the overlay on/off.
//!
//! Env vars:
//! - `SUBTILE_DEBUG` = set (to anything) to start the overlay already on.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::character::Player;
use crate::iso::{IsoGrid, tile_to_world};
use crate::subtile::{SUBTILES_PER_TILE, SUBTILE_SIZE, SubtileGrid, partial_wall_open_centers};

#[derive(Component)]
struct SubtileDebugText;

#[derive(Resource)]
struct SubtileDebugState {
    enabled: bool,
}

impl Default for SubtileDebugState {
    fn default() -> Self {
        Self {
            enabled: std::env::var("SUBTILE_DEBUG").is_ok(),
        }
    }
}

pub struct SubtileDebugPlugin;

impl Plugin for SubtileDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SubtileDebugState>()
            .add_systems(Startup, spawn_debug_text)
            .add_systems(
                Update,
                (
                    toggle_debug_on_keypress,
                    draw_subtile_grid_gizmos,
                    spawn_debug_buddy,
                    update_debug_text,
                ),
            );
    }
}

/// Pressing `g` flips the overlay on/off, every run, regardless of whether
/// `SUBTILE_DEBUG` was set at startup.
fn toggle_debug_on_keypress(keyboard: Res<ButtonInput<KeyCode>>, mut state: ResMut<SubtileDebugState>) {
    if keyboard.just_pressed(KeyCode::KeyG) {
        state.enabled = !state.enabled;
    }
}

/// Spawns a stationary marker in the *other* subtile `PARTIAL_WALL_TILE`
/// leaves open (see `subtile.rs`), once the real map has loaded (so the
/// marker lands at the correct world position instead of `IsoGrid`'s
/// pre-load default). Runs once, gated by `spawned`.
fn spawn_debug_buddy(mut commands: Commands, grid: Res<IsoGrid>, state: Res<SubtileDebugState>, mut spawned: Local<bool>) {
    if *spawned || !state.enabled || grid.y_sort_extent == 1.0 {
        return;
    }
    *spawned = true;

    let center = partial_wall_open_centers()[1];
    let world = tile_to_world(center, &grid.grid, grid.offset);
    commands.spawn((
        Sprite::from_color(Color::srgb(1.0, 0.6, 0.0), Vec2::new(20.0, 28.0)),
        Anchor::BOTTOM_CENTER,
        Transform::from_xyz(world.x, world.y, 10.0),
    ));
}

/// Draws every subtile within a few tiles of the player as a small diamond,
/// colored red (blocked) or green (walkable), so the logical grid this
/// prototype enforces can be checked against what's actually drawn.
fn draw_subtile_grid_gizmos(
    mut gizmos: Gizmos,
    subtiles: Res<SubtileGrid>,
    grid: Res<IsoGrid>,
    player: Query<&Player>,
    state: Res<SubtileDebugState>,
) {
    if !state.enabled {
        return;
    }
    let Ok(player) = player.single() else {
        return;
    };

    let center_tx = player.tile_pos.x.floor() as i32;
    let center_ty = player.tile_pos.y.floor() as i32;
    let radius_tiles = 5;

    for ty in (center_ty - radius_tiles)..(center_ty + radius_tiles) {
        for tx in (center_tx - radius_tiles)..(center_tx + radius_tiles) {
            for (lx, ly) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let coord = (tx * SUBTILES_PER_TILE + lx, ty * SUBTILES_PER_TILE + ly);
                let center = Vec2::new(
                    tx as f32 + (lx as f32 + 0.5) * SUBTILE_SIZE,
                    ty as f32 + (ly as f32 + 0.5) * SUBTILE_SIZE,
                );
                let c = tile_to_world(center, &grid.grid, grid.offset);
                let color = if subtiles.is_coord_blocked(coord) {
                    Color::srgba(1.0, 0.15, 0.15, 0.9)
                } else {
                    Color::srgba(0.2, 1.0, 0.3, 0.4)
                };
                gizmos.linestrip_2d(
                    [
                        c + Vec2::new(-16.0, 0.0),
                        c + Vec2::new(0.0, 8.0),
                        c + Vec2::new(16.0, 0.0),
                        c + Vec2::new(0.0, -8.0),
                        c + Vec2::new(-16.0, 0.0),
                    ],
                    color,
                );
            }
        }
    }
}

fn spawn_debug_text(mut commands: Commands) {
    commands.spawn((
        SubtileDebugText,
        Text::new("subtile debug: waiting for map..."),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        TextBackgroundColor(Color::BLACK.with_alpha(0.6)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(8.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn update_debug_text(
    subtiles: Res<SubtileGrid>,
    player: Query<&Player>,
    state: Res<SubtileDebugState>,
    mut text: Query<(&mut Text, &mut Visibility), With<SubtileDebugText>>,
) {
    let Ok((mut text, mut visibility)) = text.single_mut() else {
        return;
    };

    if !state.enabled {
        *visibility = Visibility::Hidden;
        return;
    }
    *visibility = Visibility::Visible;

    let Ok(player) = player.single() else {
        return;
    };
    let coord = crate::subtile::subtile_coord(player.tile_pos);
    text.0 = format!(
        "tile=({:.2},{:.2}) subtile={:?} blocked={}",
        player.tile_pos.x,
        player.tile_pos.y,
        coord,
        subtiles.is_coord_blocked(coord)
    );
}
