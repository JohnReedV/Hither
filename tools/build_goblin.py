"""Author the Mireling GLB from CC0 anatomical topology, with bespoke proportions,
PBR skin, fitted breeches, and baked 60Hz full-skeleton animation.
Run: tools/.venv/bin/python -u tools/build_goblin.py
"""
from pathlib import Path
import math, os, sys, json
import bpy, bmesh
import numpy as np
from mathutils import Vector, Quaternion, Matrix
import build_avatar as author
from avatar_animation import rotate_bone, aim, plant_leg
ROOT=Path(__file__).resolve().parents[1];OUT=ROOT/'assets/goblins';OUT.mkdir(exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
macros=author.TargetService.get_default_macro_info_dict()
macros.update(gender=1.,age=.72,muscle=.16,weight=.24,proportions=.5,height=.5)
macros['race']={'caucasian':1.,'african':0.,'asian':0.}
body=author.HumanService.create_human(macro_detail_dict=macros);body.name='Mireling • continuous anatomical skin'
targets={'head/head-oval':.4,'head/head-fat-decr':.35,'nose/nose-scale-vert-incr':.35,'nose/nose-scale-depth-incr':.4,'nose/nose-point-width-incr':.45,'nose/nose-nostrils-width-incr':.25,'chin/chin-width-decr':.12,'mouth/mouth-scale-horiz-incr':.2,'stomach/stomach-tone-decr':.3}
targets.update({'expression/units/caucasian/mouth-corner-puller':.50,'expression/units/caucasian/mouth-open':.08,'expression/units/caucasian/eye-left-opened-up':.65,'expression/units/caucasian/eye-right-opened-up':.65,'mouth/mouth-lowerlip-volume-incr':.25})
for side in ['l','r']:
 for n,w in [('eyes/'+side+'-eye-scale-incr',.8),('eyes/'+side+'-eye-height1-incr',.5),('ears/'+side+'-ear-shape-pointed',1.),('ears/'+side+'-ear-scale-incr',.65),('ears/'+side+'-ear-wing-incr',.45)]:targets[n]=w
for name,w in targets.items():
 path=author.DATA/'targets'/(name+'.target.gz')
 if path.exists():author.TargetService.load_target(body,str(path),weight=w)
author.TargetService.bake_targets(body)
rig=author.HumanService.add_builtin_rig(body,'game_engine',import_weights=True);rig.name='Mireling skeleton'
eyes=author.add_asset(body,'eyes','high-poly',author.material('Jade eyes',author.ASSETS/'eyes/materials/bluegreen_eye.png',roughness=.24,alpha=True))
# Apply helper masks and retain the complete continuous body under the clothing.
bpy.context.view_layer.objects.active=body
for mod in list(body.modifiers):
 if mod.type!='ARMATURE':bpy.ops.object.modifier_apply(modifier=mod.name)
# Normalize the variable-height anatomical preset before applying sculpt landmarks.
source_scale=1.496/rig.data.bones['head'].tail_local.z
# Sculpt proportions in rest space, applying the identical map to skeleton/eyes.
# Large head, narrow chest, long fingers and a projecting neck match the reference.
def deform(p):
 x,y,z=Vector(p)*source_scale
 head=max(0.,min(1.,(z-1.22)/.095));head=head*head*(3-2*head)
 scale=1+head*.68
 nx=x*(.83+head*.85);ny=y*scale-head*.060;nz=z+head*(z-1.30)*.55
 # Lengthen already articulated fingers without thickening the palms.
 return Vector((nx,ny,nz))
for o in [body,eyes]:
 for v in o.data.vertices:v.co=deform(v.co)
 for p in o.data.polygons:p.use_smooth=True
bpy.context.view_layer.objects.active=rig;bpy.ops.object.mode_set(mode='EDIT')
saved={b.name:(b.head.copy(),b.tail.copy()) for b in rig.data.edit_bones}
for b in rig.data.edit_bones:b.use_connect=False
for b in rig.data.edit_bones:b.head=deform(saved[b.name][0]);b.tail=deform(saved[b.name][1])
bpy.ops.object.mode_set(mode='OBJECT')
# Enlarge the fully articulated hands and bare feet, including their skeleton.
for part,amount in [('hand',.40),('foot',.30)]:
 for side in ['l','r']:
  name=part+'_'+side;center=rig.data.bones[name].head_local.copy()
  names={b.name for b in rig.data.bones if b.name==name or name in [p.name for p in b.parent_recursive]}
  for v in body.data.vertices:
   weight=sum(g.weight for g in v.groups if body.vertex_groups[g.group].name in names)
   delta=v.co-center
   if part=='foot':delta.z=0
   v.co+=delta*(amount*weight)
  bpy.context.view_layer.objects.active=rig;bpy.ops.object.mode_set(mode='EDIT')
  saved={b.name:(b.head.copy(),b.tail.copy()) for b in rig.data.edit_bones if b.name in names}
  for b in rig.data.edit_bones:
   if b.name not in names:continue
   a,c=saved[b.name]
   da=a-center;dc=c-center
   if part=='foot':da.z=dc.z=0
   b.head=a+da*amount;b.tail=c+dc*amount
  bpy.ops.object.mode_set(mode='OBJECT')
# Enlarge the actual eyeballs and surrounding eyelid loops together, keeping
# the orbital rim and eyelid opening fitted rather than adding floating eyes.
eye_centers=[]
for sign in [-1,1]:
 verts=[v.co.copy() for v in eyes.data.vertices if v.co.x*sign>0]
 center=(Vector(tuple(min(v[i] for v in verts) for i in range(3)))+Vector(tuple(max(v[i] for v in verts) for i in range(3))))*.5
 eye_centers.append(center)
 for o in [body,eyes]:
  for v in o.data.vertices:
   d=v.co-center
   if o==eyes:
    influence=1. if v.co.x*sign>0 else 0.
   else:influence=math.exp(-((d.x/.047)**4+(d.z/.035)**4))*max(0.,min(1.,(-v.co.y-.13)/.035))
   v.co.x+=d.x*.40*influence;v.co.z+=d.z*.60*influence;v.co.y-=.005*influence
# Pull the upper helix to a true point while preserving concha and ear thickness.
for v in body.data.vertices:
 x,y,z=v.co
 ear=max(0,min(1,(abs(x)-.12)/.018))*math.exp(-((z-1.49)/.032)**2)
 v.co.x+=math.copysign(.050*ear,x);v.co.z+=.020*ear
 belly=math.exp(-(x/.08)**4-((z-.96)/.075)**2)*max(0,min(1,(-y-.03)/.05))
 v.co.y-=.018*belly
# Facial bones carry the upper eyelid skin, preserving the eyeball shape.
bpy.context.view_layer.objects.active=rig;bpy.ops.object.mode_set(mode='EDIT')
for side,c in zip(['r','l'],eye_centers):
 b=rig.data.edit_bones.new('lid_'+side);b.head=c;b.tail=c+Vector((0,0,.035));b.parent=rig.data.edit_bones['head']
bpy.ops.object.mode_set(mode='OBJECT')
for side,c in zip(['r','l'],eye_centers):
 group=body.vertex_groups.new(name='lid_'+side)
 for v in body.data.vertices:
  d=v.co-c
  weight=math.exp(-((d.x/.037)**6+((d.z-.018)/.021)**4))*max(0,min(1,(-v.co.y-.135)/.03))
  if d.z<0:weight*=max(0,1+d.z/.006)
  if weight>.005:
   for existing in list(v.groups):body.vertex_groups[existing.group].add([v.index],existing.weight*(1-weight),'REPLACE')
   group.add([v.index],weight,'REPLACE')
# Subdivision preserves anatomical loops and deformation, unlike voxel assembly.
bpy.context.view_layer.objects.active=body
sub=body.modifiers.new('Sculpt resolution','SUBSURF');sub.levels=2
bpy.ops.object.modifier_move_to_index(modifier=sub.name,index=0);bpy.ops.object.modifier_apply(modifier=sub.name)
# Restrict blink influence to the lid, leaving the bony brow stationary.
for side in ['l','r']:
 c=rig.data.bones['lid_'+side].head_local;group=body.vertex_groups['lid_'+side];head=body.vertex_groups['head']
 for v in body.data.vertices:
  old=next((g.weight for g in v.groups if g.group==group.index),0.)
  if old<=0:continue
  d=v.co-c;new=old*math.exp(-((d.z-.013)/.016)**4)
  hw=next((g.weight for g in v.groups if g.group==head.index),0.)
  group.add([v.index],new,'REPLACE');head.add([v.index],hw+old-new,'REPLACE')
# Fine geometric creases and asymmetric aging around eyes and forehead.
for v in body.data.vertices:
 x,y,z=v.co
 front=max(0.,min(1.,(-y-.12)/.07))
 brow=math.exp(-((z-1.48)/.022)**2)*math.exp(-((abs(x)-.065)/.055)**2)
 v.co.y-=.005*brow*front
 forehead=math.exp(-((z-1.555)/.032)**2)*math.exp(-(x/.105)**6)*front
 v.co.y+=.0015*forehead*math.sin(z*370+x*9)
 # Nasolabial fold is sculpted into the cheek surface.
 fold=math.exp(-((abs(x)-(.050+(1.43-z)*.35))/.003)**2)*math.exp(-((z-1.407)/.036)**2)*front
 v.co.y+=.002*fold
# Spatially paint skin into the anatomical UVs (albedo remains a portable texture).
source=bpy.data.images.load(str(ROOT/'assets/avatars/source/skin_clean_base.png'))
w,h=source.size
# Preserve source capillary/anatomical variation while keeping asset size bounded.
if w>2048:source.scale(2048,2048)
w,h=source.size;rgba=np.array(source.pixels[:],dtype=np.float32).reshape(h,w,4)
rng=np.random.default_rng(67391)
def noise(cells):
 grid=rng.random((cells+1,cells+1));xx=np.linspace(0,cells,w,endpoint=False);yy=np.linspace(0,cells,h,endpoint=False);ix=xx.astype(int);iy=yy.astype(int);fx=xx-ix;fy=yy-iy;fx=fx*fx*(3-2*fx);fy=fy*fy*(3-2*fy)
 return (grid[iy[:,None],ix]*(1-fx)+grid[iy[:,None],ix+1]*fx)*(1-fy[:,None])+(grid[iy[:,None]+1,ix]*(1-fx)+grid[iy[:,None]+1,ix+1]*fx)*fy[:,None]
broad=noise(35);mottle=noise(140);grain=noise(550);pores=rng.random((h,w))
# UV-position atlas, rasterized from the actual sculpt rather than guessed islands.
pos=np.zeros((h,w,3),dtype=np.float32);mask=np.zeros((h,w),dtype=bool)
body.data.calc_loop_triangles();uv=body.data.uv_layers.active.data
for tri in body.data.loop_triangles:
 tex=np.array([uv[i].uv[:] for i in tri.loops])*[w-1,h-1];points=np.array([body.data.vertices[i].co[:] for i in tri.vertices])
 lo=np.maximum(np.floor(tex.min(axis=0)).astype(int),0);hi=np.minimum(np.ceil(tex.max(axis=0)).astype(int),[w-1,h-1])
 if np.any(hi<lo):continue
 yy,xx=np.mgrid[lo[1]:hi[1]+1,lo[0]:hi[0]+1];a,b,c=tex;den=(b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1])
 if abs(den)<1e-8:continue
 u=((b[1]-c[1])*(xx-c[0])+(c[0]-b[0])*(yy-c[1]))/den;v=((c[1]-a[1])*(xx-c[0])+(a[0]-c[0])*(yy-c[1]))/den;inside=(u>=-.01)&(v>=-.01)&(u+v<=1.01)
 value=u[:,:,None]*points[0]+v[:,:,None]*points[1]+(1-u-v)[:,:,None]*points[2]
 tile=pos[lo[1]:hi[1]+1,lo[0]:hi[0]+1];tile[inside]=value[inside];mask[lo[1]:hi[1]+1,lo[0]:hi[0]+1]|=inside
x,y,z=pos[:,:,0],pos[:,:,1],pos[:,:,2]
front=np.clip((-y-.03)/.07,0,1)
belly=np.exp(-(x/.13)**4-((z-1.02)/.23)**4)*front
face=np.exp(-((z-1.40)/.09)**2)*np.clip((-y-.16)/.06,0,1)
ear=np.clip((np.abs(x)-.12)/.04,0,1)*np.exp(-((z-1.48)/.095)**2)
eye=np.maximum.reduce([np.exp(-((x-c.x)/.040)**4-((z-c.z)/.032)**4)*np.clip((-y-.13)/.035,0,1) for c in eye_centers])
lips=np.exp(-(x/.063)**6-((z-1.383)/.013)**2)*np.clip((-y-.18)/.04,0,1)
warm=np.maximum.reduce([belly*.85,face*.62,ear*.75,lips])
green=np.zeros_like(pos)+[.32,.38,.18];ochre=np.zeros_like(pos)+[.56,.37,.24]
rgb=green*(1-warm[:,:,None])+ochre*warm[:,:,None]
source_luma=rgba[:,:,:3].mean(axis=2);rgb*=np.clip(source_luma/.48,.52,1.3)[:,:,None]
spots=np.clip((grain-.65)*5,0,1)*np.clip((mottle-.5)*5,0,1)
rgb*= (1-.24*(broad-.3)-.23*spots)[:,:,None]
rgb=rgb*(1-eye[:,:,None]*.66)+eye[:,:,None]*np.array([.065,.045,.043])*.66
rgb=rgb*(1-lips[:,:,None]*.45)+lips[:,:,None]*np.array([.37,.15,.14])*.45
height=.05*mottle+.10*grain+.024*pores-.08*spots
dy,dx=np.gradient(height);normal=np.stack((-dx*1.5,-dy*1.5,np.ones_like(dx)),axis=-1);normal/=np.linalg.norm(normal,axis=2)[:,:,None]
def material(name,color,rough=.7):
 m=bpy.data.materials.new(name);m.use_nodes=True;p=m.node_tree.nodes.get('Principled BSDF');p.inputs['Base Color'].default_value=(*color,1);p.inputs['Roughness'].default_value=rough;p.inputs['Specular IOR Level'].default_value=.28;return m
skin=material('Mireling / mottled olive, rose skin and pores',(.4,.4,.2))
def image_node(mat,name,data,noncolor=False):
 im=bpy.data.images.new(name,width=w,height=h);pixels=np.ones((h,w,4),dtype=np.float32);pixels[:,:,:3]=data
 if noncolor:im.colorspace_settings.name='Non-Color'
 im.pixels.foreach_set(pixels.ravel());im.pack();node=mat.node_tree.nodes.new('ShaderNodeTexImage');node.image=im;return node
p=skin.node_tree.nodes.get('Principled BSDF');links=skin.node_tree.links
links.new(image_node(skin,'Mireling skin albedo',np.clip(rgb,0,1)).outputs['Color'],p.inputs['Base Color'])
links.new(image_node(skin,'Mireling skin roughness',np.repeat(np.clip(.60+.14*broad-.1*lips,.45,.8)[:,:,None],3,axis=2),True).outputs['Color'],p.inputs['Roughness'])
n=skin.node_tree.nodes.new('ShaderNodeNormalMap');links.new(image_node(skin,'Mireling skin pore normal',normal*.5+.5,True).outputs['Color'],n.inputs['Color']);links.new(n.outputs['Normal'],p.inputs['Normal'])
body.data.materials.clear();body.data.materials.append(skin)
for p in body.data.polygons:p.material_index=0
# Fitted shorts copied from anatomical topology: joined crotch, rolled waistband,
# irregular hems, surface folds, stitched seams. Never two separate spheres.
pants=body.copy();pants.data=body.data.copy();bpy.context.collection.objects.link(pants);pants.name='Oxblood leather breeches'
bm=bmesh.new();bm.from_mesh(pants.data);bmesh.ops.delete(bm,geom=[v for v in bm.verts if abs(v.co.x)>.19 or v.co.z>.87 or v.co.z<.70+.009*math.sin(v.co.x*60)],context='VERTS');bm.to_mesh(pants.data);bm.free()
for v in pants.data.vertices:
 x,y,z=v.co;v.co.x*=1.065;v.co.y*=1.09
 v.co+=v.normal*(.006+.001*math.sin(z*65+x*18)*math.sin(x*25))
leather=material('Cracked oxblood leather',(.067,.033,.021),.66);pants.data.materials.clear();pants.data.materials.append(leather)
for p in pants.data.polygons:p.material_index=0
# Sparse accessory details stay skinned; continuous anatomy supplies all fingers.
def bind(o,bone,mat):
 bpy.context.view_layer.objects.active=o;bpy.ops.object.transform_apply(location=True,rotation=True,scale=True);o.data.materials.append(mat);g=o.vertex_groups.new(name=bone);g.add(list(range(len(o.data.vertices))),1.,'REPLACE');mod=o.modifiers.new('Skin','ARMATURE');mod.object=rig;o.parent=rig
 for p in o.data.polygons:p.use_smooth=True
 return o
def curve(name,points,r,bone,mat):
 data=bpy.data.curves.new(name,'CURVE');data.dimensions='3D';data.bevel_depth=r;data.bevel_resolution=3;s=data.splines.new('POLY');s.points.add(len(points)-1)
 for p,co in zip(s.points,points):p.co=(*co,1)
 o=bpy.data.objects.new(name,data);bpy.context.collection.objects.link(o);bpy.ops.object.select_all(action='DESELECT');o.select_set(True);bpy.context.view_layer.objects.active=o;bpy.ops.object.convert(target='MESH');return bind(bpy.context.object,bone,mat)
horn=material('Dark worn fingernails',(.10,.043,.028),.40)
for side in ['l','r']:
 for finger in ['index','middle','ring','pinky','thumb']:
  name=f'{finger}_03_{side}'
  bone=rig.data.bones.get(name)
  if not bone:continue
  direction=(bone.tail_local-bone.head_local).normalized()
  center=bone.tail_local-direction*.009+Vector((0,-.004,.003))
  bpy.ops.mesh.primitive_uv_sphere_add(segments=16,ring_count=10,location=center)
  o=bpy.context.object;o.name='Worn nail '+name;o.scale=(.0055,.012,.0025);o.rotation_euler=direction.to_track_quat('Y','Z').to_euler();bind(o,name,horn)
thread=material('Worn flax thread',(.36,.23,.12),.9)
for side in [-1,1]:
 for j in range(14):
  z=.72+j*.010;x=side*(.117+.006*math.sin(z*17));curve('Breeches hand stitch',[(x,-.048,z),(x+side*.007,-.052,z+.004)],.0015,'pelvis',thread)
# Merge accessories by material, retaining bone weights and minimizing draws.
for prefix in ['Breeches hand stitch','Worn nail']:
 obs=[o for o in bpy.data.objects if o.type=='MESH' and o.name.startswith(prefix)]
 bpy.ops.object.select_all(action='DESELECT')
 for o in obs:o.select_set(True)
 bpy.context.view_layer.objects.active=obs[0];bpy.ops.object.join()
# Store anatomically derived metadata and normalize the posed crown to runtime contract.
scene=bpy.context.scene;scene.render.fps=60
rest={b.name:(b.head_local.copy(),b.tail_local.copy()) for b in rig.data.bones}
sole={}
for side in ['l','r']:
 names={b.name for b in rig.data.bones if b.name=='foot_'+side or 'foot_'+side in [p.name for p in b.parent_recursive]}
 sole[side]=[v.co-rest['foot_'+side][0] for v in body.data.vertices if sum(g.weight for g in v.groups if body.vertex_groups[g.group].name in names)>.95 and v.co.z<.06]
 assert sole[side], 'Missing foot sole vertices'
def arm(side,swing,lift):
 sign=1 if side=='l' else -1;upper=rig.pose.bones['upperarm_'+side];lower=rig.pose.bones['lowerarm_'+side];shoulder=upper.head.copy()
 wrist=shoulder+Vector((sign*.055,-.045+swing,-.39+lift));d=wrist-shoulder;length=min(d.length,upper.length+lower.length-.005);axis=d.normalized();along=(upper.length**2-lower.length**2+length**2)/(2*length);pole=Vector((sign*.3,1,0));pole=(pole-axis*pole.dot(axis)).normalized();elbow=shoulder+axis*along+pole*math.sqrt(max(0,upper.length**2-along**2));aim(rig,'upperarm_'+side,shoulder,elbow);aim(rig,'lowerarm_'+side,elbow,shoulder+axis*length)
def pose(clip,u):
 for b in rig.pose.bones:b.rotation_mode='QUATERNION';b.rotation_quaternion=Quaternion();b.location=(0,0,0);b.scale=(1,1,1)
 a=u*math.tau;moving=clip=='Scamper';breath=math.sin(a)
 root=rig.pose.bones['Root'];root.location=root.bone.matrix_local.to_quaternion().inverted()@Vector((.012*math.sin(a) if moving else 0,0,-.24+(.012*math.cos(2*a) if moving else .003*breath)))
 rotate_bone(rig,'spine_01',(.11,0,.025*math.sin(a) if moving else 0));rotate_bone(rig,'spine_02',(.14+.008*breath,0,0));rotate_bone(rig,'spine_03',(.10,0,-.04*math.sin(a) if moving else .008*breath));rotate_bone(rig,'neck_01',(-.20,0,0));rotate_bone(rig,'head',(-.30,.025*math.sin(a),.045*math.sin(a) if not moving else -.025*math.sin(a)))
 if clip=='Alert':rotate_bone(rig,'head',(-.30-.13*math.sin(math.pi*u),0,.10*math.sin(a)))
 bpy.context.view_layer.update()
 for side,sign in [('l',1),('r',-1)]:
  v=(u+(0 if sign==1 else .5))%1
  travel=lift=roll=0
  if moving:
   if v<.6:travel=-.16+.32*v/.6;roll=.10*(1-v/.6)-.14*(v/.6)**6
   else:
    t=(v-.6)/.4;travel=.16+.213333333*t-1.6*t*t+1.066666667*t*t*t;lift=.075*math.sin(math.pi*t)**2;roll=-.14*(1-t)+.10*t
  sole_rotation=Quaternion((0,0,1),sign*.09)@Quaternion((1,0,0),roll)
  support_lift=min(p.z for p in sole[side])-min((sole_rotation@p).z for p in sole[side])
  ankle=rest['foot_'+side][0]+Vector((sign*.012,travel,lift+support_lift));plant_leg(rig,side,ankle,roll,sign*.09)
  swing=-.09*math.sin(a)*sign if moving else .01*breath
  up=.01*math.sin(a+.4)*sign if moving else 0
  if clip=='Swipe' and side=='r':swing-=.30*math.sin(math.pi*u)**2;up+=.22*math.sin(math.pi*u)**2
  arm(side,swing,up)
  for finger in ['index','middle','ring','pinky','thumb']:
   for seg in ['01','02','03']:
    name=f'{finger}_{seg}_{side}'
    rotate_bone(rig,name,(.12+.09*math.sin(a-.5+int(seg)*.3)+( .24*math.sin(math.pi*u)**2 if clip=='Swipe' else 0),0,0))
 for side in ['l','r']:
  blink=max(0.,1-abs(u-.70)/.035)
  lid=rig.pose.bones['lid_'+side];lid.location=lid.bone.matrix_local.to_quaternion().inverted()@Vector((0,-.004*blink,-.038*blink))
 bpy.context.view_layer.update()
for clip,seconds in [('Idle',4.),('Scamper',.72),('Alert',1.1),('Swipe',1.2)]:
 action=bpy.data.actions.new(clip);rig.animation_data_create();rig.animation_data.action=action
 frames=round(seconds*60)
 for frame in range(frames+1):
  pose(clip,frame/frames)
  for b in rig.pose.bones:b.keyframe_insert('rotation_quaternion',frame=frame);b.keyframe_insert('location',frame=frame);b.keyframe_insert('scale',frame=frame)
 rig.animation_data.action=None;track=rig.animation_data.nla_tracks.new();track.name=clip;track.strips.new(clip,0,action);track.mute=True
pose('Idle',0)
# Evaluate posed mesh extent, including crown, and normalize consistently.
deps=bpy.context.evaluated_depsgraph_get();evaluated=body.evaluated_get(deps);mesh=evaluated.to_mesh();lo=min(v.co.z for v in mesh.vertices);hi=max(v.co.z for v in mesh.vertices);evaluated.to_mesh_clear();factor=1.024/(hi-lo)
root=bpy.data.objects.new('Mireling normalized scale',None);bpy.context.collection.objects.link(root)
for o in list(bpy.context.scene.objects):
 if o!=root and o.parent is None:o.parent=root
root.scale=(factor,)*3;root.location.z=-lo*factor
(OUT/'authoring.json').write_text(json.dumps({'normalized_height':1.024,'source_height':hi-lo,'normalization':factor,'walk_cycle_seconds':round(.72*60)/60,'stance_fraction':.6,'stance_travel':.32*factor,'vertices':len(body.data.vertices),'clips':['Idle','Scamper','Alert','Swipe']},indent=2)+'\n')
for t in rig.animation_data.nla_tracks:t.mute=False
bpy.ops.export_scene.gltf(filepath=str(OUT/'mireling.glb'),export_format='GLB',export_animation_mode='NLA_TRACKS',export_force_sampling=True,export_skins=True,export_yup=True)
for t in rig.animation_data.nla_tracks:t.mute=True
rig.animation_data.action=bpy.data.actions['Idle'];scene.frame_set(0)
# Render the same exported geometry in a neutral studio for inspection.
bpy.ops.object.camera_add(location=(1.15,-2.65,1.12));cam=bpy.context.object;cam.rotation_euler=(Vector((0,-.06,.51))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.lens=64;scene.camera=cam
for at,power,size in [((1,-3,4),240,3),((-2,-1,2),120,2),((0,2,3),240,2)]:
 bpy.ops.object.light_add(type='AREA',location=at);o=bpy.context.object;o.data.energy=power;o.data.shape='DISK';o.data.size=size;o.rotation_euler=(Vector((0,0,.5))-o.location).to_track_quat('-Z','Y').to_euler()
scene.world=bpy.data.worlds.new('Studio');scene.world.use_nodes=True;scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.07,.085,.10,1)
scene.render.engine='CYCLES';scene.cycles.samples=32;scene.render.resolution_x=900;scene.render.resolution_y=1000;scene.render.resolution_percentage=100
scene.render.filepath=str(ROOT/'docs/goblin-preview.png');bpy.ops.render.render(write_still=True)
bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/'tools/build/mireling.blend'))
import subprocess
subprocess.run([sys.executable, str(ROOT / "tools/build_goblin_gameplay.py")], check=True)
subprocess.run([sys.executable, str(ROOT / "tools/check_goblin_gameplay.py")], check=True)
sys.stdout.flush();os._exit(0)
