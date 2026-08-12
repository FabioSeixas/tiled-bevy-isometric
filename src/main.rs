use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

fn main() {
    App::new()
        // Nearest-neighbor sampling avoids blurring pixel-art tiles.
        .add_plugins(DefaultPlugins.build().set(ImagePlugin::default_nearest()))
        // Pulls in bevy_ecs_tilemap::TilemapPlugin automatically.
        .add_plugins(TiledPlugin::default())
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    commands.spawn((
        TiledMap(asset_server.load("tiles/placeholder.tmx")),
        TilemapAnchor::Center,
    ));
}
