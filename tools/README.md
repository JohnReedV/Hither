# Development tools

Run these commands from the repository root. The existing script paths are
stable entry points: Blender modules import one another from this directory,
and smoke/profiling scripts locate the project relative to their own path.

| Task | Entry points |
| --- | --- |
| Build/export the player avatar | `build_avatar.py`, `export_avatar.py` |
| Check avatar animation | `check_avatar_animation.py` |
| Avatar authoring helpers | `avatar_animation.py`, `avatar_details.py`, `avatar_repairs.py`, `portrait_head.py` |
| Portrait fitting | `measure_face.py`, `prepare_portrait_fit.py`, `fit_face.py` |
| Build orcs | `build_orcs.py`, with `orc_animation.py`, `orc_outfits.py`, and `orc_surfaces.py`; validate with `check_orcs.py` and `check_orc_throw.py` |
| Build/check goblins | `build_goblin.py`, `check_goblin.py`, `smoke_goblins.sh` |
| Preview orcs | `preview_orcs.py`, `preview_orc_grip.py`, `preview_orc_carry.py` |
| Runtime smoke checks | `smoke_avatar.sh`, `smoke_graphics.sh`, `smoke_orchard.sh`, `smoke_mountains.sh`, `smoke_dens.sh`, `smoke_torch.sh` |
| Runtime profiling | `profile_forest.sh`, `profile_native.sh`, `profile_spectator.sh` |

Python authoring tools use `tools/.venv/bin/python`. Follow the avatar/orc
specifications in `docs/` for required packages and source assets. Runtime smoke
checks require a release binary, Xvfb, xdotool and ImageMagick. Script headers
and the feature documentation describe optional environment switches.

The stone throw uses the 7.5-second `Push` clip, with release at phase 0.70.
Rebuilding the orcs also exports `stone-contact.json` for physics and release
velocity, and `stone-palm.json` for the live rendered hand attachment. Keep these
files together with the GLBs. `check_orc_throw.py` checks exported foot planting,
jump motion, and agreement between the socket and the baked trajectory.
For full-body carry/throw renders, set `HITHER_ORC_FULL=1` and select 60 fps
frames with `HITHER_ORC_FRAMES` when running `preview_orc_carry.py`.

The checked-in JSON files are isolated smoke/profiling settings fixtures.
Generated screenshots, logs, profiles and temporary settings belong in the
ignored `tools/build/` directory. Downloaded authoring dependencies belong in
ignored `tools/vendor/`; the Python environment belongs in ignored `tools/.venv/`.
Do not import or build runtime code from these generated directories.
