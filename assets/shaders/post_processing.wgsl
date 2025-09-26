#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import bevy_pbr::{
    prepass_utils,
}


@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
struct PostProcessSettings {
    intensity: f32,
    world_from_clip: mat4x4<f32>,
    viewport: vec4<f32>,
    cam_pos: vec3<f32>,
}
@group(0) @binding(2) var<uniform> settings: PostProcessSettings;
@group(0) @binding(3) var depth_texture: texture_depth_multisampled_2d;
// @group(0) @binding(4) var depth_sampler: sampler;

/// Convert uv [0.0 .. 1.0] coordinate to ndc space xy [-1.0 .. 1.0]
fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0);
}

/// returns the (0.0, 0.0) .. (1.0, 1.0) position within the viewport for the current render target
/// [0 .. render target viewport size] eg. [(0.0, 0.0) .. (1280.0, 720.0)] to [(0.0, 0.0) .. (1.0, 1.0)]
fn frag_coord_to_uv(frag_coord: vec2<f32>) -> vec2<f32> {
    return (frag_coord - vec2(settings.viewport.xy)) / vec2(settings.viewport.zw);
}

/// Convert frag coord to ndc
fn frag_coord_to_ndc(frag_coord: vec4<f32>) -> vec3<f32> {
    return vec3(uv_to_ndc(frag_coord_to_uv(frag_coord.xy)), frag_coord.z);
}

@fragment
fn fragment(
    in: FullscreenVertexOutput
) -> @location(0) vec4<f32> {
    let offset_strength = settings.intensity;

    if (in.uv.y < 0.5) {
        return vec4<f32>(
            textureSample(screen_texture, texture_sampler, in.uv + vec2<f32>(offset_strength, -offset_strength)).r,
            textureSample(screen_texture, texture_sampler, in.uv + vec2<f32>(-offset_strength, 0.0)).g,
            textureSample(screen_texture, texture_sampler, in.uv + vec2<f32>(0.0, offset_strength)).b,
            1.0
        );
    } else {
        let size = textureDimensions(depth_texture).xy;
        let depth = textureLoad(depth_texture, vec2<i32>(in.uv.xy * vec2<f32>(size)), i32(0));

        var frag_coord = in.position;
        frag_coord.z = depth;

        let ndc = frag_coord_to_ndc(frag_coord);
        let world = settings.world_from_clip * vec4(ndc, 1.0);
        let pos = world.xyz / world.w;
        let d = distance(pos, settings.cam_pos);
        return vec4(vec3(d/2.)-1., 1.0);
    }
}

