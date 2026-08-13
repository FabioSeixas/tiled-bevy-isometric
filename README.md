# tiled-bevy-isometric

Prototyping playground for isometric building collision and modular
Aseprite/Tiled tilesets, ahead of bringing anything back into
`bevy-navigation-ldtk`. Disposable by design — nothing here needs to be
production-quality.

## Running

```sh
cargo run
```

This opens a window, loads the real `assets/map.tmx` isometric map (tileset
`assets/experiment.tsx`), and spawns a controllable character animated across
8 compass directions (`assets/isometric_character_idle.png` and
`assets/isometric_character_run.png`). Move with WASD or arrow keys. Movement
is blocked by each tile's hand-drawn Tile Collision Editor shape, not a
full-tile grid — see "Collision" below.

## Assets

`assets/experiment.aseprite` is the Aseprite source for the tileset;
`experiment.tsx` references the exported `.png` sibling instead, since
Bevy's `ImageLoader` can't read raw `.aseprite` files. Re-export after
editing it with:

```sh
aseprite -b assets/experiment.aseprite --save-as assets/experiment.png
```

The two character sheets are hand-authored PNGs with no `.aseprite` source
checked in. Both are 64x64-per-frame, 8 rows (one per `FacingDirection`,
see that enum's doc comment in `src/character.rs` for the row-to-direction
mapping and how it was derived) — idle is 8 columns, run is 6.

## Collision

See `src/collision.rs`. No physics engine is used — per the project brief,
collision is read directly from Tiled's Tile Collision Editor `<objectgroup>`
polygon data on each placed tile (via `bevy_ecs_tiled`'s core map-asset API,
not its optional `physics` feature), converted to world-space polygons once
when the map loads, and checked with a hand-written point-in-polygon test
against the character's tile-space position every frame. Tiles with no
collision shape in Tiled are fully walkable; tiles with a shape block only the
covered part, exactly as drawn.

To verify a new or edited shape took effect, use the deterministic probe in
`src/debug_probe.rs` (see its doc comment for env vars) instead of trying to
line up manual keypresses — it drives the same collision path as real
movement and can target an exact point or tile-space step, headless.
