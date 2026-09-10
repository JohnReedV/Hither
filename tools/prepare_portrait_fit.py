"""Bake face landmark correspondences for sculpting and projective mesh UVs."""
import json
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
records = json.loads((ROOT / "tools/build/face-measurements.json").read_text())
source = np.array(records[0]["points"])[:, :2]
texture = np.array(records[1]["points"])[:, :2]

def frame(points):
    center = (points[468] + points[473]) * .5
    axis = points[473] - points[468]
    size = np.linalg.norm(axis)
    axis /= size
    return center, np.array([axis, [-axis[1], axis[0]]]), size

center, axes, size = frame(source)
tc, ta, ts = frame(texture)
target = ((texture - tc) @ ta.T) * (size/ts) @ axes + center
# Mesh UV correspondence and sculpt destination are distinct: UVs go to the
# original texture, while anatomy uses a rigid eye-aligned reference frame.
out = {"source": source[:468].tolist(), "texture": texture[:468].tolist(),
       "target": target[:468].tolist(), "method": "468 landmark frontal projection; eye-distance scale lock"}
(ROOT / "assets/avatars/source/portrait_fit.json").write_text(json.dumps(out, separators=(",", ":")) + "\n")
print("Saved portrait-to-head correspondences")
