"""Bake conservative landmark-based sculpt controls from local portrait measurements.

Run once after measure_face.py, using the corresponding unmodified preview mesh.
Only geometric controls are saved; the portraits are not bundled with the game.
"""
import json
from pathlib import Path
import numpy as np
import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

ROOT = Path(__file__).resolve().parents[1]
records = json.loads((ROOT / "tools/build/face-measurements.json").read_text())
bpy.ops.wm.open_mainfile(filepath=str(ROOT / "assets/avatars/source/default.blend"))
body = bpy.data.objects["Body"]
tree = BVHTree.FromPolygons([v.co for v in body.data.vertices], [p.vertices[:] for p in body.data.polygons])

def pixels(record):
    return np.array(record["points"])[:, :2] * np.array(record["size"])

source = pixels(records[0])
def eye_frame(points):
    left, right = points[468], points[473]
    axis = right - left
    distance = np.linalg.norm(axis)
    axis /= distance
    return (left + right) / 2, np.array([axis, [-axis[1], axis[0]]]), distance

origin, axes, distance = eye_frame(source)
aligned = []
for record in records[1:]:
    points = pixels(record)
    center, frame, scale = eye_frame(points)
    aligned.append(((points - center) @ frame.T) * (distance / scale) @ axes + origin)
target = np.mean(aligned, axis=0)
cam = Vector((0.1, -3, 1.44))
rotation = (Vector((0, -.01, 1.42)) - cam).to_track_quat("-Z", "Y")
right, up, forward = [rotation @ Vector(a) for a in [(1,0,0),(0,1,0),(0,0,-1)]]
# Outer face, brow, nose bridge/alae, lip contour and chin; exclude unstable
# interior eye/mouth landmarks. Eye centers remain fixed during alignment.
ids = [10, 109, 67, 103, 54, 21, 162, 127, 234, 93, 132, 58, 172, 136, 150,
       149, 176, 148, 152, 377, 400, 378, 379, 365, 397, 288, 361, 323, 454,
       356, 389, 251, 284, 332, 297, 338, 6, 168, 197, 195, 5, 4, 1, 2, 98,
       327, 64, 294, 61, 291, 0, 17, 13, 14, 78, 308, 70, 63, 105, 66, 107,
       336, 296, 334, 293, 300]
controls = []
for i in ids:
    x, y = source[i] / np.array(records[0]["size"])
    ray = cam + right * ((x - .5) * .43) + up * ((.5 - y) * .43)
    hit, _, _, _ = tree.ray_cast(ray, forward)
    if hit is None or hit.z < 1.26:
        continue
    delta = (target[i] - source[i]) / np.array(records[0]["size"]) * .43
    movement = right * float(delta[0]) - up * float(delta[1])
    if movement.length > .014:
        movement *= .014 / movement.length
    controls.append({"landmark": i, "position": list(hit), "delta": list(movement)})
out = ROOT / "assets/avatars/source/face_fit.json"
out.write_text(json.dumps({"method": "eye-aligned two-portrait landmark fit; 14mm maximum displacement", "controls": controls}, indent=2) + "\n")
print("Baked", len(controls), "facial sculpt controls to", out)
import sys, os
sys.stdout.flush()
os._exit(0)
