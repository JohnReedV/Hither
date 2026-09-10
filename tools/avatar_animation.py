"""In-place, rig-space locomotion for the bundled avatar (Blender Z-up, -Y forward).

Foot targets travel at a constant speed during support; analytical two-bone IK
keeps the sole planted while the pelvis transfers weight. No runtime constraints
or extra bones are required: all motion is baked into the v1 GLB clips.
"""
import math
import json
from functools import lru_cache
from pathlib import Path
import bpy
from hand_pose import finger_angles
from mathutils import Matrix, Quaternion, Vector

CLIP_SECONDS = {"Idle": 3.0, "Walk": .8, "WalkBack": .8,
                "StrafeLeft": .8, "StrafeRight": .8,
                "Jump": .30, "Fall": .40, "Land": .30,
                "TurnLeft": .72, "TurnRight": .72, "Punch": .60}


def smooth(t):
    t = max(0., min(1., t))
    return t*t*(3-2*t)


def rotate_bone(rig, name, angles):
    bone = rig.pose.bones.get(name)
    if bone is None:
        return
    bind = bone.bone.matrix_local.to_quaternion()
    q = (Quaternion((1, 0, 0), angles[0]) @ Quaternion((0, 1, 0), angles[1])
         @ Quaternion((0, 0, 1), angles[2]))
    bone.rotation_mode = "QUATERNION"
    bone.rotation_quaternion = bind.inverted() @ q @ bind


def aim(rig, name, start, end):
    bone = rig.pose.bones[name]
    rest = bone.bone
    q = (rest.tail_local-rest.head_local).rotation_difference(end-start)
    bone.matrix = Matrix.LocRotScale(start, q @ rest.matrix_local.to_quaternion(), Vector((1, 1, 1)))
    bpy.context.view_layer.update()


def plant_leg(rig, side, ankle, roll=0., yaw=0.):
    """Solve a forward-facing knee with a stable pole, then orient the sole."""
    thigh, calf = (rig.pose.bones[n+side] for n in ("thigh_", "calf_"))
    hip = thigh.head.copy()
    offset = ankle-hip
    length = min(offset.length, thigh.length+calf.length-.0005)
    axis = offset.normalized()
    along = (thigh.length**2-calf.length**2+length**2)/(2*length)
    height = math.sqrt(max(0., thigh.length**2-along**2))
    pole = Vector((.08 if side == "l" else -.08, -1, 0))
    pole = (pole-axis*pole.dot(axis)).normalized()
    knee = hip+axis*along+pole*height
    target = hip+axis*length
    aim(rig, "thigh_"+side, hip, knee)
    aim(rig, "calf_"+side, knee, target)
    foot = rig.pose.bones["foot_"+side]
    foot.matrix = Matrix.LocRotScale(target,
        Quaternion((0, 0, 1), yaw) @ Quaternion((1, 0, 0), roll)
        @ foot.bone.matrix_local.to_quaternion(), Vector((1, 1, 1)))
    bpy.context.view_layer.update()


def swing_arm(rig, side, swing, lift):
    """Relaxed hanging arms with elbows behind wrists, not the mesh's A-pose."""
    sign = 1 if side == "l" else -1
    upper = rig.pose.bones["upperarm_"+side]
    lower = rig.pose.bones["lowerarm_"+side]
    shoulder = upper.head.copy()
    wrist = shoulder+Vector((sign*.045, swing-.025, -.385+lift))
    delta = wrist-shoulder
    length = min(delta.length, upper.length+lower.length-.002)
    axis = delta.normalized()
    along = (upper.length**2-lower.length**2+length**2)/(2*length)
    pole = Vector((sign*.35, 1, 0))
    pole = (pole-axis*pole.dot(axis)).normalized()
    elbow = shoulder+axis*along+pole*math.sqrt(max(0., upper.length**2-along**2))
    aim(rig, "upperarm_"+side, shoulder, elbow)
    aim(rig, "lowerarm_"+side, elbow, shoulder+axis*length)


def smoother(t):
    """Zero velocity and acceleration at both ends of each authored phase."""
    t = max(0., min(1., t))
    return max(0., min(1., t*t*t*(t*(t*6-15)+10)))


@lru_cache(maxsize=1)
def punch_capture():
    return json.loads((Path(__file__).resolve().parents[1]/
        "assets/avatars/source/punch-reference.json").read_text())


def punch_reference(seconds):
    capture = punch_capture()
    samples = capture["samples"]
    frame = max(0., min(len(samples)-1., seconds*capture["sample_rate"]))
    i = min(int(frame), len(samples)-2)
    u = frame-i
    return {name: Vector(samples[i][name]).slerp(Vector(samples[i+1][name]), u)
            for name in samples[i]}


def punch_arm_directions(recorded, guard, upper_length, lower_length, shoulder_shift_x, extension):
    """Aim the rear straight toward the face centerline with elbow clearance."""
    captured_elbow = recorded["upper"]*upper_length
    target = captured_elbow+recorded["lower"]*lower_length
    guard_wrist = guard["upper"]*upper_length+guard["lower"]*lower_length
    # A rear cross converges from the rear-hand guard onto the target in
    # front of the face. Holding it beside the shoulder makes the final arm
    # point outward after the chest turns, despite a constant world-space X.
    target.x = guard_wrist.x-shoulder_shift_x+.145*extension
    # Finish at full reach with a soft elbow. The reference actor's slower
    # shadowboxing extension is otherwise too short for this sharper strike.
    reach = .987*(upper_length+lower_length)
    full = Vector((target.x, -math.sqrt(max(0., reach**2-target.x**2-.025**2)), .025))
    target = target.lerp(full, extension)
    distance = target.length
    axis = target.normalized()
    along = (upper_length**2-lower_length**2+distance**2)/(2*distance)
    # The elbow has its own lateral arc. Do not aim the entire arm sideways
    # to obtain that arc: project the captured elbow onto the new strike axis.
    pole = (captured_elbow-axis*captured_elbow.dot(axis)).normalized()
    outward = Vector((-1, 0, -.25))
    outward = (outward-axis*outward.dot(axis)).normalized()
    pole = pole.slerp(outward, .75*smooth((recorded["upper"].z+.9)/.65))
    elbow = axis*along+pole*math.sqrt(max(0., upper_length**2-along**2))
    return elbow.normalized(), (target-elbow).normalized()


def punch_pose(rig, t):
    # A compact load flows directly into the rear straight. The capture supplies
    # the elbow plane; shoulder drive, reach and timing are adapted for the game.
    source_time = (t-.10)*(.15/.12) if t < .22 else .15*(1-smooth((t-.22)/.15))
    recorded = punch_reference(source_time)
    guard = punch_reference(0.)
    raised = smoother(t/.10)*(1-smoother((t-.37)/.23))
    extension = smoother((t-.13)/.09)*(1-smoother((t-.22)/.15))
    upper = rig.pose.bones["upperarm_r"]
    lower = rig.pose.bones["lowerarm_r"]
    hand = rig.pose.bones["hand_r"]
    wrist = rig.data.bones["hand_r"].head_local
    bind_forward = (rig.data.bones["middle_01_r"].head_local-wrist).normalized()
    bind_across = rig.data.bones["pinky_01_r"].head_local-rig.data.bones["index_01_r"].head_local
    bind_across = (bind_across-bind_forward*bind_across.dot(bind_forward)).normalized()
    bind_normal = bind_across.cross(bind_forward).normalized()
    rest_normal = (hand.matrix.to_quaternion()
        @ hand.bone.matrix_local.to_quaternion().inverted()) @ bind_normal
    rest_upper = (upper.tail-upper.head).normalized()
    rest_lower = (lower.tail-lower.head).normalized()
    upper_rotation = upper.matrix.to_quaternion()
    lower_rotation = lower.matrix.to_quaternion()
    rest_shoulder = upper.head.copy()

    # Drive the chest through the punch. The free arm retains its locomotion
    # pose and is only carried by this turn; it gets no guard or counter-swing.
    load = smoother(t/.075)*(1-smoother((t-.075)/.10))
    drive = smoother((t-.075)/.13)*(1-smoother((t-.22)/.24))
    torso_yaw = -.14*load+.48*drive
    for name, share in (("spine_01", .22), ("spine_02", .36), ("spine_03", .42)):
        rotate_bone(rig, name, (.045*drive*share, 0, torso_yaw*share))
    rotate_bone(rig, "neck_01", (.07*drive, 0, -.8*torso_yaw))
    bpy.context.view_layer.update()

    clavicle = rig.pose.bones["clavicle_r"]
    delta = guard["clavicle"].rotation_difference(recorded["clavicle"])
    delta = Quaternion().slerp(delta, raised)
    clavicle.matrix = Matrix.LocRotScale(clavicle.head.copy(),
        delta @ clavicle.matrix.to_quaternion(), Vector((1, 1, 1)))
    bpy.context.view_layer.update()
    shoulder = upper.head.copy()
    strike_upper, strike_lower = punch_arm_directions(
        recorded, guard, upper.length, lower.length, shoulder.x-rest_shoulder.x, extension)
    wrist_correction = recorded["lower"].rotation_difference(strike_lower)
    upper_direction = rest_upper.slerp(strike_upper, raised)
    lower_direction = rest_lower.slerp(strike_lower, raised)
    elbow = shoulder+upper_direction*upper.length
    rest_hinge = rest_upper.cross(rest_lower).normalized()
    hinge = upper_direction.cross(lower_direction).normalized()
    def limb_frame(axis, bend_axis):
        return Matrix((bend_axis, axis, bend_axis.cross(axis))).transposed()
    delta = (limb_frame(upper_direction, hinge)
             @ limb_frame(rest_upper, rest_hinge).transposed()).to_quaternion()
    upper.matrix = Matrix.LocRotScale(shoulder, delta @ upper_rotation, Vector((1, 1, 1)))
    bpy.context.view_layer.update()
    # The radial wrist markers also record pronation of the forearm. Transfer
    # it to the forearm itself, rather than twisting only the hand at the wrist.
    rest_normal = (rest_normal-rest_lower*rest_normal.dot(rest_lower)).normalized()
    target_normal = rest_normal.slerp(wrist_correction @ recorded["lower_normal"], raised)
    target_across = lower_direction.cross(target_normal).normalized()
    rest_across = rest_lower.cross(rest_normal).normalized()
    delta = (limb_frame(lower_direction, target_across)
             @ limb_frame(rest_lower, rest_across).transposed()).to_quaternion()
    lower.matrix = Matrix.LocRotScale(elbow, delta @ lower_rotation, Vector((1, 1, 1)))
    bpy.context.view_layer.update()

    # Retain the capture's wrist orientation and forearm turn. Fist geometry
    # still uses the same anatomical closed-hand pose as the first-person mesh.
    # Stack the metacarpals behind the forearm at impact instead of leaving
    # the captured wrist cocked as if tapping with the top of the knuckles.
    forward = (wrist_correction @ recorded["hand_forward"]).slerp(strike_lower, extension)
    recorded_normal = wrist_correction @ recorded["hand_normal"]
    knuckles_up = (Vector((0, 0, 1))-forward*forward.z).normalized()
    palm_normal = recorded_normal.slerp(knuckles_up, extension)
    across = forward.cross(palm_normal).normalized()
    normal = across.cross(forward).normalized()
    rotation = (Matrix((across, forward, normal)).transposed()
                @ Matrix((bind_across, bind_forward, bind_normal))).to_quaternion()
    relaxed = hand.rotation_quaternion.copy()
    hand.matrix = Matrix.LocRotScale(hand.head.copy(),
        rotation @ hand.bone.matrix_local.to_quaternion(), Vector((1, 1, 1)))
    hand.rotation_quaternion = relaxed.slerp(hand.rotation_quaternion, raised)
    clench = smoother(t/.075)*(1-smoother((t-.43)/.17))
    for name, angle in finger_angles(True).items():
        bone = rig.pose.bones[name]
        bone.rotation_quaternion = bone.rotation_quaternion.slerp(
            Quaternion((1, 0, 0), angle), clench)
    bpy.context.view_layer.update()


def step(phase):
    """Forward/back displacement, toe clearance, and foot roll for one leg."""
    p = phase % 1
    if p < .5:
        t = p*2
        # Heel settles after contact; heel lifts just before toe-off.
        roll = .12*(1-smooth(t/.18))-.18*smooth((t-.8)/.2)
        lift = .022*smooth((t-.8)/.2)
        return -.32+.64*t, lift, roll
    t = (p-.5)*2
    # Hermite return matches the support phase's velocity at either end.
    travel = .32 + .64*t - 3.84*t*t + 2.56*t*t*t
    lift = .022*(1-smooth(t))+.085*math.sin(math.pi*t)**2
    return travel, lift, -.18*(1-smooth(t))+.12*smooth(t)


def pose(rig, kind, phase):
    for bone in rig.pose.bones:
        bone.rotation_mode = "QUATERNION"
        bone.rotation_quaternion = Quaternion()
        bone.location = (0, 0, 0)
    moving = kind in ("Walk", "WalkBack", "StrafeLeft", "StrafeRight")
    turning = kind in ("TurnLeft", "TurnRight")
    turn_sign = 1 if kind == "TurnLeft" else -1
    t = max(0., min(1., phase))
    # Bake arm overlays against a neutral torso; locomotion supplies breathing
    # at runtime, so the overlay must not counter-animate a second breath cycle.
    base_phase = 0. if kind == "Punch" else phase
    cycle = math.cos(base_phase*math.tau)
    breath = math.sin(base_phase*math.tau)
    compression = 0.
    if kind == "Land":
        # Absorb impact promptly, then recover without a bounce or locked knees.
        compression = smooth(t/.24) if t < .24 else 1-smooth((t-.24)/.76)
    root_z = -.070-.022*math.cos(phase*math.tau*2) if moving else -.008
    root_z -= .075*compression
    root_x = .013*math.sin(phase*math.tau) if moving else 0.
    if turning:
        root_x = -.018*math.sin(phase*math.tau)
        root_z = -.018-.009*math.sin(phase*math.tau)**2
    root = rig.pose.bones["Root"]
    root.location = root.bone.matrix_local.to_quaternion().inverted() @ Vector((root_x, 0, root_z))
    rotate_bone(rig, "pelvis", (0, .018*breath if moving else 0, .032*cycle if moving else 0))
    rotate_bone(rig, "spine_01", (-.025 if moving else -.004*breath, 0, -.025*cycle if moving else 0))
    rotate_bone(rig, "spine_02", (-.055*compression, 0, -.035*cycle if moving else 0))
    rotate_bone(rig, "spine_03", (-.012 if moving else .007*breath, 0, -.015*cycle if moving else 0))
    rotate_bone(rig, "neck_01", (.012 if moving else 0, 0, .022*cycle if moving else 0))
    rotate_bone(rig, "head", (.008*compression, -.008*breath if moving else 0, .018*cycle if moving else 0))
    if turning:
        # Shoulders/head anticipate the step rather than rotating as one block.
        rotate_bone(rig, "spine_02", (0, 0, turn_sign*.055))
        rotate_bone(rig, "head", (0, 0, turn_sign*.045))
    arms = []
    for side, sign in (("l", 1), ("r", -1)):
        lag = math.cos(phase*math.tau-.22)*sign
        arm = .23*lag if moving else .007*breath
        if turning:
            arm = .045*lag*turn_sign
        if kind == "WalkBack":
            arm *= -.7
        if kind.startswith("Strafe"):
            arm *= .5
        if kind == "Jump":
            arm = -.10-.34*smooth(t/.65)
        elif kind == "Fall":
            arm = -.44+.26*smooth(t)
        elif kind == "Land":
            arm = -.18*(1-smooth(t))+.10*compression
        rotate_bone(rig, "clavicle_"+side, (0, sign*.012*abs(arm), -.014*lag if moving else 0))
        # Upper arms rest alongside the ribs; elbows trail the shoulder rather
        # than holding a rigid right angle. Fingers stay loosely curled.
        rotate_bone(rig, "hand_"+side, (.02*lag if moving else 0, 0, -sign*.025))
        arms.append((side, arm*.50, .012+max(0., -arm)*.10))
        for finger in ("index", "middle", "ring", "pinky"):
            for segment in ("02", "03"):
                bone = rig.pose.bones.get(f"{finger}_{segment}_{side}")
                if bone:
                    bone.rotation_quaternion = Quaternion((1, 0, 0), .38 if segment == "02" else .24)
    bpy.context.view_layer.update()
    for side, swing, lift in arms:
        swing_arm(rig, side, swing, lift)
    for side, sign in (("l", 1), ("r", -1)):
        ankle = rig.data.bones["foot_"+side].head_local.copy()
        ankle.x = sign*.123  # Natural hip-width stance, not the bind-pose spread.
        roll = 0.
        yaw = 0.
        if moving:
            travel, lift, roll = step(phase+(0 if side == "l" else .5))
            if kind.startswith("Strafe"):
                ankle.x += travel*(1 if kind == "StrafeLeft" else -1)*.48
                # Keep the trailing foot from crossing the supporting leg.
                ankle.x = sign*max(.055, sign*ankle.x)
                roll *= .25
            else:
                ankle.y += travel*(-1 if kind == "WalkBack" else 1)
                if kind == "WalkBack":
                    roll *= -.55
            ankle.z += lift
        elif kind == "Jump":
            gather = smooth(t)
            ankle.y += (.065 if side == "l" else .035)*gather
            ankle.z += (.11 if side == "l" else .08)*gather
            roll = -.20*gather
        elif turning:
            p = (phase+(0 if side == "l" else .5)) % 1
            if p < .5:
                lift = math.sin(p*math.tau)**2
                ankle.z += .055*lift
                ankle.y -= sign*turn_sign*.025*lift
            # Rotate a lifted foot toward the turn; unwind during support.
            yaw = turn_sign*.14*math.sin(p*math.tau-math.pi/2)
        elif kind == "Fall":
            extend = smooth(t)
            ankle.y += (.065 if side == "l" else .035)*(1-extend)
            ankle.z += (.11 if side == "l" else .08)*(1-extend)
            roll = -.20*(1-extend)
        plant_leg(rig, side, ankle, roll, yaw)

    if kind == "Punch":
        punch_pose(rig, t*CLIP_SECONDS["Punch"])


def bake_clips(rig):
    rig.animation_data_create()
    rig.animation_data.action = None
    for track in list(rig.animation_data.nla_tracks):
        rig.animation_data.nla_tracks.remove(track)
    bpy.context.scene.render.fps = 60
    for name, seconds in CLIP_SECONDS.items():
        old = bpy.data.actions.get(name)
        if old and old.users == 0:
            bpy.data.actions.remove(old)
        action = bpy.data.actions.new(name)
        rig.animation_data.action = action
        frames = round(seconds*60)
        for frame in range(frames+1):
            pose(rig, name, frame/frames)
            for bone in rig.pose.bones:
                bone.keyframe_insert("rotation_quaternion", frame=frame)
                if bone.name == "Root":
                    bone.keyframe_insert("location", frame=frame)
        for curve in action.fcurves:
            for key in curve.keyframe_points:
                key.interpolation = "LINEAR"
        track = rig.animation_data.nla_tracks.new()
        track.name = name
        track.strips.new(name, 0, action)
        track.mute = True
    rig.animation_data.action = None
    pose(rig, "Idle", 0)
