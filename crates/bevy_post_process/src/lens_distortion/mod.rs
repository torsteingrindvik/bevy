//! TODO

use bevy_app::{App, Plugin};
use bevy_asset::{embedded_asset, load_embedded_asset, AssetServer, Handle};
use bevy_camera::Camera;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::{QueryItem, With},
    reflect::ReflectComponent,
    resource::Resource,
    schedule::IntoScheduleConfigs as _,
    system::{lifetimeless::Read, Commands, Query, Res, ResMut},
    world::World,
};
use bevy_image::BevyDefault;
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::{
    diagnostic::RecordDiagnostics,
    extract_component::{ExtractComponent, ExtractComponentPlugin},
    render_graph::{
        NodeRunError, RenderGraphContext, RenderGraphExt as _, ViewNode, ViewNodeRunner,
    },
    render_resource::{
        binding_types::{sampler, texture_2d, uniform_buffer},
        BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
        CachedRenderPipelineId, ColorTargetState, ColorWrites, DynamicUniformBuffer, FilterMode,
        FragmentState, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
        RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
        ShaderType, SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat,
        TextureSampleType,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue},
    view::{ExtractedView, ViewTarget},
    Render, RenderApp, RenderStartup, RenderSystems,
};
use bevy_shader::Shader;
use bevy_utils::prelude::default;

use bevy_core_pipeline::{
    core_2d::graph::{Core2d, Node2d},
    core_3d::graph::{Core3d, Node3d},
    FullscreenShader,
};

#[derive(Default)]
pub struct LensDistortionPlugin;

#[derive(Reflect, Component, Clone, Default, ShaderType, Copy)]
#[reflect(Component, Default, Clone)]
pub struct LensDistortion {
    pub k1: f32,
    pub k2: f32,
    pub k3: f32,
    pub p1: f32,
    pub p2: f32,
}

impl LensDistortion {
    fn zero(&self) -> bool {
        self.k1 == 0.0 && self.k2 == 0.0 && self.k3 == 0.0 && self.p1 == 0.0 && self.p2 == 0.0
    }
}

#[derive(Resource)]
pub struct LensDistortionPipeline {
    /// The layout of bind group 0, containing the source, sampler, and settings
    bind_group_layout: BindGroupLayoutDescriptor,
    /// Specifies how to sample the source framebuffer texture.
    source_sampler: Sampler,
    /// The asset handle for the fullscreen vertex shader.
    fullscreen_shader: FullscreenShader,
    /// The fragment shader asset handle.
    fragment_shader: Handle<Shader>,
}

/// A key that uniquely identifies a built-in postprocessing pipeline.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LensDistortionPipelineKey {
    /// The format of the source and destination textures.
    texture_format: TextureFormat,
}

/// A component attached to cameras in the render world that stores the
/// specialized pipeline ID for the built-in postprocessing stack.
#[derive(Component, Deref, DerefMut)]
pub struct LensDistortionPipelineId(CachedRenderPipelineId);

/// A resource, part of the render world, that stores the
/// [`ChromaticAberrationUniform`]s for each view.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct LensDistortionUniformBuffers {
    lens_distortion: DynamicUniformBuffer<LensDistortion>,
}

/// A component, part of the render world, that stores the appropriate byte
/// offset within the [`PostProcessingUniformBuffers`] for the camera it's
/// attached to.
#[derive(Component, Deref, DerefMut)]
pub struct LensDistortionUniformBufferOffsets {
    lens_distortion: u32,
}

/// The render node that runs the built-in postprocessing stack.
#[derive(Default)]
pub struct LensDistortionNode;

impl Plugin for LensDistortionPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "lens_distortion.wgsl");

        app.add_plugins(ExtractComponentPlugin::<LensDistortion>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<SpecializedRenderPipelines<LensDistortionPipeline>>()
            .init_resource::<LensDistortionUniformBuffers>()
            .add_systems(RenderStartup, init_pipeline)
            .add_systems(
                Render,
                (
                    prepare_lens_distortion_pipelines,
                    prepare_post_processing_uniforms,
                )
                    .in_set(RenderSystems::Prepare),
            )
            .add_render_graph_node::<ViewNodeRunner<LensDistortionNode>>(
                Core3d,
                Node3d::LensDistortion,
            )
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::DepthOfField,
                    Node3d::LensDistortion,
                    Node3d::Tonemapping,
                ),
            )
            .add_render_graph_node::<ViewNodeRunner<LensDistortionNode>>(
                Core2d,
                Node2d::LensDistortion,
            )
            .add_render_graph_edges(
                Core2d,
                (Node2d::Bloom, Node2d::LensDistortion, Node2d::Tonemapping),
            );
    }
}

pub fn init_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    fullscreen_shader: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
) {
    // Create our single bind group layout.
    let bind_group_layout = BindGroupLayoutDescriptor::new(
        "lens distortion bind group layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                // Source texture (framebuffer)
                texture_2d(TextureSampleType::Float { filterable: true }),
                // Sampler for source texture
                sampler(SamplerBindingType::Filtering),
                // Chromatic aberration settings:
                uniform_buffer::<LensDistortion>(true),
            ),
        ),
    );

    let source_sampler = render_device.create_sampler(&SamplerDescriptor {
        mipmap_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mag_filter: FilterMode::Linear,
        ..default()
    });

    commands.insert_resource(LensDistortionPipeline {
        bind_group_layout,
        source_sampler,
        fullscreen_shader: fullscreen_shader.clone(),
        fragment_shader: load_embedded_asset!(asset_server.as_ref(), "lens_distortion.wgsl"),
    });
}

impl SpecializedRenderPipeline for LensDistortionPipeline {
    type Key = LensDistortionPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("lens distortion".into()),
            layout: vec![self.bind_group_layout.clone()],
            vertex: self.fullscreen_shader.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: self.fragment_shader.clone(),
                targets: vec![Some(ColorTargetState {
                    format: key.texture_format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        }
    }
}

impl ViewNode for LensDistortionNode {
    type ViewQuery = (
        Read<ViewTarget>,
        Read<LensDistortionPipelineId>,
        Read<LensDistortion>,
        Read<LensDistortionUniformBufferOffsets>,
    );

    fn run<'w>(
        &self,
        _: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (view_target, pipeline_id, _lens_distortion, uniform_buffer_offsets): QueryItem<
            'w,
            '_,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let lens_distortion_pipeline = world.resource::<LensDistortionPipeline>();
        let uniforms_buffers = world.resource::<LensDistortionUniformBuffers>();
        // let gpu_image_assets = world.resource::<RenderAssets<GpuImage>>();

        // We need a render pipeline to be prepared.
        let Some(pipeline) = pipeline_cache.get_render_pipeline(**pipeline_id) else {
            return Ok(());
        };

        // We need the settings to be uploaded to the GPU.
        let Some(uniform_buffer_binding) = uniforms_buffers.lens_distortion.binding() else {
            return Ok(());
        };

        let diagnostics = render_context.diagnostic_recorder();

        // Use the [`PostProcessWrite`] infrastructure, since this is a
        // full-screen pass.
        let post_process = view_target.post_process_write();

        let pass_descriptor = RenderPassDescriptor {
            label: Some("lens distortion"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        };

        let bind_group = render_context.render_device().create_bind_group(
            Some("lens distortion bind group"),
            &pipeline_cache.get_bind_group_layout(&lens_distortion_pipeline.bind_group_layout),
            &BindGroupEntries::sequential((
                post_process.source,
                &lens_distortion_pipeline.source_sampler,
                uniform_buffer_binding,
            )),
        );

        let mut render_pass = render_context
            .command_encoder()
            .begin_render_pass(&pass_descriptor);
        let pass_span = diagnostics.pass_span(&mut render_pass, "lens distortion");

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[**uniform_buffer_offsets]);
        render_pass.draw(0..3, 0..1);

        pass_span.end(&mut render_pass);

        Ok(())
    }
}

/// Specializes the built-in postprocessing pipeline for each applicable view.
pub fn prepare_lens_distortion_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<LensDistortionPipeline>>,
    post_processing_pipeline: Res<LensDistortionPipeline>,
    views: Query<(Entity, &ExtractedView), With<LensDistortion>>,
) {
    for (entity, view) in views.iter() {
        let pipeline_id = pipelines.specialize(
            &pipeline_cache,
            &post_processing_pipeline,
            LensDistortionPipelineKey {
                texture_format: if view.hdr {
                    ViewTarget::TEXTURE_FORMAT_HDR
                } else {
                    TextureFormat::bevy_default()
                },
            },
        );

        commands
            .entity(entity)
            .insert(LensDistortionPipelineId(pipeline_id));
    }
}

/// Gathers the built-in postprocessing settings for every view and uploads them
/// to the GPU.
pub fn prepare_post_processing_uniforms(
    mut commands: Commands,
    mut uniform_buffers: ResMut<LensDistortionUniformBuffers>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut views: Query<(Entity, &LensDistortion)>,
) {
    uniform_buffers.clear();

    // Gather up all the postprocessing settings.
    for (view_entity, lens_distortion) in views.iter_mut() {
        let uniform_buffer_offset = uniform_buffers.push(lens_distortion);
        commands
            .entity(view_entity)
            .insert(LensDistortionUniformBufferOffsets {
                lens_distortion: uniform_buffer_offset,
            });
    }

    // Upload to the GPU.
    uniform_buffers.write_buffer(&render_device, &render_queue);
}

impl ExtractComponent for LensDistortion {
    type QueryData = Read<LensDistortion>;

    type QueryFilter = With<Camera>;

    type Out = LensDistortion;

    fn extract_component(lens_distortion: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        // Skip if all parameters are zero
        if lens_distortion.zero() {
            None
        } else {
            Some(lens_distortion.clone())
        }
    }
}
