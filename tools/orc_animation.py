"""Baked NPC performance and lightweight deform-bone facial animation."""
import math
import bpy
from mathutils import Vector, Matrix, Quaternion
from avatar_animation import pose, rotate_bone, aim, smooth, plant_leg


def stone_jump(rig, t):
    """Jump under the load, solving feet after the root and before the palm.

    Phase is the shared 7.5-second Push clock; release remains at .70.
    Airborne root motion uses gravity in unscaled rig space (runtime Y scale
    is 1.12). Grounded ankle targets stay fixed through load and impact.
    """
    if t < .65:
        return
    load=smooth((t-.65)/.02)*(1-smooth((t-.67)/.015))
    flight=max(0.,min(1.,(t-.685)/.115))
    duration=.115*7.5
    height=.5*(9.81/1.12)*duration**2*flight*(1-flight)
    impact=smooth((t-.80)/.02)*(1-smooth((t-.82)/.06))
    drive=max(0.,min(1.,(t-.66)/.04))
    # Carry the shoulder laterally through release, then counterbalance over
    # the landing feet. The stone inherits the drive instead of falling back
    # into the rising head after the hand lets go.
    lean=.25*drive**2*(1-smooth((t-.70)/.10))
    root=rig.pose.bones['Root']
    root.location=root.bone.matrix_local.to_quaternion().inverted() @ Vector(
        (lean,0,-.008-.14*load+height-.12*impact))
    rotate_bone(rig,'pelvis',(.06*load+.04*impact,0,0))
    bpy.context.view_layer.update()
    tuck=math.sin(math.pi*flight)**2
    for side,sign in (('l',1),('r',-1)):
        ankle=rig.data.bones['foot_'+side].head_local.copy()
        ankle.x=sign*.123
        ankle.y+=(.055 if side=='l' else .035)*tuck
        ankle.z+=height+(.12 if side=='l' else .09)*tuck
        plant_leg(rig,side,ankle,-.20*tuck)


def arm(rig, side, offset, elbow_pole=None):
    upper=rig.pose.bones['upperarm_'+side];lower=rig.pose.bones['lowerarm_'+side]
    start=upper.head.copy();delta=Vector(offset)
    length=min(delta.length,upper.length+lower.length-.004)
    axis=delta.normalized();along=(upper.length**2-lower.length**2+length**2)/(2*length)
    pole=Vector(elbow_pole or (1 if side=='l' else -1,.3,0))
    pole=(pole-axis*pole.dot(axis)).normalized()
    elbow=start+axis*along+pole*math.sqrt(max(0,upper.length**2-along**2))
    aim(rig,'upperarm_'+side,start,elbow)
    aim(rig,'lowerarm_'+side,elbow,start+axis*length)


def weapon_wrist(rig, direction):
    """Control the haft through a target-facing strike plane."""
    bpy.context.view_layer.update()
    hand=rig.pose.bones['hand_r']
    rest=rig.data.bones['hand_r']
    shaft=(rig.data.bones['index_01_r'].head_local-rig.data.bones['pinky_01_r'].head_local).normalized()
    if shaft.z<0:shaft=-shaft
    current=hand.matrix.to_quaternion() @ rest.matrix_local.to_quaternion().inverted() @ shaft
    correction=current.rotation_difference(Vector(direction).normalized()).to_matrix().to_4x4()
    pivot=hand.head.copy()
    hand.matrix=Matrix.Translation(pivot) @ correction @ Matrix.Translation(-pivot) @ hand.matrix


def palm_frame(rig):
    """Palm center and outward normal in the hand's bind-space geometry."""
    bones=rig.data.bones
    wrist=bones['hand_l'].head_local
    fingers=(bones['middle_01_l'].head_local-wrist).normalized()
    across=bones['index_01_l'].head_local-bones['pinky_01_l'].head_local
    normal=across.cross(fingers).normalized()
    if normal.y>0:normal=-normal
    center=(wrist+bones['middle_01_l'].head_local)*.5+normal*.014
    return center,fingers,normal


def supporting_palm(rig, lift, recover):
    """Roll from the side grip to a flat palm beneath the carried stone."""
    bpy.context.view_layer.update()
    hand=rig.pose.bones['hand_l'];rest=rig.data.bones['hand_l']
    _,fingers,normal=palm_frame(rig)
    basis=Matrix((fingers.cross(normal),fingers,normal)).transposed()
    angle=lift*math.pi/2
    up=Vector((math.cos(angle),0,math.sin(angle)))
    forward=Vector((0,-1,0))
    target=Matrix((forward.cross(up),forward,up)).transposed()
    rotation=(target @ basis.transposed() @ rest.matrix_local.to_3x3()).to_quaternion()
    rotation=rotation.slerp(hand.matrix.to_quaternion(),recover)
    hand.matrix=Matrix.Translation(hand.head.copy()) @ rotation.to_matrix().to_4x4()
    for finger in ('index','middle','ring','pinky'):
        for segment in ('01','02','03'):
            bone=rig.pose.bones.get(f'{finger}_{segment}_l')
            if bone:bone.rotation_quaternion=Quaternion().slerp(bone.rotation_quaternion,recover)


def palm_surface(rig):
    center,_,_=palm_frame(rig)
    return rig.matrix_world @ rig.pose.bones['hand_l'].matrix @ rig.data.bones['hand_l'].matrix_local.inverted() @ center


def validate_weapon_clearance(rig):
    """Check the weapon mesh against head and both moving leg capsules."""
    import numpy as np
    points=[]
    for obj in bpy.context.scene.objects:
        if obj.type=='MESH' and len(obj.vertex_groups)==1 and obj.vertex_groups[0].name=='hand_r':
            points.extend([(*v.co,1.) for v in obj.data.vertices])
    points=np.asarray(points).T
    assert points.shape[1] > 0
    hand_rest=rig.data.bones['hand_r'].matrix_local.inverted()
    head_rest=rig.data.bones['head'].matrix_local
    for track in rig.animation_data.nla_tracks:track.mute=True
    for track in rig.animation_data.nla_tracks:
        if not (track.name.startswith('Attack') or track.name.startswith('Walk') or track.name.startswith('Idle') or track.name=='Push'):continue
        rig.animation_data.action=track.strips[0].action
        minimum=float('inf')
        for frame in range(int(rig.animation_data.action.frame_range[1])+1):
            bpy.context.scene.frame_set(frame)
            matrix=head_rest @ rig.pose.bones['head'].matrix.inverted() @ rig.pose.bones['hand_r'].matrix @ hand_rest
            p=(np.array(matrix) @ points)[:3].T
            distance=np.sum(((p-np.array((0.,-.025,1.40)))/np.array((.115,.14,.14)))**2,axis=1)
            minimum=min(minimum,float(distance.min()))
            world=(np.array(rig.pose.bones['hand_r'].matrix @ hand_rest) @ points)[:3].T
            for side in ('l','r'):
                for name,radius in [('thigh_',.072),('calf_',.052)]:
                    bone=rig.pose.bones.get(name+side)
                    if bone is None:raise AssertionError('Missing clearance bone '+name+side)
                    a=np.array(bone.head);b=np.array(bone.tail);d=b-a
                    u=np.clip(((world-a) @ d)/np.dot(d,d),0,1)
                    clearance=np.linalg.norm(world-(a+u[:,None]*d),axis=1).min()
                    assert clearance>radius, f'{track.name} frame {frame}: weapon intersects {name+side}: {clearance}'
        assert minimum>1., f'{track.name} intersects head envelope: {minimum}'
        print('WEAPON CLEARANCE',track.name,round(minimum,3),flush=True)
    rig.animation_data.action=None


def facial_rig(rig,body):
    # Preserve the head/neck's original smooth weights; facial controls borrow
    # only local influence rather than replacing the whole head attachment.
    controls={}
    for side,sign in [('l',1),('r',-1)]:
        controls['brow_'+side]=((sign*.034,-.12,1.414),(.020,.045,.012))
        controls['lid_'+side]=((sign*.034,-.133,1.400),(.019,.028,.0045))
        controls['lip_'+side]=((sign*.025,-.145,1.337),(.014,.025,.009))
        controls['cheek_'+side]=((sign*.047,-.12,1.365),(.023,.04,.017))
    controls['jaw']=((0,-.11,1.318),(.056,.055,.018))
    bpy.context.view_layer.objects.active=rig
    bpy.ops.object.mode_set(mode='EDIT')
    for name,(at,_) in controls.items():
        bone=rig.data.edit_bones.new('face_'+name)
        bone.head=at;bone.tail=Vector(at)+Vector((0,0,.015))
        bone.parent=rig.data.edit_bones['head'];bone.use_deform=True
    bpy.ops.object.mode_set(mode='OBJECT')
    groups={name:body.vertex_groups.new(name='face_'+name) for name in controls}
    for v in body.data.vertices:
        if v.co.z<1.29 or v.co.y>-.065:continue
        weights={}
        for name,(center,radius) in controls.items():
            d=(v.co-Vector(center));r=Vector(radius)
            weight=math.exp(-sum((d[i]/r[i])**2 for i in range(3)))
            if weight>.015:weights[name]=weight
        total=sum(weights.values())
        if not total:continue
        amount=min(.85,total*.72)
        old=[(g.group,g.weight) for g in v.groups]
        for index,weight in old:body.vertex_groups[index].add([v.index],weight*(1-amount),'REPLACE')
        for name,weight in weights.items():groups[name].add([v.index],weight/total*amount,'REPLACE')
    return controls


def animate_orc(rig,body,variant):
    controls=facial_rig(rig,body)
    performances={'Idle':3.6,'IdleWatch':4.8,'IdleWeary':4.2,'Walk':.96,
                  'WalkHeavy':1.08,'Attack':.8,'AttackBackhand':.9,
                  'AttackOverhead':1.0,'Alert':1.4}
    for name,seconds in performances.items():
        for track in list(rig.animation_data.nla_tracks):
            if track.name==name:rig.animation_data.nla_tracks.remove(track)
        action=bpy.data.actions.new(name);rig.animation_data.action=action
        frames=round(seconds*bpy.context.scene.render.fps)
        for f in range(frames+1):
            t=f/frames
            for b in rig.pose.bones:b.scale=(1,1,1)
            pose(rig,'Walk' if name.startswith('Walk') else 'Idle',t)
            breath=math.sin(t*math.tau)
            if name.startswith('Idle'):
                rotate_bone(rig,'spine_02',(.016*breath,0,.009*math.sin(t*math.tau)))
                rig.pose.bones['spine_02'].scale=(1+.010*breath,1+.018*breath,1+.005*breath)
                rotate_bone(rig,'head',(.012*breath,0,.07*math.sin(t*math.tau) if name=='IdleWatch' else .009*breath))
                if name=='IdleWeary':rotate_bone(rig,'neck_01',(.055+.02*breath,0,0))
            elif name.startswith('Walk'):
                rotate_bone(rig,'spine_02',(.025,0,-.055*math.cos(t*math.tau)))
                rotate_bone(rig,'head',(-.016,0,.023*math.cos(t*math.tau)))
            elif name.startswith('Attack'):
                # Planted feet, a high guard, shoulder-led acceleration and a
                # decelerating recovery. The elbow folds below the hand instead
                # of sticking sideways throughout a wrist-driven paddle swing.
                pose(rig,'Idle',0)
                # Brief load, fast contact, then continuous recovery. Avoid the
                # old 0.2-second frozen hit pose and leisurely full-body swing.
                wind=smooth(t/.24);strike=smooth((t-.24)/.16);recover=smooth((t-.44)/.56)
                load=wind*(1-strike);hit=strike*(1-recover)
                twist=-.16*load+.22*hit
                rotate_bone(rig,'pelvis',(.035*hit,0,twist*.4))
                rotate_bone(rig,'spine_02',(-.065*load+.12*hit,0,twist))
                rig.pose.bones['Root'].location.y -= .035*hit
                bpy.context.view_layer.update()
                diagonal = .045 if name=='AttackBackhand' else -.035
                offset=(-.045+diagonal*load+.05*hit,-.20-.18*hit,-.22+.37*load+.08*hit)
                if name=='AttackOverhead':offset=(offset[0],offset[1],offset[2]+.055*load)
                arm(rig,'r',offset,(-.35,.2,-1.))
                # Add a left-handed claw rake after contact, without changing
                # the weapon arm, torso, feet or original swing timing.
                rake=smooth((t-.54)/.16)
                reset=smooth((t-.72)/.28)
                reach=smooth((t-.40)/.14)*(1-rake)
                scratch=rake*(1-reset)
                claw=reach+scratch
                arm(rig,'l',(.06+.12*reach-.22*scratch,
                             -.23-.08*reach-.20*scratch,
                             -.20+.20*reach+.06*scratch),(.4,.2,-1.))
                rotate_bone(rig,'hand_l',(.25*claw,0,-.025-.35*claw))
                for finger,spread in [('index',-.10),('middle',-.03),('ring',.04),('pinky',.12)]:
                    for segment,bend in [('01',-.12),('02',.38),('03',.30)]:
                        bone=rig.pose.bones.get(f'{finger}_{segment}_l')
                        if bone:
                            bone.rotation_quaternion @= Quaternion((1,0,0),bend*claw)
                            if segment=='01':
                                bone.rotation_quaternion @= Quaternion((0,0,1),spread*claw)
                direction=(-.12+diagonal*load+.22*hit,-.55+.25*load-.38*hit,.82+.35*load-.94*hit)
                weapon_wrist(rig,direction)
                rotate_bone(rig,'head',(-.025+.02*hit,0,-twist*.35))
            elif name=='Alert':
                attention=math.sin(t*math.pi)**2
                rotate_bone(rig,'head',(-.09*attention,0,.04*attention))
                rotate_bone(rig,'spine_02',(-.035*attention,0,0))
            if not name.startswith('Attack'):
                bpy.context.view_layer.update()
                arm(rig,'r',(-.045,-.17,-.29))
                weapon_wrist(rig,(-.12,-.55,.82))
            for b in rig.pose.bones:
                b.keyframe_insert('rotation_quaternion',frame=f)
                b.keyframe_insert('location',frame=f)
                b.keyframe_insert('scale',frame=f)
        rig.animation_data.action=None
        track=rig.animation_data.nla_tracks.new();track.name=name
        track.strips.new(name,0,action);track.mute=True
    # Imported locomotion clips also need a deliberate weapon-carry pose.
    # Otherwise a backward/side step can carry the haft straight through a leg.
    for track in rig.animation_data.nla_tracks:
        if track.name not in ('WalkBack','StrafeLeft','StrafeRight','TurnLeft','TurnRight','Jump','Fall','Land'):
            continue
        action=track.strips[0].action;rig.animation_data.action=action
        for f in range(round(action.frame_range[1])+1):
            bpy.context.scene.frame_set(f)
            arm(rig,'r',(-.06,-.23,-.23))
            weapon_wrist(rig,(-.12,-.55,.82))
            for name in ('upperarm_r','lowerarm_r','hand_r'):
                rig.pose.bones[name].keyframe_insert('rotation_quaternion',frame=f)
        rig.animation_data.action=None
    # Facial expression channels are baked into every performance, including
    # existing jump, landing, turn and stone-pushing animations.
    for track in rig.animation_data.nla_tracks:
        action=track.strips[0].action;frames=max(1,round(action.frame_range[1]))
        name=track.name
        angry=name.startswith('Attack');strain=name in ('Push','Land','Jump')
        for f in range(frames+1):
            t=f/frames;seconds=f/bpy.context.scene.render.fps
            blink=math.exp(-((seconds-(.7+variant*.13))/.065)**2)
            effort=math.sin(t*math.pi)**2
            for control in controls:
                offset=Vector((0,0,0))
                sign=1 if control.endswith('_l') else -1
                if control.startswith('brow'):
                    offset.z=-.0025 if angry else -.0012 if strain else .0012*math.sin(t*math.tau+sign*.5)
                    if name=='Alert':offset.z+=.002*effort
                    if name=='IdleWatch':offset.z+=sign*.0015*math.sin(t*math.tau)
                elif control.startswith('lid'):offset.z=-.0065*blink-(.0012 if angry else 0)
                elif control.startswith('lip'):
                    offset.z=(.0035*effort if angry else .002*effort if strain else 0)*(1 if sign>0 else .55)
                elif control.startswith('cheek'):offset.y=-.0015*effort if angry or strain else 0
                elif control=='jaw':offset.z=-.002*effort if angry or strain else -.0006*math.sin(t*math.tau)
                bone=rig.pose.bones['face_'+control]
                local=bone.bone.matrix_local.to_quaternion().inverted() @ offset
                path=f'pose.bones["face_{control}"].location'
                for i in range(3):
                    curve=action.fcurves.find(path,index=i)
                    if curve is None:curve=action.fcurves.new(path,index=i)
                    curve.keyframe_points.insert(f,local[i],options={'REPLACE'})
    rig.animation_data.action=None
