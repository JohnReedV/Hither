# First-person hand

The default player view includes an anatomical right hand and plaid jacket cuff.
The isolated close-view asset uses the bundled avatar's skin UVs, finger anatomy,
nails, and clothing, with two additional subdivision levels on the skin. The
fingers have a graduated relaxed curl. A baked 1024² skin normal map adds fine
surface grain on a separate UV set; skin roughness is tuned for soft highlights. Asset provenance is the same as documented
in AVATAR_ASSETS.md; no external assets were downloaded for this feature.

`tools/build_first_person_hand.py` reproducibly exports the self-contained
`assets/avatars/first-person-hand.glb` from the packed avatar template, using the
existing `tools/.venv/bin/python` Blender environment. Blender is not required at
runtime. This is the bundled hand appearance, including when a custom third-person
avatar is selected.

`src/player/hand.rs` provides camera-space breathing, speed-driven walking sway,
smoothed mouse inertia, and jump/landing response. Pausing freezes the pose.
Teleports and camera-mode changes suppress large motion impulses. The hand is
hidden in second/third person and spectator mode.

A dedicated layer-2 camera renders after the world into its existing scaled image
and before the native-resolution UI. Its depth buffer clears independently, so
walls cannot slice through fingers. Its near plane is 1 cm. Two soft directional
lights preserve material readability. The camera is disabled when the hand is
hidden. The viewmodel follows the world target's resize/aspect changes and has FXAA.

Run `tools/smoke_hand.sh` after `cargo build --release --offline` for isolated
screenshots of idle, walking, jumping, camera switching, and pause, without sending
input to the desktop. Motion-state tests are under `player::hand::tests`.

## Mouse 1 punch

Left click closes the fingers into a sculpted fist using a glTF morph target,
then plays a 75 ms anticipation, 70 ms extension, brief follow-through, recoil,
and relaxed recovery (440 ms total). The fist closes before extension and stays
closed during recoil. The skin morph includes normals, while the cuff follows
the wrist. Locomotion sway is reduced during the strike.

Holding Mouse 1 does not repeat. Clicking after 270 ms queues one follow-up;
earlier clicks do not interrupt or restart the current strike. Pause freezes
both the hand pose and finger clench. Chat, an uncaptured cursor, spectator mode,
and exterior cameras cannot initiate punches. The click that recaptures the mouse
is suppressed. This feature supplies the first-person attack animation; it does
not introduce damage or a new combat system.

`tools/smoke_punch.sh` captures three live clicks to `tools/build/hand-punch.mp4`
and checks pause/recovery screenshots. Set `HITHER_BINARY` to use a QA executable.
The curve, clench timing, continuity, input buffering, hold behavior, and pause
behavior have focused tests in `src/player/hand_attack.rs`.
