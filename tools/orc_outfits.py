"""Distinct, rigged scavenged equipment; all surface textures export to glTF."""
import math
import bpy
import numpy as np


def worn_material(name, color, seed):
    from orc_surfaces import surface
    kind='cloth' if any(word in name.lower() for word in ('cloth','canvas','flax')) else 'leather'
    return surface(name,color,seed,kind)


def dress(variant, rig, armor, pants, iron, ivory, bind, sphere, rod):
    before = set(bpy.data.objects)
    hide = worn_material('Bruiser scarred oxhide', (.22,.12,.055), 91)
    cloth = worn_material('Raider faded moss canvas', (.105,.145,.085), 92)
    red = worn_material('Ironcap stained oxblood cloth', (.19,.045,.026), 93)
    strap = worn_material('Oiled cracked harness leather', (.075,.045,.023), 94)
    thread = worn_material('Dirty flax stitching', (.38,.29,.16), 95)
    base = [hide, iron, cloth][variant]
    armor.data.materials.clear(); armor.data.materials.append(base)
    armor.name = ['Bruiser hide jerkin', 'Ironcap forged breastplate', 'Raider canvas vest'][variant]
    pants.data.materials.clear(); pants.data.materials.append([strap,red,cloth][variant])

    def panel(name, points, bone, mat):
        mesh=bpy.data.meshes.new(name); mesh.from_pydata(points, [], [tuple(range(len(points)))]); mesh.update()
        if all(p[1] < -.08 for p in points):
            # Curve sewn overlays around the chest, including their interiors;
            # planar straps would otherwise disappear into the convex jerkin.
            import bmesh
            bm=bmesh.new(); bm.from_mesh(mesh)
            bmesh.ops.triangulate(bm,faces=list(bm.faces))
            bmesh.ops.subdivide_edges(bm,edges=list(bm.edges),cuts=4,use_grid_fill=True)
            rings=[(.82,.16,.10),(.86,.158,.105),(.93,.15,.12),(1.02,.18,.135),(1.10,.20,.137),(1.17,.21,.123),(1.22,.15,.10)]
            for v in bm.verts:
                x,y,z=v.co
                if .82 <= z <= 1.22:
                    for (za,xa,ya),(zb,xb,yb) in zip(rings,rings[1:]):
                        if za<=z<=zb:
                            t=(z-za)/(zb-za); rx=xa+(xb-xa)*t; ry=ya+(yb-ya)*t
                            front=-.02-ry*math.sqrt(max(0,1-(x/rx)**2))
                            v.co.y=min(y,front-.012)
                            break
            bm.to_mesh(mesh);bm.free()
        uv=mesh.uv_layers.new(name='UVMap')
        for loop in mesh.loops:
            v=mesh.vertices[loop.vertex_index].co; uv.data[loop.index].uv=(v.x*5+.5,v.z*5)
        o=bpy.data.objects.new(name,mesh); bpy.context.collection.objects.link(o)
        # Actual thickness, including edges and the back when viewed from below.
        bpy.context.view_layer.objects.active=o
        mod=o.modifiers.new('Forged or sewn thickness','SOLIDIFY'); mod.thickness=.004
        bpy.ops.object.modifier_apply(modifier=mod.name)
        return bind(o,rig,bone,mat)

    def band(name,z,rx,ry,width,mat,bone='spine_02'):
        for i in range(24):
            a=i*math.tau/24; b=(i+1)*math.tau/24
            panel(name,[(rx*math.cos(a),ry*math.sin(a)-.02,z-width/2),
                        (rx*math.cos(b),ry*math.sin(b)-.02,z-width/2),
                        (rx*math.cos(b),ry*math.sin(b)-.02,z+width/2),
                        (rx*math.cos(a),ry*math.sin(a)-.02,z+width/2)],bone,mat)

    band('Wide scavenged waist belt',.855,.167,.115,.045,strap)
    for a,b in [((-.025,-.141,.835),(.025,-.141,.835)),((-.025,-.141,.875),(.025,-.141,.875)),
                ((-.025,-.141,.835),(-.025,-.141,.875)),((.025,-.141,.835),(.025,-.141,.875))]:
        rod('Bent iron buckle',a,b,.004,rig,'spine_02',iron)
    rod('Buckle tongue',(0,-.145,.835),(0,-.145,.875),.0025,rig,'spine_02',iron)
    # A sewn pouch and flap rather than another naked ellipsoid.
    panel('Belt pouch',[(.10,-.115,.84),(.16,-.09,.84),(.17,-.11,.75),(.105,-.135,.75)],'pelvis',strap)
    panel('Pouch folded flap',[(.10,-.12,.845),(.16,-.095,.845),(.155,-.12,.81),(.115,-.14,.80)],'pelvis',hide)
    sphere('Pouch toggle',(.137,-.137,.808),(.009,.006,.004),rig,'pelvis',ivory)

    if variant == 0:
        # Asymmetric scavenged ribs over a hide jerkin, with visible repairs.
        for j in range(4):
            z=.945+j*.052
            panel('Bruiser salvaged rib plate',[(-.155,-.125,z),(.065,-.16,z+.018),(.072,-.16,z+.049),(-.15,-.13,z+.033)],'spine_02',iron)
            for x,y in [(-.135,-.137),(.05,-.167)]:
                sphere('Rib plate rivet',(x,y,z+.03),(.004,.003,.004),rig,'spine_02',iron)
        panel('Large stitched jerkin repair',[(.075,-.155,.99),(.145,-.132,.98),(.16,-.124,.91),(.073,-.14,.905)],'spine_02',strap)
        for j in range(6):
            z=.913+j*.011
            rod('Jerkin repair stitch',(.076,-.157,z),(.086,-.157,z+.006),.0015,rig,'spine_02',thread)
        for i in range(7):
            x=-.135+i*.043
            panel('Bruiser uneven hide skirt',[(x,-.126,.83),(x+.04,-.126,.83),(x+.035,-.13,.69+(i%3)*.014),(x,-.13,.705)],'pelvis',hide)
        for side in [-1,1]:
            label='l' if side>0 else 'r'
            for j in range(3):
                rod('Pauldron overlapping ridge',(side*(.12+j*.022),-.09,1.205),(side*(.14+j*.022),.055,1.19),.009,rig,'upperarm_'+label,iron)
    elif variant == 1:
        # Heavy articulated front plates with a battered heraldic cloth strip.
        for j in range(3):
            band('Ironcap overlapping waist lame',.89+j*.043,.169+j*.002,.127+j*.004,.035,iron)
        panel('Ironcap torn oxblood tabard',[(-.057,-.17,1.17),(.057,-.17,1.17),(.06,-.15,.84),(.045,-.15,.82),(-.055,-.15,.85)],'spine_02',red)
        panel('Ironcap split tabard tail',[(-.06,-.15,.835),(.06,-.15,.835),(.07,-.15,.63),(.014,-.15,.655),(0,-.15,.71),(-.018,-.15,.64),(-.065,-.15,.66)],'pelvis',red)
        for x in [-.025,0,.025]:
            rod('Crude stitched clan tally',(x,-.176,1.04),(x+.006,-.176,1.12),.003,rig,'spine_02',thread)
        for side in [-1,1]:
            label='l' if side>0 else 'r'
            panel('Ironcap knee guard',[(side*.05,-.084,.48),(side*.115,-.075,.48),(side*.12,-.085,.42),(side*.08,-.11,.40),(side*.047,-.094,.43)],'calf_'+label,iron)
            rod('Helmet brow reinforcement',(side*.008,-.113,1.452),(side*.074,-.075,1.452),.006,rig,'head',iron)
    else:
        # Lightweight raider, one shoulder covered in layered weathered hide.
        for j in range(8):
            x=-.08+j*.039
            panel('Raider ragged shoulder mantle',[(x,-.04,1.235),(x+.04,-.04,1.235),(x+.045,.065,1.17),(x+.03,.09,1.09+(j%3)*.025),(x,.085,1.12)],'spine_02',hide)
        panel('Raider diagonal bandolier',[(-.13,-.142,1.19),(-.095,-.153,1.20),(.142,-.143,.895),(.108,-.15,.88)],'spine_02',strap)
        for j in range(5):
            x=-.095+j*.047;z=1.17-j*.06
            rod('Bandolier iron clasp',(x,-.163,z),(x+.026,-.163,z+.019),.003,rig,'spine_02',iron)
        for j in range(4):
            x=-.12+j*.028;z=.79-(j%2)*.025
            rod('Trophy cord',(x,-.145,.84),(x,-.15,z),.0018,rig,'pelvis',thread)
            rod('Raider tooth trophy',(x,-.15,z),(x+.009,-.15,z-.035),.006,rig,'pelvis',ivory,tip=.0008)
        for j in range(3):
            panel('Raider stitched vest patch',[(.05,-.155,1.04+j*.031),(.11,-.145,1.04+j*.031),(.106,-.147,1.063+j*.031),(.048,-.156,1.06+j*.031)],'spine_02',hide)
    # Keep the embellishments cheap: one skinned mesh per material, not one
    # draw per rivet, stitch or belt segment. Join preserves bone groups.
    batches = {}
    for obj in set(bpy.data.objects)-before:
        if obj.type == 'MESH':
            for group in obj.vertex_groups:
                assert group.name in rig.data.bones, group.name
            batches.setdefault(obj.data.materials[0].name, []).append(obj)
    for name, objects in batches.items():
        bpy.ops.object.select_all(action='DESELECT')
        for obj in objects: obj.select_set(True)
        bpy.context.view_layer.objects.active = objects[0]
        if len(objects) > 1:
            bpy.ops.object.join()
        objects[0].name = ['Bruiser','Ironcap','Raider'][variant]+' equipment / '+name
