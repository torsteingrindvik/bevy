use std::f32::consts::PI;

use crate::{Interaction, RelativeCursorPosition};
use crate::{UiMaterial, UiMaterialPlugin};
use bevy_app::{Plugin, Update};
use bevy_asset::{load_internal_asset, Asset, Assets, Handle};
use bevy_derive::Deref;
use bevy_ecs::{
    entity::Entity,
    event::{Event, EventReader, EventWriter},
    prelude::Component,
    query::{Changed, With},
    reflect::ReflectComponent,
    schedule::IntoSystemConfigs,
    system::{Query, Res, ResMut},
};
use bevy_log::warn;
use bevy_math::{Vec2, Vec3};
use bevy_reflect::{std_traits::ReflectDefault, Reflect, TypePath};
use bevy_render::render_resource::ShaderType;
use bevy_render::{
    color::Color,
    render_resource::{AsBindGroup, Shader, ShaderRef},
};

pub const COLOR_PICKER_HUE_WHEEL_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(864139486189741938413);
pub const COLOR_PICKER_SATURATION_VALUE_BOX_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(821381985132985160161);

#[derive(Debug, Default)]
pub struct ColorPickerPlugin;

impl Plugin for ColorPickerPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        load_internal_asset!(
            app,
            COLOR_PICKER_HUE_WHEEL_SHADER_HANDLE,
            "color_picker/hue_wheel.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            COLOR_PICKER_SATURATION_VALUE_BOX_SHADER_HANDLE,
            "color_picker/saturation_value_box.wgsl",
            Shader::from_wgsl
        );

        app.add_event::<HueWheelEvent>()
            .add_event::<SaturationValueBoxEvent>()
            .add_systems(
                Update,
                (
                    hue_wheel_events,
                    update_saturation_value_box_hue,
                    saturation_value_box_events,
                )
                    .chain(),
            )
            .add_plugins((
                UiMaterialPlugin::<HueWheelMaterial>::default(),
                UiMaterialPlugin::<SaturationValueBoxMaterial>::default(),
            ));
    }
}

/// When the hue wheel is pressed, this event is generated
#[derive(Debug, Event)]
pub struct HueWheelEvent {
    /// The [`HueWheel`] entity which produced the event
    pub entity: Entity,

    /// The color pressed on the wheel
    pub color: Color,

    /// The (0., 1.) range hue value pressed
    pub hue: f32,
}

/// When the saturation-value box is pressed, this event is generated
#[derive(Debug, Event)]
pub struct SaturationValueBoxEvent {
    /// The [`SaturationValueBox`] entity which produced the event
    pub entity: Entity,

    /// The color pressed within the box
    pub color: Color,
}

/// Marks an entity as a sibling to the wrapped hue wheel entity.
/// A [`SaturationValueBox`] with this component will automatically update its
/// colors when the [`HueWheel`] changes its hue.
#[derive(Debug, Component, Deref)]
pub struct HueWheelSibling(pub Entity);

fn hue_wheel_events(
    interaction_query: Query<
        (
            Entity,
            &Handle<HueWheelMaterial>,
            &Interaction,
            &RelativeCursorPosition,
        ),
        (Changed<Interaction>, With<HueWheel>),
    >,
    wheels: Res<Assets<HueWheelMaterial>>,
    mut event_writer: EventWriter<HueWheelEvent>,
) {
    for (entity, material_handle, interaction, relative_position) in &interaction_query {
        if *interaction == Interaction::Pressed {
            if let Some(uv) = relative_position.normalized {
                // NOTE: The UV and hue calculations must sync with similar calculations in `hue_wheel.wgsl`
                let uv = (uv * 2.) - 1.;

                let Some(HueWheelMaterial { inner_radius }) = wheels.get(material_handle) else {
                    warn!("unexpected: a saturation-value box was pressed but found no asset containing its material");
                    continue;
                };
                let length = uv.length();
                if length < *inner_radius || length > 1.0 {
                    // the wheel is cut-out if below the radius in the shader so don't
                    // generate events either.
                    // similarly, don't send events from interactions outside the outer radius
                    continue;
                }

                let hue = (uv.y.atan2(uv.x) + PI) / (2. * PI);

                event_writer.send(HueWheelEvent {
                    entity,
                    color: hsv_to_rgb(hue, 1., 1.),
                    hue,
                });
            }
        }
    }
}

/// When hue wheels produce events the selected hue has changed.
/// The box UI is updated to match here.
fn update_saturation_value_box_hue(
    box_query: Query<
        (
            Entity,
            &Handle<SaturationValueBoxMaterial>,
            &HueWheelSibling,
        ),
        With<SaturationValueBox>,
    >,
    mut hue: EventReader<HueWheelEvent>,
    mut boxes: ResMut<Assets<SaturationValueBoxMaterial>>,
) {
    for event in hue.read() {
        let HueWheelEvent {
            entity,
            color: _,
            hue,
        } = event;

        if let Some((_, handle, _)) = box_query.iter().find(|(.., sibling)| *entity == ***sibling) {
            let Some(material) = boxes.get_mut(handle) else {
                continue;
            };

            material.uniform.hue = *hue;
        } else {
            continue;
        }
    }
}

fn saturation_value_box_events(
    interaction_query: Query<
        (
            Entity,
            &Interaction,
            &RelativeCursorPosition,
            &Handle<SaturationValueBoxMaterial>,
        ),
        (Changed<Interaction>, With<SaturationValueBox>),
    >,
    mut boxes: ResMut<Assets<SaturationValueBoxMaterial>>,
    mut event_writer: EventWriter<SaturationValueBoxEvent>,
) {
    for (entity, interaction, relative_position, material_handle) in &interaction_query {
        if *interaction == Interaction::Pressed {
            if let Some(uv) = relative_position.normalized {
                let Some(SaturationValueBoxMaterial { uniform }) = boxes.get_mut(material_handle)
                else {
                    warn!("unexpected: a saturation-value box was pressed but found no asset containing its material");
                    continue;
                };

                uniform.marker = Vec2::new(uv.x * 2. - 1., (uv.y * 2. - 1.) * -1.);

                // NOTE: We want "value" to increase vertically which looks most natural hence the flip
                let color = hsv_to_rgb(uniform.hue, uv.x, 1.0 - uv.y);
                event_writer.send(SaturationValueBoxEvent { entity, color });
            }
        }
    }
}

/// As ported from utils.wgsl:
///
/// ```wgsl
/// fn hsv2rgb(hue: f32, saturation: f32, value: f32) -> vec3<f32> {
///     let rgb = clamp(
///         abs(
///             ((hue * 6.0 + vec3<f32>(0.0, 4.0, 2.0)) % 6.0) - 3.0
///         ) - 1.0,
///         vec3<f32>(0.0),
///         vec3<f32>(1.0)
///     );
///
///     return value * mix(vec3<f32>(1.0), rgb, vec3<f32>(saturation));
/// }
/// ```
///
/// All inputs in range (0., 1.)
///
fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> Color {
    let rgb = ((((Vec3::splat(hue * 6.0) + Vec3::new(0.0, 4.0, 2.0)) % 6.0) - 3.0).abs() - 1.0)
        .clamp(Vec3::ZERO, Vec3::ONE);

    let result = value * Vec3::ONE.lerp(rgb, saturation);

    Color::rgb_linear(result.x, result.y, result.z)
}

/// Marker struct for hue wheels
#[derive(Component, Debug, Default, Clone, Copy, Reflect)]
#[reflect(Component, Default)]
pub struct HueWheel;

#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub struct HueWheelMaterial {
    /// The ratio of the inner radius (when the hue wheel cuts off) compared to the
    /// outer radius.
    #[uniform(0)]
    pub inner_radius: f32,
}

impl Default for HueWheelMaterial {
    fn default() -> Self {
        Self { inner_radius: 0.85 }
    }
}

/// Marker struct for saturation-value boxes
#[derive(Component, Debug, Default, Clone, Copy, Reflect)]
#[reflect(Component, Default)]
pub struct SaturationValueBox;

#[derive(Debug, ShaderType, Clone, Default)]
struct SaturationValueUniform {
    hue: f32,
    marker: Vec2,
}

#[derive(AsBindGroup, Asset, TypePath, Debug, Clone, Default)]
pub struct SaturationValueBoxMaterial {
    #[uniform(0)]
    uniform: SaturationValueUniform,
}

impl UiMaterial for HueWheelMaterial {
    fn fragment_shader() -> ShaderRef {
        COLOR_PICKER_HUE_WHEEL_SHADER_HANDLE.into()
    }
}

impl UiMaterial for SaturationValueBoxMaterial {
    fn fragment_shader() -> ShaderRef {
        COLOR_PICKER_SATURATION_VALUE_BOX_SHADER_HANDLE.into()
    }
}
