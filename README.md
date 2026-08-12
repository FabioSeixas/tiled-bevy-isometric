# tiled-bevy-isometric

Prototyping playground for isometric building collision and modular
Aseprite/Tiled tilesets, ahead of bringing anything back into
`bevy-navigation-ldtk`. Disposable by design — nothing here needs to be
production-quality.

## Running

```sh
cargo run
```

This opens a window and loads `assets/tiles/placeholder.tmx` — a trivial
4-color, 8x6 orthogonal grid that exists only to prove the Tiled loading path
works end to end.

## Dropping in your own map

Once you've composed a tileset in Aseprite and a map in Tiled:

1. Put your tileset image(s), `.tsx`, and `.tmx` files somewhere under
   `assets/` (e.g. `assets/tiles/`).
2. Update the `asset_server.load(...)` path in `src/main.rs` to point at your
   `.tmx` file instead of `tiles/placeholder.tmx`.
3. Delete `assets/tiles/placeholder.tmx`, `placeholder.tsx`, and
   `placeholder_tileset.png` once your own map is in and working.

Isometric orientation, per-edge wall/collision data, and player movement are
all out of scope for this scaffold — see `AGENTS.md` for the dependency
version notes that matter when picking these up.
