"""Headless inspection of the palm against the actual runtime stone mesh.
First export the mesh with HITHER_STONE_PREVIEW_MESH=/tmp/hither-stone-mesh.json
cargo test stone_bearing_face. HITHER_ORC_FRAMES accepts 60 fps Push clip frames.
"""
from pathlib import Path
import bpy, json, os, sys
from mathutils import Vector
from orc_animation import palm_surface
ROOT=Path(__file__).resolve().parents[1]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.context.scene.render.fps=60
bpy.ops.import_scene.gltf(filepath=str(ROOT/'assets/orcs/orc-0.glb'))
rig=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE')
for track in rig.animation_data.nla_tracks:track.mute=True
rig.animation_data.action=next(a for a in bpy.data.actions if a.name.startswith('Push_'))
rig.scale=(1.18,1.12,1.12)
shape=json.loads(Path('/tmp/hither-stone-mesh.json').read_text())
vertices=[(x,-z,y) for x,y,z in shape['positions']]
indices=shape['indices']
mesh=bpy.data.meshes.new('Runtime stone');mesh.from_pydata(vertices,[],[indices[i:i+3] for i in range(0,len(indices),3)]);mesh.update()
rock=bpy.data.objects.new('Runtime stone',mesh);bpy.context.collection.objects.link(rock)
material=bpy.data.materials.new('Shale');material.diffuse_color=(.46,.48,.43,1);rock.data.materials.append(material)
contact=json.loads((ROOT/'assets/orcs/stone-contact.json').read_text())
bpy.ops.object.camera_add();camera=bpy.context.object;camera.data.lens=60
scene=bpy.context.scene;scene.camera=camera;scene.render.engine='CYCLES';scene.cycles.samples=32
scene.world=bpy.data.worlds.new('Studio');scene.world.use_nodes=True
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.12,.12,.12,1)
for location,power in [((0,-4,5),550),((3,-2,1),300),((-3,0,4),450)]:
    bpy.ops.object.light_add(type='AREA',location=location);light=bpy.context.object;light.data.energy=power;light.data.size=3
    light.rotation_euler=(Vector((0,0,2))-light.location).to_track_quat('-Z','Y').to_euler()
scene.render.resolution_x=1000;scene.render.resolution_y=850;scene.render.resolution_percentage=100
full=os.environ.get('HITHER_ORC_FULL')=='1'
if full:
    bpy.ops.mesh.primitive_plane_add(size=200, location=(0,0,0))
    ground=bpy.context.object
    mat=bpy.data.materials.new('Ground');mat.diffuse_color=(.16,.18,.20,1)
    ground.data.materials.append(mat)
    camera.data.lens=45
for frame in map(int,os.environ.get('HITHER_ORC_FRAMES','120,240,300').split(',')):
    scene.frame_set(frame);bpy.context.view_layer.update()
    if frame<=315:
        x,y,z=contact[frame]
    else:
        seconds=(frame-315)/60
        velocity=(Vector(contact[315])-Vector(contact[314]))*60
        x,y,z=Vector(contact[315])+velocity*seconds-Vector((0,4.905*seconds*seconds,0))
    rock.location=(x,-z,y)
    target=palm_surface(rig)
    if 113<=frame<=315:
        bottom=rock.location+Vector((0,0,-1.40))
        assert (bottom-target).length<.003, f'Rock/palm gap at {frame}: {bottom-target}'
    camera.location=target+Vector((1.25,-2.15,-.4))
    camera.rotation_euler=(target+Vector((0,0,.08))-camera.location).to_track_quat('-Z','Y').to_euler()
    if full:
        camera.location=(4,-8,3.2)
        camera.rotation_euler=(Vector((.4,0,2.0))-camera.location).to_track_quat('-Z','Y').to_euler()
    scene.render.filepath=str(ROOT/f'tools/build/orc-carry-{frame}.png')
    bpy.ops.render.render(write_still=True)
sys.stdout.flush();os._exit(0)
