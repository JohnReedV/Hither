"""Isolated GLB animation/grip close-up; no desktop window."""
from pathlib import Path
import bpy,os,sys
from mathutils import Vector
root=Path(__file__).resolve().parents[1]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(root/'assets/orcs/orc-0.glb'))
rig=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE')
for track in rig.animation_data.nla_tracks:track.mute=True
rig.animation_data.action=next(a for a in bpy.data.actions if a.name.startswith('Idle'))
bpy.context.scene.frame_set(1)
bpy.context.view_layer.update()
target=rig.matrix_world@rig.pose.bones['middle_02_r'].head
bpy.ops.object.camera_add(location=target+Vector((-.45,-.7,.22)))
camera=bpy.context.object;camera.rotation_euler=(target-camera.location).to_track_quat('-Z','Y').to_euler()
camera.data.lens=65;bpy.context.scene.camera=camera
for offset in [(0,-2,3),(-2,0,1)]:
    bpy.ops.object.light_add(type='AREA',location=target+Vector(offset))
    light=bpy.context.object;light.data.energy=160;light.data.size=2
    light.rotation_euler=(target-light.location).to_track_quat('-Z','Y').to_euler()
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.samples=24
scene.world=bpy.data.worlds.new('Studio');scene.world.use_nodes=True
scene.render.resolution_x=800;scene.render.resolution_y=800;scene.render.resolution_percentage=100
scene.render.filepath=str(root/'tools/build/orc-grip.png')
bpy.ops.render.render(write_still=True)
sys.stdout.flush();os._exit(0)
