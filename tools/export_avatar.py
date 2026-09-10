"""Export a Hither Avatar v1 from an edited Blender template.

tools/.venv/bin/python tools/export_avatar.py --blend avatar.blend --output assets/avatars/mine.hither-avatar.glb
Also works from Blender's Scripting workspace: call export_avatar(output_path).
"""
import argparse
import json
from pathlib import Path
import struct

import bpy

PROFILE = {
    "version": 1, "eye_height": 1.43, "locomotion_speed": 1.6, "forward": "+Z",
    "bones": {"hips": "pelvis", "head": "head", "left_hand": "hand_l",
              "right_hand": "hand_r", "left_foot": "foot_l", "right_foot": "foot_r"},
}
CLIPS = ("Idle", "Walk", "WalkBack", "StrafeLeft", "StrafeRight", "Jump", "Fall", "Land")


def export_avatar(output_path, profile=None):
    output_path = Path(output_path).resolve()
    profile = profile or PROFILE
    rigs = [obj for obj in bpy.context.scene.objects if obj.type == "ARMATURE"]
    if len(rigs) != 1:
        raise ValueError("Expected exactly one armature")
    rig = rigs[0]
    if not rig.animation_data:
        raise ValueError("Missing armature animation data")
    tracks = rig.animation_data.nla_tracks
    if any(sum(track.name == name for track in tracks) != 1 for name in CLIPS):
        raise ValueError("Expected one NLA track for each required clip: " + ", ".join(CLIPS))
    rig.animation_data.action = None
    for track in tracks:
        track.mute = False
    bpy.ops.export_scene.gltf(filepath=str(output_path), export_format="GLB",
        export_animations=True, export_animation_mode="NLA_TRACKS",
        export_force_sampling=True, export_nla_strips=True, export_skins=True,
        export_def_bones=True, export_morph=False, export_extras=False,
        export_yup=True, export_materials="EXPORT", export_cameras=False,
        export_lights=False)
    raw = output_path.read_bytes()
    json_length = struct.unpack_from("<I", raw, 12)[0]
    doc = json.loads(raw[20:20 + json_length])
    doc["asset"]["extras"] = {"hither_avatar": profile}
    for mat in doc.get("materials", []):
        if mat.get("name") in ("Brown hair", "Brows", "Eyes", "Orc brown eyes"):
            mat["alphaMode"], mat["alphaCutoff"], mat["doubleSided"] = "MASK", 0.35, True
        # The default hair's Blender Multiply node is not a glTF shader node.
        # Represent its tint with the standard glTF baseColorFactor instead.
        if mat.get("name") in ("Brown hair", "Portrait hair", "Leather boots", "American blue denim"):
            mat["pbrMetallicRoughness"]["baseColorFactor"] = list(bpy.data.materials[mat["name"]].diffuse_color)
    # Export the striking arm plus the chest drive and head counter-turn.
    # The free arm and lower body keep their existing locomotion channels.
    for animation in doc.get("animations", []):
        if animation["name"] == "Punch":
            arm = rig.data.bones.get("clavicle_r")
            if arm is None:
                raise ValueError("Punch export requires the right clavicle bone")
            targets = {arm.name, *(bone.name for bone in arm.children_recursive),
                       "spine_01", "spine_02", "spine_03", "neck_01"}
            animation["channels"] = [channel for channel in animation["channels"]
                if doc["nodes"][channel["target"]["node"]].get("name", "") in targets]
    encoded = json.dumps(doc, separators=(",", ":")).encode()
    encoded += b" " * (-len(encoded) % 4)
    binary_chunk = raw[20 + json_length:]
    output_path.write_bytes(struct.pack("<4sII", b"glTF", 2, 20 + len(encoded) + len(binary_chunk))
        + struct.pack("<I4s", len(encoded), b"JSON") + encoded + binary_chunk)
    for track in tracks:
        track.mute = True
    print("EXPORTED", output_path, "clips", [c["name"] for c in doc["animations"]])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--blend", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--profile", help="Optional JSON metadata for a different rig/proportions")
    args = parser.parse_args()
    bpy.ops.wm.open_mainfile(filepath=str(Path(args.blend).resolve()))
    profile = json.loads(Path(args.profile).read_text()) if args.profile else PROFILE
    export_avatar(args.output, profile)
    # bpy 4.2's standalone Python wheel can fault during interpreter teardown
    # after opening a packed .blend. Export has completed synchronously here;
    # bypass Blender's teardown only on this successful CLI path, not errors.
    import os
    import sys
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(0)
