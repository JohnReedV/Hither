"""Portable, baked surface detail and fitted equipment for the three orcs."""
import math
import bpy
import numpy as np


def noise_field(rng, size, cells):
    grid = rng.random((cells+1, cells+1)).astype(np.float32)
    t = np.linspace(0, cells, size, endpoint=False)
    i = t.astype(int); f = t-i; f = f*f*(3-2*f)
    return ((grid[i[:, None], i]*(1-f)+grid[i[:, None], i+1]*f)*(1-f[:, None])
            +(grid[i[:, None]+1, i]*(1-f)+grid[i[:, None]+1, i+1]*f)*f[:, None])


def maps(mat, color, roughness, height, metallic=None, strength=1.):
    nodes=mat.node_tree.nodes; links=mat.node_tree.links
    p=nodes.get('Principled BSDF'); size=color.shape[0]
    def image(label, values, noncolor=False):
        im=bpy.data.images.new(mat.name+' / '+label, width=size, height=size)
        if noncolor: im.colorspace_settings.name='Non-Color'
        rgba=np.ones((size,size,4), dtype=np.float32)
        rgba[:,:,:3]=values if values.ndim==3 else values[:,:,None]
        im.pixels.foreach_set(rgba.ravel()); im.pack()
        node=nodes.new('ShaderNodeTexImage'); node.image=im
        return node.outputs['Color']
    links.new(image('albedo',np.clip(color,0,1)), p.inputs['Base Color'])
    links.new(image('roughness',roughness,True),p.inputs['Roughness'])
    if metallic is not None: links.new(image('metalness',metallic,True),p.inputs['Metallic'])
    dy,dx=np.gradient(height)
    normal=np.stack((-dx,-dy,np.ones_like(dx)),axis=-1)
    normal/=np.linalg.norm(normal,axis=-1)[:,:,None]
    n=nodes.new('ShaderNodeNormalMap'); n.inputs['Strength'].default_value=strength
    links.new(image('normal',normal*.5+.5,True),n.inputs['Color'])
    links.new(n.outputs['Normal'],p.inputs['Normal'])


def surface(name,color,seed,kind='leather'):
    m=bpy.data.materials.new(name);m.use_nodes=True
    m.node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value=.28
    size=512; rng=np.random.default_rng(seed)
    broad=noise_field(rng,size,9); grain=noise_field(rng,size,110)
    fine=rng.random((size,size)); y,x=np.mgrid[:size,:size]
    if kind=='iron':
        oxide=np.clip((broad-.47)*3.7,0,.85)
        pits=np.clip((grain-.64)*5,0,1)
        scratches=(np.sin(x*.63+y*.025+np.sin(y*.07)*.7)>.993)*(grain>.5)
        rgb=np.array([.19,.205,.205])[None,None,:]*(.65+.55*grain[:,:,None])
        rgb=rgb*(1-oxide[:,:,None])+np.array([.20,.078,.029])*oxide[:,:,None]
        rgb+=scratches[:,:,None]*.11
        rough=np.clip(.46+.35*oxide+.12*pits-.16*scratches,.3,.92)
        maps(m,rgb,rough,.24*grain+.065*fine-.38*pits,.86-.70*oxide)
    else:
        cloth=kind=='cloth'
        # Fine woven fibers or irregular leather fissures, independent of albedo.
        fissure=np.exp(-((grain-.43)/.027)**2)
        weave=(np.sin(x*math.pi/2)*np.sin(y*math.pi/3))
        rgb=np.array(color)[None,None,:]*(.72+.43*broad+.20*grain)[:,:,None]
        rgb*= (1-(.055 if cloth else .19)*fissure)[:,:,None]
        rgb+=((fine-.5)*.018)[:,:,None]
        height=(.075*weave+.09*grain if cloth else .26*grain-.18*fissure)+.035*fine
        maps(m,rgb,np.clip((.79 if cloth else .65)+.15*broad+.08*fissure,.55,.95),height)
    return m


def atlas(mesh,size):
    """Rasterize rest-space landmarks into the existing anatomical skin UVs."""
    pos=np.zeros((size,size,3),dtype=np.float32)
    mesh.calc_loop_triangles(); uv=mesh.uv_layers.active.data
    for tri in mesh.loop_triangles:
        tex=np.array([uv[i].uv[:] for i in tri.loops])*(size-1)
        points=np.array([mesh.vertices[i].co[:] for i in tri.vertices])
        lo=np.maximum(np.floor(tex.min(axis=0)).astype(int),0)
        hi=np.minimum(np.ceil(tex.max(axis=0)).astype(int),size-1)
        if np.any(hi<lo):continue
        yy,xx=np.mgrid[lo[1]:hi[1]+1,lo[0]:hi[0]+1]; a,b,c=tex
        den=(b[1]-c[1])*(a[0]-c[0])+(c[0]-b[0])*(a[1]-c[1])
        if abs(den)<1e-8:continue
        u=((b[1]-c[1])*(xx-c[0])+(c[0]-b[0])*(yy-c[1]))/den
        v=((c[1]-a[1])*(xx-c[0])+(a[0]-c[0])*(yy-c[1]))/den
        inside=(u>=-.01)&(v>=-.01)&(u+v<=1.01)
        tile=pos[lo[1]:hi[1]+1,lo[0]:hi[0]+1]
        values=u[:,:,None]*points[0]+v[:,:,None]*points[1]+(1-u-v)[:,:,None]*points[2]
        tile[inside]=values[inside]
    return pos


def skin(body,variant,source_path):
    source=bpy.data.images.load(str(source_path));source.scale(2048,2048)
    size=2048; rgba=np.array(source.pixels[:],dtype=np.float32).reshape(size,size,4)
    x,y,z=np.moveaxis(atlas(body.data,size),-1,0)
    rng=np.random.default_rng(8103+variant*97)
    broad=noise_field(rng,size,35);mottle=noise_field(rng,size,145)
    grain=noise_field(rng,size,550);fine=rng.random((size,size))
    front=np.clip((-y-.045)/.065,0,1)
    face=np.exp(-((z-1.36)/.060)**2)*front
    nose=np.exp(-(x/.025)**4-((z-1.377)/.024)**4)*front
    ears=np.clip((abs(x)-.066)/.025,0,1)*np.exp(-((z-1.40)/.055)**2)
    lips=np.exp(-(x/.039)**6-((z-1.329)/.010)**2)*front
    sockets=np.exp(-((abs(x)-.033)/.022)**4-((z-1.397)/.013)**4)*front
    warmth=np.maximum.reduce([face*.45,nose*.85,ears*.7,lips*.9])
    base=np.array([(.32,.365,.185),(.37,.355,.225),(.255,.345,.275)][variant])
    rgb=base[None,None,:]*(1-warmth[:,:,None])+np.array([.48,.29,.20])*warmth[:,:,None]
    rgb*=np.clip(rgba[:,:,:3].mean(axis=-1)/.48,.55,1.3)[:,:,None]
    spots=np.clip((grain-.67)*5,0,1)*np.clip((mottle-.49)*4,0,1)
    rgb*=(1-.19*(broad-.3)-.23*spots)[:,:,None]
    rgb=rgb*(1-sockets[:,:,None]*.40)+np.array([.085,.065,.048])*sockets[:,:,None]*.40
    rgb=rgb*(1-lips[:,:,None]*.35)+np.array([.28,.135,.095])*lips[:,:,None]*.35
    scar=np.exp(-((x-.038-(z-1.36)*.30)/.0024)**2-((z-1.367)/.025)**4)*front
    rgb=rgb*(1-scar[:,:,None]*.45)+np.array([.47,.34,.235])*scar[:,:,None]*.45
    mat=bpy.data.materials.new('Orc %d / olive hide, warm cartilage, healed scars'%variant);mat.use_nodes=True
    mat.node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value=.28
    maps(mat,rgb,np.clip(.64+.16*broad-.10*lips,.48,.84),.10*mottle+.18*grain+.04*fine-.14*spots)
    return mat


def sculpt(body,variant):
    bpy.context.view_layer.objects.active=body
    sub=body.modifiers.new('Anatomical sculpt resolution','SUBSURF');sub.levels=2
    bpy.ops.object.modifier_move_to_index(modifier=sub.name,index=0)
    bpy.ops.object.modifier_apply(modifier=sub.name)
    for v in body.data.vertices:
        x,y,z=v.co
        front=max(0,min(1,(-y-.060)/.06))
        if z<1.29:continue
        forehead=math.exp(-((z-1.442)/.026)**4-(x/.061)**6)*front
        # Three interrupted, slightly asymmetric furrows rather than corrugated skin.
        for k,level in enumerate((1.428,1.443,1.456)):
            line=level+.002*math.sin(x*43+k+variant*.4)
            fade=math.exp(-((x-(.006 if k==1 else -.004))/.049)**6)
            v.co.y+=.00065*forehead*fade*math.exp(-((z-line)/.0014)**2)
        fold=math.exp(-((abs(x)-(.025+(1.36-z)*.40))/.0028)**2-((z-1.344)/.024)**4)*front
        v.co.y+=.0022*fold
        frown=math.exp(-((abs(x)-.009)/.0026)**2-((z-1.419)/.016)**4)*front
        v.co.y+=.0017*frown
        cheek=math.exp(-((abs(x)-.052)/.018)**2-((z-1.355)/.017)**2)*front
        v.co.y+=.0025*cheek


def tailor(armor,variant):
    """Sewn folds and hammered dents break the perfectly extruded torso."""
    bpy.context.view_layer.objects.active=armor
    sub=armor.modifiers.new('Equipment surface resolution','SUBSURF');sub.subdivision_type='SIMPLE';sub.levels=2
    bpy.ops.object.modifier_move_to_index(modifier=sub.name,index=0);bpy.ops.object.modifier_apply(modifier=sub.name)
    for v in armor.data.vertices:
        x,y,z=v.co; a=math.atan2(y+.02,x)
        if variant==1:
            amount=.0018*math.sin(a*13+z*51)*math.sin(z*74-a*9)
        else:
            waist=math.exp(-((z-.89)/.072)**2)
            amount=(.0025*math.sin(a*17+z*13)+.004*waist*math.sin(z*150+a*7))
        v.co.x+=math.cos(a)*amount;v.co.y+=math.sin(a)*amount
        if z>1.205:
            v.co.z-=.028*max(0.,-math.sin(a))**5
            v.co.z+=.0015*math.sin(a*9+.6)
    solid=armor.modifiers.new('Visible garment rim','SOLIDIFY');solid.thickness=.0035
    bpy.ops.object.modifier_move_to_index(modifier=solid.name,index=0);bpy.ops.object.modifier_apply(modifier=solid.name)


def forged_shell(name,at,size,rig,bone,mat,bind,helmet=False):
    """Open, dented plate with a rolled lip, not a solid oval shoulder pad."""
    segments=40;rings=10;verts=[];faces=[]
    for j in range(rings+1):
        t=.025+(1.50 if helmet else 1.82)*j/rings
        for i in range(segments):
            a=i*math.tau/segments
            dent=1+.022*math.sin(a*7+t*9)+.012*math.cos(a*13-t*5)
            verts.append((at[0]+size[0]*math.sin(t)*math.cos(a)*dent,
                          at[1]+size[1]*math.sin(t)*math.sin(a)*dent,
                          at[2]+size[2]*math.cos(t)+.0015*math.sin(a*5)*j/rings))
    for j in range(rings):
        for i in range(segments):
            n=(i+1)%segments;faces.append((j*segments+i,(j+1)*segments+i,(j+1)*segments+n,j*segments+n))
    faces.append(tuple(range(segments)))
    mesh=bpy.data.meshes.new(name);mesh.from_pydata(verts,[],faces);mesh.update()
    uv=mesh.uv_layers.new(name='UVMap')
    for loop in mesh.loops:
        k=loop.vertex_index;uv.data[loop.index].uv=((k%segments)/segments,(k//segments)/rings)
    o=bpy.data.objects.new(name,mesh);bpy.context.collection.objects.link(o)
    bpy.context.view_layer.objects.active=o
    solid=o.modifiers.new('Forged plate thickness','SOLIDIFY');solid.thickness=.004
    bpy.ops.object.modifier_apply(modifier=solid.name)
    bevel=o.modifiers.new('Worn plate edges','BEVEL');bevel.width=.0015;bevel.segments=2
    bpy.ops.object.modifier_apply(modifier=bevel.name)
    return bind(o,rig,bone,mat)


def tusk(side,rig,mat,bind,variant):
    verts=[];faces=[];steps=12;segments=16
    for j in range(steps+1):
        t=j/steps;radius=.0075*(1-t)**.72+.00015
        center=(side*(.031+.004*math.sin(t*math.pi)-.002*t),
                -.141-.019*t+.006*t*t,1.326+.037*t)
        for i in range(segments):
            a=i*math.tau/segments
            verts.append((center[0]+radius*math.cos(a),center[1]+radius*math.sin(a),center[2]))
    for j in range(steps):
        for i in range(segments):
            n=(i+1)%segments;faces.append((j*segments+i,j*segments+n,(j+1)*segments+n,(j+1)*segments+i))
    faces.append(tuple(reversed(range(segments))))
    faces.append(tuple(steps*segments+i for i in range(segments)))
    mesh=bpy.data.meshes.new('Continuous curved tusk');mesh.from_pydata(verts,[],faces);mesh.update()
    uv=mesh.uv_layers.new(name='UVMap')
    for loop in mesh.loops:
        k=loop.vertex_index;uv.data[loop.index].uv=((k%segments)/segments,(k//segments)/steps)
    o=bpy.data.objects.new('Worn curved lower tusk',mesh);bpy.context.collection.objects.link(o)
    return bind(o,rig,'head',mat)
