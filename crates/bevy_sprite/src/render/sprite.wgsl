#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping
#endif

struct View {
    view_proj: mat4x4<f32>,
    inverse_view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    inverse_view: mat4x4<f32>,
    projection: mat4x4<f32>,
    inverse_projection: mat4x4<f32>,
    world_position: vec3<f32>,
    // viewport(x_origin, y_origin, width, height)
    viewport: vec4<f32>,
};
@group(0) @binding(0)
var<uniform> view: View;

struct VertexOutput {
    @location(0) uv: vec2<f32>,
    @location(1) entity_index: u32,
#ifdef COLORED
    @location(2) color: vec4<f32>,
#endif
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vertex(
    @location(0) vertex_position: vec3<f32>,
    @location(1) vertex_uv: vec2<f32>,
    @location(2) entity_index: u32,
#ifdef COLORED
    @location(3) vertex_color: vec4<f32>,
#endif
) -> VertexOutput {
    var out: VertexOutput;
    out.uv = vertex_uv;
    out.entity_index = entity_index;
    out.position = view.view_proj * vec4<f32>(vertex_position, 1.0);
#ifdef COLORED
    out.color = vertex_color;
#endif
    return out;
}

@group(1) @binding(0)
var sprite_texture: texture_2d<f32>;
@group(1) @binding(1)
var sprite_sampler: sampler;

struct FragmentOutput {
   @location(0) color: vec4<f32>,
   @location(1) picking: u32,
 }

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var color = textureSample(sprite_texture, sprite_sampler, in.uv);
#ifdef COLORED
    color = in.color * color;
#endif

#ifdef TONEMAP_IN_SHADER
    color = vec4<f32>(reinhard_luminance(color.rgb), color.a);
#endif

    var out: FragmentOutput;

    out.color = color;
    out.picking = in.entity_index;

    return out;
}
