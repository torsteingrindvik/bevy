use std::sync::{Arc, Mutex};

use bevy_derive::Deref;
use bevy_ecs::{prelude::*, query::QueryItem};
use bevy_math::UVec2;
use bevy_utils::HashMap;
use wgpu::{
    BufferDescriptor, BufferUsages, BufferView, Extent3d, ImageCopyBuffer, ImageDataLayout,
    Maintain, MapMode, Operations, RenderPassColorAttachment, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages,
};

use crate::{
    camera::{Camera, ExtractedCamera},
    extract_component::ExtractComponent,
    prelude::Color,
    render_resource::Buffer,
    renderer::{RenderContext, RenderDevice},
    texture::{CachedTexture, TextureCache},
    view::{Msaa, ViewDepthTexture},
};

#[cfg(feature = "trace")]
use bevy_utils::tracing::info_span;

/// See [WebGPU format capabilities].
///
/// We have to:
/// 1. Use a texture format that supports blending. This implies "float" in the sample type in the link above.
/// 2. Have an alpha channel such that we can blend- which allows us to cut out the background from e.g. a partially transparent image.
/// 3. Have enough precision to be able to decompose an entity index across channels.
/// The entity index is a u32, so across three channels we could do e.g. 12 bits, 12 bits, 8 bits.
/// The largest possible number stored in a single channel is then 2^12 = 4096, which is well within the limits of 16 bit floats
/// according to [Wikipedia half precision floating point].
///
/// [WebGPU format capabilities]: https://www.w3.org/TR/webgpu/#texture-format-caps
/// [Wikipedia half precision floating point]: https://en.wikipedia.org/wiki/Half-precision_floating-point_format
pub const PICKING_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

pub fn copy_to_buffer(
    picking: &Picking,
    picking_textures: &PickingTextures,
    depth: Option<&ViewDepthTexture>,
    render_context: &mut RenderContext,
) {
    let mut binding = picking.try_lock().expect("TODO: Can we lock here?");
    let picking_resources = binding.as_mut().expect("Buffer should have been prepared");

    let size = &picking_resources.size;

    render_context.command_encoder.copy_texture_to_buffer(
        picking_textures.main.texture.as_image_copy(),
        ImageCopyBuffer {
            buffer: &picking_resources.pick_buffer,
            layout: ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(
                    std::num::NonZeroU32::new(size.padded_bytes_per_row as u32).unwrap(),
                ),
                rows_per_image: None,
            },
        },
        Extent3d {
            width: size.texture_size.x,
            height: size.texture_size.y,
            depth_or_array_layers: 1,
        },
    );

    // Only `Some(...)` if picking from 3D cameras.
    if let Some(depth) = depth {
        render_context.command_encoder.copy_texture_to_buffer(
            depth.texture.as_image_copy(),
            ImageCopyBuffer {
                buffer: &picking_resources.depth_buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(
                        std::num::NonZeroU32::new(size.padded_bytes_per_row as u32).unwrap(),
                    ),
                    rows_per_image: None,
                },
            },
            Extent3d {
                width: size.texture_size.x,
                height: size.texture_size.y,
                depth_or_array_layers: 1,
            },
        );
    }
}

#[derive(Debug, Clone)]
pub struct PickingResources {
    // Buffer written by GPU and read by CPU. Holds entity indices.
    pick_buffer: Buffer,

    // Accompanies the above. Allows reading the depth too.
    depth_buffer: Buffer,

    // A wrapper around the rendered size.
    // The buffer might be larger due to padding.
    size: PickingBufferSize,
}

/// Add this to a camera in order for the camera to also render to a buffer
/// with entity indices instead of colors.
#[derive(Component, Debug, Clone, Default, Deref)]
pub struct Picking(Arc<Mutex<Option<PickingResources>>>);

impl Picking {
    /// Get the entity at the given coordinate.
    /// If there is no entity, returns `None`.
    ///
    /// Panics if the coordinate is out of bounds.
    pub fn get_entity(&self, camera: &Camera, coordinates: UVec2) -> Option<Entity> {
        let guard = self.try_lock().expect("Should have been unlocked");
        let Some(resources) = guard.as_ref() else {
            // Picking resources not yet prepared.
            return None
        };

        let slice = resources.pick_buffer.slice(..);

        let virtual_entity_index = coords_to_data(
            coordinates,
            camera,
            &resources.size,
            &slice.get_mapped_range(),
            |bytes| {
                // Four channels, 16 bites per channel.
                assert!(bytes.len() == 4 * 2, "It's {:?}", bytes.len());
                let f16_to_u16 = |bytes: &[u8], start: usize| {
                    half::f16::from_le_bytes(bytes[start..start + 2].try_into().unwrap()).to_f32()
                        as u16
                };

                let u16_lower_8 = f16_to_u16(bytes, 0);
                let u16_mid_12 = f16_to_u16(bytes, 2);
                let u16_upper_12 = f16_to_u16(bytes, 4);

                u16_lower_8 as u32 | ((u16_mid_12 as u32) << 8) | ((u16_upper_12 as u32) << 20)
            },
        );

        // See picking.wgsl for the explanation of the virtual entity index.
        if virtual_entity_index == 0 {
            None
        } else {
            Some(Entity::from_raw(virtual_entity_index - 1))
        }
    }

    /// Get the depth at the given coordinate.
    ///
    /// Panics if the coordinate is out of bounds.
    pub fn depth(&self, camera: &Camera, coordinates: UVec2) -> f32 {
        let guard = self.try_lock().expect("Should have been unlocked");
        let resources = guard.as_ref().expect("Resources should have been prepared");

        let slice = resources.depth_buffer.slice(..);

        let depth = coords_to_data(
            coordinates,
            camera,
            &resources.size,
            &slice.get_mapped_range(),
            |bytes| {
                f32::from_le_bytes(
                    bytes
                        .try_into()
                        .expect("Should be able to make f32 (depth) out of 4 bytes"),
                )
            },
        );

        depth
    }
}

#[derive(Debug, Clone, Default)]
pub struct PickingBufferSize {
    pub texture_size: UVec2,
    pub padded_bytes_per_row: usize,
}

impl PickingBufferSize {
    pub fn new(width: u32, height: u32) -> Self {
        let bytes_per_pixel = PICKING_TEXTURE_FORMAT.describe().block_size as usize;
        // Four channels per pixel.
        let unpadded_bytes_per_row = width as usize * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;

        // See: https://github.com/gfx-rs/wgpu/blob/master/wgpu/examples/capture/main.rs#L193
        let padded_bytes_per_row_padding = (align - (unpadded_bytes_per_row % align)) % align;
        let padded_bytes_per_row = unpadded_bytes_per_row + padded_bytes_per_row_padding;

        Self {
            texture_size: UVec2 {
                x: width,
                y: height,
            },
            padded_bytes_per_row,
        }
    }

    pub fn total_needed_bytes(&self) -> u64 {
        (self.texture_size.y as usize * self.padded_bytes_per_row) as u64
    }
}

impl From<Extent3d> for PickingBufferSize {
    fn from(texture_extent: Extent3d) -> Self {
        Self::new(texture_extent.width, texture_extent.height)
    }
}

impl From<UVec2> for PickingBufferSize {
    fn from(texture_extent: UVec2) -> Self {
        Self::new(texture_extent.x, texture_extent.y)
    }
}

impl ExtractComponent for Picking {
    type Query = &'static Self;
    type Filter = With<Camera>;
    type Out = Self;

    fn extract_component(item: QueryItem<'_, Self::Query>) -> Option<Self::Out> {
        Some(item.clone())
    }
}

#[derive(Component, Clone)]
pub struct PickingTextures {
    pub main: CachedTexture,
    pub sampled: Option<CachedTexture>,
}

impl PickingTextures {
    pub fn clear_color() -> wgpu::Color {
        Color::BLACK.into()
    }

    /// Retrieve this target's color attachment. This will use [`Self::sampled`] and resolve to [`Self::main`] if
    /// the target has sampling enabled. Otherwise it will use [`Self::main`] directly.
    pub fn get_color_attachment(&self, ops: Operations<wgpu::Color>) -> RenderPassColorAttachment {
        match &self.sampled {
            Some(sampled_texture) => RenderPassColorAttachment {
                view: &sampled_texture.default_view,
                resolve_target: Some(&self.main.default_view),
                ops,
            },
            None => RenderPassColorAttachment {
                view: &self.main.default_view,
                resolve_target: None,
                ops,
            },
        }
    }

    pub fn get_unsampled_color_attachment(
        &self,
        ops: Operations<wgpu::Color>,
    ) -> RenderPassColorAttachment {
        RenderPassColorAttachment {
            view: &self.main.default_view,
            resolve_target: None,
            ops,
        }
    }
}

fn coords_to_data<F, T>(
    coords: UVec2,
    camera: &Camera,
    picking_buffer_size: &PickingBufferSize,
    buffer_view: &BufferView,
    make_data_from_viewed_bytes: F,
) -> T
where
    F: FnOnce(&[u8]) -> T,
{
    let camera_size = camera
        .physical_target_size()
        .expect("Camera passed should have a size");

    // The GPU image has a top-left origin,
    // but the cursor has a bottom-left origin.
    // Therefore we must flip the vertical axis.
    let x = coords.x as usize;

    // TODO: This can fail. Make it not do this.
    let y = (camera_size.y as usize).saturating_sub(coords.y as usize);

    // We know the coordinates, but in order to find the true position of the 4 bytes
    // we're interested in, we have to know how wide a single line in the GPU written buffer is.
    // Due to alignment requirements this may be wider than the physical camera size because
    // of padding.
    let padded_width = picking_buffer_size.padded_bytes_per_row;

    let pixel_size = PICKING_TEXTURE_FORMAT.describe().block_size as usize;

    let start = (y * padded_width) + (x * pixel_size);
    let end = start + pixel_size;

    // TODO: Sometimes we're able to go out of bounds here:
    //  "panicked at 'range end index 7381600 out of range for slice of length 7372800'",
    // we have to figure out when this can happen and why.
    let view_bytes = &buffer_view[start..end];

    make_data_from_viewed_bytes(view_bytes)
}

pub fn map_buffers(query: Query<(&Picking, &Camera)>, render_device: Res<RenderDevice>) {
    #[cfg(feature = "trace")]
    let _picking_span = info_span!("picking_map", name = "picking_map").entered();

    for (picking, camera) in query.iter() {
        let Some(camera_size) = camera.physical_target_size() else { continue };

        if camera_size.x == 0 || camera_size.y == 0 {
            continue;
        }

        // TODO: Is it possible the GPU tries this at the same time as us?
        let picking_resources = picking.try_lock().unwrap();

        let Some(picking_resources) = picking_resources.as_ref() else { continue };

        let picking_buffer_slice = picking_resources.pick_buffer.slice(..);
        picking_buffer_slice.map_async(MapMode::Read, move |result| {
            if let Err(e) = result {
                panic!("{e}");
            }
        });

        let depth_buffer_slice = picking_resources.depth_buffer.slice(..);
        depth_buffer_slice.map_async(MapMode::Read, move |result| {
            if let Err(e) = result {
                panic!("{e}");
            }
        });
    }

    {
        #[cfg(feature = "trace")]
        let _poll_span = info_span!("picking_poll", name = "picking_poll").entered();

        // For the above mapping to complete
        render_device.poll(Maintain::Wait);
    }
}

pub fn unmap_buffers(query: Query<(&Picking, &Camera)>) {
    #[cfg(feature = "trace")]
    let _picking_span = info_span!("picking_unmap", name = "picking_unmap").entered();

    for (picking, camera) in query.iter() {
        let Some(camera_size) = camera.physical_target_size() else { continue };

        if camera_size.x == 0 || camera_size.y == 0 {
            continue;
        }

        // TODO: Is it possible the GPU tries this at the same time as us?
        let picking_resources = picking.try_lock().unwrap();

        let Some(picking_resources) = picking_resources.as_ref() else { continue };

        picking_resources.pick_buffer.unmap();
        picking_resources.depth_buffer.unmap();
    }
}

pub fn prepare_picking_targets(
    mut commands: Commands,
    msaa: Res<Msaa>,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    cameras: Query<(Entity, &ExtractedCamera, &Picking)>,
) {
    #[cfg(feature = "trace")]
    let _picking_span = info_span!("picking_prepare", name = "picking_prepare").entered();

    let mut textures = HashMap::default();
    for (entity, camera, picking) in cameras.iter() {
        if let Some(target_size) = camera.physical_target_size {
            let size = Extent3d {
                width: target_size.x,
                height: target_size.y,
                depth_or_array_layers: 1,
            };
            let picking_buffer_dimensions = PickingBufferSize::from(size);
            let needed_buffer_size = picking_buffer_dimensions.total_needed_bytes();

            let mut picking_resources = picking.try_lock().expect("TODO: Are we ok to lock here?");

            let make_buffer = || {
                #[cfg(feature = "trace")]
                bevy_utils::tracing::debug!("Creating new picking buffer");

                render_device.create_buffer(&BufferDescriptor {
                    label: Some("Picking buffer"),
                    size: needed_buffer_size,
                    usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                })
            };

            match picking_resources.as_mut() {
                Some(mut pr) => {
                    if pr.pick_buffer.size() != needed_buffer_size
                        || pr.depth_buffer.size() != needed_buffer_size
                        || pr.size.texture_size != target_size
                    {
                        pr.pick_buffer = make_buffer();
                        pr.depth_buffer = make_buffer();
                        pr.size = size.into();
                    }
                }
                None => {
                    *picking_resources = Some(PickingResources {
                        pick_buffer: make_buffer(),
                        depth_buffer: make_buffer(),
                        size: size.into(),
                    });
                }
            }

            let picking_textures = textures.entry(camera.target.clone()).or_insert_with(|| {
                let descriptor = TextureDescriptor {
                    label: None,
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: PICKING_TEXTURE_FORMAT,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
                };

                PickingTextures {
                    main: texture_cache.get(
                        &render_device,
                        TextureDescriptor {
                            label: Some("main_picking_texture"),
                            ..descriptor
                        },
                    ),
                    sampled: (msaa.samples > 1).then(|| {
                        texture_cache.get(
                            &render_device,
                            TextureDescriptor {
                                label: Some("main_picking_texture_sampled"),
                                size,
                                mip_level_count: 1,
                                sample_count: msaa.samples,
                                dimension: TextureDimension::D2,
                                format: PICKING_TEXTURE_FORMAT,
                                usage: TextureUsages::RENDER_ATTACHMENT,
                            },
                        )
                    }),
                }
            });

            commands.entity(entity).insert(picking_textures.clone());
        }
    }
}
