mod character;
mod collision;
mod debug_probe;
mod iso;
mod zsort_debug;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy_ecs_tiled::prelude::*;

use character::CharacterPlugin;
use collision::CollisionPlugin;
use debug_probe::ProbePlugin;
use iso::IsoPlugin;
use zsort_debug::ZsortDebugPlugin;

fn main() {
    let mut app = App::new();
    app
        // Nearest-neighbor sampling avoids blurring pixel-art tiles.
        .add_plugins(DefaultPlugins.build().set(ImagePlugin::default_nearest()))
        // Pulls in bevy_ecs_tilemap::TilemapPlugin automatically.
        .add_plugins(TiledPlugin::default())
        .add_plugins((CollisionPlugin, IsoPlugin, CharacterPlugin, ProbePlugin))
        .add_plugins(ZsortDebugPlugin)
        .add_systems(
            Update,
            zsort_debug::zsort_debug_update.after(character::sync_player_transform),
        )
        .add_systems(Startup, setup);

    // Debug-only: set SCREENSHOT_PATH to capture one frame and exit, for
    // verifying rendering in headless/no-WindowServer environments. See
    // AGENTS.md "Verifying the app actually renders".
    if let Ok(path) = std::env::var("SCREENSHOT_PATH") {
        app.add_systems(Update, take_screenshot_and_exit(path));
    }

    // Debug-only: capture a run of *consecutive* frames instead of one. A
    // single screenshot can only ever show whichever side won the depth sort
    // on that one frame, so it cannot distinguish "consistently wrong" from
    // "flickering" — see AGENTS.md "A single screenshot cannot see draw-order
    // instability".
    if let Ok(prefix) = std::env::var("SCREENSHOT_SEQ_PATH") {
        app.add_systems(Update, capture_frame_sequence(prefix));
    }

    app.run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    commands.spawn((
        TiledMap(asset_server.load("map.tmx")),
        TilemapAnchor::Center,
        // Tiles here are taller than the grid's diamond footprint (64x64
        // art on a 64x32 grid), so two diamond-adjacent tiles can overlap
        // on screen. `bevy_ecs_tilemap` only reorders draws *between*
        // render chunks (via `y_sort`, keyed on each chunk's world Y), not
        // within one — and the whole map fits in a single default-sized
        // chunk, so without shrinking chunks to one tile each, overlapping
        // tiles draw in tile-spawn order instead of back-to-front.
        TilemapRenderSettings {
            render_chunk_size: UVec2::new(1, 1),
            y_sort: true,
        },
    ));
}

/// Saves one screenshot per frame for `SCREENSHOT_SEQ_COUNT` (default 20)
/// consecutive frames starting at frame `SCREENSHOT_SEQ_START` (default 120,
/// late enough that the map asset has loaded and `IsoGrid` has settled), to
/// `{prefix}_000.png`, `{prefix}_001.png`, ... then exits.
///
/// The point is what a *single* screenshot structurally cannot show: with the
/// character held still and no animation running, every frame in the run must
/// be byte-identical. Any difference between consecutive frames means the draw
/// order (or something else per-frame) is unstable rather than merely wrong —
/// a distinction invisible to the one-shot `SCREENSHOT_PATH` mode.
///
/// Logs `SEQ_CAPTURE frame=N file=...` per frame so a capture can be
/// correlated with `ZSORT_DEBUG`'s own frame counter.
fn capture_frame_sequence(
    prefix: String,
) -> impl FnMut(Commands, Local<u32>, MessageWriter<AppExit>) {
    let start: u32 = std::env::var("SCREENSHOT_SEQ_START")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    let count: u32 = std::env::var("SCREENSHOT_SEQ_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    move |mut commands: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>| {
        *frame += 1;
        if *frame >= start && *frame < start + count {
            let path = format!("{}_{:03}.png", prefix, *frame - start);
            info!("SEQ_CAPTURE frame={} file={}", *frame, path);
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        } else if *frame == start + count + 10 {
            exit.write(AppExit::Success);
        }
    }
}

fn take_screenshot_and_exit(
    path: String,
) -> impl FnMut(Commands, Local<u32>, MessageWriter<AppExit>) {
    move |mut commands: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>| {
        *frame += 1;
        if *frame == 30 {
            let path = path.clone();
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        } else if *frame == 35 {
            exit.write(AppExit::Success);
        }
    }
}
