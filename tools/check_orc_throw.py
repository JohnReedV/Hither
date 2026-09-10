"""Check exported jump kinematics and the runtime palm socket, not source poses.
Run with tools/.venv/bin/python tools/check_orc_throw.py.
"""
import json
import struct
from pathlib import Path
import numpy as np
import bpy  # Initializes mathutils in the standalone Blender Python wheel.
from mathutils import Matrix, Quaternion, Vector

ROOT=Path(__file__).resolve().parents[1]


def load_pose(path):
    raw=path.read_bytes();size=struct.unpack_from('<I',raw,12)[0]
    doc=json.loads(raw[20:20+size]);binary=raw[28+size:]
    def accessor(index):
        a=doc['accessors'][index];v=doc['bufferViews'][a['bufferView']]
        width={'SCALAR':1,'VEC3':3,'VEC4':4}[a['type']]
        return np.frombuffer(binary,dtype='<f4',count=a['count']*width,
            offset=v.get('byteOffset',0)+a.get('byteOffset',0)).reshape(-1,width)
    clip=next(a for a in doc['animations'] if a['name']=='Push')
    curves=[]
    for channel in clip['channels']:
        sampler=clip['samplers'][channel['sampler']]
        interpolation=sampler.get('interpolation','LINEAR')
        assert interpolation in ('LINEAR','STEP')
        curves.append((channel['target'],accessor(sampler['input']).ravel(),accessor(sampler['output']),interpolation))
    parents={child:i for i,n in enumerate(doc['nodes']) for child in n.get('children',[])}
    names={n['name']:i for i,n in enumerate(doc['nodes']) if 'name' in n}
    def evaluate(seconds):
        nodes=[dict(n) for n in doc['nodes']]
        for target,times,values,interpolation in curves:
            if len(times)==1:
                nodes[target['node']][target['path']]=values[0]
                continue
            i=max(0,min(len(times)-2,int(np.searchsorted(times,seconds,side='right'))-1))
            u=float(np.clip((seconds-times[i])/(times[i+1]-times[i]),0,1))
            if interpolation=='STEP':u=1. if seconds>=times[i+1] else 0.
            a,b=values[i],values[i+1]
            if target['path']=='rotation':
                q=Quaternion((a[3],*a[:3])).slerp(Quaternion((b[3],*b[:3])),u)
                value=(q.x,q.y,q.z,q.w)
            else:value=a*(1-u)+b*u
            nodes[target['node']][target['path']]=value
        matrices={}
        def world(i):
            if i not in matrices:
                n=nodes[i];q=n.get('rotation',(0,0,0,1))
                m=Matrix.LocRotScale(Vector(n.get('translation',(0,0,0))),
                    Quaternion((q[3],*q[:3])),Vector(n.get('scale',(1,1,1))))
                if 'matrix' in n:m=Matrix(np.array(n['matrix']).reshape(4,4).T.tolist())
                matrices[i]=world(parents[i]) @ m if i in parents else m
            return matrices[i]
        return {name:world(names[name]) for name in ('Root','hand_l','foot_l','foot_r','thigh_l','calf_l')}
    return evaluate


def check():
    contact=np.array(json.loads((ROOT/'assets/orcs/stone-contact.json').read_text()))
    socket=Vector(json.loads((ROOT/'assets/orcs/stone-palm.json').read_text()))
    scale=np.array((1.18,1.12,1.12))
    maximum=0.
    for variant in range(3):
        pose=load_pose(ROOT/f'assets/orcs/orc-{variant}.glb')
        rest=pose(7.5)
        for frame in range(293,397):
            p=pose(frame/60)
            phase=frame/450
            # Both ankles stay planted during crouch, extension and landing.
            if phase<=.685 or phase>=.80:
                for name in ('foot_l','foot_r'):
                    assert (p[name].translation-rest[name].translation).length<.001,(variant,frame,name,'foot sliding')
        for frame in range(113,316):
            p=pose(frame/60)
            palm=np.array(p['hand_l'] @ socket)*scale
            gap=np.linalg.norm(palm+np.array((0,1.40,0))-contact[frame])
            maximum=max(maximum,gap)
            assert gap<.0001,(variant,frame,'palm socket mismatch',gap)
        apex=pose((.685+.115/2)*7.5)
        for name in ('foot_l','foot_r'):
            assert apex[name].translation.y-rest[name].translation.y>.45,(variant,name,'no airborne tuck')
        crouch=pose(.67*7.5)
        landing=pose(.82*7.5)
        assert crouch['Root'].translation.y<rest['Root'].translation.y-.13
        assert landing['Root'].translation.y<rest['Root'].translation.y-.11
        # Check the rise decelerates and the descent accelerates, with no hover.
        heights=[pose(phase*7.5)['Root'].translation.y for phase in (.70,.72,.74,.76,.78)]
        assert np.all(np.diff(heights,n=2)<-.15),(variant,'nonballistic jump',heights)
        print(f'PASS orc-{variant}: planted launch/landing, crouch, tuck, ballistic flight, palm contact')
    print(f'Maximum exported palm/baked trajectory discrepancy: {maximum*1000:.4f} mm')


if __name__=='__main__':check()
