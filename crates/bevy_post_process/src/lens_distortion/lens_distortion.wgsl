#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

// See `bevy_core_pipeline::post_process::TODO` for more
// information on these fields.
struct LensDistortionSettings {
    k1: f32,
    k2: f32,
    k3: f32,
    p1: f32,
    p2: f32,
}
// The source framebuffer texture.
@group(0) @binding(0) var source_texture: texture_2d<f32>;
// The sampler used to sample the source framebuffer texture.
@group(0) @binding(1) var source_sampler: sampler;
// The settings supplied by the developer.
@group(0) @binding(2) var<uniform> settings: LensDistortionSettings;

@fragment
fn fragment_main(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // UVs in range (0.0, 1.0)
    var xy = in.uv;

    // Scale range to (-1.0, 1.0)
    xy = (xy * 2.0) - 1.0;

    // Prepare terms
    let yy = xy.y * xy.y;
    let xx = xy.x * xy.x;
    let r2 = xx + yy;
    let r4 = r2 * r2;
    let r6 = r4 * r2;

    let radial_distortion = xy * (settings.k1 * r2 + settings.k2 * r4 + settings.k3 * r6);

    let tangential_distortion = vec2(
        2.0 * settings.p1 * xy.x * xy.y + settings.p2 * (r2 + 2.0 * xx),
        settings.p1 * (r2 + 2.0 * yy) + 2.0 * settings.p2 * xy.x * xy.y
    );

    xy = xy + radial_distortion + tangential_distortion;

    // Go back to range (0.0, 1.0) for sampling.
    // Values may now be out of range after distortion.
    xy = (xy + 1.0) / 2.;

    if ((xy.x < 0.0) || (xy.y < 0.0) || (xy.x > 1.0) || (xy.y > 1.0)) {
        return vec4(vec3(0.0), 1.0);
    } else {
        return vec4(textureSampleLevel(
            source_texture,
            source_sampler,
            xy,
            0.0,
        ).rgb, 1.0);
    }
}
