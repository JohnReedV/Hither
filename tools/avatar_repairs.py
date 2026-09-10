"""Native mesh repairs for layered clothing, compact ears and attached fringe."""
import bpy
import math
import numpy as np
from mathutils import Vector
from mathutils.bvhtree import BVHTree
from mathutils.kdtree import KDTree


def repair_jacket_shoulders(jacket):
    """Blend the sleeve/chest seam and keep the shoulder cap above the skin.

    Run after static cloth modifiers and the initial four-influence reduction.
    A physical-space kernel treats the inner/outer cloth layers consistently;
    smoothing by edge count would depend on the dense armhole subdivision.
    """
    if jacket.get("hither_shoulder_repair") == 1:
        return
    vertices = jacket.data.vertices
    positions = np.array([v.co[:] for v in vertices])
    weights = np.zeros((len(vertices), len(jacket.vertex_groups)))
    tree = KDTree(len(vertices))
    for vertex in vertices:
        tree.insert(vertex.co, vertex.index)
        for group in vertex.groups:
            weights[vertex.index, group.group] = group.weight
    tree.balance()

    def ramp(t):
        t = np.clip(t, 0, 1)
        return t*t*(3-2*t)

    x, z = np.abs(positions[:, 0]), positions[:, 2]
    blend = (ramp((x-.09)/.045)*ramp((.32-x)/.055)
             * ramp((z-1.00)/.045)*ramp((1.315-z)/.045))
    result = weights.copy()
    for i in np.flatnonzero(blend > 0):
        nearby = tree.find_range(vertices[int(i)].co, .045)
        indices = np.array([n[1] for n in nearby])
        distances = np.array([n[2] for n in nearby])
        kernel = np.exp(-.5*(distances/.018)**2)
        average = kernel @ weights[indices] / kernel.sum()
        result[i] = weights[i]*(1-blend[i])+average*blend[i]
        # Match the exported four-influence field before correcting the cap.
        result[i, np.argsort(result[i])[:-4]] = 0
        result[i] /= result[i].sum()

    neck = jacket.vertex_groups["neck_01"].index
    for vertex in vertices:
        i = vertex.index
        px, py, pz = positions[i]
        # The shoulder follows its clavicle, not the head counter-turn.
        cap = ramp((abs(px)-.085)/.030)*(1-ramp((pz-1.285)/.025))
        transfer = result[i, neck]*cap
        if transfer > 1e-7:
            clavicle = jacket.vertex_groups["clavicle_l" if px > 0 else "clavicle_r"].index
            result[i, clavicle] += transfer
            result[i, neck] -= transfer
        if blend[i] > 0 or transfer > 1e-7:
            result[i, np.argsort(result[i])[:-4]] = 0
            result[i] /= result[i].sum()
            for group in list(vertex.groups):
                jacket.vertex_groups[group.group].remove([i])
            for group in np.flatnonzero(result[i] > 1e-7):
                jacket.vertex_groups[int(group)].add([i], float(result[i, group]), "REPLACE")
        # Six millimeters of feathered ease over the top of each shoulder.
        vertex.co.z += .006*math.exp(-.5*(
            ((abs(px)-.125)/.035)**2+((py-.01)/.045)**2+((pz-1.255)/.018)**2))
    jacket["hither_shoulder_repair"] = 1


def smaller_ears(body):
    group = body.vertex_groups["ears"].index
    for vertex in body.data.vertices:
        weight = next((g.weight for g in vertex.groups if g.group == group), 0)
        if not weight:
            continue
        sign = 1 if vertex.co.x > 0 else -1
        pivot = Vector((sign*.062, -.046, 1.373))
        offset = vertex.co-pivot
        fitted = pivot + Vector((offset.x*.68, offset.y*.75, offset.z*.72))
        fitted.x -= sign*.002
        vertex.co = vertex.co.lerp(fitted, weight)


def tuck_undershirt(tee, jacket):
    # Resolve the actual outer surface, including its tailored collar, before
    # animation. Uniformly cropping sleeves left triangles protruding at seams.
    bpy.context.view_layer.update()
    tree = BVHTree.FromObject(jacket, bpy.context.evaluated_depsgraph_get())
    bpy.context.view_layer.objects.active = tee
    mod = tee.modifiers.new("Undershirt seam resolution", "SUBSURF")
    mod.levels = 2
    bpy.ops.object.modifier_apply(modifier=mod.name)
    corrected = 0
    for vertex in tee.data.vertices:
        x,y,z = vertex.co
        hit, _, _, _ = tree.ray_cast(Vector((x,-1,z)), Vector((0,1,0)), 2)
        if hit is not None and hit.y < -.025 and abs(x) > .045 and y < hit.y+.012:
            vertex.co.y = hit.y+.012
            corrected += 1
    print("TUCKED UNDERSHIRT VERTICES", corrected)


def fit_jacket_over_jeans(jacket, pants):
    """Give the rear jacket skirt clearance over the moving denim hip surface."""
    bpy.context.view_layer.update()
    tree = BVHTree.FromObject(pants, bpy.context.evaluated_depsgraph_get())
    for vertex in jacket.data.vertices:
        x, y, z = vertex.co
        if y <= 0 or z >= .91:
            continue
        hit, _, _, _ = tree.ray_cast(Vector((x, 1, z)), Vector((0, -1, 0)), 2)
        if hit is not None and hit.y > 0:
            vertex.co.y = max(y, hit.y+.028)


def fit_hair_to_head(body, hair):
    """Unbury the existing continuous hairline after the portrait head sculpt."""
    bpy.context.view_layer.update()
    tree = BVHTree.FromObject(body, bpy.context.evaluated_depsgraph_get())
    for vertex in hair.data.vertices:
        x,y,z=vertex.co
        if y >= -.06 or z < 1.405 or abs(x) > .074:
            continue
        front=min(1,max(0,(-y-.06)/.05))
        front*=min(1,max(0,(.072-abs(x))/.028))
        lower=min(1,max(0,(1.465-z)/.025))
        vertex.co.z -= .009*front*lower
        hit, _, _, _=tree.ray_cast(Vector((x,-.5,vertex.co.z)),Vector((0,1,0)),1)
        if hit is not None:
            vertex.co.y=y-max(0,min(.016,y-hit.y+.004))*front

    # The reference has close-cut temples and sides, with volume concentrated
    # in the swept top. Compress the hair-to-scalp gap rather than scaling the
    # whole hairstyle (which would bury the fringe and change the hairline).
    for vertex in hair.data.vertices:
        side = min(1,max(0,(abs(vertex.co.x)-.025)/.035))
        if side == 0:
            continue
        surface, normal, _, distance = tree.find_nearest(vertex.co)
        if surface is None or distance > .06:
            continue
        offset=vertex.co-surface
        if offset.dot(normal) <= 0:
            continue
        clearance=.0035
        desired=clearance+max(0,distance-clearance)*.35
        vertex.co=vertex.co.lerp(surface+offset.normalized()*desired, side)


def tuck_waist(tee, pants):
    """Continue the gray shirt into the waistband, with the belt in front."""
    bpy.context.view_layer.update()
    tree = BVHTree.FromObject(pants, bpy.context.evaluated_depsgraph_get())
    for vertex in tee.data.vertices:
        x,y,z = vertex.co
        if z >= .91:
            continue
        # Extend the cropped hem about three centimeters. Keep the upper torso
        # unchanged and retain the existing UVs and skeletal weights.
        extend = min(1,max(0,(.91-z)/.071))
        vertex.co.z -= .031*extend
        hit, _, _, _ = tree.ray_cast(Vector((x,-1,vertex.co.z)), Vector((0,1,0)), 2)
        if hit is None or hit.y >= 0:
            continue
        # Above the belt, cover the old suit's dark waistband strip. Below it,
        # turn the hem inward so the buckle/belt is not painted over by cotton.
        tuck = min(1,max(0,(.843-vertex.co.z)/.015))
        tuck = tuck*tuck*(3-2*tuck)
        front = min(y,hit.y-.004)
        behind = max(y,hit.y+.006)
        vertex.co.y = front*(1-tuck)+behind*tuck
