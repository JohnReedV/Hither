"""Editable mesh tailoring and portrait-guided face sculpt for the default avatar."""
import json
import math
from pathlib import Path

import bpy
import bmesh
import numpy as np
from scipy.interpolate import RBFInterpolator
from scipy.spatial import cKDTree
from mathutils import Vector

ROOT = Path(__file__).resolve().parents[1]


def register_cloth_border(obj, image_path):
    """Adjust mesh UVs where generated cloth island edges drifted; no bitmap edits."""
    from PIL import Image
    pixels = np.array(Image.open(image_path).convert("RGB"))
    dark_cloth = (pixels[:,:,0] < 145) & (pixels[:,:,1] < 135) & (pixels[:,:,2] < 130)
    # Exclude transition pixels to keep bilinear filtering within the cloth.
    from scipy.ndimage import binary_erosion
    interior = binary_erosion(dark_cloth, iterations=3)
    candidates = np.argwhere(interior)
    tree = cKDTree(candidates)
    h,w = pixels.shape[:2]
    uv = obj.data.uv_layers.active.data
    for face in obj.data.polygons:
        if face.material_index != 0:
            continue
        for index in face.loop_indices:
            u,v = uv[index].uv
            y,x = int(np.clip((1-v)*h,0,h-1)),int(np.clip(u*w,0,w-1))
            if not interior[y,x]:
                distance, nearest = tree.query([y,x])
                if distance < 35:
                    y,x = candidates[nearest]
                    uv[index].uv = ((x+.5)/w,1-(y+.5)/h)


def sculpt_face():
    data = json.loads((ROOT / "assets/avatars/source/face_fit.json").read_text())
    points = np.array([c["position"] for c in data["controls"]])
    offsets = np.array([c["delta"] for c in data["controls"]])
    # Anchor scalp/neck and side silhouette to avoid extrapolated stretching.
    anchors = np.array([(x, -.1, z) for x in [-.15, 0, .15] for z in [1.23, 1.58]])
    points = np.concatenate([points, anchors])
    offsets = np.concatenate([offsets, np.zeros_like(anchors)])
    warp = RBFInterpolator(points[:, [0, 2]], offsets, smoothing=.00002)
    for obj in bpy.context.scene.objects:
        if obj.type != "MESH":
            continue
        verts = [v for v in obj.data.vertices if v.co.z > 1.25 and abs(v.co.x) < .14]
        if not verts:
            continue
        locations = np.array([list(v.co) for v in verts])
        deltas = warp(locations[:, [0, 2]])
        for vert, delta in zip(verts, deltas):
            # Preserve the skull behind the ears. Facial features and separate
            # eye/brow meshes receive the same continuous deformation.
            front = min(1, max(0, (-vert.co.y + .01) / .10))
            strength = min(1, max(0, (vert.co.z - 1.25) / .035)) * front
            delta = np.clip(delta, -.014, .014) * strength
            vert.co += Vector(delta)




def tailor_jacket(obj, lining):
    """Open a fitted shirt shell into a hip-length, fleece-edged jacket."""
    obj.name = "Open plaid jacket"
    obj.data.materials.append(lining)
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if f.calc_center_median().z < .85], context="FACES")
    # Give the jacket room over the undershirt, while preserving fitted sleeves.
    for v in bm.verts:
        if abs(v.co.x) < .23:
            v.co.x *= 1.06
            v.co.y *= 1.12
            if v.co.z < 1.0:
                v.co.z -= max(0, (1.0 - v.co.z)) * .72
        else:
            v.co.y *= 1.07
    for x in [-.072, .072]:
        bmesh.ops.bisect_plane(bm, geom=list(bm.verts)+list(bm.edges)+list(bm.faces),
                              plane_co=(x, 0, 0), plane_no=(1, 0, 0), dist=.00001)
    cut = [f for f in bm.faces if abs(f.calc_center_median().x) < .0719
           and f.calc_center_median().y < -.025]
    bmesh.ops.delete(bm, geom=cut, context="FACES")
    # Bisecting the source shirt's collar leaves near-duplicate cut vertices.
    # Weld only this seam, not unrelated cloth folds or sleeve geometry.
    seam = [v for v in bm.verts if abs(abs(v.co.x)-.072) < .001 and v.co.y < -.025]
    bmesh.ops.remove_doubles(bm, verts=seam, dist=.0006)
    uv = bm.loops.layers.uv.verify()
    deform = bm.verts.layers.deform.verify()
    edges = [e for e in bm.edges if e.is_boundary and
             all(abs(abs(v.co.x)-.072) < .001 and v.co.y < -.025 for v in e.verts)]
    # Relax the old button-shirt collar's doubled-back cut into a flowing
    # opening. Move its adjacent cloth with it so this is not a floating ribbon.
    neighbors = {}
    for edge in edges:
        a, b = edge.verts
        neighbors.setdefault(a, []).append(b)
        neighbors.setdefault(b, []).append(a)
    original = {v: v.co.copy() for v in neighbors}
    relaxed = {v: co.copy() for v, co in original.items()}
    for _ in range(12):
        updated = {}
        for v, adjacent in neighbors.items():
            strength = .45 * min(1, max(0, (original[v].z-1.14)/.05))
            average = sum((relaxed[n] for n in adjacent), Vector()) / len(adjacent)
            updated[v] = relaxed[v].lerp(average, strength) if len(adjacent) == 2 else relaxed[v]
        relaxed = updated
    for v in bm.verts:
        if v in relaxed:
            v.co = relaxed[v]
        elif v.co.z > 1.12 and v.co.y < 0:
            nearest = min(original, key=lambda n: (original[n]-v.co).length_squared)
            distance = (original[nearest]-v.co).length
            influence = max(0, 1-distance/.045)
            v.co += (relaxed[nearest]-original[nearest])*influence
    outer = {}
    for edge in edges:
        for vert in edge.verts:
            if vert not in outer:
                # Extend INTO the opening. Folding outward over existing faces
                # made the trim self-overlap and disappear under the collar.
                new = bm.verts.new(vert.co + Vector((-math.copysign(.018, vert.co.x), -.002, 0)))
                for group, weight in vert[deform].items():
                    new[deform][group] = weight
                outer[vert] = new
        a, b = edge.verts
        face = bm.faces.new((a, b, outer[b], outer[a]))
        face.material_index = 1
        for loop in face.loops:
            # Sample an unused part of the generated atlas's sherpa background.
            loop[uv].uv = (.38 + (abs(loop.vert.co.x)-.072)*1.0,
                           .87 + (loop.vert.co.z-.75)*.20)
    # Both opening edges must be single, welded, unbranched hem-to-collar chains.
    assert all(len(ns) in (1, 2) for ns in neighbors.values()), "Branched jacket trim"
    for side in (-1, 1):
        remaining = {v for v in neighbors if v.co.x*side > 0}
        pending = [next(iter(remaining))]
        connected = set()
        while pending:
            v = pending.pop()
            if v in connected:
                continue
            connected.add(v)
            pending.extend(neighbors[v])
        assert connected == remaining, "Disconnected jacket trim"
        assert sum(len(neighbors[v]) == 1 for v in remaining) == 2
        assert min(v.co.z for v in remaining) < .78 and max(v.co.z for v in remaining) > 1.27
    bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
    bm.to_mesh(obj.data)
    bm.free()
    solid = obj.modifiers.new("Jacket cloth thickness", "SOLIDIFY")
    solid.thickness = .003
    solid.offset = 0
    smooth = obj.modifiers.new("Tailored collar smoothing", "SUBSURF")
    smooth.levels = 1


def crop_mesh(obj, predicate):
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if predicate(f.calc_center_median())], context="FACES")
    bm.to_mesh(obj.data)
    bm.free()
