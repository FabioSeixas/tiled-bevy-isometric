# Project agent memory

This file is the project's committed home for project-intrinsic agent knowledge: build, test, release, architecture, and sharp-edge notes that should travel with the code.

- Add durable project-specific notes here as they are discovered through real work.

## Dependency versions are pinned deliberately

`bevy_ecs_tiled` is pinned to `0.12.0`, not the latest crates.io release. As of
2026-08, `bevy_ecs_tiled` 0.13+ depends on `bevy 0.19` / `bevy_ecs_tilemap
0.19`, which conflicts with this repo's `bevy = "0.18"` (chosen to match
`bevy-navigation-ldtk`, which also pins `bevy_ecs_tilemap = "0.18"`). `0.12.0`
is the newest `bevy_ecs_tiled` release that still resolves to a single `bevy
0.18.x` in the dependency tree — verified by checking `Cargo.lock` after
`cargo add`, not by trusting the crate's stated version range. Before bumping
either `bevy` or `bevy_ecs_tiled`, re-check that `cargo add` still locks to
one `bevy` version (`grep -A1 '^name = "bevy"$' Cargo.lock` should show a
single version).

## Verifying the app actually renders (not just compiles)

This worktree's shell session runs in a background launchd session without
WindowServer access, so real screenshots (`screencapture`) capture the
physical desktop, not this app's window — the app window never composites to
the display even though the process runs and the GPU renders correctly. To
verify rendering, use Bevy's own screenshot API instead, which captures the
GPU frame directly:

```rust
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
// inside a system, after a short delay:
commands.spawn(Screenshot::primary_window())
    .observe(save_to_disk("/tmp/capture.png"));
```

This is an environment quirk, not a bug in the app or the dependency stack.

## Collision is hand-rolled from Tiled data, not a physics engine

`src/collision.rs` reads each placed tile's Tile Collision Editor
`<objectgroup>` polygon straight from `TiledMapAsset.map` (via
`bevy_ecs_tiled`'s core API, never its `physics` feature — no Avian/Rapier
dependency exists in this repo, deliberately) and converts it to a world-space
polygon once per `TiledEvent<MapCreated>`. `src/character.rs` does a plain
point-in-polygon check against the character's position every frame; no
collider shapes are spawned as entities.

Tile-space-to-world-space conversion (`src/iso.rs::tile_to_world`) must
include the same `TilemapAnchor` offset that `tile_relative_position` (used
to place the collision polygons) already bakes in — `bevy_ecs_tilemap`'s raw
diamond-grid formula alone puts tile `(0,0)` at world origin, but an anchored
map (this repo uses `TilemapAnchor::Center`) doesn't. `IsoGrid` computes and
caches that offset once, from the map's own `tile_relative_position` at tile
`(0,0)`, when the map loads. Getting this wrong doesn't error — it silently
moves the character out of sync with the collision polygons, so collisions
either never trigger or trigger in the wrong place. If collision seems off
after touching this math, re-derive the offset rather than guessing at it.

To verify a hand-drawn shape actually took effect (new tile, edited polygon),
don't rely on eyeballing gameplay — use the scripted probe in
`src/debug_probe.rs` (env vars documented in its header comment). It runs the
exact same `try_move`/`is_point_blocked` path as real movement, either
stepping the character through tile-space or testing one exact world point,
and can save a screenshot at the end. `cargo run` also still logs `Built N
tile collision polygon(s) from Tiled data` on map load as a quick sanity count.
`SIM_KEYS` in the same file drives real `KeyCode`s into `ButtonInput` instead,
so it exercises `move_player`'s keyboard mapping itself (not just collision)
and reports both the tile-space and world-space delta produced — use it to
check that a given key actually moves the character the way it looks like it
should on screen.

## Tile-space axes are diagonal on screen — keyboard input needs conversion

`iso.rs::tile_to_world` is the standard isometric diamond projection: moving
along tile-x *or* tile-y alone always produces a diagonal screen direction,
never straight up/down/left/right. So `move_player` must not feed keyboard
axes into tile-space directly — it converts screen-space intent (what Up/
Down/Left/Right should look like on screen) into a tile-space delta via
`iso.rs::screen_dir_to_tile_dir`, the inverse of `tile_to_world`'s linear
part. If movement ever looks diagonal again, this is the first thing to
check; `iso.rs`'s unit test asserts the inverse round-trips for all four
cardinal screen directions.

## Tall tile art needs `y_sort` with 1-tile render chunks, not just `y_sort`

`assets/experiment.tsx` tiles are 64x64 against this map's 64x32 grid
footprint, so diamond-adjacent tiles routinely overlap on screen and need
back-to-front draw order to composite correctly (this is *not* the collision
system — collision is a separate hand-rolled polygon check, untouched by
this). `bevy_ecs_tilemap`'s `TilemapRenderSettings.y_sort` only reorders
draws *between* render chunks, keyed on each chunk's own world-space Y — it
does nothing for tiles within the same chunk. Since this map (30x20) fits
inside the crate's default chunk size (64x64), the whole layer was one
chunk, so enabling `y_sort` alone would have been a no-op. The fix (in
`main.rs`'s `setup`) sets `render_chunk_size: UVec2::new(1, 1)` *together
with* `y_sort: true`, so every tile gets its own chunk and its own correct
sort key. If wall/stack rendering ever looks broken again, check both of
these are still set together, not just one.

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
