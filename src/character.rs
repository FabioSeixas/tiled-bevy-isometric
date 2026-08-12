use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::collision::{TileCollisionPolygons, is_point_blocked};
use crate::iso::{IsoGrid, tile_to_world};

/// The controllable character. Position is tracked in fractional tile-space
/// (not world pixels) since that's the natural unit for isometric movement
/// input; `sync_player_transform` projects it to world space every frame.
#[derive(Component)]
pub struct Player {
    pub tile_pos: Vec2,
}

const MOVE_SPEED_TILES_PER_SEC: f32 = 3.0;
const SPRITE_FRAME_SIZE: UVec2 = UVec2::new(32, 48);
const SPRITE_COLUMNS: u32 = 4;
const SPRITE_ROWS: u32 = 4;

/// Open ground away from every placed collidable tile, so the character
/// always spawns walkable regardless of which shapes are drawn on the map.
const SPAWN_TILE_POS: Vec2 = Vec2::new(6.0, 14.0);

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player).add_systems(
            Update,
            (move_player, sync_player_transform, follow_player).chain(),
        );
    }
}

fn spawn_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let image = asset_server.load("isometric_char_1.png");
    let layout = layouts.add(TextureAtlasLayout::from_grid(
        SPRITE_FRAME_SIZE,
        SPRITE_COLUMNS,
        SPRITE_ROWS,
        None,
        None,
    ));

    commands.spawn((
        Player {
            tile_pos: SPAWN_TILE_POS,
        },
        Sprite::from_atlas_image(image, TextureAtlas { layout, index: 0 }),
        Anchor::BOTTOM_CENTER,
        Transform::from_xyz(0.0, 0.0, 10.0),
    ));
}

fn move_player(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    grid: Res<IsoGrid>,
    polygons: Res<TileCollisionPolygons>,
    mut query: Query<&mut Player>,
) {
    let Ok(mut player) = query.single_mut() else {
        return;
    };

    let mut input = Vec2::ZERO;
    if keyboard.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        input.y -= 1.0;
    }
    if keyboard.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        input.y += 1.0;
    }
    if keyboard.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        input.x -= 1.0;
    }
    if keyboard.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        input.x += 1.0;
    }
    if input == Vec2::ZERO {
        return;
    }

    let delta = input.normalize() * MOVE_SPEED_TILES_PER_SEC * time.delta_secs();

    // Resolve each axis independently so the character slides along a
    // collision edge instead of stopping dead when only one axis is blocked.
    try_move(&mut player.tile_pos, Vec2::new(delta.x, 0.0), &grid, &polygons);
    try_move(&mut player.tile_pos, Vec2::new(0.0, delta.y), &grid, &polygons);
}

pub(crate) fn try_move(
    tile_pos: &mut Vec2,
    delta: Vec2,
    grid: &IsoGrid,
    polygons: &TileCollisionPolygons,
) {
    let candidate = *tile_pos + delta;
    if !is_point_blocked(tile_to_world(candidate, &grid.grid, grid.offset), polygons) {
        *tile_pos = candidate;
    }
}

fn sync_player_transform(grid: Res<IsoGrid>, mut query: Query<(&Player, &mut Transform)>) {
    for (player, mut transform) in &mut query {
        let world = tile_to_world(player.tile_pos, &grid.grid, grid.offset);
        transform.translation.x = world.x;
        transform.translation.y = world.y;
    }
}

fn follow_player(
    player: Query<&Transform, With<Player>>,
    mut camera: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera.single_mut() else {
        return;
    };
    camera_transform.translation.x = player_transform.translation.x;
    camera_transform.translation.y = player_transform.translation.y;
}
