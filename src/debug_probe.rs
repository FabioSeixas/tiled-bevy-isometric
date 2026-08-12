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

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::character::{Player, try_move};
use crate::collision::{TileCollisionPolygons, is_point_blocked};
use crate::iso::IsoGrid;

pub struct ProbePlugin;

impl Plugin for ProbePlugin {
    fn build(&self, app: &mut App) {
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
