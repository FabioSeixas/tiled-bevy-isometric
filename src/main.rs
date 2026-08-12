mod character;
mod collision;
mod debug_probe;
mod iso;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy_ecs_tiled::prelude::*;

use character::CharacterPlugin;
use collision::CollisionPlugin;
use debug_probe::ProbePlugin;
use iso::IsoPlugin;

fn main() {
    let mut app = App::new();
    app
        // Nearest-neighbor sampling avoids blurring pixel-art tiles.
        .add_plugins(DefaultPlugins.build().set(ImagePlugin::default_nearest()))
        // Pulls in bevy_ecs_tilemap::TilemapPlugin automatically.
        .add_plugins(TiledPlugin::default())
        .add_plugins((CollisionPlugin, IsoPlugin, CharacterPlugin, ProbePlugin))
        .add_systems(Startup, setup);

    // Debug-only: set SCREENSHOT_PATH to capture one frame and exit, for
    // verifying rendering in headless/no-WindowServer environments. See
    // AGENTS.md "Verifying the app actually renders".
    if let Ok(path) = std::env::var("SCREENSHOT_PATH") {
        app.add_systems(Update, take_screenshot_and_exit(path));
    }

    app.run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    commands.spawn((
        TiledMap(asset_server.load("map.tmx")),
        TilemapAnchor::Center,
    ));
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
