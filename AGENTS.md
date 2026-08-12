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

## Maintaining this file

Keep this file for knowledge useful to almost every future agent session in this project.
Do not repeat what the codebase already shows; point to the authoritative file or command instead.
Prefer rewriting or pruning existing entries over appending new ones.
When updating this file, preserve this bar for all agents and keep entries concise.
