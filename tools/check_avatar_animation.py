"""Headless Blender regression checks and representative animation renders.

tools/.venv/bin/python tools/check_avatar_animation.py [--render]
"""
import math
import json
import struct
import os
from pathlib import Path
import sys
import bpy
import numpy as np
from mathutils import Vector
from mathutils.bvhtree import BVHTree
from avatar_animation import pose, step, CLIP_SECONDS, punch_capture
from hand_pose import finger_angles

ROOT = Path(__file__).resolve().parents[1]
bpy.ops.wm.open_mainfile(filepath=str(ROOT / "assets/avatars/source/default.blend"))
rig = next(o for o in bpy.context.scene.objects if o.type == "ARMATURE")
eyes = next(o for o in bpy.context.scene.objects if o.type == "MESH"
            and any(m and m.name == "Eyes" for m in o.data.materials))
assert all(eyes.data.materials[p.material_index].name == "Eyes"
           for p in eyes.data.polygons), "Portrait material replaced green irises"
assert any(n.type == "TEX_IMAGE" and n.image and n.image.name.startswith("green_eye")
           for n in bpy.data.materials["Eyes"].node_tree.nodes), "Missing green iris texture"
print("Green iris material checks passed")
for track in rig.animation_data.nla_tracks:
    track.mute = True
rig.animation_data.action = None

def snapshot():
    bpy.context.view_layer.update()
    return {b.name: b.matrix.copy() for b in rig.pose.bones}

for kind in CLIP_SECONDS:
    pose(rig, kind, 0)
    start = snapshot()
    pose(rig, kind, 1)
    end = snapshot()
    if kind in ("Idle", "Walk", "WalkBack", "StrafeLeft", "StrafeRight", "TurnLeft", "TurnRight"):
        assert max(abs(start[n][i][j]-end[n][i][j]) for n in start for i in range(4) for j in range(4)) < .0001, kind+" loop seam"
    for frame in range(25):
        phase = frame/24
        pose(rig, kind, phase)
        snap = snapshot()
        assert all(math.isfinite(v) for m in snap.values() for row in m for v in row)
        for side, shift in (("l", 0), ("r", .5)):
            # IK must be rotations only; detached translated joints are invalid.
            for name in ("thigh_", "calf_", "foot_", "upperarm_", "lowerarm_", "hand_"):
                assert rig.pose.bones[name+side].location.length < .0001, (kind, phase, name, "joint translation")
            if kind in ("Walk", "WalkBack"):
                travel, lift, _ = step(phase+shift)
                target = rig.data.bones["foot_"+side].head_local.copy()
                target.x = .123 if side == "l" else -.123
                target.y += travel*(-1 if kind == "WalkBack" else 1)
                target.z += lift
                error = (rig.pose.bones["foot_"+side].head-target).length
                assert error < .005, (kind, phase, side, "unreachable foot", error)
print("Animation checks passed: loop seams, finite transforms, connected joints, foot targets")

# Ensure source-only cleanup still reproduces the clips in the packed template.
for track in rig.animation_data.nla_tracks:
    kind = track.name
    action = track.strips[0].action
    frames = round(CLIP_SECONDS[kind]*60)
    for frame in (0, frames//4, frames//2, frames*3//4, frames):
        rig.animation_data.action = None
        pose(rig, kind, frame/frames)
        expected = snapshot()
        rig.animation_data.action = action
        bpy.context.scene.frame_set(frame)
        actual = snapshot()
        error = max(abs(expected[n][i][j]-actual[n][i][j])
                    for n in expected for i in range(4) for j in range(4))
        assert error < .0005, (kind, frame, "baked/source mismatch", error)
rig.animation_data.action = None
print("Packed animation/source equivalence checks passed")

raw = (ROOT / "assets/avatars/default.hither-avatar.glb").read_bytes()
doc = json.loads(raw[20:20+struct.unpack_from("<I", raw, 12)[0]])
punch = next(a for a in doc["animations"] if a["name"] == "Punch")
targets = {doc["nodes"][c["target"]["node"]]["name"] for c in punch["channels"]}
assert {"upperarm_r", "lowerarm_r", "hand_r"} <= targets
right_arm = rig.data.bones["clavicle_r"]
assert targets == {right_arm.name, *(b.name for b in right_arm.children_recursive),
                   "spine_01", "spine_02", "spine_03", "neck_01"}, targets
for phase in (0, .10/.60, .22/.60, .37/.60, 1):
    pose(rig, "Idle", 0)
    baseline = {b.name: b.matrix_basis.copy() for b in rig.pose.bones}
    pose(rig, "Punch", phase)
    actual = {b.name: b.matrix_basis.copy() for b in rig.pose.bones}
    for name in baseline.keys()-targets:
        assert max(abs(baseline[name][i][j]-actual[name][i][j])
                   for i in range(4) for j in range(4)) < .0001, (phase, name, "other body part moved")
pose(rig, "Punch", .22/.60)
from mathutils import Quaternion
for name, angle in finger_angles(True).items():
    assert rig.pose.bones[name].rotation_quaternion.rotation_difference(
        Quaternion((1, 0, 0), angle)).angle < .0001, (name, "first-person fist mismatch")
# Check visible mechanics independently: straight fist travel, full reach,
# outward elbow clearance, chest drive, aligned wrist and connected joints.
wrists, elbows = [], []
for i in range(33):
    t = .10+i*(.27/32)
    pose(rig, "Punch", t/.60)
    upper, lower = (rig.pose.bones[n] for n in ("upperarm_r", "lowerarm_r"))
    wrists.append(lower.tail.copy())
    elbows.append(lower.head.copy())
    assert abs((lower.head-upper.head).length-upper.length) < .0001
    assert abs((lower.tail-lower.head).length-lower.length) < .0001
assert min(p.x for p in wrists) >= wrists[0].x-.001, "fist swings away from the target"
assert min(e.x-w.x for e, w in zip(elbows, wrists)) < -.065, "elbow must clear the fist lane"
pose(rig, "Punch", .22/.60)
upper, lower = (rig.pose.bones[n] for n in ("upperarm_r", "lowerarm_r"))
assert (lower.tail-upper.head).length/(upper.length+lower.length) > .98, "strike needs full extension"
assert lower.tail.y < wrists[0].y-.28, "strike lacks forward travel"
assert -.07 < lower.tail.x < 0, "rear cross must converge toward the face centerline"
hand = rig.pose.bones["hand_r"]
bind_forward = (rig.data.bones["middle_01_r"].head_local-hand.bone.head_local).normalized()
forward = (hand.matrix.to_quaternion() @ hand.bone.matrix_local.to_quaternion().inverted()) @ bind_forward
assert forward.dot((lower.tail-lower.head).normalized()) > .999, "cocked impact wrist"
pose(rig, "Idle", 0)
rest_chest = rig.pose.bones["spine_03"].matrix.to_quaternion()
pose(rig, "Punch", .22/.60)
assert rest_chest.rotation_difference(rig.pose.bones["spine_03"].matrix.to_quaternion()).angle > .40, "missing chest drive"
print("Punch mechanics passed: chest drive, elbow clearance, forward reach, aligned wrist")
print("Free arm retains locomotion pose; first-person fist geometry is unchanged")

# Clothing must survive the motion too: discontinuous armhole weights used to
# stretch short edges 17x and pull the shoulder cap below the skin.
jacket = bpy.data.objects["Open plaid jacket"]
body = bpy.data.objects["Body"]
points = np.array([v.co[:] for v in jacket.data.vertices])
edges = np.array([e.vertices[:] for e in jacket.data.edges])
centers = (points[edges[:, 0]]+points[edges[:, 1]])/2
lengths = np.linalg.norm(points[edges[:, 0]]-points[edges[:, 1]], axis=1)
patch = ((np.abs(centers[:, 0]) > .08) & (np.abs(centers[:, 0]) < .28)
         & (centers[:, 2] > 1.08) & (centers[:, 2] < 1.31) & (lengths > .0005))
edges, lengths = edges[patch], lengths[patch]
assert jacket.get("hither_shoulder_repair") == 1
for vertex in jacket.data.vertices:
    active = [g.weight for g in vertex.groups if g.weight > 1e-7]
    assert len(active) <= 4 and abs(sum(active)-1) < .0001
samples = [(kind, p) for kind in CLIP_SECONDS for p in (0, .25, .5, .75, 1)]
samples += [("Punch", i/36) for i in range(37)]
worst_stretch = 0.
for kind, phase in samples:
    pose(rig, kind, phase)
    depsgraph = bpy.context.evaluated_depsgraph_get()
    evaluated = jacket.evaluated_get(depsgraph)
    mesh = evaluated.to_mesh()
    deformed = np.array([v.co[:] for v in mesh.vertices])
    ratios = np.linalg.norm(deformed[edges[:, 0]]-deformed[edges[:, 1]], axis=1)/lengths
    worst_stretch = max(worst_stretch, float(ratios.max()))
    assert ratios.max() < 4., (kind, phase, "armhole spike", ratios.max())
    evaluated.to_mesh_clear()
    if kind == "Punch":
        cloth_tree = BVHTree.FromObject(jacket, depsgraph)
        skin_tree = BVHTree.FromObject(body, depsgraph)
        for side in (-1, 1):
            for x in np.linspace(.065, .25, 38)*side:
                for z in np.linspace(1.15, 1.30, 31):
                    origin, direction = Vector((x, -1, z)), Vector((0, 1, 0))
                    cloth, _, _, _ = cloth_tree.ray_cast(origin, direction, 2)
                    skin, _, face, _ = skin_tree.ray_cast(origin, direction, 2)
                    if skin is None or cloth is None:
                        continue
                    rest = body.data.polygons[face].center
                    # Exclude the exposed hand and the intentional neck opening.
                    if .10 < abs(rest.x) < .22 and 1.18 < rest.z < 1.28:
                        assert skin.y >= cloth.y-.001, (phase, x, z, "skin through shoulder")
print(f"Jacket checks passed across all clips: maximum shoulder edge stretch {worst_stretch:.2f}x; covered skin")

if "--render" in sys.argv:
    # Render real deformed meshes, including jacket/tee, under consistent light.
    from build_avatar import render_preview
    pose(rig, "Idle", 0)
    render_preview(rig)
    scene = bpy.context.scene
    scene.render.resolution_x, scene.render.resolution_y = 640, 800
    cam = scene.camera
    cam.location = (1.8, -3.2, 1.1)
    cam.rotation_euler = (Vector((0, 0, .78))-cam.location).to_track_quat("-Z", "Y").to_euler()
    cam.data.ortho_scale = 1.8
    for kind, phase in (("Walk", 0), ("Walk", .25), ("Walk", .5), ("WalkBack", .25),
                        ("StrafeLeft", .25), ("Jump", .8), ("Fall", .9), ("Land", .24),
                        ("TurnLeft", .25), ("TurnRight", .75)):
        pose(rig, kind, phase)
        bpy.context.view_layer.update()
        scene.render.filepath = str(ROOT / f"tools/build/animation-{kind}-{phase}.png")
        bpy.ops.render.render(write_still=True)
    cam.location = (1.8, 3.2, 1.05)
    cam.rotation_euler = (Vector((0, 0, .80))-cam.location).to_track_quat("-Z", "Y").to_euler()
    cam.data.ortho_scale = 1.0
    for kind, phase in (("Idle", 0), ("Walk", .25), ("Walk", .75), ("TurnLeft", .25), ("Jump", .8)):
        pose(rig, kind, phase)
        bpy.context.view_layer.update()
        scene.render.filepath = str(ROOT / f"tools/build/animation-rear-{kind}-{phase}.png")
        bpy.ops.render.render(write_still=True)
    # The continuous opening trim must remain exposed over the undershirt,
    # especially where the chest turns into the collar.
    cam.location = (.18, -3.2, 1.12)
    cam.rotation_euler = (Vector((0, -.04, 1.02))-cam.location).to_track_quat("-Z", "Y").to_euler()
    cam.data.ortho_scale = .72
    for kind, phase in (("Idle", 0), ("Walk", .25), ("TurnLeft", .25), ("Jump", .8)):
        pose(rig, kind, phase)
        bpy.context.view_layer.update()
        scene.render.filepath = str(ROOT / f"tools/build/animation-trim-{kind}-{phase}.png")
        bpy.ops.render.render(write_still=True)
sys.stdout.flush()
os._exit(0)
