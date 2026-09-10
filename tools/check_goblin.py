"""Inspect the actual shipped GLB, including skinning and animation samples."""
from pathlib import Path
import bpy, json, struct, math, sys, os
from mathutils import Vector
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'tools/build';OUT.mkdir(exist_ok=True)
path=ROOT/'assets/goblins/mireling.glb';data=path.read_bytes();doc=json.loads(data[20:20+struct.unpack_from('<I',data,12)[0]])
assert {a['name'] for a in doc['animations']}=={'Idle','Scamper','Alert','Swipe'}
assert doc.get('skins') and len(doc['skins'][0]['joints'])>50
assert all('JOINTS_0' in p['attributes'] and 'WEIGHTS_0' in p['attributes'] for m in doc['meshes'] for p in m['primitives'])
assert all(math.isfinite(v) for a in doc['accessors'] for key in ['min','max'] for v in a.get(key,[]))
bpy.ops.wm.read_factory_settings(use_empty=True);scene=bpy.context.scene;scene.render.fps=60;bpy.ops.import_scene.gltf(filepath=str(path))
rig=next(o for o in bpy.data.objects if o.type=='ARMATURE');rig.animation_data.action=None
for t in rig.animation_data.nla_tracks:t.mute=True
body=max((o for o in bpy.data.objects if o.type=='MESH'),key=lambda o:len(o.data.vertices))
report={'file_bytes':len(data),'joints':len(doc['skins'][0]['joints']),'meshes':len(doc['meshes']),'clips':{}}
for clip in ['Idle','Scamper','Alert','Swipe']:
 action=next(a for a in bpy.data.actions if a.name.startswith(clip));rig.animation_data.action=action
 start,end=action.frame_range;bounds=[];feet=[]
 for frame in [start,(end-start)*.25,(end-start)*.5,(end-start)*.75,end]:
  scene.frame_set(round(frame));deps=bpy.context.evaluated_depsgraph_get();obj=body.evaluated_get(deps);mesh=obj.to_mesh();pts=[obj.matrix_world@v.co for v in mesh.vertices];obj.to_mesh_clear()
  assert all(math.isfinite(c) for p in pts for c in p)
  # Imported glTF is Blender Z-up; all soles must stay near supporting plane.
  bounds.append([min(p.z for p in pts),max(p.z for p in pts)])
  feet.append([list(rig.matrix_world@rig.pose.bones['foot_'+s].head) for s in ['l','r']])
 assert min(b[0] for b in bounds)>-.002,(clip,bounds)
 assert max(b[1] for b in bounds)<1.12,(clip,bounds)
 if clip in ['Idle','Scamper']:
  assert abs(bounds[0][1]-bounds[-1][1])<.005,(clip,'loop jump',bounds)
 report['clips'][clip]={'frames':[start,end],'bounds':bounds,'feet':feet}
metadata=json.loads((ROOT/'assets/goblins/authoring.json').read_text())
assert abs(report['clips']['Idle']['bounds'][0][1]-report['clips']['Idle']['bounds'][0][0]-metadata['normalized_height'])<.0001
assert abs(report['clips']['Scamper']['frames'][1]/60-metadata['walk_cycle_seconds'])<.0001
(OUT/'goblin-validation.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:v for k,v in report.items() if k!='clips'}));print('PASS: GLB skin attributes, finite animation samples, planted bounds, loop continuity')
# Actual GLB stills, not the authoring scene, at locomotion contact and swing.
bpy.ops.object.camera_add(location=(1.05,-2.65,1.12));cam=bpy.context.object;cam.rotation_euler=(Vector((0,-.07,.51))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.lens=62;scene.camera=cam
for at,power,size in [((1,-3,4),260,3),((-2,-1,2),140,2),((0,2,3),220,2)]:
 bpy.ops.object.light_add(type='AREA',location=at);o=bpy.context.object;o.data.energy=power;o.data.shape='DISK';o.data.size=size;o.rotation_euler=(Vector((0,0,.5))-o.location).to_track_quat('-Z','Y').to_euler()
scene.world=bpy.data.worlds.new('QA studio');scene.world.use_nodes=True;scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.07,.085,.10,1)
scene.render.engine='CYCLES';scene.cycles.samples=24;scene.render.resolution_x=700;scene.render.resolution_y=800;scene.render.resolution_percentage=100
for clip,frame in [('Idle',0),('Scamper',10),('Scamper',30),('Idle',168)]:
 rig.animation_data.action=next(a for a in bpy.data.actions if a.name.startswith(clip));scene.frame_set(frame);scene.render.filepath=str(OUT/f'goblin-{clip}-{frame}.png');bpy.ops.render.render(write_still=True)
sys.stdout.flush();os._exit(0)
