"""Build the close-view hand from the bundled CC0 avatar. Run with tools/.venv/bin/python.
Preserves the authored skin UVs and jacket; subdivides the isolated anatomy only.
"""
from pathlib import Path
from hand_pose import finger_angles
import bpy, bmesh
from mathutils import Vector, Matrix, Quaternion
ROOT = Path(__file__).resolve().parents[1]
bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/avatars/source/default.blend'))
rig = bpy.data.objects['HitherAvatar']
rig.animation_data_clear()
for b in rig.pose.bones:
    b.matrix_basis.identity()
def finger_pose(clenched):
    for name, angle in finger_angles(clenched).items():
        rig.pose.bones[name].rotation_quaternion = Quaternion((1, 0, 0), angle)
    bpy.context.view_layer.update()
finger_pose(False)
wrist = rig.data.bones['hand_r'].head_local.copy()
forward = (rig.data.bones['middle_01_r'].head_local-wrist).normalized()
across = rig.data.bones['pinky_01_r'].head_local-rig.data.bones['index_01_r'].head_local
across = (across-forward*across.dot(forward)).normalized()
normal = across.cross(forward).normalized()
basis = Matrix((across, forward, normal))
# Tilt the fingers upward; expose the dorsal surface and relaxed silhouette.
orientation = Matrix.Rotation(.85, 3, 'X') @ Matrix.Rotation(.40, 3, 'Z')
bpy.context.view_layer.update()
export=[]
for name in ('Body','Open plaid jacket'):
    obj=bpy.data.objects[name]
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active=obj
    # Crop in rest coordinates before applying the finger pose.
    bm=bmesh.new(); bm.from_mesh(obj.data)
    remove=[v for v in bm.verts if v.co.x > -.27 or (v.co-wrist).dot(forward)<-.24]
    bmesh.ops.delete(bm,geom=remove,context='VERTS')
    bm.to_mesh(obj.data);bm.free()
    if name=='Body':
        mod=obj.modifiers.new('Close-view anatomical smoothing','SUBSURF');mod.levels=2
    def evaluated():
        bpy.context.view_layer.update()
        return bpy.data.meshes.new_from_object(obj.evaluated_get(bpy.context.evaluated_depsgraph_get()), preserve_all_data_layers=True, depsgraph=bpy.context.evaluated_depsgraph_get())
    finger_pose(False)
    resting=evaluated()
    clenched=None
    if name=='Body':
        finger_pose(True)
        clenched=evaluated()
        finger_pose(False)
        assert len(resting.vertices)==len(clenched.vertices)
    obj.modifiers.clear()
    obj.data=resting
    obj.parent=None
    obj.matrix_world=Matrix.Identity(4)
    for v in obj.data.vertices:
        local=basis @ (v.co-wrist)
        if name=='Open plaid jacket':
            # Continue the proximal sleeve beyond the close-view crop. At full
            # extension the elbow must remain below the viewport, never expose
            # the cut end of the source mesh to the camera.
            local.y -= .28 * min(1.0,max(0.0,(-local.y-.10)/.14))
        v.co=orientation @ local
    if clenched:
        obj.shape_key_add(name='Basis')
        fist=obj.shape_key_add(name='Fist')
        for v, target in zip(clenched.vertices, fist.data):
            target.co=orientation @ (basis @ (v.co-wrist))
        bpy.data.meshes.remove(clenched)
    if name=='Body':
        skin = obj.data.materials[0]
        pbr = skin.node_tree.nodes.get('Principled BSDF')
        pbr.inputs['Roughness'].default_value = .52
        # Fine surface grain is baked into a dedicated normal-map UV set, so it
        # stays attached to the skin and requires no custom runtime shader.
        original_uv = obj.data.uv_layers.active.name
        obj.data.uv_layers.new(name='Hand detail UV')
        obj.data.uv_layers.active_index = len(obj.data.uv_layers)-1
        bpy.ops.object.mode_set(mode='EDIT'); bpy.ops.mesh.select_all(action='SELECT')
        bpy.ops.uv.smart_project(island_margin=.015)
        bpy.ops.object.mode_set(mode='OBJECT')
        nodes=skin.node_tree.nodes; links=skin.node_tree.links
        noise=nodes.new('ShaderNodeTexNoise');noise.inputs['Scale'].default_value=1800
        noise.inputs['Detail'].default_value=2
        coord=nodes.new('ShaderNodeTexCoord');links.new(coord.outputs['Object'],noise.inputs['Vector'])
        bump=nodes.new('ShaderNodeBump');bump.inputs['Strength'].default_value=.22;bump.inputs['Distance'].default_value=.00012
        links.new(noise.outputs['Fac'],bump.inputs['Height']);links.new(bump.outputs['Normal'],pbr.inputs['Normal'])
        detail=bpy.data.images.new('First person skin microdetail',width=1024,height=1024)
        detail.colorspace_settings.name='Non-Color'
        tex=nodes.new('ShaderNodeTexImage');tex.image=detail;nodes.active=tex
        scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.samples=8
        scene.render.bake.margin=12;scene.render.bake.use_selected_to_active=False
        bpy.ops.object.bake(type='NORMAL', uv_layer='Hand detail UV')
        nodes.remove(bump);nodes.remove(noise);nodes.remove(coord)
        uv=nodes.new('ShaderNodeUVMap');uv.uv_map='Hand detail UV';links.new(uv.outputs['UV'],tex.inputs['Vector'])
        normal=nodes.new('ShaderNodeNormalMap');normal.uv_map='Hand detail UV'
        links.new(tex.outputs['Color'],normal.inputs['Color']);links.new(normal.outputs['Normal'],pbr.inputs['Normal'])
        obj.data.uv_layers.active_index=0
        obj.data.uv_layers[0].active_render=True
        detail.pack()
    for p in obj.data.polygons:p.use_smooth=True
    obj.name='First person skin' if name=='Body' else 'First person tailored cuff'
    export.append(obj)
bpy.ops.object.select_all(action='DESELECT')
for obj in export:obj.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(ROOT/'assets/avatars/first-person-hand.glb'),use_selection=True,export_format='GLB',export_animations=False,export_yup=True)
print('Exported',sum(len(o.data.vertices) for o in export),'vertices')

# Blender 4.2 embedded Python can fault during interpreter teardown after export.
import os, sys
sys.stdout.flush()
os._exit(0)
