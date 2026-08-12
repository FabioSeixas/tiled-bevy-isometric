//! Deterministic collision probe used to verify a Tile Collision Editor shape
//! actually took effect, without a real keyboard or WindowServer. Drives the
//! same `try_move` collision path the player uses, just with a scripted step
//! instead of keyboard input, so the result reflects real gameplay behavior.
//! See AGENTS.md "Verifying tile collision".
//!
//! Only runs when `PROBE_STEP` is set; a no-op plugin otherwise.
//!
//! Env vars:
//! - `PROBE_START`  = "tx,ty" starting tile position (default: player spawn)
//! - `PROBE_STEP`   = "dx,dy" tile-space delta applied every frame
//! - `PROBE_FRAMES` = frame count to run before reporting (default 60)
//! - `PROBE_SCREENSHOT_PATH` = optional path to save a screenshot on the last frame
//!
//! `SIM_KEYS` drives a different probe: it presses real `KeyCode`s into the
//! `ButtonInput<KeyCode>` resource every frame, so the actual `move_player`
//! system (keyboard mapping included) runs exactly as it would for a human
//! player, instead of bypassing it like the tile-step probe above does.
//! - `SIM_KEYS` = comma-separated `KeyCode` names, e.g. "ArrowUp" or "ArrowUp,ArrowRight"
//! - `SIM_FRAMES` = frame count to hold the keys before reporting (default 60)
//! - `SIM_SCREENSHOT_PATH` = optional path to save a screenshot on the last frame

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::character::{Player, try_move};
use crate::collision::{TileCollisionPolygons, is_point_blocked};
use crate::iso::IsoGrid;

pub struct ProbePlugin;

impl Plugin for ProbePlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("SIM_KEYS").is_ok() {
            app.add_systems(Update, run_key_simulation);
            return;
        }
        if std::env::var("PROBE_WORLD_POINT").is_ok() {
            app.add_systems(Update, run_world_point_probe);
            return;
        }
        if std::env::var("PROBE_STEP").is_err() {
            return;
        }
        app.add_systems(Update, run_probe);
    }
}

fn key_code_from_name(name: &str) -> Option<KeyCode> {
    match name.trim() {
        "ArrowUp" => Some(KeyCode::ArrowUp),
        "ArrowDown" => Some(KeyCode::ArrowDown),
        "ArrowLeft" => Some(KeyCode::ArrowLeft),
        "ArrowRight" => Some(KeyCode::ArrowRight),
        "KeyW" => Some(KeyCode::KeyW),
        "KeyA" => Some(KeyCode::KeyA),
        "KeyS" => Some(KeyCode::KeyS),
        "KeyD" => Some(KeyCode::KeyD),
        _ => None,
    }
}

/// Holds the keys named in `SIM_KEYS` for `SIM_FRAMES` frames, driving the
/// real `move_player` system, then logs how far the character actually
/// moved (`SIM_RESULT tile_delta=(...) world_delta=(...)`) so the reported
/// direction can be checked against what the keys were supposed to do on
/// screen (e.g. `ArrowUp` should produce a world delta with `x == 0`).
///
/// Waits `SIM_WARMUP_FRAMES` (default 90) frames with no keys held before
/// starting to record, so the map asset has already finished loading and
/// `IsoGrid`'s grid/offset (which jump once, from their startup defaults, on
/// `TiledEvent<MapCreated>`) are stable for the whole measurement window.
fn run_key_simulation(
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut frame: Local<u32>,
    mut start: Local<Option<(Vec2, Vec3)>>,
    mut started_pos: Local<bool>,
    mut commands: Commands,
    mut query: Query<(&mut Player, &Transform)>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((mut player, transform)) = query.single_mut() else {
        return;
    };

    // Optional override so the simulation can start right next to a wall
    // corner instead of always at the open-ground spawn point.
    if !*started_pos {
        if let Ok(s) = std::env::var("SIM_START") {
            let mut parts = s.split(',');
            if let (Some(x), Some(y)) = (parts.next(), parts.next()) {
                if let (Ok(x), Ok(y)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>()) {
                    player.tile_pos = Vec2::new(x, y);
                }
            }
        }
        *started_pos = true;
    }

    let warmup_frames: u32 = std::env::var("SIM_WARMUP_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(90);

    *frame += 1;
    if *frame <= warmup_frames {
        return;
    }

    if start.is_none() {
        *start = Some((player.tile_pos, transform.translation));
    }

    let keys: Vec<KeyCode> = std::env::var("SIM_KEYS")
        .unwrap_or_default()
        .split(',')
        .filter_map(key_code_from_name)
        .collect();
    for key in &keys {
        keyboard.press(*key);
    }

    let total_frames: u32 = std::env::var("SIM_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let elapsed = *frame - warmup_frames;

    if elapsed == total_frames {
        let (start_tile, start_world) = start.unwrap();
        let tile_delta = player.tile_pos - start_tile;
        let world_delta = transform.translation - start_world;
        info!(
            "SIM_RESULT tile_delta=({:.4},{:.4}) world_delta=({:.3},{:.3},{:.3})",
            tile_delta.x, tile_delta.y, world_delta.x, world_delta.y, world_delta.z
        );
        if let Ok(path) = std::env::var("SIM_SCREENSHOT_PATH") {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
    } else if elapsed == total_frames + 5 {
        exit.write(AppExit::Success);
    }
}

/// Tests one exact world-space point against the loaded collision polygons,
/// bypassing tile-space movement entirely. Set `PROBE_WORLD_POINT="wx,wy"`.
/// Useful for checking a specific vertex/corner of a hand-drawn shape.
fn run_world_point_probe(
    mut frame: Local<u32>,
    polygons: Res<TileCollisionPolygons>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    if *frame == 10 {
        let point = parse_vec2("PROBE_WORLD_POINT", Vec2::ZERO);
        info!(
            "PROBE_WORLD_RESULT point=({:.3},{:.3}) blocked={}",
            point.x,
            point.y,
            is_point_blocked(point, &polygons)
        );
    } else if *frame == 15 {
        exit.write(AppExit::Success);
    }
}

fn parse_vec2(var: &str, default: Vec2) -> Vec2 {
    std::env::var(var)
        .ok()
        .and_then(|s| {
            let mut parts = s.split(',');
            let x = parts.next()?.trim().parse::<f32>().ok()?;
            let y = parts.next()?.trim().parse::<f32>().ok()?;
            Some(Vec2::new(x, y))
        })
        .unwrap_or(default)
}

fn run_probe(
    mut commands: Commands,
    mut frame: Local<u32>,
    mut started: Local<bool>,
    grid: Res<IsoGrid>,
    polygons: Res<TileCollisionPolygons>,
    mut query: Query<&mut Player>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(mut player) = query.single_mut() else {
        return;
    };

    if !*started {
        player.tile_pos = parse_vec2("PROBE_START", player.tile_pos);
        *started = true;
    }

    let step = parse_vec2("PROBE_STEP", Vec2::ZERO);
    let total_frames: u32 = std::env::var("PROBE_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);

    try_move(&mut player.tile_pos, step, &grid, &polygons);
    *frame += 1;

    if *frame == total_frames {
        info!(
            "PROBE_RESULT tile_pos=({:.3},{:.3})",
            player.tile_pos.x, player.tile_pos.y
        );
        if let Ok(path) = std::env::var("PROBE_SCREENSHOT_PATH") {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
    } else if *frame == total_frames + 5 {
        exit.write(AppExit::Success);
    }
}
