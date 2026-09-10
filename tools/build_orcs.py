"""Author three skinned orcs from the bundled CC0 humanoid template (headless).
Keeps the player's skeleton/locomotion contract, not the player's portrait.
Run: tools/.venv/bin/python -u tools/build_orcs.py
"""
from pathlib import Path
import math, os, sys
import bpy
import numpy as np
from mathutils import Vector, Quaternion
from avatar_animation import pose, rotate_bone, aim
from export_avatar import export_avatar
import build_avatar as author
from avatar_animation import bake_clips
from orc_surfaces import surface, skin as paint_skin, sculpt, tailor, forged_shell, tusk

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'assets/orcs'
OUT.mkdir(exist_ok=True)

def material(name, color, metal=0., rough=.9):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    p = m.node_tree.nodes.get('Principled BSDF')
    p.inputs['Base Color'].default_value = (*color, 1)
    p.inputs['Metallic'].default_value = metal
    p.inputs['Roughness'].default_value = rough
    return m

def bind(obj, rig, bone, mat):
    obj.data.materials.clear(); obj.data.materials.append(mat)
    bpy.context.view_layer.objects.active=obj
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    group=obj.vertex_groups.new(name=bone)
    group.add(list(range(len(obj.data.vertices))),1.,'REPLACE')
    mod=obj.modifiers.new('Skeleton','ARMATURE');mod.object=rig
    for p in obj.data.polygons:p.use_smooth=True
    return obj

def sphere(name, at, size, rig, bone, mat):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=24,ring_count=12,location=at)
    o=bpy.context.object;o.name=name;o.scale=size
    return bind(o,rig,bone,mat)

def rod(name,a,b,r,rig,bone,mat,tip=None):
    a,b=Vector(a),Vector(b)
    bpy.ops.mesh.primitive_cone_add(vertices=16,radius1=r,radius2=r if tip is None else tip,depth=(b-a).length,location=(a+b)/2)
    o=bpy.context.object;o.name=name;o.rotation_mode='QUATERNION';o.rotation_quaternion=(b-a).to_track_quat('Z','Y')
    return bind(o,rig,bone,mat)

def build(variant):
    bpy.ops.wm.open_mainfile(filepath=str(ROOT/'assets/avatars/source/default.blend'))
    old_rig=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE')
    bpy.data.objects.remove(bpy.data.objects['Body'],do_unlink=True)
    macros=author.TargetService.get_default_macro_info_dict()
    macros.update(gender=1.,age=.30,muscle=.78,weight=.55,proportions=.5,height=.5)
    macros['race']={'caucasian':1.,'african':0.,'asian':0.}
    body=author.HumanService.create_human(macro_detail_dict=macros)
    body.name='Body'
    author.TargetService.bake_targets(body)
    rig=author.HumanService.add_builtin_rig(body,'game_engine',import_weights=True)
    for o in bpy.context.scene.objects:
        for modifier in o.modifiers:
            if modifier.type=='ARMATURE' and modifier.object==old_rig:modifier.object=rig
    bpy.data.objects.remove(old_rig,do_unlink=True)
    rig.name='OrcSkeleton'
    # Apply helper-geometry masks only; no clothing masks are added to this body.
    bpy.context.view_layer.objects.active=body
    for modifier in list(body.modifiers):
        if modifier.type!='ARMATURE':bpy.ops.object.modifier_apply(modifier=modifier.name)
    bake_clips(rig)
    for t in rig.animation_data.nla_tracks:t.mute=True
    rig.animation_data.action=None
    for b in rig.pose.bones:b.matrix_basis.identity()
    bpy.context.view_layer.update()
    for name in ['Open plaid jacket','Gray crewneck T-shirt','Body.short02','Body.shoes02']:
        o=bpy.data.objects.get(name)
        if o:bpy.data.objects.remove(o,do_unlink=True)
    body=bpy.data.objects['Body'];body.data.materials.clear()
    for f in body.data.polygons:f.material_index=0
    # Anatomical mesh deformation: broad jaw, compressed muzzle, heavy brow,
    # pointed ears. No human portrait or disconnected primitive head.
    for v in body.data.vertices:
        x,y,z=v.co
        if z>1.29:
            jaw=math.exp(-((z-1.325)/.032)**2)
            brow=math.exp(-((z-1.408)/.014)**2)
            front=max(0,min(1,(-y-.035)/.055))
            v.co.x*=1.07+(.13+.02*variant)*jaw
            cheek=math.exp(-((z-1.373)/.018)**2)*math.exp(-((abs(x)-.042)/.025)**2)
            v.co.y-=front*(.010*jaw+.007*brow+.004*cheek)
            # Localized upper helix only: preserve the concha, lobe and neck.
            ear=max(0,min(1,(abs(x)-.068)/.018))*math.exp(-((z-1.402)/.021)**2)
            v.co.x+=math.copysign(.010*ear,x)
            v.co.z+=.009*ear
            # Uneven brow and a healed shallow cheek gouge in the actual mesh.
            scar=math.exp(-((x-.038-(z-1.36)*.30)/.0028)**2)*math.exp(-((z-1.366)/.024)**2)
            v.co.y+=front*.0018*scar
    # Paint landmarks on the base UV topology before increasing sculpt density.
    body.data.materials.append(paint_skin(body,variant,ROOT/'assets/avatars/source/skin_clean_base.png'))
    sculpt(body,variant)
    eyes=bpy.data.objects['Body.high-poly']
    eyes.data.materials.clear()
    eyes.data.materials.append(author.material('Orc brown eyes',
        author.ASSETS/'eyes/materials/brown_eye.png',roughness=.3,alpha=True))
    for polygon in eyes.data.polygons:polygon.material_index=0
    for v in eyes.data.vertices:v.co.x*=1.07;v.co.y-=.002
    leather=material('Cracked umber leather',(.065,.038,.018))
    pants=bpy.data.objects['Blue denim jeans'];pants.name='Ragged leather trousers'
    pants.data.materials.clear();pants.data.materials.append(leather)
    for f in pants.data.polygons:f.material_index=0
    iron=surface('Hammered iron / dark steel and localized oxide',(.19,.20,.20),773,'iron');bone=surface('Worn tusk ivory',(.72,.63,.45),412)
    # A fitted, hollow forged shell: armor has its own smooth topology rather
    # than inheriting nipples and skin folds from the anatomical body.
    import bmesh
    rings=[(.82,.16,.10),(.86,.158,.105),(.93,.15,.12),(1.02,.18,.135),(1.10,.20,.137),(1.17,.21,.123),(1.22,.15,.10)]
    verts=[];faces=[]
    for z,rx,ry in rings:
        for i in range(32):
            a=i*math.tau/32
            verts.append((math.cos(a)*rx,math.sin(a)*ry-.02,z))
    for j in range(len(rings)-1):
        for i in range(32):
            n=(i+1)%32;faces.append((j*32+i,j*32+n,(j+1)*32+n,(j+1)*32+i))
    mesh=bpy.data.meshes.new('Forged cuirass shell');mesh.from_pydata(verts,[],faces);mesh.update()
    uv=mesh.uv_layers.new(name='UVMap')
    for loop in mesh.loops:
        i=loop.vertex_index;uv.data[loop.index].uv=((i%32)/32,(i//32)/6)
    armor=bpy.data.objects.new('Hammered riveted cuirass',mesh);bpy.context.collection.objects.link(armor);bind(armor,rig,'spine_02',iron)
    for side in [-1,1]:
        label='l' if side>0 else 'r'
        if variant != 2 or side == -1:
            forged_shell('Dented open shoulder plate', (side*.17,-.016,1.175),(.083,.085,.068) if variant==1 else (.073,.078,.046),rig,'upperarm_'+label,iron,bind)
        rod('Forearm iron splint',(side*.32,-.04,1.01),(side*.39,-.13,.95),.039,rig,'lowerarm_'+label,iron if variant==1 or side==-1 else leather,tip=.028)
        tusk(side,rig,bone,bind,variant)
        for z in [.87,.97,1.07,1.17]:
            sphere('Forged cuirass rivet',(side*.115,-.119,z),(.005,.004,.005),rig,'spine_02',iron)
    if variant==1:
        forged_shell('Forged fitted skullcap',(0,-.024,1.447),(.079,.088,.054),rig,'head',iron,bind,helmet=True)
    if variant==2:
        for i in range(5):
            sphere('Knotted black crest',(0,-.075+i*.03,1.48),(.017,.025,.024),rig,'head',leather)
    from orc_outfits import dress
    dress(variant,rig,armor,pants,iron,bone,bind,sphere,rod)
    tailor(armor,variant)
    # Close every finger around the haft, and derive its position from the
    # curled middle finger rather than offsetting it from the wrist joint.
    grip={}
    for finger in ['index','middle','ring','pinky']:
        for segment,angle in [('01',.70),('02',1.30),('03',.90)]:
            name=f'{finger}_{segment}_r'
            if name in rig.pose.bones:
                grip[name]=Quaternion((1,0,0),angle)
    for segment,angle in [('01',.55),('02',.8),('03',.6)]:
        name=f'thumb_{segment}_r'
        if name in rig.pose.bones:grip[name]=Quaternion((1,0,0),angle)
    for name,q in grip.items():rig.pose.bones[name].rotation_quaternion=q
    bpy.context.view_layer.update()
    middle=rig.pose.bones['middle_02_r']
    hand=(middle.head+rig.pose.bones['middle_03_r'].tail)*.5
    axis=(rig.data.bones['index_01_r'].head_local-rig.data.bones['pinky_01_r'].head_local).normalized()
    if axis.z<0:axis=-axis
    end=hand+axis*.52
    rod('Oak tool haft',hand-axis*.13,end,.014,rig,'hand_r',leather)
    for i in range(7):
        rod('Grip wrap',hand+axis*(i*.014-.05),hand+axis*(i*.014-.045),.016,rig,'hand_r',leather)
    if variant==0:
        rod('Forged mace socket',end-axis*.075,end+axis*.075,.027,rig,'hand_r',iron)
        tangent=axis.cross(Vector((1,0,0))).normalized()
        bitangent=axis.cross(tangent).normalized()
        for i in range(6):
            a=i*math.tau/6
            radial=tangent*math.cos(a)+bitangent*math.sin(a)
            across=axis.cross(radial)
            profile=[(.021,-.073),(.058,-.061),(.074,-.027),(.072,.047),(.037,.078),(.021,.073)]
            points=[end+radial*r+axis*z+across*thickness for thickness in [-.004,.004] for r,z in profile]
            faces=[tuple(reversed(range(6))),tuple(range(6,12))]
            faces += [(j,(j+1)%6,(j+1)%6+6,j+6) for j in range(6)]
            mesh=bpy.data.meshes.new('Forged angular mace flange');mesh.from_pydata(points,[],faces);mesh.update()
            uv=mesh.uv_layers.new(name='UVMap')
            for loop in mesh.loops:
                co=mesh.vertices[loop.vertex_index].co-end
                uv.data[loop.index].uv=(co.dot(radial)*7,co.dot(axis)*6+.5)
            o=bpy.data.objects.new('Beveled steel mace flange',mesh);bpy.context.collection.objects.link(o)
            bpy.context.view_layer.objects.active=o
            bevel=o.modifiers.new('Worn flange edges','BEVEL');bevel.width=.0018;bevel.segments=2
            bpy.ops.object.modifier_apply(modifier=bevel.name)
            bind(o,rig,'hand_r',iron)
            for polygon in o.data.polygons:polygon.use_smooth=False
    else:
        # A beveled wedge tool head, not a cartoon oversized axe.
        verts=[(-.02,-.03,-.04),(-.02,.03,-.04),(-.02,-.03,.06),(-.02,.03,.06),(.16,-.007,-.095),(.16,.007,-.095),(.16,-.007,.09),(.16,.007,.09)]
        if variant==2:verts=[(x*.8,y,z*.75) for x,y,z in verts]
        mesh=bpy.data.meshes.new('Axe wedge');mesh.from_pydata([Vector(v)+end for v in verts],[],[(0,1,3,2),(4,6,7,5),(0,4,5,1),(2,3,7,6),(0,2,6,4),(1,5,7,3)]);mesh.update()
        uv=mesh.uv_layers.new(name='UVMap')
        for loop in mesh.loops:
            co=mesh.vertices[loop.vertex_index].co-end
            uv.data[loop.index].uv=(co.x*4+.1,co.z*4+.5)
        o=bpy.data.objects.new('Rusted hewing axe',mesh);bpy.context.collection.objects.link(o)
        bpy.context.view_layer.objects.active=o
        bevel=o.modifiers.new('Forged bevel and cutting edge','BEVEL');bevel.width=.003;bevel.segments=3
        bpy.ops.object.modifier_apply(modifier=bevel.name)
        bind(o,rig,'hand_r',iron)
        for polygon in o.data.polygons:polygon.use_smooth=False
    # Retain the anatomically skinned feet/ankles; trousers only hide the legs.
    nail=material('Filthy split yellow-brown toenails',(.29,.235,.09),rough=.96)
    for side in [-1,1]:
        bone='foot_l' if side>0 else 'foot_r'
        center=rig.data.bones[bone].head_local
        for toe in range(5):
            x=center.x+side*(-.035+toe*.018)
            y=-.165+toe*.009
            base=Vector((x,y,.024))
            length=.032+(4-toe)*.004+variant*.002
            tip=base+Vector((side*.004,-length,-.008))
            rod('Grimy overgrown toenail',base,tip,.008-toe*.0007,rig,bone,nail,tip=.0006)
    bm=bmesh.new();bm.from_mesh(body.data)
    # Keep continuous chest/axilla/shoulder skin through the entire swing.
    # Only the lower legs fully enclosed by trousers are masked.
    bmesh.ops.delete(bm,geom=[v for v in bm.verts if .18<v.co.z<.82],context='VERTS')
    for v in bm.verts:
        if .84<v.co.z<1.20:
            core=math.exp(-(abs(v.co.x)/.13)**4)
            v.co.y*=1-.12*core
    bm.to_mesh(body.data);bm.free()
    for p in body.data.polygons:p.use_smooth=True
    action=bpy.data.actions.new('Attack');rig.animation_data.action=action
    for f in range(31):
        t=f/30;pose(rig,'Idle',0)
        swing=math.sin(t*math.pi)**2
        rotate_bone(rig,'upperarm_r',(-1.4*swing,.1,-.25*swing))
        rotate_bone(rig,'lowerarm_r',(-.6*swing,0,0))
        for b in rig.pose.bones:
            b.keyframe_insert('rotation_quaternion',frame=f)
            b.keyframe_insert('location',frame=f)
    rig.animation_data.action=None
    track=rig.animation_data.nla_tracks.new();track.name='Attack';track.strips.new('Attack',0,action);track.mute=True
    # Lift, carry overhead, then cast the boulder aside. The free palm supports
    # its underside while the weapon stays lowered in the opposite hand.
    action=bpy.data.actions.new('Push');rig.animation_data.action=action
    for f in range(451):
        t=f/450
        if t<.25:pose(rig,'StrafeLeft',(t*7.5/.8)%1.)
        elif t<.65:pose(rig,'Walk',((t-.25)*7.5/.96)%1.)
        else:pose(rig,'Idle',0)
        if .60<t<.65:
            # Settle the final carrying step before loading the jump.
            walking={b.name:(b.location.copy(),b.rotation_quaternion.copy()) for b in rig.pose.bones}
            pose(rig,'Idle',0)
            from avatar_animation import smooth
            settle=smooth((t-.60)/.05)
            for b in rig.pose.bones:
                location,rotation=walking[b.name]
                b.location=location.lerp(b.location,settle)
                b.rotation_quaternion=rotation.slerp(b.rotation_quaternion,settle)
        effort=max(0,min(1,t/.25));effort=effort*effort*(3-2*effort)
        release=max(0,min(1,(t-.66)/.04))
        recover=max(0,min(1,(t-.78)/.22))
        from orc_animation import stone_jump
        stone_jump(rig,t)
        rotate_bone(rig,'spine_02',(.13*(1-effort),-.12*math.sin(release*math.pi),0))
        bpy.context.view_layer.update()
        upper=rig.pose.bones['upperarm_l'];lower=rig.pose.bones['lowerarm_l']
        shoulder=upper.head.copy()
        from avatar_animation import smooth
        follow_phase=max(0.,min(1.,(t-.70)/.04))
        follow_through=.45*(1-(1-follow_phase)**3)*(1-recover)
        target=shoulder+Vector((.02+.24*release*release,-.02-.06*release,
                               .06+.43*effort-.32*recover-follow_through))
        delta=target-shoulder;length=min(delta.length,upper.length+lower.length-.003)
        axis=delta.normalized();along=(upper.length**2-lower.length**2+length**2)/(2*length)
        height=math.sqrt(max(0,upper.length**2-along**2))
        pole=Vector((-.35,.45,-1));pole=(pole-axis*pole.dot(axis)).normalized()
        elbow=shoulder+axis*along+pole*height
        aim(rig,'upperarm_l',shoulder,elbow);aim(rig,'lowerarm_l',elbow,shoulder+axis*length)
        from orc_animation import arm, weapon_wrist, supporting_palm
        supporting_palm(rig,effort,recover)
        arm(rig,'r',(-.06,-.23,-.23));weapon_wrist(rig,(-.12,-.55,.82))
        for b in rig.pose.bones:
            b.keyframe_insert('rotation_quaternion',frame=f)
            b.keyframe_insert('location',frame=f)
    rig.animation_data.action=None
    track=rig.animation_data.nla_tracks.new();track.name='Push';track.strips.new('Push',0,action);track.mute=True
    from orc_animation import animate_orc
    animate_orc(rig,body,variant)
    # Keep the grip in every locomotion and attack clip, not only the bind pose.
    for track in rig.animation_data.nla_tracks:
        for strip in track.strips:
            action=strip.action
            for name,q in grip.items():
                path=f'pose.bones["{name}"].rotation_quaternion'
                for curve in list(action.fcurves):
                    if curve.data_path==path:action.fcurves.remove(curve)
                for component,value in enumerate(q):
                    curve=action.fcurves.new(path,index=component)
                    curve.keyframe_points.insert(0,value)
                    curve.keyframe_points.insert(max(1,strip.action_frame_end),value)
    from orc_animation import validate_weapon_clearance
    if os.environ.get('HITHER_ORC_SAVE_BLEND'):
        bpy.ops.wm.save_as_mainfile(filepath=str(ROOT/f'tools/build/orc-authored-{variant}.blend'))
    validate_weapon_clearance(rig)
    if variant == 0:
        # Bake actual palm contact into the runtime trajectory. This avoids
        # separately guessed curves drifting away from the animated hand.
        import json
        rig.animation_data.action=next(t.strips[0].action for t in rig.animation_data.nla_tracks if t.name=='Push')
        contact=[]
        for frame in range(316):
            bpy.context.scene.frame_set(frame)
            from orc_animation import palm_surface, palm_frame
            p=palm_surface(rig)
            if frame>=113:
                normal=palm_frame(rig)[2]
                normal=rig.pose.bones['hand_l'].matrix.to_quaternion() @ rig.data.bones['hand_l'].matrix_local.to_quaternion().inverted() @ normal
                assert normal.z>.999, f'Carrying palm must face upward at frame {frame}: {normal}' 
            lift=min(1.,(frame/450)/.25);lift=lift*lift*(3-2*lift)
            angle=lift*math.pi/2
            # Grip the side first, then transfer the load to the underside.
            # The carrier steps inward as the rock clears his skull.
            contact.append([p.x*1.18+1.95*math.cos(angle),p.z*1.12+1.40*math.sin(angle),-p.y*1.12])
            center=Vector(contact[-1])
            head=rig.pose.bones['head'].matrix @ rig.data.bones['head'].matrix_local.inverted() @ Vector((0,-.025,1.4))
            head=Vector((head.x*1.18,head.z*1.12,-head.y*1.12))
            delta=head-center
            clearance=sum((delta[i]/r)**2 for i,r in enumerate((1.90,1.61,2.01)))
            assert clearance>1., f'Boulder intersects skull during lift frame {frame}: {clearance}'
            for name in ('upperarm_l','upperarm_r','lowerarm_l','lowerarm_r'):
                bone=rig.pose.bones[name]
                for u in (0.,.25,.5,.75):
                    point=bone.head.lerp(bone.tail,u)
                    point=Vector((point.x*1.18,point.z*1.12,-point.y*1.12))
                    delta=point-center
                    clearance=sum((delta[i]/r)**2 for i,r in enumerate((1.80,1.45,1.90)))
                    assert clearance>1., f'Boulder intersects {name} frame {frame}: {clearance}, center {center}, point {point}'
        velocity=(Vector(contact[-1])-Vector(contact[-2]))*60
        assert velocity.y>0., f'Throw must clear the carrier upward: {velocity}'
        for frame in range(316,451):
            bpy.context.scene.frame_set(frame)
            seconds=(frame-315)/60
            center=Vector(contact[-1])+velocity*seconds-Vector((0,4.905*seconds*seconds,0))
            head=rig.pose.bones['head'].matrix @ rig.data.bones['head'].matrix_local.inverted() @ Vector((0,-.025,1.4))
            head=Vector((head.x*1.18,head.z*1.12,-head.y*1.12))
            delta=head-center
            clearance=sum((delta[i]/r)**2 for i,r in enumerate((1.90,1.61,2.01)))
            assert clearance>1., f'Thrown boulder intersects skull frame {frame}: {clearance}'
            for name in ('upperarm_l','upperarm_r','lowerarm_l','lowerarm_r'):
                bone=rig.pose.bones[name]
                for u in (0.,.25,.5,.75,1.):
                    point=bone.head.lerp(bone.tail,u)
                    point=Vector((point.x*1.18,point.z*1.12,-point.y*1.12))
                    # The supporting wrist can touch the bearing face at
                    # release. Below its actual lowest plane is clear, even
                    # inside the padded ellipsoid used for the rest of the arm.
                    if name=='lowerarm_l' and u==1. and frame<=387 and point.y<center.y-1.40:
                        continue
                    delta=point-center
                    clearance=sum((delta[i]/r)**2 for i,r in enumerate((1.80,1.45,1.90)))
                    assert clearance>1., f'Thrown boulder intersects {name} frame {frame}: {clearance}'
        (OUT/'stone-contact.json').write_text(json.dumps(contact))
        rig.animation_data.action=None
    export_avatar(OUT/f'orc-{variant}.glb')
    if variant == 0:
        # glTF inverse bind matrices express the skin surface in the exported
        # joint's actual coordinate frame, including exporter axis conversion.
        import struct
        raw=(OUT/'orc-0.glb').read_bytes()
        size=struct.unpack_from('<I',raw,12)[0]
        doc=json.loads(raw[20:20+size]);binary=raw[28+size:]
        skin=doc['skins'][0]
        joint=next(i for i,n in enumerate(skin['joints']) if doc['nodes'][n]['name']=='hand_l')
        a=doc['accessors'][skin['inverseBindMatrices']];view=doc['bufferViews'][a['bufferView']]
        offset=view.get('byteOffset',0)+a.get('byteOffset',0)+joint*64
        inverse=np.frombuffer(binary,dtype='<f4',count=16,offset=offset).reshape(4,4).T
        p=rig.matrix_world @ palm_frame(rig)[0]
        socket=(inverse @ np.array((p.x,p.z,-p.y,1.)))[:3]
        (OUT/'stone-palm.json').write_text(json.dumps(socket.tolist()))
    print('BUILT',variant,flush=True)

for variant in map(int,os.environ.get('HITHER_ORC_VARIANTS','0,1,2').split(',')):build(variant)
sys.stdout.flush();os._exit(0)
