const PAGE_TEXELS: u32 = 87381u;
@compute @workgroup_size(8,8)
fn bake(@builtin(global_invocation_id) local:vec3<u32>) {
    if local.y >= uniforms.tile.y {return;}
    let id=local+vec3<u32>(0u,uniforms.tile.x,0u);
    let size=uniforms.mip.x;
    if any(id.xy>=vec2<u32>(size)) {return;}
    let xz=uniforms.page.xy+(vec2<f32>(id.xy)+0.5)/f32(size)*uniforms.page.z;
    let p=vec3<f32>(xz.x,terrain_height(xz),xz.y);
    let n=normalize(vec3<f32>(terrain_height(xz-vec2<f32>(0.5,0.0))-terrain_height(xz+vec2<f32>(0.5,0.0)),1.0,terrain_height(xz-vec2<f32>(0.0,0.5))-terrain_height(xz+vec2<f32>(0.0,0.5))));
    let fields=surface_climate(xz);
    let snow=climate_snow(p,n,fields);
    let mountain=fields.y;
    var color=vec3<f32>(0.0);var normal=n;
    if mountain<1.0 {color=material_color_climate(1.0,p,n,snow,fields.z,uniforms.page.z/f32(size));}
    if mountain>0.0 {
        let alpine=mountain_surface_snow(p,n,0.0,uniforms.page.z/f32(size),snow);
        let exposed=(1.0-smoothstep(0.55,0.83,n.y))*smoothstep(0.02,0.20,mountain);
        color=mix(color,alpine.color,max(mountain,exposed));normal=normalize(mix(n,alpine.normal,mountain));
    }
    let i=u32(uniforms.page.w)*PAGE_TEXELS+id.y*size+id.x;
    pixels[i]=vec2<u32>(pack4x8unorm(vec4<f32>(color,1.0)),pack4x8unorm(vec4<f32>(normal*0.5+0.5,snow)));
    if uniforms.page.w<16.0 {
        pixels[48u*PAGE_TEXELS+u32(uniforms.page.w)*65536u+id.y*256u+id.x]=vec2<u32>(pack2x16float(fields.xy),pack2x16float(fields.zw));
    }
}
@compute @workgroup_size(8,8)
fn mip(@builtin(global_invocation_id) id:vec3<u32>){
    let size=uniforms.mip.x;if any(id.xy>=vec2<u32>(size)){return;}
    let base=u32(uniforms.page.w)*PAGE_TEXELS;
    var color=vec4<f32>(0.0);var normal=vec4<f32>(0.0);
    for(var y=0u;y<2u;y++){for(var x=0u;x<2u;x++){
        let value=pixels[base+uniforms.mip.z+(id.y*2u+y)*size*2u+id.x*2u+x];
        color+=unpack4x8unorm(value.x);normal+=unpack4x8unorm(value.y);
    }}
    normal*=0.25;normal=vec4<f32>(normalize(normal.xyz*2.0-1.0)*0.5+0.5,normal.w);
    pixels[base+uniforms.mip.y+id.y*size+id.x]=vec2<u32>(pack4x8unorm(color*0.25),pack4x8unorm(normal));
}
@compute @workgroup_size(1)
fn publish(){pages[u32(uniforms.page.w)]=vec4<f32>(uniforms.page.xyz,1.0);}

@compute @workgroup_size(64)
fn rock_bake(@builtin(global_invocation_id) id:vec3<u32>){
    if id.x>=uniforms.mip.x{return;}
    let vertex=rock_input[id.x];let p=vertex.position.xyz;let n=normalize(vertex.normal.xyz);
    var surface=MountainSurface(material_color(1.0,p,n),n);
    if mountain_amount(p.xz)>0.0 {surface=mountain_surface(p,n,1.0,vertex.position.w);}
    rock_pixels[u32(uniforms.page.w)+id.x]=vec2<u32>(pack4x8unorm(vec4<f32>(surface.color,1.0)),pack4x8unorm(vec4<f32>(surface.normal*0.5+0.5,surface_snow(p,n))));
}

// Exact provisional terrain data, reused by all raster passes. Publication is
// safe here because no draw can consume this buffer until the compute pass ends.
@compute @workgroup_size(8,8)
fn terrain_bake(@builtin(global_invocation_id) id:vec3<u32>) {
    if any(id.xy>=vec2<u32>(33u)) {return;}
    let p=uniforms.page.xy+vec2<f32>(id.xy);
    let n=normalize(vec3<f32>(terrain_height(p-vec2<f32>(0.5,0.0))-terrain_height(p+vec2<f32>(0.5,0.0)),1.0,terrain_height(p-vec2<f32>(0.0,0.5))-terrain_height(p+vec2<f32>(0.0,0.5))));
    let slot=u32(uniforms.page.w);
    terrain_samples[slot*1089u+id.y*33u+id.x]=vec4<f32>(terrain_height(p),n);
    if all(id.xy==vec2<u32>(0u)) {pages[48u+slot]=vec4<f32>(uniforms.page.xy,32.0,1.0);}
}

// Reduce every vertex used by the provisional mesh, including tile edges.
// A separate dispatch makes the preceding sample writes visible to all lanes.
var<workgroup> tile_min: array<f32,64>;
var<workgroup> tile_max: array<f32,64>;
@compute @workgroup_size(64)
fn terrain_bounds(@builtin(local_invocation_index) lane:u32) {
    let slot=u32(uniforms.page.w);
    var low=1e20; var high=-1e20;
    for(var i=lane;i<1089u;i+=64u) {
        let h=terrain_samples[slot*1089u+i].x;
        low=min(low,h); high=max(high,h);
    }
    tile_min[lane]=low; tile_max[lane]=high;
    workgroupBarrier();
    for(var stride=32u;stride>0u;stride/=2u) {
        if lane<stride {
            tile_min[lane]=min(tile_min[lane],tile_min[lane+stride]);
            tile_max[lane]=max(tile_max[lane],tile_max[lane+stride]);
        }
        workgroupBarrier();
    }
    if lane==0u {pages[48u+1024u+slot]=vec4<f32>(uniforms.page.xy,tile_min[0]-0.05,tile_max[0]+0.05);}
}
