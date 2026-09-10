# Hither Avatar v1

One shareable file: **`<name>.hither-avatar.glb`**, a glTF 2.0 binary (GLB). The suffix is descriptive; Bevy loads its standard `.glb` extension. No ZIP extraction, executable scripts, proprietary mesh format, sidecar manifest or loose textures are needed at runtime.

The bundled `assets/avatars/default.hither-avatar.glb` is the reference implementation, not a separate hard-coded character. It contains 107,152 triangles, 53 joints, seven meshes, eight materials, eight embedded images and ten skeletal clips (about 27 MiB). The hoodless open plaid jacket, fleece edges, dark gray undershirt and blue denim jeans are skinned 3D surfaces, not a character billboard. The portrait-fitted face uses ordinary UVs and a baked 4096² base-color texture. The eyeballs retain their anatomical UVs and separate green iris material.

## Coordinates and rig

- Use meters, right-handed coordinates, **+Y up, +Z character-forward**, feet at Y=0 and the center between the feet at X=Z=0. Blender's exporter converts its Z-up / -Y-forward character into these coordinates.
- One default scene containing one character/armature hierarchy. No cameras or lights. All character meshes must belong to this hierarchy.
- A real deforming humanoid skin, with 1–128 joints per skin, at most four normalized influences per vertex, inverse-bind matrices and unique bone-node names. Keep the face, clothes and hair attached to the same skeleton.
- Topology and UVs are not globally fixed. Include that avatar's own clips; v1 does **not** retarget an arbitrary skeleton to the default animations.
- Metadata maps the hips, head, hands and feet to joints. The runtime uniformly scales the asset's declared eye height to the existing player eye height. Collision dimensions and gameplay physics are independent of the cosmetic asset.

## Materials and budget

- Standard glTF metallic/roughness PBR materials. Base color and emissive maps use sRGB; normal, roughness, metalness and occlusion maps use linear values. glTF normal maps use +Y (OpenGL convention).
- Embed PNG/JPEG images and the mesh buffer in the GLB. No `uri` fields, including data URLs or remote references. No required compression extensions.
- Skin/cloth should be nonmetallic with material-appropriate roughness. Use OPAQUE for skin and clothes; prefer alpha MASK for hair/eyelashes.
- Hard file limit: 64 MiB. Authoring targets: ≤120,000 triangles for a detailed close-up avatar (prefer ≤60,000 for multiplayer crowds), ≤16 materials, textures no larger than 4096², preferably 1024–2048². The topology/texture targets are recommendations, not currently enforced allocation limits.

## Required animation clips

Names are case-sensitive and each must occur exactly once. Bake bone transforms, not Blender constraints. Use in-place locomotion: the game owns player translation and heading. Keep loop endpoints continuous and do not animate world/root travel.

| Name | Runtime behavior |
|---|---|
| `Idle` | Looped relaxed stance, restrained breathing |
| `Walk` | Looped forward locomotion |
| `WalkBack` | Looped backward locomotion |
| `StrafeLeft` | Looped leftward locomotion, body still facing forward |
| `StrafeRight` | Looped rightward locomotion, body still facing forward |
| `Jump` | Once on ascent; hold final pose until falling |
| `Fall` | Settle into descent; hold final pose until contact |
| `Land` | Short one-shot landing recovery |

Optional backward-compatible clips: `TurnLeft` and `TurnRight`, looping in-place stepping turns without root yaw/translation. The bundled asset includes both; v1 custom avatars without them use their corresponding strafe clip. Turn cycles are normalized to the runtime's 0.72-second footwork cycle. Small mouse adjustments stay inside a standing dead zone; larger adjustments trigger body rotation synchronized to alternating steps. The player's collision position does not change.

The game crossfades states, selects movement clips using actual displacement, adjusts playback using `locomotion_speed`, and pauses all clips with the pause menu. Direction changes preserve normalized stride phase. Jump and landing clips restart on each occurrence; landing playback fits the whole recovery into the available recovery interval. The default animations are authored/baked skeletal cycles, not motion capture. Facial animation and hand/item attachment behavior are future extensions.

## Embedded metadata

Store this object at `asset.extras.hither_avatar` in the GLB JSON chunk:

```json
{
  "version": 1,
  "eye_height": 1.43,
  "locomotion_speed": 1.6,
  "forward": "+Z",
  "bones": {
    "hips": "pelvis",
    "head": "head",
    "left_hand": "hand_l",
    "right_hand": "hand_r",
    "left_foot": "foot_l",
    "right_foot": "foot_r"
  }
}
```

`eye_height` is the unscaled standing eye height in meters (0.5–2.5). `locomotion_speed` is the unscaled nominal walk speed in meters/second (0.1–10). Both must be finite. A different rig can supply different bone names. Future incompatible profiles must increment `version`; v1 files must not silently change interpretation. Unknown metadata fields may be added compatibly.

## Make a custom skin

1. Open `assets/avatars/source/default.blend` in Blender 4.2. Images are packed, so no MPFB installation or vendor files are needed to edit it. Save a copy. NLA tracks are muted for a clean authoring view; preview one track at a time.
2. Paint the existing UV textures or modify the fitted meshes. Preserve skin weights, unique joint names and eight animation tracks unless deliberately supplying a new rig/profile. Keep all exported mesh objects visible.
3. Export using `tools/export_avatar.py`, which embeds images, emits NLA tracks as clips, writes metadata, and handles the default hair tint/alpha materials.

```bash
# Only for headless authoring/export, not playing. Reuse an existing environment.
uv venv tools/.venv --python 3.11
uv pip install --python tools/.venv/bin/python bpy==4.2.0 numpy==2.4.6
tools/.venv/bin/python tools/export_avatar.py \
  --blend path/to/my-character.blend \
  --output assets/avatars/mine.hither-avatar.glb
cargo run --release -- --validate-avatar assets/avatars/mine.hither-avatar.glb
HITHER_AVATAR=avatars/mine.hither-avatar.glb cargo run --release
```

In Blender's Scripting workspace the same export module can be loaded and `export_avatar(output_path)` called without installing Python separately. Supply `--profile metadata.json` to the headless exporter when changing proportions or joint names; this authoring sidecar is embedded, not distributed.

The validator checks the GLB envelope, size, metadata, bone mapping, skeleton presence, embedded-only resources and required clip names. It is a **profile check**, not a full glTF validator or sandbox. The glTF loader performs further structural validation; visually test custom assets before distributing them. Rejected/missing custom selections and top-level load failures fall back to the default avatar. Only trusted local files should be used at this stage. A picker, downloads, multiplayer distribution and hostile-file hardening are not implemented yet.

## Rebuild the default from source assets

Editing the packed template is the fastest workflow. For a complete anatomical rebuild with `tools/build_avatar.py`, also provision these authoring-only inputs:

- MPFB2: `https://github.com/makehumancommunity/mpfb2.git`, revision `437dd513888a92399d1d3200d2e80859fae55abc`, checked out at `tools/vendor/mpfb2`.
- MakeHuman CC0 system-assets archive: `https://files2.makehumancommunity.org/asset_packs/makehuman_system_assets/makehuman_system_assets_cc0.zip`, extracted so `tools/vendor/system_assets` contains `clothes`, `hair`, `eyes`, etc.
- Archive SHA-256: `b542127a8e25547c7c29c19f2d1d2adb9a664c80396ecd694095dbc8028a0107`.
- Source maps and baked facial controls: `assets/avatars/source/skin_clean_base.png`, `outfit_basecolor.png`, `face_portrait.png`, `face_fit.json` and `portrait_fit.json`, already supplied. `skin_fitted.png` is the generated Blender material bake. The superseded `skin_basecolor.png` was removed from the repo during cleanup; its prompt history remains in `AVATAR_ASSETS.md`.
- Full rebuilding additionally needs `scipy==1.17.1` and Pillow in the authoring environment. The packed-template exporter does not need these.

Run `tools/.venv/bin/python tools/build_avatar.py`. It regenerates the packed template, runtime GLB and studio previews in `tools/build/`. Authoring dependencies are excluded from version control and never needed by the game.

`tools/avatar_animation.py` bakes 60 Hz rotations and vertical/lateral root weight transfer. Analytic two-bone IK supplies planted support steps and relaxed arm poses, requiring no runtime IK or extra joints. `tools/check_avatar_animation.py --render` checks loop seams, finite transforms, connected joints and foot targets, then renders representative front/rear movement poses. `tools/smoke_avatar.sh` checks in-game walking, stationary mouse-only turns, jumping and pause/resume in an isolated display.

`tools/avatar_repairs.py` handles compact ears, continuous hairline fitting against the evaluated head, and undershirt clearance beneath the tailored jacket. The shirt is subdivided before fitting so its triangles cannot cut across the collar surface. `tools/portrait_head.py` excludes photographed fringe above the eyebrow contour from the skin bake; the actual hair mesh supplies the hairline.

The brow/ear cleanup uses the unmodified CC0 skin base and original UV registration, eliminating the older generated brow layer. The portrait blend stops before the ears and rear jaw so extrapolated background colors cannot form pale neck patches. Side hair volume is reduced relative to the evaluated scalp surface, retaining the central swept top and the existing rig/UVs.

`tools/avatar_details.py` contains the editable jacket tailoring, facial sculpt and cloth UV-registration steps. Superseded hood construction and face UV warping have been removed. `tools/portrait_head.py` fits facial proportions and bakes a portrait-blended material onto the head's existing UV layout; no custom runtime shader is required. Facial controls were baked from local measurements with `tools/measure_face.py`, `tools/fit_face.py` and `tools/prepare_portrait_fit.py`; these optional measurement tools require MediaPipe and its FaceLandmarker task model. Rebuilding the supplied avatar uses the saved controls and generated source plate, and does not require the original photographs or MediaPipe. The measurement tools expect fresh measurements made from the appropriate source mesh and references, not retained diagnostic files from an older build.

### Optional punch overlay

The bundled avatar supplies a `Punch` clip. Exterior camera views play it over
locomotion using the same attack clock as the first-person fist. The bundled
exporter retains the striking `clavicle_r` subtree plus `spine_01`, `spine_02`,
`spine_03` and `neck_01` for chest drive and head counter-turn. The free arm and
lower body retain their locomotion channels; the free arm is carried by the
chest turn without a separate guard or counter-swing. The runtime gives the
clip's targets a masked locomotion layer with complementary punch weights.

The exterior punch lasts 0.60 seconds, reaching extension at 0.22 seconds and
recoiling immediately. First-person timing remains 0.44 seconds; camera changes
map between matching preparation, impact and recovery phases. Existing custom
avatars without this optional clip keep their locomotion; they need a `Punch`
clip to display exterior punching.

The elbow motion is derived from CMU boxing capture `14_01` and adapted for
reach, timing, wrist alignment and torso drive; see
`assets/avatars/source/PUNCH_REFERENCE.md` for source and reproduction steps.
