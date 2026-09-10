"""Whole-character LODs sharing the master bind pose, with baked sculpt normals.
Run with tools/.venv/bin/python tools/build_goblin_gameplay.py.
The master and its animation tracks are never rewritten.
"""
from pathlib import Path
import bpy, json, struct, os, sys
from build_goblin_lod import read_glb, palette
ROOT=Path(__file__).resolve().parents[1]
BUILD=ROOT/'tools/build/goblin-gameplay';BUILD.mkdir(parents=True,exist_ok=True)
SOURCE=ROOT/'assets/goblins/mireling.glb'
original, original_binary=read_glb(SOURCE)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(SOURCE))
source_objects=[o for o in bpy.data.objects if o.type=='MESH' and o.data.name in {m['name'] for m in original['meshes']}]
for o in list(bpy.data.objects):
    if o.type=='MESH' and o not in source_objects:bpy.data.objects.remove(o,do_unlink=True)
source_by_name={o.data.name:o for o in source_objects}
scene=bpy.context.scene
scene.render.engine='CYCLES';scene.cycles.samples=1
scene.render.bake.use_selected_to_active=True
scene.render.bake.cage_extrusion=.025
scene.render.bake.max_ray_distance=.05
scene.render.bake.margin=12
report=[]
for level, ratio in enumerate([.15,.06,.015,.004]):
    targets=[]
    for high in source_objects:
        low=high.copy();low.data=high.data.copy();bpy.context.collection.objects.link(low)
        low.name=f'lod{level}_{high.data.name}';low.data.name=low.name
        bpy.context.view_layer.objects.active=low
        mod=low.modifiers.new('Gameplay topology','DECIMATE')
        # Preserve small eye and tooth silhouettes; simplify every sizeable part.
        count=sum(len(p.vertices)-2 for p in low.data.polygons)
        part_ratio=ratio if high.data.name=="base" else ratio*2
        mod.ratio=min(1.,max(part_ratio, min(1.,256/max(1,count))))
        bpy.ops.object.modifier_move_to_index(modifier=mod.name,index=0)
        bpy.ops.object.modifier_apply(modifier=mod.name)
        if high.data.name in ('base','base.001'):
            # Selected-to-active normal baking captures the high sculpt and its
            # existing pore normal shader in the derivative's tangent basis.
            mat=low.data.materials[0].copy();low.data.materials[0]=mat
            size=([2048,1024,512,256] if high.data.name=="base" else [1024,512,256,128])[level]
            image=bpy.data.images.new(f'Mireling {high.data.name} sculpt normal LOD {level}',width=size,height=size)
            image.colorspace_settings.name='Non-Color'
            target=mat.node_tree.nodes.new('ShaderNodeTexImage');target.image=image
            mat.node_tree.nodes.active=target
            bpy.ops.object.select_all(action='DESELECT');high.select_set(True);low.select_set(True)
            bpy.context.view_layer.objects.active=low
            bpy.ops.object.bake(type='NORMAL')
            normal=next((n for n in mat.node_tree.nodes if n.type=='NORMAL_MAP'),None)
            if normal is None:
                normal=mat.node_tree.nodes.new('ShaderNodeNormalMap')
                shader=next(n for n in mat.node_tree.nodes if n.type=='BSDF_PRINCIPLED')
                mat.node_tree.links.new(normal.outputs['Normal'],shader.inputs['Normal'])
            mat.node_tree.links.new(target.outputs['Color'],normal.inputs['Color'])
            image.pack()
        targets.append(low)
        report.append(dict(level=level,source=high.data.name,mesh=low.data.name,triangles=sum(len(p.vertices)-2 for p in low.data.polygons)))
    # All levels remain in the export; runtime loads only their mesh subassets.
for high in source_objects:bpy.data.objects.remove(high,do_unlink=True)
temporary=BUILD/'export.glb'
bpy.ops.export_scene.gltf(filepath=str(temporary),export_format='GLB',export_animations=False,export_skins=True,export_tangents=True,export_yup=True)
doc,binary=read_glb(temporary)
# Mesh subassets are attached to the live master rig, never this export's rig.
for index,mesh in enumerate(doc['meshes']):
    name=mesh['name'].split('_',1)[1]
    source_index=next(i for i,m in enumerate(original['meshes']) if m['name']==name)
    remap=[palette(original,source_index).index(n) for n in palette(doc,index)]
    for primitive in mesh['primitives']:
        a=doc['accessors'][primitive['attributes']['JOINTS_0']];v=doc['bufferViews'][a['bufferView']]
        fmt={5121:'B',5123:'H'}[a['componentType']];stride=v.get('byteStride',4*struct.calcsize(fmt));start=v.get('byteOffset',0)+a.get('byteOffset',0)
        for vertex in range(a['count']):
            offset=start+vertex*stride;j=struct.unpack_from('<4'+fmt,binary,offset)
            struct.pack_into('<4'+fmt,binary,offset,*(remap[k] for k in j))
        a.pop('min',None);a.pop('max',None)
# Use the exact master bind matrices and node transforms, not a re-exported rig.
doc['nodes']=[{k:v for k,v in n.items() if k not in ('mesh','skin')} for n in original['nodes']]
doc['scenes']=original['scenes'];doc['scene']=original.get('scene',0)
for i,m in enumerate(doc['meshes']):
    source_index=next(i for i,s in enumerate(original['meshes']) if s['name']==m['name'].split('_',1)[1])
    source_node=next(n for n in original['nodes'] if n.get('mesh')==source_index)
    node={k:v for k,v in source_node.items() if k not in ('children','mesh','skin')};node.update(mesh=i,skin=0)
    doc['scenes'][doc['scene']]['nodes'].append(len(doc['nodes']));doc['nodes'].append(node)
a=dict(original['accessors'][original['skins'][0]['inverseBindMatrices']]);v=dict(original['bufferViews'][a['bufferView']])
while len(binary)%4:binary.append(0)
start=v.get('byteOffset',0);offset=len(binary);binary.extend(original_binary[start:start+v['byteLength']]);v.update(buffer=0,byteOffset=offset)
a['bufferView']=len(doc['bufferViews']);doc['bufferViews'].append(v)
doc['skins']=[dict(original['skins'][0],inverseBindMatrices=len(doc['accessors']))];doc['accessors'].append(a)
while len(binary)%4:binary.append(0)
doc['buffers'][0]['byteLength']=len(binary)
encoded=json.dumps(doc,separators=(',',':')).encode();encoded+=b' '*(-len(encoded)%4)
output=ROOT/'assets/goblins/mireling-gameplay.glb'
output.write_bytes(struct.pack('<III',0x46546c67,2,28+len(encoded)+len(binary))+struct.pack('<II',len(encoded),0x4e4f534a)+encoded+struct.pack('<II',len(binary),0x004e4942)+binary)
(BUILD/'build.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report),flush=True);sys.stdout.flush();os._exit(0)
