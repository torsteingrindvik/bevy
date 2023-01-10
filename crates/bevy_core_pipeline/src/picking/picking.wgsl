#define_import_path bevy_core_pipeline::picking

fn picking_alpha(a: f32) -> f32 {
    // If e.g. a sprite has slightly transparent pixels, we make that opaque (by setting alpha to 1.0)
    // for picking purposes.
    // If we don't do this, blending will occur on the entity index value, which makes no sense.
    //
    // An alternative is to truncate the alpha to 0.0 unless it's 1.0, but that would shrink
    // which parts of the translucent entity are pickable, which is not desirable.
    return ceil(a);
}

// TODO: Describe why/what
fn entity_index_to_vec3_f32(entity_index: u32) -> vec3<f32> {
    let mask_8 = 0x000000FFu;
    let mask_12 = 0x00000FFFu;

    let lower_8 = entity_index & mask_8;
    let mid_12 = (entity_index >> 8u) & mask_12;
    let up_12 = (entity_index >> 20u) & mask_12;

    return vec3(
        f32(lower_8),
        f32(mid_12),
        f32(up_12),
    );
}
