"""Apply a portrait-guided sculpt and texture projection to the actual 3D head.

The result is ordinary skinned mesh geometry and glTF base-color UVs. No
billboard, view-facing plane, custom runtime shader or reference photos needed.
"""
import json
from pathlib import Path
import numpy as np
from scipy.interpolate import RBFInterpolator
from mathutils import Vector
import bpy

ROOT = Path(__file__).resolve().parents[1]


def bake_face_material(body, portrait_material, mapping, project):
    """Bake the authored 3D material into one ordinary glTF skin texture."""
    skin = body.data.materials[0]
    original_uv = body.data.uv_layers.active.name
    original_coords = np.array([list(v.uv) for v in body.data.uv_layers[original_uv].data])
    body.data.uv_layers.new(name="PortraitProjection")
    body.data.color_attributes.new(name="PortraitBlend", type="FLOAT_COLOR", domain="CORNER")
    body.data.color_attributes.new(name="Forehead", type="FLOAT_COLOR", domain="CORNER")
    # Adding/removing custom-data layers can invalidate Blender RNA pointers.
    # Reacquire by name after allocation before touching loop data.
    projection = body.data.uv_layers["PortraitProjection"]
    weights = body.data.color_attributes["PortraitBlend"]
    fit = json.loads((ROOT / "assets/avatars/source/portrait_fit.json").read_text())
    brow = sorted(fit["texture"][i] for i in [70,63,105,66,107,336,296,334,293,300])
    brow_u, brow_v = np.array(brow).T
    for face in body.data.polygons:
        verts = [body.data.vertices[i] for i in face.vertices]
        points = np.array([project(v.co) for v in verts])
        coords = mapping(points) if any(v.co.z > 1.22 for v in verts) else np.zeros((len(verts),2))
        for vertex, index, (u,v) in zip(verts, face.loop_indices, coords):
            projection.data[index].uv = (float(np.clip(u,0,1)), float(1-np.clip(v,0,1)))
            front = float(np.clip((-vertex.co.y-.065)/.055,0,1))
            front = front*front*(3-2*front)
            neck = float(np.clip((vertex.co.z-1.235)/.045,0,1))
            # End the front projection before the ears: extrapolated portrait
            # background at the temples otherwise becomes pale neck patches.
            side = float(np.clip((.072-abs(vertex.co.x))/.032,0,1))
            side = side*side*(3-2*side)
            strength = front*neck*side if vertex.co.z < 1.56 else 0
            weights.data[index].color = (strength,strength,strength,1)
            eyebrow_top = float(np.interp(u,brow_u,brow_v))
            forehead = float(np.clip((eyebrow_top-v-.001)/.006,0,1)) if vertex.co.z > 1.405 else 0
            body.data.color_attributes["Forehead"].data[index].color = (forehead,forehead,forehead,1)
    nodes, links = skin.node_tree.nodes, skin.node_tree.links
    original = next(n for n in nodes if n.type == "TEX_IMAGE")
    uv_node = nodes.new("ShaderNodeUVMap")
    uv_node.uv_map = original_uv
    links.new(uv_node.outputs["UV"], original.inputs["Vector"])
    portrait = nodes.new("ShaderNodeTexImage")
    portrait.image = next(n.image for n in portrait_material.node_tree.nodes if n.type == "TEX_IMAGE")
    uv_node = nodes.new("ShaderNodeUVMap")
    uv_node.uv_map = projection.name
    links.new(uv_node.outputs["UV"], portrait.inputs["Vector"])
    blend = nodes.new("ShaderNodeVertexColor")
    blend.layer_name = weights.name
    mix = nodes.new("ShaderNodeMixRGB")
    # Remove photographed hair from skin only on the forehead. The replacement
    # fringe is rigged geometry, so no detached painted strands remain below it.
    forehead = nodes.new("ShaderNodeVertexColor")
    forehead.layer_name = "Forehead"
    keep = nodes.new("ShaderNodeMath")
    keep.operation = "SUBTRACT"
    keep.inputs[0].default_value = 1
    links.new(forehead.outputs[0], keep.inputs[1])
    strength = nodes.new("ShaderNodeMath")
    strength.operation = "MULTIPLY"
    links.new(blend.outputs[0], strength.inputs[0])
    links.new(keep.outputs[0], strength.inputs[1])
    links.new(strength.outputs[0], mix.inputs[0])
    links.new(original.outputs["Color"], mix.inputs[1])
    links.new(portrait.outputs["Color"], mix.inputs[2])
    emission = nodes.new("ShaderNodeEmission")
    links.new(mix.outputs[0], emission.inputs["Color"])
    output = nodes.get("Material Output")
    links.new(emission.outputs[0], output.inputs["Surface"])
    baked = bpy.data.images.new("Fitted portrait skin", width=4096, height=4096, alpha=False)
    destination = nodes.new("ShaderNodeTexImage")
    destination.image = baked
    nodes.active = destination
    body.data.uv_layers.active_index = body.data.uv_layers.find(original_uv)
    body.data.uv_layers.active.active_render = True
    assert np.allclose(original_coords, [list(v.uv) for v in body.data.uv_layers[original_uv].data]), "Original skin UVs changed during projection setup"
    bpy.ops.object.select_all(action="DESELECT")
    body.select_set(True)
    bpy.context.view_layer.objects.active = body
    bpy.context.scene.render.engine = "CYCLES"
    bpy.context.scene.cycles.samples = 8
    bpy.ops.object.bake(type="EMIT", margin=16, use_clear=True)
    baked.filepath_raw = str(ROOT / "assets/avatars/source/skin_fitted.png")
    baked.file_format = "PNG"
    baked.save()
    nodes.clear()
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = baked
    pbr = nodes.new("ShaderNodeBsdfPrincipled")
    pbr.inputs["Roughness"].default_value = .86
    pbr.inputs["Specular IOR Level"].default_value = .22
    output = nodes.new("ShaderNodeOutputMaterial")
    links.new(tex.outputs["Color"],pbr.inputs["Base Color"])
    links.new(pbr.outputs[0],output.inputs["Surface"])
    body.data.uv_layers.remove(body.data.uv_layers["PortraitProjection"])
    body.data.color_attributes.remove(body.data.color_attributes["PortraitBlend"])
    body.data.color_attributes.remove(body.data.color_attributes["Forehead"])
    assert np.allclose(original_coords, [list(v.uv) for v in body.data.uv_layers[original_uv].data]), "Original skin UVs changed during bake cleanup"


def fit_portrait(body, portrait_material):
    fit = json.loads((ROOT / "assets/avatars/source/portrait_fit.json").read_text())
    source, target, texture = (np.array(fit[k]) for k in ["source", "target", "texture"])
    # Drop duplicate corner points; regularization keeps the fit smooth between
    # features instead of chasing individual pixel/landmark detection noise.
    _, unique = np.unique(np.round(source, 5), axis=0, return_index=True)
    source, target, texture = source[unique], target[unique], texture[unique]
    sculpt = RBFInterpolator(source, target-source, smoothing=.00001)
    mapping = RBFInterpolator(source, texture, smoothing=.00001)
    cam = Vector((.1,-3,1.44))
    rotation = (Vector((0,-.01,1.42))-cam).to_track_quat("-Z", "Y")
    right, up = rotation @ Vector((1,0,0)), rotation @ Vector((0,1,0))

    def project(p):
        relative = p-cam
        return [.5+relative.dot(right)/.43, .5-relative.dot(up)/.43]

    bake_face_material(body, portrait_material, mapping, project)
    # Keep the eyeballs' anatomical UVs and green iris material. Projecting the
    # portrait onto them replaced their green irises with its brown/hazel eyes
    # (and baked highlights). The geometry still receives the facial fit below.

    for obj in bpy.context.scene.objects:
        if obj.type != "MESH":
            continue
        is_hair = any(m and m.name == "Brown hair" for m in obj.data.materials)
        verts = [v for v in obj.data.vertices if 1.265 < v.co.z < 1.57 and abs(v.co.x) < .15]
        if not verts:
            continue
        points = np.array([project(v.co) for v in verts])
        offsets = sculpt(points)*.43
        for vertex, (dx,dy) in zip(verts, offsets):
            if is_hair:
                if vertex.co.z > 1.46:
                    vertex.co.z = 1.46+(vertex.co.z-1.46)*.68
                continue
            weight = min(1, max(0,(-vertex.co.y+.018)/.105))
            weight *= min(1, max(0,(vertex.co.z-1.265)/.02))
            # The forehead above measured anatomy tapers to the existing scalp.
            weight *= min(1, max(0,(1.565-vertex.co.z)/.045))
            displacement = right*float(dx)-up*float(dy)
            if displacement.length > .025:
                displacement *= .025/displacement.length
            vertex.co += displacement*weight
            if vertex.co.z > 1.46:
                vertex.co.z = 1.46+(vertex.co.z-1.46)*.68
            if vertex.co.y < -.15 and abs(vertex.co.x) < .035 and 1.31 < vertex.co.z < 1.42:
                vertex.co.y = -.15+(vertex.co.y+.15)*.80
