# Code organization

Hither is one Rust binary with internal modules grouped by responsibility. The
executable entry point delegates to `app::run`; application composition stays in
one place so Bevy scheduling is visible and reviewable.

| Location | Responsibility |
| --- | --- |
| `src/app/` | CLI dispatch, startup composition, shared pause state, persisted settings and frame pacing |
| `src/player/` | Camera rig and obstruction, movement and collision, cursor input, spectator mode, avatar and picking |
| `src/world/` | Seeded biomes, castle bounds, terrain, floor queries, navigation and snow tracks |
| `src/world/streaming.rs` | Shared streaming schedule, loading envelope, work admission and background-job lifetime |
| `src/world/orchard/` | Botanical geometry, tree streaming, materials and fruit growth/picking state |
| `src/world/orcs/` | Den placement, encounters, animation, collision, floor adapters, navigation and torches |
| `src/world/orcs/art.rs` | Den meshes and materials |
| `src/world/vegetation/` | Grass and understory streaming |
| `src/rendering/` | Shared mesh/raycast primitives, SDF material and scene setup, live graphics controls and view distance |
| `src/ui/` | Pause/options menu, layout, FPS HUD and local chat/commands |
| `src/diagnostics/` | Opt-in runtime profiling |
| `assets/` | Runtime shaders, fonts, avatar and orc assets, plus their existing editable sources |
| `tools/` | Stable authoring, smoke-test and profiling entry points; see [tools guide](../tools/README.md) |
| `docs/` | Feature specifications, technical notes and visual reference captures |

## Dependency boundaries

Import cross-feature types and functions from their owning module. Do not add a
root-level collection of re-exports or a catch-all `utils` module. Keep helpers
private unless another feature actually needs them; use crate-scoped visibility
for shared runtime resources and feature entry points. Parent imports within a
feature are local implementation details, not a cross-feature API.

`world::random` owns the deterministic generator used by procedural content.
`rendering::geometry` owns mesh assembly, compaction, tangents, triangle and ray
queries, and primitive transforms. Both trees and dens use these primitives;
orc code does not depend on orchard internals. Tree-specific LOD, branches,
leaves and surface generation remain under `world::orchard`.

The current graphics feature includes its advanced controls and render-target
management because the controls directly drive those settings. General menu
layout and input editing live in `ui`, while serialized settings and their
on-disk compatibility live in `app::settings`.

## Scheduling and behavior contracts

`app::run` preserves the explicit chained update order: cursor and pause/menu
input, camera movement/view calculation, fruit simulation and picking, snow,
shader uniforms, then the FPS display. Settings saving and frame limiting run
in `Last`, in that order. Feature plugins declare their existing `.before()` and
`.after()` relationships against the actual owning system functions.

`app::setup` remains one startup system. It calls `rendering::scene::setup_scene`
and then `ui::layout::setup_ui` using the same command buffer. Splitting those
helpers into independent startup systems would require checking deferred command
application and feature setup dependencies again.

Keep settings JSON keys/defaults, CLI arguments, environment variables, asset
paths, shader bindings and gameplay constants stable during structural work.
Compile-time embedded assets use `CARGO_MANIFEST_DIR`, so moving a Rust module
cannot silently break a relative asset include. Runtime asset paths are unchanged.

See [world streaming](WORLD_STREAMING.md) for shared admission limits, adapter scheduling,
render-distance coverage and worker lifetime rules.

## Tests and development

Small unit tests remain beside the code. Larger navigation, chat, orchard and
orc suites live in adjacent test files and compile only under `cfg(test)`.
Shared mesh regression tests live beside `rendering::geometry`.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --release
cargo build --release
bash tools/smoke_avatar.sh
bash tools/smoke_graphics.sh
```

Release builds intentionally use full LTO; preserve those performance settings
unless changing build policy is the task. Smoke scripts use isolated settings
and Xvfb displays. They do not require modifying desktop settings.
