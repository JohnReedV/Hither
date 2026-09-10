"""Check shipped orc PBR maps, rig contract and finite skinned animation data.
Run with tools/.venv/bin/python tools/check_orcs.py.
"""
from pathlib import Path
import json, struct
import numpy as np

ROOT=Path(__file__).resolve().parents[1]
EXPECTED={'Idle','Walk','WalkBack','StrafeLeft','StrafeRight','Jump','Fall','Land',
          'TurnLeft','TurnRight','Push','IdleWatch','IdleWeary','WalkHeavy',
          'Attack','AttackBackhand','AttackOverhead','Alert'}

def check(path):
    raw=path.read_bytes(); length=struct.unpack_from('<I',raw,12)[0]
    doc=json.loads(raw[20:20+length]);binary=raw[28+length:]
    def accessor(index):
        a=doc['accessors'][index];view=doc['bufferViews'][a['bufferView']]
        types={5126:'<f4',5125:'<u4',5123:'<u2',5121:'u1'}
        count={'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4,'MAT4':16}[a['type']]
        dtype=np.dtype(types[a['componentType']]);offset=view.get('byteOffset',0)+a.get('byteOffset',0)
        return np.ndarray((a['count'],count),dtype=dtype,buffer=binary,offset=offset,
                          strides=(view.get('byteStride',count*dtype.itemsize),dtype.itemsize))
    assert {a['name'] for a in doc['animations']}==EXPECTED
    for s in doc['skins']:
        names={doc['nodes'][i]['name'] for i in s['joints']}
        assert {'head','pelvis','hand_l','hand_r','foot_l','foot_r'}<=names
        assert len(names)==62
    vertices=0
    for mesh in doc['meshes']:
        for p in mesh['primitives']:
            at=p['attributes'];assert {'POSITION','NORMAL','TEXCOORD_0','JOINTS_0','WEIGHTS_0'}<=at.keys(),mesh['name']
            positions=accessor(at['POSITION']);vertices+=len(positions)
            assert np.isfinite(positions).all()
            weights=accessor(at['WEIGHTS_0'])
            if weights.dtype.kind!='f':weights=weights/np.iinfo(weights.dtype).max
            assert np.allclose(weights.sum(axis=1),1,atol=.002),mesh['name']
            assert accessor(at['JOINTS_0']).max()<62
    for animation in doc['animations']:
        for sampler in animation['samplers']:
            times=accessor(sampler['input']).ravel();assert np.all(np.diff(times)>0)
            assert np.isfinite(accessor(sampler['output'])).all()
    materials=[]
    for mat in doc['materials']:
        if 'olive hide' in mat['name'] or any(t in mat['name'] for t in ('Hammered iron','canvas','oxblood cloth','oxhide','harness leather')):
            p=mat['pbrMetallicRoughness']
            assert 'baseColorTexture' in p and 'metallicRoughnessTexture' in p and 'normalTexture' in mat,mat['name']
            materials.append(mat['name'])
    assert len(materials)>=4
    assert all('bufferView' in i for i in doc['images'])
    return {'file':path.name,'bytes':len(raw),'vertices':vertices,'clips':len(doc['animations']),'pbr_materials':materials}

if __name__=='__main__':
    report=[check(ROOT/f'assets/orcs/orc-{i}.glb') for i in range(3)]
    out=ROOT/'tools/build/orc-validation.json';out.parent.mkdir(exist_ok=True)
    out.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report,indent=2));print('PASS: all variants, embedded PBR maps, 62 joints, 18 clips and normalized skin weights')
