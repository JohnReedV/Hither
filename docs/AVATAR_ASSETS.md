# Avatar asset provenance

The runtime avatar is a fitted/animated assembly using MakeHuman CC0 human geometry and CC0 system assets. It does not contain Call of Duty assets; that reference describes the grounded game-art direction only.

- [MakeHuman asset license](https://github.com/makehumancommunity/makehuman/blob/master/LICENSE.ASSETS.md): CC0 core assets.
- [MakeHuman CC0 system asset pack](https://static.makehumancommunity.org/assets/assetpacks/makehuman_system_assets.html): `male_casualsuit01` tailored into an open jacket/jeans, `male_casualsuit06` undershirt, `shoes02`, `short02` hair, high-poly eye geometry, and young light-skinned male diffuse UV base. Pack headers explicitly record their CC0 release. The fleece-edge geometry is authored in this project. The earlier hood and generic eyebrow meshes are no longer included.
- [MPFB2](https://github.com/makehumancommunity/mpfb2): build-time Blender tooling and anatomical targets; see its own software license. Its Python code is not embedded in the game or GLB.

The default face is an artistic approximation of the user's reference photographs. Those photographs are not copied into the distributed project. The skin UV map was edited with the imagegen skill to add facial-hair/skin appearance while retaining the human mesh's UV layout; geometry fitting, material setup, rigging and animation are performed in Blender. Earlier billboard/atlas images are not used.

## Initial skin pass (superseded by the likeness revision below)

Initial destination: `assets/avatars/source/skin_basecolor.png` (1254 × 1254 PNG), subsequently revised below. References: original CC0 skin UV texture, user's white-shirt forest portrait, and plaid-jacket mountain portrait with sunglasses on the head.

Prompt used:

> Edit image 1, a 2048x2048 unwrapped human skin diffuse texture for a REAL 3D mesh. Images 2 and 3 are ONLY likeness references of the user. Output the same square UV texture map with EXACT identical island layout, coordinates, outline, orientation and proportions. Keep all body islands unchanged. On the sideways face island at the right side, edit only facial skin coloration/details to resemble references: fair skin, natural subtle pores, brown mustache above the existing lips, a small separate soul patch and a narrow brown goatee around existing chin, light jaw stubble with mostly clean cheeks. The man has a distinct separated mustache and narrow chin beard, NOT a full beard. Preserve exact location of lips, nose, eyes, ears and all seams. Face is sideways on purpose, do not straighten or relocate it. Avoid baked lighting/highlights/shadows. Desired style is grounded realistic game character, a little softer than raw photography, not cartoon. No clothes or hair on body or scalp. Preserve the original texture registration perfectly. Deliver texture only, NO mockup, NO human portrait, NO scene.

The tool returned a lower-resolution square image than requested. That superseded skin pass used explicit UV re-registration around eyes, nose, lips and chin. The current pipeline no longer uses this texture or that warp; both were removed from the repo during cleanup. Prompts below remain as provenance.

## Likeness revision

Built-in image generation (not the CLI) produced the earlier `assets/avatars/source/skin_basecolor.png` (now removed). References were the preceding UV map, the user's gray-hoodie portrait and white-T-shirt forest portrait. Final prompt:

> Use case: identity-preserve. Asset: diffuse UV texture map for a real rigged 3D human. Image 1 is the EDIT TARGET, an unwrapped skin texture. Images 2 and 3 are IDENTITY references of the same user. His current model is too generic: improve the face texture to unmistakably match his features. Edit ONLY the sideways head/face island on the RIGHT of image 1. Keep EXACT normalized UV registration: eyes, lips, nostrils and ear centers must not move. Preserve every other body island. His visible traits: pale slightly pink warm skin with light natural unevenness, subtle under-eye coloration, distinct medium-brown eyebrows, a FULLER medium brown mustache extending down at its corners, small separate dark soul patch, narrow dark brown goatee along the chin/jaw edge with clean cheeks and sparse light stubble. Lips are muted pink with a defined cupid's bow. Face MUST resemble the man in the two portrait references, not a generic male. No glossy highlights, no baked directional lighting, no painted eye irises (mesh has separate eyes), no hair on scalp. Preserve this sideways UV layout exactly, no portrait mockup. More photographic facial detail but a soft, grounded video-game surface, NOT toon or waxy. Output a high-detail square texture, preferably 2048x2048; do not add text or change body map.

The prompt records intent, not a guarantee of exact likeness. [MediaPipe FaceLandmarker](https://ai.google.dev/edge/api/mediapipe/python/mp/tasks/vision/FaceLandmarker) measured two portraits locally. Eye-aligned landmark offsets were averaged and capped at 14 mm, then baked into `assets/avatars/source/face_fit.json`. Raw reference photos are not shipped. Existing MakeHuman anatomical targets plus a smooth front-face deformation shape the mesh; this is not a photogrammetric scan.

## Matching outfit revision

Built-in image generation produced `assets/avatars/source/outfit_basecolor.png` (1254 × 1254 PNG), embedded in the GLB and packed in the Blender template. Inputs: original `male_casualsuit01_diffuse.png` as edit target, earlier realistic front-view character render as outfit reference. Final prompt:

> Use case: precise-object-edit. Asset: base-color UV texture for a 3D game outfit. Image 1 is the EDIT TARGET: preserve the EXACT normalized positions, rotations, shapes and outlines of EVERY UV island. Image 2 is the outfit reference. Replace ONLY the blue shirt fabric islands (two large torso pieces at left, two sleeves at right, narrow collar strip middle right) with the reference's dark navy flannel plaid: muted red crossing stripes, thin tan/cream crossing lines, very dark navy main squares. Preserve cloth folds, seams, buttons, pockets and all garment UV layout exactly. Recolor the two denim leg islands in the center to faded dark charcoal black jeans, NOT blue jeans. Make shoe islands charcoal hiking leather with dark laces. Preserve realistic woven fabric detail and mild wear. Remove text/logo in top left. Fill the otherwise unused background with a uniform beige sherpa fleece microtexture, so it can also be sampled for jacket lining. No character render, no lighting, no clothing arrangement changes. Output exactly this flat square UV map at high detail, ideally 2048x2048.

The existing shoe mesh uses its own original CC0 map with a dark material tint. Generated shoe islands are unused. Clothing construction, thickness, fitting, UV sampling, rig weights and animations are actual editable 3D data, not generated character sprites.

## Hoodless portrait-fitted revision

The hood was removed from both the GLB and editable template. Built-in imagegen produced `assets/avatars/source/face_portrait.png` (1254²), using the user's white-T-shirt forest portrait as the main identity reference and gray-hoodie portrait as supporting reference. Final prompt:

> Use case: identity-preserve. Image 1 is the main EDIT TARGET and exact identity; image 2 is supporting identity. Produce a face texture reference plate for projecting onto a 3D game head. This MUST be the same man, with his exact facial proportions and recognizable identity, NOT a generic handsome man or game avatar. Straight-on, eye-level, no head tilt or perspective exaggeration, closed relaxed mouth, neutral expression matching reference 1. Crop to complete head and a short bare neck, square image. Hair is his own dark brown tousled side-swept fringe, green-gray/hazel eyes, brown eyebrows, full brown mustache with downward corners, separate small soul patch and narrow beard on chin and lower jaw with mostly clean cheeks. Preserve individual details of his face, nose, eyelids, lips and skin. Flatten directional illumination into soft even diffuse frontal lighting without shadows or shiny highlights: this image is used as a color texture, not final lit rendering. Background uniform muted skin-beige. No clothing, hat, glasses, jewelry, room or trees. Photographic identity and color detail, no stylization, no beauty retouching or smooth plastic skin. Do not elongate/narrow the face or change his nose. Head takes 85% image height, ears both visible, enough margin above hair and below chin.

`tools/portrait_head.py` applies a bounded smooth deformation using saved 468-landmark correspondences in `portrait_fit.json`, then blends the generated plate with the body material and bakes it into `assets/avatars/source/skin_fitted.png` (4096²). This is native Blender material baking, not an in-game face plane. The fitted skin and eye texture are embedded in the GLB and packed in the template. The generated source plate is derived from the user's photos; the original photos are not distributed. This remains an artistic reconstruction, not a scan or a guarantee of exact likeness.

## Clothing, hairline and ear repairs

The user's later side-view photograph guided a roughly 28% ear-height reduction and reduced protrusion, applied to the ear vertex group after face fitting. The existing 3D hair mesh is fitted to the evaluated head surface; detached photographed forehead hair is excluded from the skin material bake using the eyebrow landmarks. The undershirt is subdivided and tucked behind the outer jacket to eliminate collar-area poke-through. These are native Blender geometry/material changes; no new AI image was generated for this repair. The original side-view photograph is not shipped.

## Brow, ear-to-neck and side-hair cleanup

`assets/avatars/source/skin_clean_base.png` is an unmodified copy of the CC0 `young_caucasian_male/young_lightskinned_male_diffuse.png` map. It replaces the earlier generated UV base in the build, preventing a second set of eyebrows from bleeding through above the portrait brows. The old UV-registration warp is no longer applied. The portrait blend now fades out before the ears/rear jaw to avoid projecting pale background onto the ear-to-neck transition. Native mesh fitting also reduces excess volume at the temples and sides without scaling the fringe into the forehead. No new generated images or original user photos were added for these changes.

## Green iris restoration

The eyes retain the original MakeHuman CC0 `eyes/materials/green_eye.png` texture and eye UVs. The facial fitting still sculpts their positions, but no longer replaces the eye surfaces with the generated portrait's browner irises. Their pupil/iris detail is on the curved eye geometry, using a separate nonmetallic material with roughness 0.3. The build's animation checker also verifies that the green eye material has not been overwritten. No new generated images or raster edits were used.

## Continuous jacket opening trim

The front fleece/zipper edges are welded hem-to-collar strips extending into the open jacket, rather than folded back over the plaid surface. Near-duplicate cut vertices are welded, the collar cut is relaxed together with adjacent cloth, and the undershirt clearance includes the narrower inner edges. Skin weights are inherited from the attached jacket seam. The builder asserts two connected, unbranched, full-height seam chains; `check_avatar_animation.py --render` includes close-up idle/walk/turn/jump views. The GLB and packed Blender template contain the same repair. See `avatar-jacket-trim.png` for the updated close-up.

## Waist layering repair

The gray T-shirt hem extends about 3 cm into the waistband and tucks behind the belt, closing the exposed dark strip between shirt and belt. `tuck_waist` in `tools/avatar_repairs.py` fits the lower hem against the evaluated jeans while preserving existing UVs and skin weights. Standing, walking and jumping waist previews were checked; the rebuilt GLB and packed Blender template include the same repair.

## Blue denim, darker tee and movement revision

The jeans now use the original CC0 `male_casualsuit01_diffuse.png` denim islands with a restrained blue material tint; the generated charcoal-denim islands are no longer sampled by the jeans. The plaid jacket retains its existing generated outfit map. The T-shirt uses a darker gray material. No new image generation or raster editing was needed. The rear jacket skirt is fitted outside the evaluated denim hips with additional animation clearance, addressing the left rear hip poke-through.

Ten native skeletal clips now include left/right turn-in-place footwork. The walk uses analytic leg/arm posing, weight transfer, opposing shoulder/hip motion and soft hands. Jump/fall/land have gathering, extension and compression/recovery phases. Existing meshes, facial fitting and skin UVs remain unchanged; the packed template and GLB contain the same baked motion. Regression and posed rear-view checks cover the repaired clothing while walking, turning and jumping.
