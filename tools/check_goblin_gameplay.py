"""Validate all gameplay parts and LODs on the original animated skeleton."""
from check_goblin_lod import *

def check():
    source,src=load(ROOT/'assets/goblins/mireling.glb');lod,data=load(ROOT/'assets/goblins/mireling-gameplay.glb')
    assert [lod['nodes'][i]['name'] for i in lod['skins'][0]['joints']] == [source['nodes'][i]['name'] for i in source['skins'][0]['joints']]
    assert np.array_equal(values(lod,data,lod['skins'][0]['inverseBindMatrices']),values(source,src,source['skins'][0]['inverseBindMatrices']))
    # Verify that both sculpted surfaces actually reference their baked normal
    # maps, including the smaller textures used by the distant levels.
    for level in range(4):
        for name, sizes in [('base',[2048,1024,512,256]), ('base.001',[1024,512,256,128])]:
            primitive=next(m for m in lod['meshes'] if m['name']==f'lod{level}_{name}')['primitives'][0]
            normal=lod['materials'][primitive['material']]['normalTexture']
            assert normal.get('texCoord',0)==0
            image=lod['images'][lod['textures'][normal['index']]['source']]
            view=lod['bufferViews'][image['bufferView']];start=view.get('byteOffset',0)
            png=data[start:start+view['byteLength']]
            assert png[:8]==b'\x89PNG\r\n\x1a\n'
            assert struct.unpack('>II',png[16:24])==(sizes[level],sizes[level])
    samples=[]
    for clip in source['animations']:
        for fraction in [0,.25,.5,.70,.75,1]:
            matrices=pose(source,src,clip,fraction)
            for mesh in source['meshes']:
                high=mesh['primitives'][0];original=skin(source,src,high,matrices)
                selected=original[::max(1,len(original)//3000)]
                original_bvh=BVHTree.FromPolygons(original.tolist(),values(source,src,high['indices']).reshape(-1,3).tolist(),all_triangles=True)
                for level in range(4):
                    low=next(m for m in lod['meshes'] if m['name']==f"lod{level}_{mesh['name']}")['primitives'][0]
                    for index in low['attributes'].values():assert np.isfinite(values(lod,data,index)).all()
                    assert {'NORMAL','TANGENT','TEXCOORD_0','JOINTS_0','WEIGHTS_0'}<=low['attributes'].keys()
                    reduced=skin(lod,data,low,matrices);faces=values(lod,data,low['indices']).reshape(-1,3)
                    assert faces.max()<len(reduced)
                    bvh=BVHTree.FromPolygons(reduced.tolist(),faces.tolist(),all_triangles=True)
                    errors=[bvh.find_nearest(Vector(p))[3] for p in selected]
                    errors += [original_bvh.find_nearest(Vector(p))[3] for p in reduced[::max(1,len(reduced)//3000)]]
                    limit=[.008,.012,.025,.06][level]
                    assert max(errors)<limit,(mesh['name'],clip['name'],fraction,level,max(errors))
                    samples.append(dict(part=mesh['name'],clip=clip['name'],fraction=fraction,level=level,max_normalized_m=max(errors),p99_normalized_m=float(np.quantile(errors,.99))))
    (ROOT/'tools/build/goblin-gameplay/validation.json').write_text(json.dumps(dict(passed=True,poses=samples),indent=2)+'\n')
    print('PASS',len(samples),'part/LOD/pose combinations',flush=True)
if __name__=='__main__':
    check();sys.stdout.flush();os._exit(0)
