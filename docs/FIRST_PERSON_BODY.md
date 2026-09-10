# First-person legs and feet

Looking down in player mode reveals the bundled avatar's denim jeans and leather
boots. The close-view mesh is exported from the same packed Blender source as the
hand, with two subdivision levels on the lower-body silhouettes and normalized
skin weights. Existing texture UVs, seams, soles, and clothing materials survive
export. No external assets are required; provenance is in AVATAR_ASSETS.md.

`tools/build_first_person_body.py` rebuilds
`assets/avatars/first-person-body.glb` using `tools/.venv/bin/python`. The asset
contains the jeans, boots, and lower jacket/shirt, along with the complete
skeleton and ten animation clips. The head, upper chest, and sleeves are removed
to keep them out of the eye camera and the independently animated hand.

The body uses the world camera and depth buffer. Its physical foot height is
independent of the camera's landing bob, and camera pitch never tilts the body.
An authored eye-to-abdomen offset places the waist behind the camera; runtime
compensation keeps it there during sharp turns, with at most 20 degrees of body
heading lag. World lighting and occlusion apply naturally; the hand retains its close-view
render pass. A separate animation graph preserves the bundled first-person
appearance even with a custom exterior avatar.

The shared avatar controller supplies idle, forward/backward walking, strafing,
jumping, falling, landing, and alternating planted turn steps. Gait transitions
preserve cycle phase and playback follows actual movement. Pause freezes animation.
Only one body is visible in each camera mode; spectator mode hides both. Snow
contacts are emitted once, by the exterior avatar controller.

`tools/smoke_body.sh` captures level and downward views, all four movement
directions, jump/landing, turning, punching, camera switching, and pause on an
isolated Xvfb display. Set `HITHER_BINARY` to test a specific executable.

Visual QA: `cargo check --offline`, avatar validation, and all 33 player tests
passed. The isolated smoke run covered locomotion, jump/landing, turning, punch,
camera switching, and pause with no runtime errors. Paused screenshots differ
only in the FPS counter. See `first-person-body.png` for the resulting view.
