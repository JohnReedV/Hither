# Mireling goblins

Mirelings are a separate skinned mob, inspired by the supplied goblin sculpture: bald oversized head, pointed ears, wide jade eyes and dark sockets, olive/pink mottled skin, a narrow hunched torso, bent knees, long hands, bare feet, and dark ragged shorts.

The shipped asset is `assets/goblins/mireling.glb`. It embeds five skinned mesh primitives, a 55-joint skeleton, skin albedo/roughness/normal maps, eye material, leather, stitching, and nails. The anatomical skin has approximately 214,000 vertices before glTF UV/normal splits. The mesh is authored from the existing CC0 MakeHuman anatomical template and eye assets, with custom proportions, facial sculpting, pigmentation, clothing, and animation. It does not reuse the player's portrait. The reference photograph is not embedded in the asset.

The nominal idle crown-to-sole height is one third of the bundled player's standing mesh bounds (including shoes and hair), approximately 0.442 meters. Breathing and gait introduce small natural height changes. This ratio currently targets the bundled avatar; a custom replacement avatar with different crown-to-eye proportions may differ.

## Behavior and motion

Residents stream from mountain dens, with four goblins per home after den geometry is ready. Normal gameplay has no independent surface spawner. They wander near their home, pause, notice nearby players, approach, and swipe at close range. Spectators are ignored. As with the existing orcs, the swipe is a visual behavior; this change does not introduce a health/damage system.

The four independently authored clips are `Idle`, `Scamper`, `Alert`, and `Swipe`. They include crouched two-bone leg IK, planted stance travel, lifted swing feet, foot roll, arm counter-motion, articulated fingers, head motion, breathing, and short eyelid blinks. The gait's Hermite return matches stance velocity at both ends. Runtime crossfades take 150 ms; locomotion playback follows accepted world displacement and authoring stride metadata. Pausing also pauses the animation players.

Movement uses the shared tiled navigator and swept walk/settle queries with a small radius and step height. The existing world adapter conservatively checks the player's body clearance, so goblins do not gain access to otherwise blocked low passages. Neighbor separation keeps residents from stacking. Spawning checks support and clearance; distance-based den streaming bounds overhead.

## Rebuild and verify

```sh
tools/.venv/bin/python -u tools/build_goblin.py
tools/.venv/bin/python -u tools/check_goblin.py
tools/.venv/bin/python -u tools/check_goblin_lod.py
cargo test goblins
cargo build --release
tools/smoke_goblins.sh
```

The builder produces the GLB, `authoring.json`, an editable `tools/build/mireling.blend`, and `docs/goblin-preview.png`. Build dependencies are the same Blender/MPFB environment used by the avatar tools. The runtime needs only the GLB; stride metadata is compiled into the executable.

The asset checker imports the actual GLB and checks clip names, skin attributes, joint count, finite sampled poses, nominal floor bounds, and idle/walk loop continuity. It renders imported idle, walking, and blinking poses into `tools/build/`. It is a sampled regression check, not a claim that every possible terrain contact or pose has been exhaustively verified.

`HITHER_GOBLIN_PREVIEW=1 target/release/hither-sdf` provides a close, reproducible spectator view of a resident in the courtyard. Normal launches spawn goblins only in their mountain dens. The smoke script runs this view on a private Xvfb display and saves in-game captures without controlling the desktop.

## Verified result

The exported GLB passes the skin/clip checks and sampled pose validation. Idle height is 1.024 normalized meters; the final sampled walking soles remain within 0.00011 normalized meters of the floor. `cargo check`, focused goblin tests, release compilation, and the private-display runtime smoke check passed. Runtime captures confirm the model loads, changes position and pose, and responds to pause. See `docs/goblin-in-game.png` and `docs/goblin-preview.png`.

## Distance detail

The gameplay asset contains four whole-character levels: 69,948 / 28,144 / 7,588 / 2,734 triangles. Selection uses projected crown height at the actual world render resolution and FOV, with thresholds of 600 / 220 / 80 pixels and 10% hysteresis. Every part uses the original live skeleton. Fine skin sculpt, pores and clothing folds are baked into tangent-space normal maps. The untouched master is available through `HITHER_GOBLIN_PREVIEW=1 HITHER_GOBLIN_SCULPT=1`.

The two closest levels use a separate 2,734-triangle whole-character shadow counterpart across the five parts on layer 1. At the two distant levels, the visible mesh is already cheap and also casts shadows; its duplicate proxy is hidden and stops updating animated bounds. World lights already include that layer; gameplay cameras exclude it. Proxies use the same joints and dynamic animated bounds. Resident physics runs at 30 Hz with interpolated rendering and the existing swept collision checks. All resident positions used for collision and separation are authoritative simulation positions. Pause freezes the simulation clock.

Requested locomotion speed is separate from observed animation speed. Small world-space steps retain a rounding remainder so distant dens do not prevent acceleration at high FPS. The remainder never includes collision-rejected motion. Residents rotate through the shared planning budget; physics work does not consume another resident's planning slice.
