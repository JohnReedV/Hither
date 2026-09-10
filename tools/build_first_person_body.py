"""Export close-view denim and boots with the bundled skeleton and all gait clips.
Run with tools/.venv/bin/python tools/build_first_person_body.py.
"""
from pathlib import Path
import os
import sys
import bpy
import bmesh
from export_avatar import export_avatar

ROOT = Path(__file__).resolve().parents[1]
bpy.ops.wm.open_mainfile(filepath=str(ROOT / 'assets/avatars/source/default.blend'))
# Keep the entire covered lower body, including the waist. There is no head or
# upper torso to intersect the eye camera, and no exposed cut through a leg.
for obj in list(bpy.context.scene.objects):
    if obj.type == 'MESH' and obj.name not in ('Blue denim jeans', 'Body.shoes02', 'Open plaid jacket', 'Gray crewneck T-shirt'):
        bpy.data.objects.remove(obj, do_unlink=True)
for obj in list(bpy.context.scene.objects):
    if obj.type != 'MESH':
        continue
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    if obj.name in ('Open plaid jacket', 'Gray crewneck T-shirt'):
        arm_groups = {g.index for g in obj.vertex_groups if any(
            name in g.name for name in ('arm', 'hand', 'thumb', 'clavicle'))}
        sleeve_vertices = {v.index for v in obj.data.vertices if any(
            g.group in arm_groups and g.weight > .05 for g in v.groups)}
        bm = bmesh.new()
        bm.from_mesh(obj.data)
        bmesh.ops.delete(bm, geom=[v for v in bm.verts if v.index in sleeve_vertices or v.co.z > 1.12 or abs(v.co.x) > .29], context='VERTS')
        bm.to_mesh(obj.data)
        bm.free()
        continue
    sub = obj.modifiers.new('Close-view smooth silhouette', 'SUBSURF')
    sub.levels = 2
    # Subdivide rest geometry and weights before skeletal deformation.
    bpy.ops.object.modifier_move_up(modifier=sub.name)
    bpy.ops.object.modifier_apply(modifier=sub.name)
    bpy.ops.object.vertex_group_limit_total(limit=4)
    bpy.ops.object.vertex_group_normalize_all(lock_active=False)
    for face in obj.data.polygons:
        face.use_smooth = True
# The eye sits ahead of the abdomen. Keeping the source rig's origin directly
# below the eye exposes the inside of the waistband when looking straight down.
anchor = bpy.data.objects.new('First person eye alignment', None)
bpy.context.scene.collection.objects.link(anchor)
for obj in list(bpy.context.scene.objects):
    if obj != anchor and obj.parent is None:
        obj.parent = anchor
anchor.location.y = .38
# Broad, restrained highlights read as worn leather at eye distance.
bpy.data.materials['Leather boots'].node_tree.nodes.get('Principled BSDF').inputs['Roughness'].default_value = .68
export_avatar(ROOT / 'assets/avatars/first-person-body.glb')
sys.stdout.flush()
sys.stderr.flush()
os._exit(0)
