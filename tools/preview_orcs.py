"""Headless asset QA; never opens or grabs a desktop window."""
from pathlib import Path
import bpy,sys,os
from mathutils import Vector
ROOT=Path(__file__).resolve().parents[1]
faces=os.environ.get('HITHER_ORC_FACES')=='1'
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.context.scene.render.fps=60
for i in range(3):
    before=set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=str(ROOT/f'assets/orcs/orc-{i}.glb'))
    added=set(bpy.data.objects)-before
    for obj in added:
        if obj.parent is None:obj.location.x+=(i-1)*(.36 if faces else 1.25)
        if obj.type=='ARMATURE' and obj.animation_data:
            obj.animation_data.action=None
            for track in obj.animation_data.nla_tracks:track.mute=True
            for bone in obj.pose.bones:bone.matrix_basis.identity()
            clip=os.environ.get('HITHER_ORC_CLIP')
            if clip:
                obj.animation_data.action=next(a for a in bpy.data.actions if a.name.startswith(clip+'_') and a not in [o.animation_data.action for o in bpy.data.objects if o.type=='ARMATURE' and o!=obj and o.animation_data])
bpy.context.scene.frame_set(int(os.environ.get('HITHER_ORC_FRAME','15')))
bpy.ops.object.camera_add(location=(.12,-1.8,1.48) if faces else (1.5,-5.5,2.1))
camera=bpy.context.object;camera.rotation_euler=(Vector((0,-.02,1.39) if faces else (0,0,.9))-camera.location).to_track_quat('-Z','Y').to_euler()
bpy.context.scene.camera=camera;camera.data.lens=65 if faces else 55
for p,power,size in [((0,-4,5),650,5),((-3,1,3),500,3)]:
    bpy.ops.object.light_add(type='AREA',location=p);light=bpy.context.object;light.data.energy=power;light.data.shape='DISK';light.data.size=size
    light.rotation_euler=(Vector((0,0,1))-light.location).to_track_quat('-Z','Y').to_euler()
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.samples=24
scene.world=bpy.data.worlds.new('Studio');scene.world.use_nodes=True;scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.12,.12,.12,1)
scene.render.resolution_x=1400;scene.render.resolution_y=950;scene.render.resolution_percentage=100
for frame in os.environ.get('HITHER_ORC_FRAMES',os.environ.get('HITHER_ORC_FRAME','15')).split(','):
    scene.frame_set(int(frame))
    suffix='-'+frame if 'HITHER_ORC_FRAMES' in os.environ else ''
    scene.render.filepath=str(ROOT/('tools/build/orcs-faces'+suffix+'.png' if faces else 'tools/build/orcs-lineup'+suffix+'.png'))
    bpy.ops.render.render(write_still=True)
sys.stdout.flush();os._exit(0)
