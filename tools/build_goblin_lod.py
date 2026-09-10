"""Build a distance-only skin mesh; keep the shipped close-up asset untouched.
Run with tools/.venv/bin/python tools/build_goblin_lod.py after build_goblin.py.
The derivative uses the source joint palette and is swapped onto its live skin.
"""
from pathlib import Path
import hashlib, json, struct, sys, os
import bpy

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'assets/goblins/mireling.glb'
OUTPUT = ROOT / 'assets/goblins/mireling-distance.glb'
BUILD = ROOT / 'tools/build/goblin-lod'
BUILD.mkdir(parents=True, exist_ok=True)

def read_glb(path):
    data = path.read_bytes()
    n = struct.unpack_from('<I', data, 12)[0]
    doc = json.loads(data[20:20+n])
    size = struct.unpack_from('<I', data, 20+n)[0]
    return doc, bytearray(data[28+n:28+n+size])

def palette(doc, mesh):
    node = next(n for n in doc['nodes'] if n.get('mesh') == mesh)
    return [doc['nodes'][i]['name'] for i in doc['skins'][node['skin']]['joints']]

def build():
    original, original_binary = read_glb(SOURCE)
    source_index = next(i for i,m in enumerate(original['meshes']) if m['name'] == 'base')
    source_palette = palette(original, source_index)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(SOURCE))
    body = max((o for o in bpy.data.objects if o.type == 'MESH'), key=lambda o: len(o.data.vertices))
    bpy.context.view_layer.objects.active = body
    # Applying before the armature keeps bind-space positions and interpolated
    # skin weights; animation and the high-detail master are not modified.
    mod = body.modifiers.new('Distance topology', 'DECIMATE')
    mod.ratio = 0.15
    bpy.ops.object.modifier_move_to_index(modifier=mod.name, index=0)
    bpy.ops.object.modifier_apply(modifier=mod.name)
    temporary = BUILD / 'export.glb'
    bpy.ops.export_scene.gltf(filepath=str(temporary), export_format='GLB',
                             export_animations=False, export_skins=True,
                             export_tangents=True, export_yup=True)
    doc, binary = read_glb(temporary)
    index = next(i for i,m in enumerate(doc['meshes']) if m['name'] == body.data.name)
    mesh = doc['meshes'][index]
    exported_palette = palette(doc, index)
    remap = [source_palette.index(name) for name in exported_palette]
    # Exporters may reorder bones. Rewrite JOINTS_0 to the original runtime
    # palette instead of relying on exporter order or a second animation rig.
    for primitive in mesh['primitives']:
        assert 'TANGENT' in primitive['attributes']
        accessor = doc['accessors'][primitive['attributes']['JOINTS_0']]
        view = doc['bufferViews'][accessor['bufferView']]
        fmt = {5121:'B', 5123:'H'}[accessor['componentType']]
        size = struct.calcsize(fmt)
        stride = view.get('byteStride', 4*size)
        start = view.get('byteOffset',0) + accessor.get('byteOffset',0)
        for vertex in range(accessor['count']):
            offset = start + vertex*stride
            joints = struct.unpack_from('<4'+fmt, binary, offset)
            struct.pack_into('<4'+fmt, binary, offset, *(remap[j] for j in joints))
        accessor.pop('min',None); accessor.pop('max',None)
        primitive.pop('material',None)
    # Keep the derivative mesh and bind reference, without textures or animations.
    # A skinned node is required for Bevy to retain joint attributes and generate
    # animated bounds. Runtime loads the mesh subasset; it never spawns this rig.
    out = {'asset':{'version':'2.0','generator':'Hither goblin distance mesh'},
           'meshes':[mesh], 'nodes':[{'mesh':0}], 'scenes':[{'nodes':[0]}], 'scene':0,
           'accessors':[], 'bufferViews':[], 'buffers':[{'byteLength':0}]}
    packed = bytearray(); copied = {}
    def copy_accessor(old):
        if old in copied: return copied[old]
        a = dict(doc['accessors'][old]); assert 'sparse' not in a
        v = dict(doc['bufferViews'][a['bufferView']])
        while len(packed)%4: packed.append(0)
        offset = len(packed); start = v.get('byteOffset',0)
        packed.extend(binary[start:start+v['byteLength']])
        v.update(buffer=0,byteOffset=offset)
        a['bufferView'] = len(out['bufferViews']); out['bufferViews'].append(v)
        copied[old] = len(out['accessors']); out['accessors'].append(a)
        return copied[old]
    for p in mesh['primitives']:
        p['indices'] = copy_accessor(p['indices'])
        p['attributes'] = {k:copy_accessor(v) for k,v in p['attributes'].items()}
    out['nodes'] = [{k:v for k,v in n.items() if k not in ('mesh','skin')} for n in original['nodes']]
    source_node = next(i for i,n in enumerate(original['nodes']) if n.get('mesh') == source_index)
    out['nodes'][source_node].update(mesh=0,skin=0)
    out['scenes'] = original['scenes']
    out['scene'] = original.get('scene',0)
    bind = dict(original['accessors'][original['skins'][0]['inverseBindMatrices']])
    view = dict(original['bufferViews'][bind['bufferView']])
    while len(packed)%4: packed.append(0)
    offset = len(packed); start = view.get('byteOffset',0)
    packed.extend(original_binary[start:start+view['byteLength']])
    view.update(buffer=0,byteOffset=offset)
    bind['bufferView'] = len(out['bufferViews']); out['bufferViews'].append(view)
    out['skins'] = [dict(original['skins'][0],inverseBindMatrices=len(out['accessors']))]
    out['accessors'].append(bind)
    while len(packed)%4: packed.append(0)
    out['buffers'][0]['byteLength'] = len(packed)
    encoded = json.dumps(out,separators=(',',':')).encode()
    encoded += b' ' * (-len(encoded)%4)
    OUTPUT.write_bytes(struct.pack('<III',0x46546c67,2,28+len(encoded)+len(packed))+
                       struct.pack('<II',len(encoded),0x4e4f534a)+encoded+
                       struct.pack('<II',len(packed),0x004e4942)+packed)
    triangles = lambda d,m: sum(d['accessors'][p['indices']]['count']//3 for p in m['primitives'])
    report = {'source_sha256':hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
              'source_body_triangles':triangles(original,original['meshes'][source_index]),
              'distance_body_triangles':triangles(out,mesh), 'joint_palette':source_palette,
              'output_bytes':OUTPUT.stat().st_size}
    (BUILD/'build.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report),flush=True)

if __name__ == '__main__':
    build()
    sys.stdout.flush()
    os._exit(0) # Blender's embedded-Python shutdown can crash after successful export.
