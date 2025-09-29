#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_render::view::{frag_coord_to_ndc, uv_to_ndc, View};

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var depth_texture: texture_depth_multisampled_2d;
@group(0) @binding(3) var<uniform> views: View;

@fragment
fn fragment(
    in: FullscreenVertexOutput
) -> @location(0) vec4<f32> {
    let size = textureDimensions(depth_texture).xy;
    let depth = textureLoad(depth_texture, vec2<i32>(in.uv.xy * vec2<f32>(size)), i32(0));

    var frag_coord = in.position;
    frag_coord.z = depth;

    let ndc = frag_coord_to_ndc(frag_coord, views.viewport);
    let world = views.world_from_clip * vec4(ndc, 1.0);
    let pos = world.xyz / world.w;
    let d = distance(pos, views.world_position);

    return vec4(vec3(d/2.)-1., 1.0);
}

