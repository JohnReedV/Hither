"""Build the bundled Hither avatar from CC0 MakeHuman meshes with Blender 4.2.

Run with tools/.venv/bin/python tools/build_avatar.py. See docs/HITHER_AVATAR_V1.md.
Authoring dependencies stay in tools/vendor; the game only needs the exported GLB.
"""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / "tools/build"
BUILD.mkdir(parents=True, exist_ok=True)
sys.path.insert(0, str(ROOT / "tools/vendor/mpfb2/src"))

import bpy
from mathutils import Vector

# Keep MPFB's authoring cache inside this project, including in headless Blender.
bpy.utils.extension_path_user = lambda *a, **kw: str(BUILD / "mpfb")
import mpfb
mpfb.get_preference = lambda key: False if key.startswith("enable") else ""
mpfb.register()
from mpfb.services.humanservice import HumanService
from mpfb.services.targetservice import TargetService
from export_avatar import export_avatar
from avatar_details import sculpt_face, tailor_jacket, crop_mesh, register_cloth_border
from portrait_head import fit_portrait
from avatar_repairs import smaller_ears, tuck_undershirt, fit_hair_to_head, tuck_waist, fit_jacket_over_jeans, repair_jacket_shoulders
from avatar_animation import pose, bake_clips

ASSETS = ROOT / "tools/vendor/system_assets"
DATA = ROOT / "tools/vendor/mpfb2/src/mpfb/data"


def material(name, texture=None, normal=None, roughness=0.8, color=(1, 1, 1, 1), alpha=False):
    mat = bpy.data.materials.new(name)
    mat.use_fake_user = True
    mat.use_nodes = True
    mat.diffuse_color = color
    pbr = mat.node_tree.nodes.get("Principled BSDF")
    pbr.inputs["Base Color"].default_value = color
    pbr.inputs["Roughness"].default_value = roughness
    pbr.inputs["Metallic"].default_value = 0
    pbr.inputs["Specular IOR Level"].default_value = 0.28
    if texture:
        tex = mat.node_tree.nodes.new("ShaderNodeTexImage")
        tex.image = bpy.data.images.load(str(texture), check_existing=True)
        mat.node_tree.links.new(tex.outputs["Color"], pbr.inputs["Base Color"])
        if color != (1, 1, 1, 1):
            tint = mat.node_tree.nodes.new("ShaderNodeMixRGB")
            tint.blend_type = "MULTIPLY"
            tint.inputs[0].default_value = 1.0
            tint.inputs[2].default_value = color
            mat.node_tree.links.new(tex.outputs["Color"], tint.inputs[1])
            mat.node_tree.links.new(tint.outputs[0], pbr.inputs["Base Color"])
        if alpha:
            mat.node_tree.links.new(tex.outputs["Alpha"], pbr.inputs["Alpha"])
            mat.surface_render_method = "DITHERED"
            mat.use_backface_culling = False
    if normal:
        tex = mat.node_tree.nodes.new("ShaderNodeTexImage")
        tex.image = bpy.data.images.load(str(normal), check_existing=True)
        tex.image.colorspace_settings.name = "Non-Color"
        n = mat.node_tree.nodes.new("ShaderNodeNormalMap")
        n.inputs["Strength"].default_value = 0.45
        mat.node_tree.links.new(tex.outputs["Color"], n.inputs["Color"])
        mat.node_tree.links.new(n.outputs["Normal"], pbr.inputs["Normal"])
    return mat


def apply_material(obj, mat):
    obj.data.materials.clear()
    obj.data.materials.append(mat)


def add_asset(body, category, name, mat):
    obj = HumanService.add_mhclo_asset(str(ASSETS / category / name / (name + ".mhclo")),
        body, asset_type=category.capitalize(), subdiv_levels=0, material_type="NONE")
    apply_material(obj, mat)
    return obj


def finalize(body, rig):
    # Apply static masks/subdivision while retaining deformation weights.
    for obj in list(bpy.context.scene.objects):
        if obj.type != "MESH":
            continue
        bpy.ops.object.select_all(action="DESELECT")
        obj.select_set(True)
        bpy.context.view_layer.objects.active = obj
        for modifier in list(obj.modifiers):
            if modifier.type != "ARMATURE":
                bpy.ops.object.modifier_apply(modifier=modifier.name)
        for face in obj.data.polygons:
            face.use_smooth = True
        # All joints in a GLB vertex use at most four normalized influences.
        bpy.ops.object.vertex_group_limit_total(limit=4)
        bpy.ops.object.vertex_group_normalize_all(lock_active=False)
        if obj.name == "Open plaid jacket":
            repair_jacket_shoulders(obj)
    bake_clips(rig)
    bpy.context.view_layer.update()
    bpy.ops.file.pack_all()
    bpy.ops.wm.save_as_mainfile(filepath=str(ROOT / "assets/avatars/source/default.blend"))
    # NLA strips become the named clips. All images are packed into the GLB.
    export_avatar(ROOT / "assets/avatars/default.hither-avatar.glb")
    for track in rig.animation_data.nla_tracks:
        track.mute = True
    pose(rig, "Idle", 0)


def render_preview(rig):
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 16
    scene.render.resolution_x = 960
    scene.render.resolution_y = 960
    scene.render.resolution_percentage = 100
    scene.world.color = (0.18, 0.18, 0.18)
    scene.view_settings.view_transform = "AgX"
    for name, pos, energy, size in [("Key", (-2, -3, 4), 400, 4), ("Fill", (2, -1, 2), 180, 3), ("Rim", (0, 2, 3), 250, 2)]:
        data = bpy.data.lights.new(name, "AREA")
        data.energy, data.shape, data.size = energy, "DISK", size
        light = bpy.data.objects.new(name, data)
        scene.collection.objects.link(light)
        light.location = pos
        light.rotation_euler = (Vector((0, 0, 1)) - light.location).to_track_quat("-Z", "Y").to_euler()
    data = bpy.data.cameras.new("PreviewCamera")
    cam = bpy.data.objects.new("PreviewCamera", data)
    scene.collection.objects.link(cam)
    cam.location = (0.85, -3.5, 1.35)
    cam.rotation_euler = (Vector((0, 0, 0.8)) - cam.location).to_track_quat("-Z", "Y").to_euler()
    data.type, data.ortho_scale = "ORTHO", 1.9
    scene.camera = cam
    scene.render.filepath = str(BUILD / "avatar-preview.png")
    bpy.ops.render.render(write_still=True)
    cam.location = (0.1, -3.0, 1.44)
    cam.rotation_euler = (Vector((0, -0.01, 1.42)) - cam.location).to_track_quat("-Z", "Y").to_euler()
    data.ortho_scale = 0.43
    scene.render.filepath = str(BUILD / "avatar-face.png")
    bpy.ops.render.render(write_still=True)
    cam.location = (.95, -1.35, 1.44)
    cam.rotation_euler = (Vector((0, -.01, 1.40)) - cam.location).to_track_quat("-Z", "Y").to_euler()
    scene.render.filepath = str(BUILD / "avatar-face-side.png")
    bpy.ops.render.render(write_still=True)
    cam.location = (.12, -3, .9)
    cam.rotation_euler = (Vector((0, -.02, .85)) - cam.location).to_track_quat("-Z", "Y").to_euler()
    data.ortho_scale = .38
    scene.render.filepath = str(BUILD / "avatar-waist.png")
    bpy.ops.render.render(write_still=True)


def build():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    macros = TargetService.get_default_macro_info_dict()
    macros.update(gender=1.0, age=0.30, muscle=0.48, weight=0.40, proportions=0.5, height=0.5)
    # MakeHuman's anatomical template selector; this is an authoring preset.
    macros["race"] = {"caucasian": 1.0, "african": 0.0, "asian": 0.0}
    body = HumanService.create_human(macro_detail_dict=macros)
    body.name = "Body"
    targets = {
        "head/head-oval": 0.32,
        "head/head-scale-horiz-decr": 0.20,
        "head/head-scale-vert-incr": 0.15,
        "head/head-fat-decr": 0.22,
        "nose/nose-scale-vert-incr": 0.62,
        "nose/nose-scale-depth-incr": 0.28,
        "nose/nose-trans-forward": 0.36,
        "nose/nose-point-down": 0.12,
        "nose/nose-width2-decr": 0.22,
        "chin/chin-width-decr": 0.28,
        "chin/chin-height-incr": 0.12,
        "chin/chin-prominent-incr": 0.18,
        "cheek/l-cheek-volume-decr": 0.12,
        "cheek/r-cheek-volume-decr": 0.12,
        "eyes/l-eye-height1-incr": 0.14,
        "eyes/r-eye-height1-incr": 0.14,
        "eyes/l-eye-trans-out": 0.08,
        "eyes/r-eye-trans-out": 0.08,
        "eyebrows/eyebrows-trans-down": 0.08,
        "mouth/mouth-scale-horiz-incr": 0.10,
    }
    for target, weight in targets.items():
        path = DATA / "targets" / (target + ".target.gz")
        if not path.exists():
            raise FileNotFoundError(path)
        TargetService.load_target(body, str(path), weight=weight)
    TargetService.bake_targets(body)
    rig = HumanService.add_builtin_rig(body, "game_engine", import_weights=True)
    rig.name = "HitherAvatar"
    # A clean, correctly registered CC0 base prevents the earlier generated
    # eyebrows/ear details from showing through the personalized portrait bake.
    skin = ROOT / "assets/avatars/source/skin_clean_base.png"
    apply_material(body, material("Skin", skin, roughness=0.72))
    suit_dir = ASSETS / "clothes/male_casualsuit01"
    outfit = ROOT / "assets/avatars/source/outfit_basecolor.png"
    cloth_mat = material("Navy red plaid and charcoal denim", outfit,
                         suit_dir / "male_casualsuit01_normal.png", 0.94)
    clothes = add_asset(body, "clothes", "male_casualsuit01", cloth_mat)
    pants = clothes.copy()
    pants.data = clothes.data.copy()
    bpy.context.scene.collection.objects.link(pants)
    pants.name = "Blue denim jeans"
    apply_material(pants, material("American blue denim",
        suit_dir / "male_casualsuit01_diffuse.png",
        suit_dir / "male_casualsuit01_normal.png", .94, color=(.68, .84, 1., 1.)))
    crop_mesh(pants, lambda p: p.z >= .85)
    tailor_jacket(clothes, material("Beige sherpa lining", outfit, roughness=.98))
    register_cloth_border(clothes, outfit)
    fit_jacket_over_jeans(clothes, pants)
    tee = add_asset(body, "clothes", "male_casualsuit06", material("Heather gray cotton",
        normal=ASSETS / "clothes/male_casualsuit06/male_casualsuit06_normal.png",
        roughness=.94, color=(.09,.095,.10,1)))
    tee.name = "Gray crewneck T-shirt"
    # Hidden sleeves/shoulders are removed from the undershirt so they cannot
    # poke through the outer jacket during shoulder/elbow animation.
    crop_mesh(tee, lambda p: p.z < .84 or abs(p.x) > .105 or p.y > -.035)
    tuck_undershirt(tee, clothes)
    tuck_waist(tee, pants)
    # No hood: the jacket has only its folded flannel collar and fleece trim.
    add_asset(body, "clothes", "shoes02", material("Leather boots", ASSETS / "clothes/shoes02/shoes02_diffuse.png", roughness=0.90, color=(.20,.22,.24,1)))
    hair = add_asset(body, "hair", "short02", material("Brown hair", ASSETS / "hair/short02/short02_diffuse.png", roughness=0.92, color=(0.32, 0.25, 0.20, 1), alpha=True))
    # Break the symmetric fringe with a gentle side sweep, retaining the
    # original layered strand cards and their transparent, irregular edges.
    for vertex in hair.data.vertices:
        x, y, z = vertex.co
        front = max(0.0, min(1.0, (-y - 0.015) / 0.085))
        vertex.co.x += front * max(0.0, min(1.0, (z - 1.44) / 0.12)) * 0.012
    eyes = add_asset(body, "eyes", "high-poly", material("Eyes", ASSETS / "eyes/materials/green_eye.png", roughness=0.3, alpha=True))
    # Brows are the portrait's own shape/color on the face, not generic cards.
    sculpt_face()
    fit_portrait(body, material("Portrait face", ROOT / "assets/avatars/source/face_portrait.png", roughness=.86))
    smaller_ears(body)
    fit_hair_to_head(body, hair)
    bpy.context.view_layer.update()
    print("BONES", [b.name for b in rig.pose.bones])
    print("BOUNDS", body.dimensions[:])
    for b in rig.pose.bones:
        print("BONE", b.name, list(b.head), list(b.tail))
    finalize(body, rig)
    render_preview(rig)
    return body, rig


if __name__ == "__main__":
    build()
    # The standalone bpy wheel can fault on teardown after Cycles rendering.
    # Reach this only after both exports and both renders finish successfully.
    import os
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(0)
