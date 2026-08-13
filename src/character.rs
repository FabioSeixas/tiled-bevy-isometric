use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::iso::{IsoGrid, screen_dir_to_tile_dir, tile_to_world};
use crate::subtile::SubtileGrid;

/// The controllable character. Position is tracked in fractional tile-space
/// (not world pixels) since that's the natural unit for isometric movement
/// input; `sync_player_transform` projects it to world space every frame.
#[derive(Component)]
pub struct Player {
    pub tile_pos: Vec2,
}

const MOVE_SPEED_TILES_PER_SEC: f32 = 3.0;

/// Both `isometric_character_idle.png` (512x512) and
/// `isometric_character_run.png` (384x512) use this frame size, confirmed by
/// slicing each sheet into a 64x64 grid and visually inspecting every cell:
/// each cell holds exactly one clean, non-clipped pose (no straddling or
/// cropping), and it evenly divides both sheets' dimensions (idle: 8x8,
/// run: 6x8).
const FRAME_SIZE: UVec2 = UVec2::new(64, 64);
const IDLE_COLUMNS: u32 = 8;
const IDLE_ROWS: u32 = 8;
const RUN_COLUMNS: u32 = 6;
const RUN_ROWS: u32 = 8;

/// Seconds per run-cycle frame while moving (~8 frames/sec — same cadence
/// `WALK_FRAME_SECS` used before this sheet swap, still readable at this
/// sprite's small on-screen size without looking frantic).
const RUN_FRAME_SECS: f32 = 0.12;
/// Seconds per idle frame. The idle sheet's 8 columns aren't a walk-style
/// cycle — pixel-diffing every column against column 0 shows columns 0-1 are
/// byte-identical and columns 2-7 are a second, different-but-mutually-
/// identical pose (a small breathing shift, not a walk cycle), so this runs
/// much slower than the run cycle to read as an idle breathing loop rather
/// than a flicker.
const IDLE_FRAME_SECS: f32 = 0.5;

/// Open ground away from every placed collidable tile, so the character
/// always spawns walkable regardless of which shapes are drawn on the map.
const SPAWN_TILE_POS: Vec2 = Vec2::new(6.0, 14.0);

/// Nudge added to the character's y_sort Z so it wins *exact* ties against
/// tilemap chunks.
///
/// `sync_player_transform` deliberately reproduces `bevy_ecs_tilemap`'s y_sort
/// key exactly, which means a character standing on a tile gets *bit-identical*
/// Z to that tile's own chunk (measured: player `z=1.01250`, tile (9,3)
/// `z=1.01250`). `Transparent2d` is ordered by a **stable** radix sort
/// (`bevy_core_pipeline`'s `core_2d`), so an exact tie preserves queue order —
/// and tilemap chunks are queued after sprites, so the tile wins and draws over
/// the character standing on it. Measured on this map: 79% of the character's
/// pixels disappeared behind the tile under its own feet, on every tile of a
/// raised block, and along any screen-horizontal walk (which holds `ty - tx`,
/// hence world Y, hence Z, exactly constant).
///
/// One screen row is `grid.y / y_sort_extent` of Z (0.025 here: 32px / 1280),
/// and f32 resolves ~1.2e-7 near Z=1, so a 1e-3 bias is far too small to
/// reorder the character against a genuinely different row and far too large to
/// be lost to rounding: it changes the outcome for exact ties only.
const Z_TIE_BIAS: f32 = 1e-3;

/// Which row of `isometric_character_idle.png`/`isometric_character_run.png`
/// (both share the same 8-row layout) faces which compass direction. Neither
/// sheet carries frame-tag metadata, so this was determined by slicing both
/// sheets into their 64x64 cells and visually/pixel-diffing every row:
///
/// - Row-pair mirroring: diffing each row against every other row *flipped
///   horizontally* found exact byte-for-byte matches (mean diff 0.000) for
///   (row1,row2), (row3,row7) and (row4,row6) on both sheets — confirming
///   those are mirrored left/right pairs of the same pose, and leaving row0
///   and row5 as the only unmirrored (front/back) rows.
/// - Front/back split: row0 is a symmetric, face-forward standing/running
///   pose (least self-shadow of any row); row5 is a symmetric pose with only
///   the back of the head visible, no face (most self-shadow) — so row0 is
///   Down (toward camera) and row5 is Up (away from camera).
/// - Down family vs. up family: rows 1 and 2 still show a visible face; rows
///   4 and 6 show only hood/hair with no face, matching row5's back-facing
///   look. So (row1,row2) are the two "Down" diagonals and (row4,row6) are
///   the two "Up" diagonals.
/// - Left vs. right within each pair: the idle sheet's self-shadow tone
///   turned out to be a fixed-light-source artifact (row0 and row5 — neither
///   mirrored, both unambiguous — are *both* slightly more shadowed on
///   screen-right), not a facing cue, so it was discarded as unreliable.
///   Instead, the run sheet's kinematics settle it unambiguously: across
///   every column of row3, the character's whole body leans and sprints
///   toward screen-left (head top-left, trailing leg kicked out to the
///   right) — a pure left profile — and row7 is the exact mirror, sprinting
///   right. Applying the same "which way is the trailing leg / lean facing"
///   read to the diagonal pairs: row1's trailing leg kicks left (facing
///   right) so row1 is Down-Right and row2 is Down-Left; row4 leans/sprints
///   left like row3 (Up-Left) and row6 mirrors it (Up-Right).
///
/// Final row order: 0 Down, 1 Down-Right, 2 Down-Left, 3 Left, 4 Up-Left,
/// 5 Up, 6 Up-Right, 7 Right.
#[derive(Clone, Copy, PartialEq, Default)]
enum FacingDirection {
    #[default]
    Down,
    DownRight,
    DownLeft,
    Left,
    UpLeft,
    Up,
    UpRight,
    Right,
}

impl FacingDirection {
    fn row(self) -> u32 {
        match self {
            FacingDirection::Down => 0,
            FacingDirection::DownRight => 1,
            FacingDirection::DownLeft => 2,
            FacingDirection::Left => 3,
            FacingDirection::UpLeft => 4,
            FacingDirection::Up => 5,
            FacingDirection::UpRight => 6,
            FacingDirection::Right => 7,
        }
    }

    /// Buckets screen-space movement input into 8 compass directions by
    /// angle, replacing the old 4-direction "larger axis wins" comparison
    /// (which could structurally never produce a diagonal). Sector 0 is
    /// centered on screen-right and sectors advance counter-clockwise in
    /// 45-degree steps, matching `atan2`'s convention.
    fn from_screen_input(input: Vec2) -> Self {
        let angle = input.y.atan2(input.x);
        let sector = (angle / (std::f32::consts::TAU / 8.0)).round() as i32;
        match sector.rem_euclid(8) {
            0 => FacingDirection::Right,
            1 => FacingDirection::UpRight,
            2 => FacingDirection::Up,
            3 => FacingDirection::UpLeft,
            4 => FacingDirection::Left,
            5 => FacingDirection::DownLeft,
            6 => FacingDirection::Down,
            7 => FacingDirection::DownRight,
            _ => unreachable!("rem_euclid(8) is always in 0..8"),
        }
    }
}

/// Animation state, driven by `move_player` (which sets `facing` and
/// `moving`) and applied to the sprite's image/atlas by `animate_player`.
/// Idle and run are separate sheets (not columns of one shared atlas), so
/// both the image and the atlas layout must be swapped together based on
/// `moving`, not just the atlas index within a single layout.
#[derive(Component)]
struct PlayerAnimation {
    facing: FacingDirection,
    moving: bool,
    /// Tracks the previous frame's `moving` so a transition between idle and
    /// running can reset `frame` and both timers instead of carrying over an
    /// index/timer progress from the other sheet's cycle.
    was_moving: bool,
    frame: u32,
    idle_timer: Timer,
    run_timer: Timer,
    idle_image: Handle<Image>,
    run_image: Handle<Image>,
    idle_layout: Handle<TextureAtlasLayout>,
    run_layout: Handle<TextureAtlasLayout>,
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
    let idle_image = asset_server.load("isometric_character_idle.png");
    let run_image = asset_server.load("isometric_character_run.png");
    let idle_layout = layouts.add(TextureAtlasLayout::from_grid(
        FRAME_SIZE,
        IDLE_COLUMNS,
        IDLE_ROWS,
        None,
        None,
    ));
    let run_layout = layouts.add(TextureAtlasLayout::from_grid(
        FRAME_SIZE,
        RUN_COLUMNS,
        RUN_ROWS,
        None,
        None,
    ));

    commands.spawn((
        Player {
            tile_pos: SPAWN_TILE_POS,
        },
        PlayerAnimation {
            facing: FacingDirection::default(),
            moving: false,
            was_moving: false,
            frame: 0,
            idle_timer: Timer::from_seconds(IDLE_FRAME_SECS, TimerMode::Repeating),
            run_timer: Timer::from_seconds(RUN_FRAME_SECS, TimerMode::Repeating),
            idle_image: idle_image.clone(),
            run_image,
            idle_layout: idle_layout.clone(),
            run_layout,
        },
        Sprite::from_atlas_image(
            idle_image,
            TextureAtlas {
                layout: idle_layout,
                index: (FacingDirection::default().row() * IDLE_COLUMNS) as usize,
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
    subtiles: Res<SubtileGrid>,
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

    let moved_x = try_move(&mut player.tile_pos, step_x, &subtiles);
    let moved_y = try_move(&mut player.tile_pos, step_y, &subtiles);

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

/// Cycles the run-cycle column while `moving` (set by `move_player`), cycles
/// the idle sheet's columns otherwise, and picks the row from `facing`.
/// Swaps both the sprite's image and its atlas layout on every idle/run
/// transition, since the two states are separate sheets, not columns of one
/// shared atlas.
fn animate_player(time: Res<Time>, mut query: Query<(&mut PlayerAnimation, &mut Sprite)>) {
    for (mut anim, mut sprite) in &mut query {
        if anim.moving != anim.was_moving {
            anim.frame = 0;
            anim.idle_timer.reset();
            anim.run_timer.reset();
            anim.was_moving = anim.moving;
        }

        let frame_count = if anim.moving { RUN_COLUMNS } else { IDLE_COLUMNS };
        let timer = if anim.moving {
            &mut anim.run_timer
        } else {
            &mut anim.idle_timer
        };
        timer.tick(time.delta());
        if timer.just_finished() {
            anim.frame = (anim.frame + 1) % frame_count;
        }

        sprite.image = if anim.moving {
            anim.run_image.clone()
        } else {
            anim.idle_image.clone()
        };
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.layout = if anim.moving {
                anim.run_layout.clone()
            } else {
                anim.idle_layout.clone()
            };
            atlas.index = (anim.facing.row() * frame_count + anim.frame) as usize;
        }
    }
}

/// Returns whether the move actually happened (i.e. wasn't blocked).
///
/// Checks only the candidate subtile itself — this is deliberate, not an
/// oversight: it's what makes diagonal corner-cutting through a blocked
/// corner possible, per the design brief this prototype validates. Adding a
/// check on the two subtiles flanking a diagonal step would close that gap,
/// but that's explicitly not wanted here.
pub(crate) fn try_move(tile_pos: &mut Vec2, delta: Vec2, subtiles: &SubtileGrid) -> bool {
    let candidate = *tile_pos + delta;
    if subtiles.is_blocked(candidate) {
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
///
/// `Z_TIE_BIAS` is then added so the character wins *exact* ties — see that
/// constant's comment for why the formula alone isn't enough.
pub(crate) fn sync_player_transform(grid: Res<IsoGrid>, mut query: Query<(&Player, &mut Transform)>) {
    for (player, mut transform) in &mut query {
        let world = tile_to_world(player.tile_pos, &grid.grid, grid.offset);
        transform.translation.x = world.x;
        transform.translation.y = world.y;
        transform.translation.z = 1.0 - world.y / grid.y_sort_extent + Z_TIE_BIAS;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `Z_TIE_BIAS` exists to break exact ties and nothing more. It must stay
    /// strictly inside the gap between two adjacent screen rows' sort keys,
    /// or the character would start winning against tiles that are genuinely
    /// one row in front of it — trading this bug for a worse one. It must also
    /// stay well clear of f32's resolution near Z=1, or it would round away
    /// and stop breaking the tie at all.
    #[test]
    fn z_tie_bias_breaks_ties_without_crossing_a_row() {
        // This map: 20 tiles tall, 64px tiles => extent 1280; rows are 32px apart.
        let y_sort_extent = 20.0 * 64.0_f32;
        let row_step = 32.0 / y_sort_extent;

        assert!(
            Z_TIE_BIAS < row_step,
            "bias {Z_TIE_BIAS} must be smaller than one row's Z step {row_step}"
        );
        assert!(
            Z_TIE_BIAS > f32::EPSILON * 8.0,
            "bias {Z_TIE_BIAS} must survive f32 rounding near Z=1"
        );

        // A tie must break in the character's favour...
        let tile_z = 1.0 - (-16.0 / y_sort_extent);
        let player_z = 1.0 - (-16.0 / y_sort_extent) + Z_TIE_BIAS;
        assert!(player_z > tile_z, "character must win an exact tie");

        // ...but the tile one row in front must still occlude the character.
        let row_in_front_z = 1.0 - (-48.0 / y_sort_extent);
        assert!(
            row_in_front_z > player_z,
            "a tile one row in front must still sort above the character"
        );
    }
}
