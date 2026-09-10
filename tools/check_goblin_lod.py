"""Validate the derivative on the original skeleton in all four animation clips.
No rendering or asset rewriting. Run with the project's Blender Python venv.
"""
from pathlib import Path
import json, struct, math, os, sys
import bpy  # Initializes Blender's bundled mathutils module.
import numpy as np
from mathutils import Vector, Quaternion, Matrix
from mathutils.bvhtree import BVHTree
ROOT=Path(__file__).resolve().parents[1]

def load(path):
    raw=path.read_bytes();n=struct.unpack_from('<I',raw,12)[0]
    return json.loads(raw[20:20+n]),raw[28+n:]

def values(doc,data,index):
    a=doc['accessors'][index];v=doc['bufferViews'][a['bufferView']]
    dtype=np.dtype({5121:'u1',5123:'<u2',5125:'<u4',5126:'<f4'}[a['componentType']])
    width={'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4,'MAT4':16}[a['type']]
    return np.ndarray((a['count'],width),dtype=dtype,buffer=data,
                      offset=v.get('byteOffset',0)+a.get('byteOffset',0),
                      strides=(v.get('byteStride',width*dtype.itemsize),dtype.itemsize)).copy()

def skin(doc,data,primitive,matrices):
    attr=primitive['attributes'];p=values(doc,data,attr['POSITION'])
    j=values(doc,data,attr['JOINTS_0']);w=values(doc,data,attr['WEIGHTS_0'])
    assert np.isfinite(p).all() and np.isfinite(w).all()
    assert (j<len(matrices)).all() and (w>=0).all()
    assert np.max(np.abs(w.sum(axis=1)-1))<1e-4
    points=np.column_stack((p,np.ones(len(p))))
    out=np.zeros((len(p),3))
    for k in range(4):
        out+=np.einsum('nij,nj->ni',matrices[j[:,k]],points)[:,:3]*w[:,k,None]
    return out

def pose(doc,data,animation,fraction):
    nodes=doc['nodes'];trs=[{k:list(v) for k,v in n.items() if k in ('translation','rotation','scale')} for n in nodes]
    for channel in animation['channels']:
        sampler=animation['samplers'][channel['sampler']]
        assert sampler.get('interpolation','LINEAR') in ('LINEAR','STEP')
        times=values(doc,data,sampler['input']).ravel();frames=values(doc,data,sampler['output'])
        time=times[0]+fraction*(times[-1]-times[0])
        hi=min(np.searchsorted(times,time,side='right'),len(times)-1);lo=max(0,hi-1)
        amount=0 if times[hi]==times[lo] else (time-times[lo])/(times[hi]-times[lo])
        if sampler.get('interpolation')=='STEP': amount=0; lo=min(np.searchsorted(times,time,side='right')-1,len(times)-1)
        path=channel['target']['path'];a,b=frames[lo],frames[hi]
        if path=='rotation':
            qa=Quaternion((a[3],*a[:3]));qb=Quaternion((b[3],*b[:3]));q=qa.slerp(qb,float(amount))
            v=[q.x,q.y,q.z,q.w]
        else:v=(a*(1-amount)+b*amount).tolist()
        trs[channel['target']['node']][path]=v
    parents={c:i for i,n in enumerate(nodes) for c in n.get('children',[])};cache={}
    def world(i):
        if i in cache:return cache[i]
        t=trs[i];q=t.get('rotation',[0,0,0,1])
        local=Matrix.LocRotScale(Vector(t.get('translation',[0,0,0])),Quaternion((q[3],*q[:3])),Vector(t.get('scale',[1,1,1])))
        cache[i]=world(parents[i])@local if i in parents else local
        return cache[i]
    bind=values(doc,data,doc['skins'][0]['inverseBindMatrices']).reshape(-1,4,4).transpose(0,2,1)
    return np.array([np.array(world(node))@inverse for node,inverse in zip(doc['skins'][0]['joints'],bind)])

def main():
    source,src=load(ROOT/'assets/goblins/mireling.glb');lod,data=load(ROOT/'assets/goblins/mireling-distance.glb')
    assert [lod['nodes'][i]['name'] for i in lod['skins'][0]['joints']] == [source['nodes'][i]['name'] for i in source['skins'][0]['joints']]
    assert np.array_equal(values(lod,data,lod['skins'][0]['inverseBindMatrices']),values(source,src,source['skins'][0]['inverseBindMatrices']))
    body=next(m for m in source['meshes'] if m['name']=='base')['primitives'][0]
    low=lod['meshes'][0]['primitives'][0]
    assert not any(k in lod for k in ['images','textures','animations'])
    assert {'POSITION','NORMAL','TANGENT','TEXCOORD_0','JOINTS_0','WEIGHTS_0'}<=low['attributes'].keys()
    for index in low['attributes'].values():assert np.isfinite(values(lod,data,index)).all()
    faces=values(lod,data,low['indices']).reshape(-1,3)
    assert len(faces)<source['accessors'][body['indices']]['count']/3*.2
    samples=[]
    for clip in source['animations']:
        for fraction in [0,.25,.5,.70,.75,1]:
            matrices=pose(source,src,clip,fraction)
            original=skin(source,src,body,matrices);reduced=skin(lod,data,low,matrices)
            assert faces.max()<len(reduced)
            bvh=BVHTree.FromPolygons(reduced.tolist(),faces.tolist(),all_triangles=True)
            selected=original[::max(1,len(original)//10000)]
            errors=np.array([bvh.find_nearest(Vector(p))[3] for p in selected])
            # 8mm normalized is 3.45mm at runtime, about one 4K pixel at
            # the near edge of the LOD band. Most vertices are far closer.
            assert errors.max()<.008,(clip['name'],fraction,errors.max())
            samples.append({'clip':clip['name'],'fraction':fraction,'samples':len(errors),
                            'max_normalized_m':float(errors.max()),'p99_normalized_m':float(np.quantile(errors,.99))})
    report={'passed':True,'poses':samples,'distance_body_triangles':len(faces)}
    (ROOT/'tools/build/goblin-lod/validation.json').write_text(json.dumps(report,indent=2)+'\n')
    print('PASS:',len(samples),'animated poses; max surface error',max(s['max_normalized_m'] for s in samples),flush=True)

if __name__=='__main__':
    main();sys.stdout.flush();os._exit(0)
