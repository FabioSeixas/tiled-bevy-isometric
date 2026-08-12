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
`assets/experiment.tsx`), and spawns a controllable character
(`assets/isometric_char_1.png`). Move with WASD or arrow keys. Movement is
blocked by each tile's hand-drawn Tile Collision Editor shape, not a full-tile
grid — see "Collision" below.

## Assets

`assets/experiment.aseprite` and `assets/isometric_char_1.aseprite` are the
Aseprite sources; `experiment.tsx` and the character sprite loading both
reference the exported `.png` siblings instead, since Bevy's `ImageLoader`
can't read raw `.aseprite` files. Re-export after editing the `.aseprite`
sources with:

```sh
aseprite -b assets/experiment.aseprite --save-as assets/experiment.png
aseprite -b assets/isometric_char_1.aseprite --save-as assets/isometric_char_1.png
```

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
