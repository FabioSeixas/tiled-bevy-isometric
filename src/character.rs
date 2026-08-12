use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::collision::{TileCollisionPolygons, is_point_blocked};
use crate::iso::{IsoGrid, screen_dir_to_tile_dir, tile_to_world};

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
/// Seconds per walk-cycle frame while moving (~8 frames/sec — readable at
/// this sprite's small on-screen size without looking frantic).
const WALK_FRAME_SECS: f32 = 0.12;

/// Open ground away from every placed collidable tile, so the character
/// always spawns walkable regardless of which shapes are drawn on the map.
const SPAWN_TILE_POS: Vec2 = Vec2::new(6.0, 14.0);

/// Which row of `isometric_char_1.png`'s 4x4 atlas faces which screen
/// direction. Confirmed by inspecting the sheet (see `assets/isometric_char_1.aseprite`,
/// which has no frame-tag metadata to go by — it's a single flat frame):
/// row 0 is a side profile facing screen-right, row 2 the same profile
/// mirrored, facing screen-left — confirmed pixel-exactly by checking that
/// the head's skin-tone pixels sit right-of-center in row 0's hair silhouette
/// and left-of-center in row 2's (the two look deceptively similar to the
/// naked eye at this sprite's small size). Rows 1 and 3 both show mostly the
/// back of the head with only a sliver of face, consistent with facing away
/// from the camera; row 1's sliver leans right and row 3's leans left, and
/// row 3 shows roughly twice row 1's visible-face pixel count, so row 1 is
/// "facing up" (walking away, almost no face) and row 3 is "facing down"
/// (walking toward camera, marginally more face) — the closest approximation
/// available, since no frame in this sheet is a true front-on toward-camera
/// pose. Each row's 4 columns are a walk cycle; column 0 doubles as the idle
/// frame in every row.
#[derive(Clone, Copy, PartialEq, Default)]
enum FacingDirection {
    Right,
    Up,
    Left,
    #[default]
    Down,
}

impl FacingDirection {
    fn row(self) -> u32 {
        match self {
            FacingDirection::Right => 0,
            FacingDirection::Up => 1,
            FacingDirection::Left => 2,
            FacingDirection::Down => 3,
        }
    }

    /// Picks a facing direction from screen-space movement input, using
    /// whichever axis has the larger magnitude (so a diagonal key combo like
    /// W+D still resolves to a single unambiguous facing).
    fn from_screen_input(input: Vec2) -> Self {
        if input.x.abs() >= input.y.abs() {
            if input.x >= 0.0 {
                FacingDirection::Right
            } else {
                FacingDirection::Left
            }
        } else if input.y > 0.0 {
            FacingDirection::Up
        } else {
            FacingDirection::Down
        }
    }
}

/// Walk-cycle animation state, driven by `move_player` (which sets `facing`
/// and `moving`) and applied to the sprite's atlas index by `animate_player`.
#[derive(Component, Default)]
struct PlayerAnimation {
    facing: FacingDirection,
    moving: bool,
    frame: u32,
    timer: Timer,
}

pub struct CharacterPlugin;

impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player).add_systems(
            Update,
            (
                move_player,
                animate_player,
                sync_player_transform,
                follow_player,
            )
                .chain(),
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
        PlayerAnimation {
            timer: Timer::from_seconds(WALK_FRAME_SECS, TimerMode::Repeating),
            ..default()
        },
        Sprite::from_atlas_image(
            image,
            TextureAtlas {
                layout,
                index: (FacingDirection::default().row() * SPRITE_COLUMNS) as usize,
            },
        ),
        Anchor::BOTTOM_CENTER,
        // Z is overwritten every frame by `sync_player_transform` to match
        // the tilemap's `y_sort`; this placeholder never renders as-is.
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

fn move_player(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    grid: Res<IsoGrid>,
    polygons: Res<TileCollisionPolygons>,
    mut query: Query<(&mut Player, &mut PlayerAnimation)>,
) {
    let Ok((mut player, mut anim)) = query.single_mut() else {
        return;
    };

    // Screen-space intent: Up/Down/Left/Right as they'd look on screen, not
    // as tile-space axes (which are diagonal on screen — see `iso.rs`).
    let mut screen_input = Vec2::ZERO;
    if keyboard.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        screen_input.y += 1.0;
    }
    if keyboard.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        screen_input.y -= 1.0;
    }
    if keyboard.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        screen_input.x -= 1.0;
    }
    if keyboard.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        screen_input.x += 1.0;
    }
    if screen_input == Vec2::ZERO {
        anim.moving = false;
        return;
    }
    anim.moving = true;

    let tile_dir = screen_dir_to_tile_dir(screen_input, &grid.grid);
    // Same overall tile-space speed as before: `tile_dir` is scaled so its
    // length matches a full step, then that same scale is applied below to
    // each screen-axis sub-step (screen_dir_to_tile_dir is linear, so the two
    // sub-steps sum to exactly this vector when both succeed).
    let scale = MOVE_SPEED_TILES_PER_SEC * time.delta_secs() / tile_dir.length();

    // Resolve each *screen*-space axis independently, not each tile-space
    // axis — tile-space axes are diagonal on screen (see `iso.rs`), so
    // splitting on them let a single cardinal key (e.g. only "Up") produce a
    // tile-space sub-step with a nonzero sideways component, which could
    // slide the character sideways when only part of it was blocked even
    // though no sideways key was ever pressed. Splitting on screen axes means
    // a step only ever moves along a direction the player actually pressed.
    let step_x = screen_dir_to_tile_dir(Vec2::new(screen_input.x, 0.0), &grid.grid) * scale;
    let step_y = screen_dir_to_tile_dir(Vec2::new(0.0, screen_input.y), &grid.grid) * scale;

    let moved_x = try_move(&mut player.tile_pos, step_x, &grid, &polygons);
    let moved_y = try_move(&mut player.tile_pos, step_y, &grid, &polygons);

    // Face whichever screen-axis intent actually resulted in movement, not
    // just the raw key intent — otherwise a diagonal press that's fully
    // blocked on one axis shows a facing that disagrees with the direction
    // the character actually slid.
    let effective_screen = Vec2::new(
        if moved_x { screen_input.x } else { 0.0 },
        if moved_y { screen_input.y } else { 0.0 },
    );
    if effective_screen != Vec2::ZERO {
        anim.facing = FacingDirection::from_screen_input(effective_screen);
    }
}

/// Cycles the walk-cycle column while `moving` (set by `move_player`), holds
/// column 0 as the idle frame otherwise, and picks the row from `facing`.
fn animate_player(time: Res<Time>, mut query: Query<(&mut PlayerAnimation, &mut Sprite)>) {
    for (mut anim, mut sprite) in &mut query {
        if anim.moving {
            anim.timer.tick(time.delta());
            if anim.timer.just_finished() {
                anim.frame = (anim.frame + 1) % SPRITE_COLUMNS;
            }
        } else {
            anim.timer.reset();
            anim.frame = 0;
        }

        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.index = (anim.facing.row() * SPRITE_COLUMNS + anim.frame) as usize;
        }
    }
}

/// Returns whether the move actually happened (i.e. wasn't blocked).
pub(crate) fn try_move(
    tile_pos: &mut Vec2,
    delta: Vec2,
    grid: &IsoGrid,
    polygons: &TileCollisionPolygons,
) -> bool {
    let candidate = *tile_pos + delta;
    if is_point_blocked(tile_to_world(candidate, &grid.grid, grid.offset), polygons) {
        false
    } else {
        *tile_pos = candidate;
        true
    }
}

/// Projects the character's tile position to world space every frame, and
/// derives its Z from world Y using the exact same formula `bevy_ecs_tilemap`
/// uses for `y_sort` (`1.0 - world_y / (map_height_tiles * tile_pixel_height)`,
/// see `render/material.rs` in that crate), against the topmost tile layer's
/// baseline of Z=0 (the last-spawned layer always lands there — see
/// `bevy_ecs_tiled`'s `spawn_layers`, which accumulates each layer's Z from
/// `-(layer_count - 1) * offset` up to exactly `0` for the last one). A fixed
/// Z can't interleave with a Y-sorted layer as the character crosses rows;
/// this keeps it correctly sorted against every wall tile regardless of row.
fn sync_player_transform(grid: Res<IsoGrid>, mut query: Query<(&Player, &mut Transform)>) {
    for (player, mut transform) in &mut query {
        let world = tile_to_world(player.tile_pos, &grid.grid, grid.offset);
        transform.translation.x = world.x;
        transform.translation.y = world.y;
        transform.translation.z = 1.0 - world.y / grid.y_sort_extent;
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
