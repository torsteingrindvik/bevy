//! The checkmate!

// TODO:
// - UVs are not working with clearcoat normal map, investigate
//   - Is there a way to debug view it?
// - Post-processing:
//   - Noise
//   - Solarisation (?)
//   - Color jittering (?)
// - Try using world-space corner positions, display in viewport via gizmos
//  - Gizmos are not it. 2d gizmos are not 2d.
// - "Settled" component: For all things with transforms, mark as settled when not moving for e.g. 3 frames
// - Host online wasm
// - Another render target: Shows the scene from afar such that we can see gizmos for lights etc., maybe orthographic?
// - Macro for creating marker component
// - More HDRIs, perhaps https://github.com/bytestring-net/bevy_skybox_cli
// - Render resolution
// - Some way to make a key of all settings such that we can go from image name to key and recreate images?
// - Intrinsics with distortion model
//  - Can custom projections help?
//  - How do we ensure our perfect information is still correct?
// - Hover zoom thing
// - Axes checkmark
// - Camera distance to checkerboard center text display

// Scratchpad:
//
// Is it possible to use itertools which has a multi cartesian product iterator for planning out all the
// combinations of settings?
//
// Let's say we have things which are able to be interpolated over, like:
// - Color A -> Color B
// - Position A -> Position B
// - Rotation A -> Rotation B
// - Scale A -> Scale B
// - Focal distance A -> Focal distance B
// - PBR settings A -> PBR settings B
//
// and for each of these we define a number of steps, e.g.
// - 5 steps from Color A to Color B
// - 3 steps from Position A to Position B
// and so on.
//
// Then we can generate the cartesian product of all these iterators with the number of steps defined,
// which gives us a list of all combinations of settings to render.
// This way we can plan out a large number of renders with different settings if we have a goal of e.g. 100k renders.
//
// Also we want to be able to plan this out in a menu step-by-step, as well as preview running through it without doing
// any save to disk

#[path = "../helpers/camera_controller.rs"]
mod camera_controller;

use std::iter::zip;
use std::ops::{Deref, DerefMut};

use bevy::anti_alias::fxaa::Fxaa;
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::dev_tools::picking_debug::{DebugPickingMode, DebugPickingPlugin};
use bevy::feathers::controls::{
    button, checkbox, radio, ButtonProps, ButtonVariant, CheckboxProps,
};
use bevy::feathers::theme::ThemedText;
use bevy::math::Affine2;
use bevy::pbr::decal::{ForwardDecal, ForwardDecalMaterial, ForwardDecalMaterialExt};
use bevy::platform::collections::HashMap;
use bevy::post_process::bloom::Bloom;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy::prelude::*;
use bevy::ui::Checked;
use bevy::ui_widgets::{Activate, Callback, RadioGroup, Slider, UiWidgetsPlugins, ValueChange};
use bevy::window::{PresentMode, PrimaryWindow};
use bevy::{
    asset::RenderAssetUsages, color::palettes, core_pipeline::Skybox, mesh::Indices,
    render::render_resource::PrimitiveTopology,
};
use bevy::{
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    text::FontSmoothing,
};
use bevy::{
    feathers::{
        controls::{slider, SliderProps},
        dark_theme::create_dark_theme,
        theme::{ThemeBackgroundColor, UiTheme},
        tokens, FeathersPlugin,
    },
    input_focus::{
        tab_navigation::{TabGroup, TabNavigationPlugin},
        InputDispatchPlugin,
    },
    ui_widgets::{SliderPrecision, SliderValue},
};
use bevy_image::{ImageLoaderSettings, ImageSampler};
use bevy_render::render_resource::TextureFormat;
use bevy_render::view::Hdr;

// use crate::camera_controller::CameraController;
use crate::camera_controller::CameraControllerPlugin;

const UI_TEXT_SMALL: f32 = 12.0;
const UI_TEXT_BIG: f32 = 16.0;

const UI_ROW_GAP_PER_TAB: f32 = 8.0;

#[derive(Component)]
struct ShowAxes;

#[derive(Component)]
struct SliderCheckerboardRows;

#[derive(Component)]
struct SliderCheckerboardCols;

#[derive(Component)]
struct SliderCheckerboardSquareSizeMillimeters;

#[derive(Component)]
struct SliderCameraX;

#[derive(Component)]
struct SliderCameraY;

#[derive(Component)]
struct SliderCameraZ;

#[derive(Component)]
struct SliderMetallic;

#[derive(Component)]
struct SliderRoughness;

#[derive(Component)]
struct SliderClearcoat;

#[derive(Component)]
struct SliderClearcoatRoughness;

#[derive(Component)]
struct SliderSkyboxBrightness;

#[derive(Component)]
struct SliderEnvironmentIntensity;

#[derive(Component)]
struct SliderFocalDistance;

#[derive(Component)]
struct SliderSensorHeight;

#[derive(Component)]
struct SliderFStops;

#[derive(Component)]
struct SliderCameraProjectionFov;

#[derive(Component)]
struct SliderCameraAspectRatioNumerator;

#[derive(Component)]
struct SliderCameraAspectRatioDenominator;

#[derive(Component)]
struct SliderColorR;

#[derive(Component)]
struct SliderColorG;

#[derive(Component)]
struct SliderColorB;

#[derive(Component)]
struct SliderCheckerboardX;

#[derive(Component)]
struct SliderCheckerboardY;

#[derive(Component)]
struct SliderCheckerboardZ;

#[derive(Component)]
struct SliderCheckerboardRotX;

#[derive(Component)]
struct SliderCheckerboardRotY;

#[derive(Component)]
struct SliderCheckerboardRotZ;

#[derive(Component)]
struct CheckboxShowGizmos;

#[derive(Component)]
struct CheckboxShowFpsOverlay;

#[derive(Component)]
struct CheckboxUseVsync;

#[derive(Component)]
struct SliderLightDirectionX;

#[derive(Component)]
struct SliderLightDirectionY;

#[derive(Component)]
struct SliderLightDirectionZ;

#[derive(Component)]
struct SliderLightPointX;

#[derive(Component)]
struct SliderLightPointY;

#[derive(Component)]
struct SliderLightPointZ;

#[derive(Component)]
struct SliderLightDirectionColorR;

#[derive(Component)]
struct SliderLightDirectionColorG;

#[derive(Component)]
struct SliderLightDirectionColorB;

#[derive(Component)]
struct SliderLightPointColorR;

#[derive(Component)]
struct SliderLightPointColorG;

#[derive(Component)]
struct SliderLightPointColorB;

#[derive(Component)]
struct CheckboxUiDebugEnabled;

#[derive(Component)]
struct CheckboxUiDebugShowHidden;

#[derive(Component)]
struct CheckboxUiDebugShowClipped;

#[derive(Component)]
struct RadioPickingDebugDisabled;

#[derive(Component)]
struct RadioPickingDebugNormal;

#[derive(Component)]
struct RadioPickingDebugNoisy;

#[derive(Component)]
struct UiTabNode;

#[derive(Component)]
struct CornerId(usize);

#[derive(Component, Clone, Copy, Hash, PartialEq, Eq, Debug)]
enum UiTabVariant {
    Geometry,
    Material,
    Environment,
    Light,
    Camera,
    Debug,
}

#[derive(Component)]
struct SliderDecalColorR;

#[derive(Component)]
struct SliderDecalColorG;

#[derive(Component)]
struct SliderDecalColorB;

#[derive(Component)]
struct SliderDecalColorA;

#[derive(Component)]
struct SliderDecalScale;

#[derive(Component)]
struct SliderDecalAngle;

#[derive(Component)]
struct RadioPickingDecalTextureFingerprints;

#[derive(Component)]
struct RadioPickingDecalTextureRaindrops;

#[derive(Component)]
struct RadioPickingDecalTextureChewingGum;

#[derive(Component)]
struct CheckboxShowCheckerboardCornerGizmos;

#[derive(Component)]
struct CheckboxShowCheckerboardCornerViewportSpheres; // Instead of in world

#[derive(Debug, Resource)]
struct Decals {
    fingerprints: Handle<Image>,
    raindrops: Handle<Image>,
    chewing_gum: Handle<Image>,
}

#[derive(Debug, Resource)]
struct UnlitViewportSpheresParent {
    parent: Entity,
    mesh: Handle<Mesh>,
}

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Checkmate".into(),
                        present_mode: PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    filter: "bevy_dev_tools=trace".into(), // Show picking logs trace level and up
                    ..default()
                }),
            CameraControllerPlugin,
            DebugPickingPlugin,
            UiWidgetsPlugins,
            InputDispatchPlugin,
            TabNavigationPlugin,
            FeathersPlugin,
            FpsOverlayPlugin {
                config: FpsOverlayConfig {
                    text_config: TextFont {
                        // Here we define size of our overlay
                        font_size: 22.0,
                        // If we want, we can use a custom font
                        font: default(),
                        // We could also disable font smoothing,
                        font_smoothing: FontSmoothing::default(),
                        ..default()
                    },
                    // We can also change color of the overlay
                    text_color: palettes::css::GREEN.into(),
                    // We can also set the refresh interval for the FPS counter
                    refresh_interval: core::time::Duration::from_millis(10),
                    enabled: true,
                    frame_time_graph_config: FrameTimeGraphConfig {
                        enabled: true,
                        // The minimum acceptable fps
                        min_fps: 30.0,
                        // The target fps
                        target_fps: 144.0,
                    },
                },
            },
        ))
        .insert_resource(UiTheme(create_dark_theme()))
        .insert_resource(DebugPickingMode::Normal)
        .add_systems(Startup, setup)
        .add_systems(Update, draw_axes)
        .add_systems(
            Update,
            (
                update_camera_transform_from_sliders,
                update_camera_projection_from_sliders,
                update_square_size_from_slider,
                update_checkerboard_transform_from_sliders,
                update_checkerboard_transform_from_settings,
                update_decal_transform,
                unlit_corner_spheres,
            )
                .chain(),
        )
        .add_systems(Update, update_checkerboard_color_from_sliders)
        .add_systems(Update, update_slider_height)
        .add_systems(Update, update_slider_font_size)
        .add_systems(Update, update_checkerboard_material_from_sliders)
        .add_systems(Update, update_environment_from_sliders)
        .add_systems(Update, update_depth_of_field_from_sliders)
        .add_systems(Update, update_node_visibility_from_ui_tab_variant)
        .add_systems(Update, enable_gizmos)
        .add_systems(Update, enable_fps_overlay)
        .add_systems(Update, enable_vsync)
        .add_systems(Update, update_directional_lights_from_sliders)
        .add_systems(Update, update_point_lights_from_sliders)
        .add_systems(Update, set_ui_debug_options)
        .add_systems(Update, set_picking_debug)
        .add_systems(Update, set_decal_texture)
        .add_systems(Update, update_decal_from_sliders)
        .add_systems(
            Update,
            (
                update_rows_cols_from_sliders,
                generate_checkerboard,
                maintain_corners_as_checkerboard_children,
            )
                .chain(),
        )
        .add_systems(Last, corners_gizmos)
        .run();
}

fn draw_axes(mut gizmos: Gizmos, query: Query<&Transform, With<ShowAxes>>) {
    for &transform in &query {
        gizmos.axes(transform, 1.0);
    }
}

#[derive(Component, Resource, PartialEq, Clone, Copy, Debug)]
struct CheckerboardSettings {
    rows: usize,
    cols: usize,
    /// Metric size of each square
    square_size: f32,
}

fn image_render_target(images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new_target_texture(1920, 1080, TextureFormat::bevy_default());
    image.sampler = ImageSampler::nearest();

    images.add(image)
}

fn camera_depth_of_field() -> impl Bundle {
    DepthOfField {
        mode: DepthOfFieldMode::Bokeh,
        ..default()
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut decal_standard_materials: ResMut<Assets<ForwardDecalMaterial<StandardMaterial>>>,
) {
    let checkerboard = CheckerboardSettings {
        rows: 4,
        cols: 3,
        square_size: 15e-3, // 15mm
    };
    let checkboard_handle: Handle<Mesh> = meshes.add(create_checkerboard(checkerboard));

    commands.insert_resource(checkerboard);

    commands.spawn((
        Mesh3d(checkboard_handle),
        MeshMaterial3d(materials.add(StandardMaterial {
            clearcoat: 1.0,
            clearcoat_perceptual_roughness: 0.3,
            clearcoat_normal_texture: Some(asset_server.load_with_settings(
                "textures/ScratchedGold-Normal.png", // todo debug view this?
                |settings: &mut ImageLoaderSettings| settings.is_srgb = false,
            )),
            metallic: 0.9,
            perceptual_roughness: 0.1,
            ..default()
        })),
        ShowAxes,
        checkerboard,
    ));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_and_light_transform =
        Transform::from_xyz(0.8, 0.8, 0.8).looking_at(Vec3::ZERO, Vec3::Y);

    // let scene_image = image_render_target(&asset_server);
    let scene_image = image_render_target(&mut images);

    // Camera in 3D space.
    commands
        .spawn((
            Camera3d::default(),
            // CameraController::default(),
            Msaa::Off,
            Fxaa::default(), // Supports both decals and WebGPU at the same time
            Hdr,
            Camera {
                clear_color: ClearColorConfig::Custom(palettes::tailwind::PINK_600.into()),
                target: bevy::camera::RenderTarget::Image(scene_image.clone().into()),
                ..default()
            },
            DepthPrepass,
            camera_and_light_transform,
            Tonemapping::AcesFitted,
            Bloom::NATURAL,
            camera_depth_of_field(),
        ))
        .insert(Skybox {
            brightness: 5000.0,
            image: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            ..default()
        })
        .insert(EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            intensity: 2000.0,
            ..default()
        });

    let ui_camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: 1,
                ..Default::default()
            },
            IsDefaultUiCamera,
        ))
        .id();

    let load_settings = |settings: &mut ImageLoaderSettings| {
        settings.sampler.get_or_init_descriptor().address_mode_u =
            bevy_image::ImageAddressMode::Repeat;
        settings.sampler.get_or_init_descriptor().address_mode_v =
            bevy_image::ImageAddressMode::Repeat;
    };

    let decals = Decals {
        fingerprints: asset_server
            .load_with_settings("textures/decals/imperfection.png", load_settings),
        raindrops: asset_server.load_with_settings("textures/decals/raindrops.png", load_settings),
        chewing_gum: asset_server
            .load_with_settings("textures/decals/chewing_gum.png", load_settings),
    };
    let init_decal = decals.fingerprints.clone();

    commands.insert_resource(decals);

    commands.spawn((
        Name::new("Decal"),
        ForwardDecal,
        MeshMaterial3d(decal_standard_materials.add(ForwardDecalMaterial {
            base: StandardMaterial {
                base_color_texture: Some(init_decal),
                alpha_mode: AlphaMode::Blend,
                ..default()
            },
            extension: ForwardDecalMaterialExt {
                depth_fade_factor: 1.0,
            },
        })),
        Transform::from_scale(Vec3::splat(4.0)),
    ));

    commands.spawn((PointLight::default(), camera_and_light_transform));
    commands.spawn((DirectionalLight::default(), camera_and_light_transform));

    let root = root_node(&mut commands, ui_camera, &scene_image);
    commands.spawn(root);

    let unlit_parent = commands
        .spawn((Transform::default(), Visibility::default()))
        .id();

    commands.insert_resource(UnlitViewportSpheresParent {
        parent: unlit_parent,
        mesh: meshes.add(Sphere::new(0.0004)),
    });
}

/// Create a checkerboard mesh with the specified number of rows and columns.
/// Note that rows and cols is not the same as inner corners.
fn create_checkerboard(settings: CheckerboardSettings) -> Mesh {
    let CheckerboardSettings { rows, cols, .. } = settings;
    // Strategy for creating the vertex positions:
    //
    // - Columns increase in the +X direction (right) by one unit per column
    // - Rows increase in the +Z direction (back) by one unit per row
    //
    // Lastly we reposition the whole grid so that it's centered around the origin,
    // followed by scaling it by the desired size.

    // Just to avoid annoying corner cases
    assert!(
        rows >= 2 && cols >= 2,
        "Must have at least 2 rows and 2 columns"
    );

    // Top left (start) is white, unless inverted
    let color = |x, z, invert| {
        let sum_even = (x + z) % 2 == 0;

        match (sum_even, invert) {
            (true, false) | (false, true) => Color::WHITE.to_linear().to_f32_array(),
            (false, false) | (true, true) => Color::BLACK.to_linear().to_f32_array(),
        }
    };

    let mut positions = vec![];
    let mut normals = vec![];
    let mut uvs = vec![];
    let mut colors = vec![];
    let mut indices = vec![];

    for row in 0..rows {
        for col in 0..cols {
            // Each cell is made up of two triangles, which means we need 4 vertices per cell.
            // Add normals and colors for each vertex too.
            for local_row in 0..=1 {
                for local_col in 0..=1 {
                    let x = (col + local_col) as f32;
                    let z = (row + local_row) as f32;

                    positions.push(Vec3::new(x, 0.0, z));
                    normals.push(Vec3::Y);
                    uvs.push([x / cols as f32, z / rows as f32]);
                    colors.push(color(col, row, false));
                }
            }

            // Take the linear vertex index as base- each cell has 4 vertices.
            let base = ((row * cols) + col) * 4;

            // Now add the two triangles for this cell- triangles need to
            // be counter-clockwise when viewed from the front:
            //
            // 0---1
            // |  /|
            // | / | +X
            // |/  |
            // 2---3
            //  +Z
            indices.extend([0, 2, 1, 1, 2, 3].map(|i| (base + i) as u16));
        }
    }

    // Now "post-process" the positions to center them.
    let half = Vec3::new(cols as f32, 0.0, rows as f32) / 2.0;
    let positions = positions.into_iter().map(|p| p - half).collect::<Vec<_>>();

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U16(indices))
}

fn button_selector(clicked: In<Activate>, mut buttons: Query<(Entity, &mut ButtonVariant)>) {
    info!("Clicked! {clicked:?}");
    for (e, mut variant) in &mut buttons {
        if clicked.0 .0 == e {
            *variant = ButtonVariant::Primary;
        } else {
            *variant = ButtonVariant::Normal;
        }
    }
}

fn tabs_node(commands: &mut Commands) -> impl Bundle + use<> {
    let tabs_callback = commands.register_system(button_selector);

    // Tabs
    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Start,
            row_gap: px(8),
            column_gap: px(8),
            ..default()
        },
        children![
            button(
                ButtonProps {
                    on_click: Callback::System(tabs_callback),
                    variant: ButtonVariant::Primary,
                    ..default()
                },
                UiTabVariant::Geometry,
                Spawn((Text::new("Geometry"), ThemedText))
            ),
            button(
                ButtonProps {
                    on_click: Callback::System(tabs_callback),
                    ..default()
                },
                UiTabVariant::Material,
                Spawn((Text::new("Material"), ThemedText))
            ),
            button(
                ButtonProps {
                    on_click: Callback::System(tabs_callback),
                    ..default()
                },
                UiTabVariant::Environment,
                Spawn((Text::new("Environment"), ThemedText))
            ),
            button(
                ButtonProps {
                    on_click: Callback::System(tabs_callback),
                    ..default()
                },
                UiTabVariant::Light,
                Spawn((Text::new("Light"), ThemedText))
            ),
            button(
                ButtonProps {
                    on_click: Callback::System(tabs_callback),
                    ..default()
                },
                UiTabVariant::Camera,
                Spawn((Text::new("Camera"), ThemedText))
            ),
            button(
                ButtonProps {
                    on_click: Callback::System(tabs_callback),
                    ..default()
                },
                UiTabVariant::Debug,
                Spawn((Text::new("Debug"), ThemedText))
            ),
        ],
    )
}

fn geometry_node() -> impl Bundle {
    // Checkerboard settings node
    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(UI_ROW_GAP_PER_TAB),
            ..default()
        },
        UiTabVariant::Geometry,
        UiTabNode,
        children![
            (
                Text("Geometry".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            (
                Text("Rows".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 2.0,
                    value: 11.0,
                    max: 20.0,
                    ..default()
                },
                (SliderPrecision(0), SliderCheckerboardRows),
            ),
            (
                Text("Cols".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 2.0,
                    value: 10.0,
                    max: 20.0,
                    ..default()
                },
                (SliderPrecision(0), SliderCheckerboardCols),
            ),
            (
                Text("Square Size (mm)".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 5.0,
                    value: 78.0,
                    max: 100.0,
                    ..default()
                },
                (SliderPrecision(0), SliderCheckerboardSquareSizeMillimeters),
            ),
            (
                Text("Translation".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(4.),
                    ..default()
                },
                children![
                    (
                        Text("X".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: -2.0,
                            value: 0.0,
                            max: 2.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderCheckerboardX),
                    ),
                    (
                        Text("Y".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: -2.0,
                            value: 0.0,
                            max: 2.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderCheckerboardY),
                    ),
                    (
                        Text("Z".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: -2.0,
                            value: 0.0,
                            max: 2.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderCheckerboardZ),
                    ),
                ]
            ),
            (
                Text("Rotation".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            (
                Text("about world X/Y/Z (degrees)".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(4.),
                    ..default()
                },
                children![
                    (
                        Text("X".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: -90.0,
                            value: 0.0,
                            max: 90.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderCheckerboardRotX),
                    ),
                    (
                        Text("Y".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: -90.0,
                            value: 0.0,
                            max: 90.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderCheckerboardRotY),
                    ),
                    (
                        Text("Z".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: -90.0,
                            value: 0.0,
                            max: 90.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderCheckerboardRotZ),
                    )
                ]
            )
        ],
    )
}

fn material_node(commands: &mut Commands) -> impl Bundle + use<> {
    #[derive(Component)]
    struct LocalRadio;

    let radio_check = commands.register_system(
        |ent: In<Activate>, q_radio: Query<Entity, With<LocalRadio>>, mut commands: Commands| {
            for radio in q_radio.iter() {
                if radio == ent.0 .0 {
                    commands.entity(radio).insert(Checked);
                } else {
                    commands.entity(radio).remove::<Checked>();
                }
            }
        },
    );

    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(UI_ROW_GAP_PER_TAB),
            ..default()
        },
        UiTabVariant::Material,
        UiTabNode,
        children![
            (
                Text("PBR".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            (
                Text("Metallic".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 0.0,
                    value: 0.9,
                    max: 1.0,
                    ..default()
                },
                (SliderPrecision(2), SliderMetallic),
            ),
            (
                Text("Roughness".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 0.0,
                    value: 0.1,
                    max: 1.0,
                    ..default()
                },
                (SliderPrecision(2), SliderRoughness),
            ),
            (
                Text("Clearcoat".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 0.0,
                    value: 1.0,
                    max: 1.0,
                    ..default()
                },
                (SliderPrecision(2), SliderClearcoat),
            ),
            (
                Text("Clearcoat Roughness".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 0.0,
                    value: 0.5,
                    max: 1.0,
                    ..default()
                },
                (SliderPrecision(2), SliderClearcoatRoughness),
            ),
            (
                Text("Color".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(4.),
                    ..default()
                },
                children![
                    (
                        Text("R".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.88,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderColorR),
                    ),
                    (
                        Text("G".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.89,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderColorG),
                    ),
                    (
                        Text("B".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.91,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderColorB),
                    ),
                ]
            ),
            // Decal
            (
                Text("Decal".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            (
                Text("Texture".to_owned()),
                TextLayout::new_with_justify(Justify::Left),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            // Texture
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    column_gap: px(4),
                    ..default()
                },
                RadioGroup {
                    on_change: Callback::System(radio_check),
                },
                children![
                    radio(
                        (Checked, LocalRadio, RadioPickingDecalTextureFingerprints),
                        Spawn((Text::new("Fingerprints"), ThemedText))
                    ),
                    radio(
                        (LocalRadio, RadioPickingDecalTextureRaindrops),
                        Spawn((Text::new("Raindrops"), ThemedText))
                    ),
                    radio(
                        (LocalRadio, RadioPickingDecalTextureChewingGum),
                        Spawn((Text::new("Chewing Gum"), ThemedText))
                    ),
                ]
            ),
            (
                Text("Color".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(4.),
                    ..default()
                },
                children![
                    (
                        Text("R".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.28,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderDecalColorR),
                    ),
                    (
                        Text("G".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.23,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderDecalColorG),
                    ),
                    (
                        Text("B".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.11,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderDecalColorB),
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.57,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderDecalColorA),
                    ),
                ]
            ),
            (
                Text("Scale".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 0.1,
                    value: 1.86,
                    max: 10.0,
                    ..default()
                },
                (SliderPrecision(2), SliderDecalScale),
            ),
            (
                Text("Rotation (degrees)".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 0.0,
                    value: 86.0,
                    max: 360.0,
                    ..default()
                },
                (SliderPrecision(1), SliderDecalAngle),
            )
        ],
    )
}

fn environment_node() -> impl Bundle {
    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(UI_ROW_GAP_PER_TAB),
            ..default()
        },
        UiTabVariant::Environment,
        UiTabNode,
        children![
            (
                Text("Environment".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            (
                Text("Skybox Brightness".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 0.0,
                    value: 7000.0,
                    max: 10000.0,
                    ..default()
                },
                (SliderPrecision(-3), SliderSkyboxBrightness),
            ),
            (
                Text("Environment Intensity".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 0.0,
                    value: 300.0,
                    max: 10000.0,
                    ..default()
                },
                (SliderPrecision(-2), SliderEnvironmentIntensity),
            ),
        ],
    )
}

fn light_node() -> impl Bundle {
    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(UI_ROW_GAP_PER_TAB),
            ..default()
        },
        UiTabVariant::Light,
        UiTabNode,
        children![
            (
                Text("Light".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            (
                Text("Direction".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            // Direction XYZ
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(4.),
                    ..default()
                },
                children![
                    (
                        Text("X".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightDirectionX),
                    ),
                    (
                        Text("Y".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.1,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightDirectionY),
                    ),
                    (
                        Text("Z".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightDirectionZ),
                    ),
                ]
            ),
            // Direction RGB
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(4.),
                    ..default()
                },
                children![
                    (
                        Text("R".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightDirectionColorR),
                    ),
                    (
                        Text("G".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.1,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightDirectionColorG),
                    ),
                    (
                        Text("B".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightDirectionColorB),
                    ),
                ]
            ),
            (
                Text("Point".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(4.),
                    ..default()
                },
                children![
                    (
                        Text("X".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightPointX),
                    ),
                    (
                        Text("Y".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.1,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightPointY),
                    ),
                    (
                        Text("Z".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightPointZ),
                    ),
                ]
            ),
            // Point RGB
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(4.),
                    ..default()
                },
                children![
                    (
                        Text("R".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightPointColorR),
                    ),
                    (
                        Text("G".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.1,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightPointColorG),
                    ),
                    (
                        Text("B".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.8,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderLightPointColorB),
                    ),
                ]
            ),
        ],
    )
}

fn camera_node(commands: &mut Commands) -> impl Bundle + use<> {
    let checkies = commands.register_system(
        |change: In<ValueChange<bool>>,
         camera: Single<Entity, (With<Camera>, With<Camera3d>)>,
         mut commands: Commands| {
            info!("Depth of field to {}", change.value);

            let checkbox = change.source;

            if change.value {
                commands.entity(checkbox).insert(Checked);
                commands.entity(*camera).insert(camera_depth_of_field());
            } else {
                commands.entity(checkbox).remove::<Checked>();
                commands.entity(*camera).remove::<DepthOfField>();
            }
        },
    );

    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(UI_ROW_GAP_PER_TAB),
            ..default()
        },
        UiTabVariant::Camera,
        UiTabNode,
        children![
            (
                Text("Position".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(4.),
                    ..default()
                },
                children![
                    (
                        Text("X".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.43,
                            max: 3.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderCameraX),
                    ),
                    (
                        Text("Y".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.1,
                            value: 0.81,
                            max: 3.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderCameraY),
                    ),
                    (
                        Text("Z".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 0.01,
                            max: 3.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderCameraZ),
                    ),
                ]
            ),
            // Projection
            (
                Text("Projection".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            (
                Text("Field of View (degrees)".to_owned()),
                TextFont::from_font_size(UI_TEXT_SMALL)
            ),
            slider(
                SliderProps {
                    min: 10.0,
                    value: 45.0,
                    max: 135.0,
                    ..default()
                },
                (SliderPrecision(1), SliderCameraProjectionFov),
            ),
            // Aspect ratio
            (
                Text("Aspect Ratio".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    row_gap: px(UI_ROW_GAP_PER_TAB),
                    ..default()
                },
                children![
                    slider(
                        SliderProps {
                            min: 1.0,
                            value: 16.0,
                            max: 30.0,
                            ..default()
                        },
                        (SliderPrecision(0), SliderCameraAspectRatioNumerator),
                    ),
                    (
                        Text(":".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 1.0,
                            value: 9.0,
                            max: 30.0,
                            ..default()
                        },
                        (SliderPrecision(0), SliderCameraAspectRatioDenominator),
                    ),
                ],
            ),
            // DoF
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    row_gap: px(UI_ROW_GAP_PER_TAB),
                    ..default()
                },
                children![
                    (
                        Text("Depth of Field".to_owned()),
                        TextLayout::new_with_justify(Justify::Center),
                        TextFont::from_font_size(UI_TEXT_BIG)
                    ),
                    checkbox(
                        CheckboxProps {
                            on_change: Callback::System(checkies),
                        },
                        (Checked, CheckboxShowGizmos),
                        Spawn((Text::new("Enabled"), ThemedText))
                    ),
                    // Focal distance node
                    (
                        Text("Focal Distance".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.3,
                            value: 0.61,
                            max: 10.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderFocalDistance),
                    ),
                    // Sensor height node
                    (
                        Text("Sensor Height (mm)".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 5.0,
                            value: 10.18,
                            max: 50.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderSensorHeight),
                    ),
                    // F-stops node
                    (
                        Text("F-stops".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.1,
                            value: 2.5,
                            max: 3.0,
                            ..default()
                        },
                        (SliderPrecision(1), SliderFStops),
                    ),
                ],
            ),
        ],
    )
}

fn debug_node(commands: &mut Commands) -> impl Bundle {
    #[derive(Component)]
    struct LocalRadio;

    let radio_check = commands.register_system(
        |ent: In<Activate>, q_radio: Query<Entity, With<LocalRadio>>, mut commands: Commands| {
            for radio in q_radio.iter() {
                if radio == ent.0 .0 {
                    commands.entity(radio).insert(Checked);
                } else {
                    commands.entity(radio).remove::<Checked>();
                }
            }
        },
    );

    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(UI_ROW_GAP_PER_TAB),
            ..default()
        },
        UiTabVariant::Debug,
        UiTabNode,
        children![
            (
                Text("Debug".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            checkbox(
                CheckboxProps {
                    on_change: Callback::Ignore,
                },
                (Checked, CheckboxShowGizmos),
                Spawn((Text::new("Gizmos enabled"), ThemedText))
            ),
            checkbox(
                CheckboxProps {
                    on_change: Callback::Ignore,
                },
                CheckboxShowCheckerboardCornerGizmos,
                Spawn((Text::new("Checkerboard corner world gizmos"), ThemedText))
            ),
            checkbox(
                CheckboxProps {
                    on_change: Callback::Ignore,
                },
                CheckboxShowCheckerboardCornerViewportSpheres,
                Spawn((
                    Text::new("Checkerboard corner viewport spheres"),
                    ThemedText
                ))
            ),
            checkbox(
                CheckboxProps {
                    on_change: Callback::Ignore,
                },
                CheckboxShowFpsOverlay,
                Spawn((Text::new("Show FPS"), ThemedText))
            ),
            checkbox(
                CheckboxProps {
                    on_change: Callback::Ignore,
                },
                (Checked, CheckboxUseVsync),
                Spawn((Text::new("Vsync"), ThemedText))
            ),
            // Picking
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    column_gap: px(4),
                    ..default()
                },
                RadioGroup {
                    on_change: Callback::System(radio_check),
                },
                children![
                    (
                        Text("Picking".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    radio(
                        (Checked, RadioPickingDebugDisabled, LocalRadio),
                        Spawn((Text::new("Disabled"), ThemedText))
                    ),
                    radio(
                        (RadioPickingDebugNormal, LocalRadio),
                        Spawn((Text::new("Normal"), ThemedText))
                    ),
                    radio(
                        (RadioPickingDebugNoisy, LocalRadio),
                        Spawn((Text::new("Noisy"), ThemedText))
                    ),
                ]
            ),
            // UI debug
            (
                Text("UI debug".to_owned()),
                TextLayout::new_with_justify(Justify::Center),
                TextFont::from_font_size(UI_TEXT_BIG)
            ),
            checkbox(
                CheckboxProps {
                    on_change: Callback::Ignore,
                },
                CheckboxUiDebugEnabled,
                Spawn((Text::new("Enabled"), ThemedText))
            ),
            checkbox(
                CheckboxProps {
                    on_change: Callback::Ignore,
                },
                CheckboxUiDebugShowHidden,
                Spawn((Text::new("Show hidden"), ThemedText))
            ),
            checkbox(
                CheckboxProps {
                    on_change: Callback::Ignore,
                },
                CheckboxUiDebugShowClipped,
                Spawn((Text::new("Show clipped"), ThemedText))
            )
        ],
    )
}

fn root_node(
    commands: &mut Commands,
    camera_entity: Entity,
    scene_image: &Handle<Image>,
) -> impl Bundle {
    let tabs = tabs_node(commands);
    let geometry = geometry_node();
    let material = material_node(commands);
    let environment = environment_node();
    let light = light_node();
    let camera = camera_node(commands);
    let debug = debug_node(commands);

    (
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            ..default()
        },
        UiTargetCamera(camera_entity),
        TabGroup::default(),
        ThemeBackgroundColor(tokens::WINDOW_BG),
        children![
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    justify_content: JustifyContent::Start,
                    padding: UiRect::all(px(8)),
                    row_gap: px(16),
                    width: auto(),
                    max_width: percent(40),
                    ..default()
                },
                children![tabs, geometry, material, environment, light, camera, debug]
            ),
            (
                ImageNode::new(scene_image.clone()),
                Node {
                    width: percent(70),
                    ..default()
                },
            )
        ],
    )
}

fn update_rows_cols_from_sliders(
    mut settings: ResMut<CheckerboardSettings>,
    slider_rows: Single<&SliderValue, With<SliderCheckerboardRows>>,
    slider_cols: Single<&SliderValue, With<SliderCheckerboardCols>>,
) {
    if settings.rows != slider_rows.0 as usize {
        settings.rows = slider_rows.0 as _;
    }

    if settings.cols != slider_cols.0 as usize {
        settings.cols = slider_cols.0 as _;
    }
}

fn update_square_size_from_slider(
    mut settings: ResMut<CheckerboardSettings>,
    slider: Single<&SliderValue, With<SliderCheckerboardSquareSizeMillimeters>>,
) {
    // Convert from mm to meters
    let slider_size = slider.0 as f32 * 1e-3;

    if settings.square_size != slider_size {
        settings.square_size = slider_size;
    }
}

fn update_checkerboard_transform_from_settings(
    settings: Res<CheckerboardSettings>,
    mut checkerboard: Single<&mut Transform, With<CheckerboardSettings>>,
) {
    checkerboard.scale = Vec3::splat(settings.square_size);
}

fn update_checkerboard_transform_from_sliders(
    slider_x: Single<&SliderValue, With<SliderCheckerboardX>>,
    slider_y: Single<&SliderValue, With<SliderCheckerboardY>>,
    slider_z: Single<&SliderValue, With<SliderCheckerboardZ>>,

    slider_rot_x: Single<&SliderValue, With<SliderCheckerboardRotX>>,
    slider_rot_y: Single<&SliderValue, With<SliderCheckerboardRotY>>,
    slider_rot_z: Single<&SliderValue, With<SliderCheckerboardRotZ>>,

    mut checkerboard: Single<&mut Transform, With<CheckerboardSettings>>,
) {
    checkerboard.translation = Vec3::new(slider_x.0, slider_y.0, slider_z.0);
    checkerboard.rotation = Quat::from_euler(
        EulerRot::XYZEx,
        slider_rot_x.0.to_radians(),
        slider_rot_y.0.to_radians(),
        slider_rot_z.0.to_radians(),
    );
}

fn generate_checkerboard(
    settings: Res<CheckerboardSettings>,
    mut checkerboards: Query<(&mut Mesh3d, &mut CheckerboardSettings)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if settings.is_changed() {
        for (mut mesh, mut mesh_settings) in &mut checkerboards {
            if *settings == *mesh_settings {
                // don't recreate mesh if settings are the same
                continue;
            }

            info!("changed to {:?}", *settings);

            **mesh = meshes.add(create_checkerboard(*settings));
            *mesh_settings = *settings;
        }
    }
}

fn update_checkerboard_color_from_sliders(
    slider_r: Single<&SliderValue, With<SliderColorR>>,
    slider_g: Single<&SliderValue, With<SliderColorG>>,
    slider_b: Single<&SliderValue, With<SliderColorB>>,
    checkerboard: Single<&MeshMaterial3d<StandardMaterial>, With<CheckerboardSettings>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let color = Color::srgb_from_array([slider_r.0, slider_g.0, slider_b.0]);

    let handle = checkerboard.0.clone();

    if let Some(material) = materials.get_mut(&handle) {
        material.base_color = color;
    }
}

fn update_camera_transform_from_sliders(
    slider_x: Single<&SliderValue, With<SliderCameraX>>,
    slider_y: Single<&SliderValue, With<SliderCameraY>>,
    slider_z: Single<&SliderValue, With<SliderCameraZ>>,
    mut camera_transform: Single<&mut Transform, With<Camera3d>>,
) {
    camera_transform.translation = Vec3::new(slider_x.0, slider_y.0, slider_z.0);
    camera_transform.look_at(Vec3::ZERO, Vec3::Y);
}

fn update_camera_projection_from_sliders(
    slider_fov: Single<&SliderValue, With<SliderCameraProjectionFov>>,
    slider_aspect_ratio_numerator: Single<&SliderValue, With<SliderCameraAspectRatioNumerator>>,
    slider_aspect_ratio_denominator: Single<&SliderValue, With<SliderCameraAspectRatioDenominator>>,
    mut camera_transform: Single<&mut Projection, With<Camera3d>>,
) {
    let perspective = match camera_transform.as_mut() {
        Projection::Perspective(p) => p,
        _ => {
            unimplemented!();
        }
    };

    perspective.fov = slider_fov.0.to_radians();
    perspective.aspect_ratio = slider_aspect_ratio_numerator.0 / slider_aspect_ratio_denominator.0;
}

fn update_slider_height(mut sliders: Query<&mut Node, With<Slider>>) {
    for mut node in &mut sliders {
        node.height = px(12);
    }
}

fn update_slider_font_size(
    mut q_sliders: Query<Entity, With<Slider>>,
    q_children: Query<&Children>,
    mut q_slider_text: Query<&mut TextFont>,
) {
    for slider_ent in q_sliders.iter_mut() {
        q_children.iter_descendants(slider_ent).for_each(|child| {
            if let Ok(mut text) = q_slider_text.get_mut(child) {
                text.font_size = UI_TEXT_SMALL;
            }
        });
    }
}

fn update_checkerboard_material_from_sliders(
    metallic: Single<&SliderValue, With<SliderMetallic>>,
    roughness: Single<&SliderValue, With<SliderRoughness>>,
    clearcoat: Single<&SliderValue, With<SliderClearcoat>>,
    clearcoat_roughness: Single<&SliderValue, With<SliderClearcoatRoughness>>,
    checkerboard: Single<&mut MeshMaterial3d<StandardMaterial>, With<CheckerboardSettings>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let handle = checkerboard.0.clone();

    let material = materials.get_mut(&handle).unwrap();

    material.metallic = metallic.0;
    material.perceptual_roughness = roughness.0;
    material.clearcoat = clearcoat.0;
    material.clearcoat_perceptual_roughness = clearcoat_roughness.0;
}

fn update_environment_from_sliders(
    slider_skybox_brightness: Single<&SliderValue, With<SliderSkyboxBrightness>>,
    slider_environment_intensity: Single<&SliderValue, With<SliderEnvironmentIntensity>>,
    mut q_skybox: Query<&mut Skybox>,
    mut q_envmap: Query<&mut EnvironmentMapLight>,
) {
    for mut skybox in &mut q_skybox {
        skybox.brightness = slider_skybox_brightness.0;
    }

    for mut envmap in &mut q_envmap {
        envmap.intensity = slider_environment_intensity.0;
    }
}

fn update_depth_of_field_from_sliders(
    slider_focal_distance: Single<&SliderValue, With<SliderFocalDistance>>,
    slider_sensor_height: Single<&SliderValue, With<SliderSensorHeight>>,
    slider_f_stops: Single<&SliderValue, With<SliderFStops>>,
    mut dof: Single<&mut DepthOfField>,
) {
    dof.focal_distance = slider_focal_distance.0;
    dof.sensor_height = slider_sensor_height.0 * 1e-3; // mm to meters
    dof.aperture_f_stops = slider_f_stops.0;
}

fn update_node_visibility_from_ui_tab_variant(
    buttons: Query<(&ButtonVariant, &UiTabVariant)>,
    mut query: Query<(&UiTabVariant, &mut Node), With<UiTabNode>>,
) {
    let tab_variant_to_visibility = buttons
        .iter()
        .map(|(button_variant, tab_variant)| {
            (
                *tab_variant,
                if button_variant == &ButtonVariant::Primary {
                    Display::Flex
                } else {
                    Display::None
                },
            )
        })
        .collect::<HashMap<_, _>>();

    for (tab_variant, mut node) in &mut query {
        if let Some(display) = tab_variant_to_visibility.get(tab_variant) {
            node.display = *display;
        }
    }
}

fn maintain_corners_as_checkerboard_children(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    unlit_parent: Res<UnlitViewportSpheresParent>,
    checkerboard_res: Res<CheckerboardSettings>,
    checkerboards: Query<(Entity, &CheckerboardSettings)>,
) {
    if checkerboard_res.is_changed() {
        for (checkerboard, settings) in &checkerboards {
            let CheckerboardSettings { rows, cols, .. } = *settings;

            info!("making new corner position kids for {checkerboard}, settings: {rows}x{cols} vs {:?}", checkerboard_res.deref());

            // Start over
            commands.entity(checkerboard).despawn_children();
            commands.entity(unlit_parent.parent).despawn_children();

            let mut checkerboard_children = vec![];
            let mut unlit_children = vec![];

            let half = Vec3::new(cols as f32, 0.0, rows as f32) / 2.0;

            let green = palettes::basic::GREEN;
            let red = palettes::basic::RED;

            let num_corners = (rows - 1) * (cols - 1);

            let mut id = 0;
            for row in 1..rows {
                for col in 1..cols {
                    let position = Vec3::new(col as f32, 0.0, row as f32) - half;

                    checkerboard_children.push(
                        commands
                            .spawn((Transform::from_translation(position), CornerId(id)))
                            .id(),
                    );

                    let color = green.mix(&red, id as f32 / num_corners as f32);

                    unlit_children.push(
                        commands
                            .spawn((
                                Transform::default(),
                                CornerId(id),
                                Mesh3d(unlit_parent.mesh.clone()),
                                MeshMaterial3d(materials.add(StandardMaterial {
                                    base_color: color.into(),
                                    unlit: true,
                                    cull_mode: None,
                                    depth_bias: 1.0,
                                    double_sided: true,
                                    ..default()
                                })),
                            ))
                            .id(),
                    );

                    id += 1;
                }
            }

            commands
                .entity(checkerboard)
                .add_children(&checkerboard_children);

            commands
                .entity(unlit_parent.parent)
                .add_children(&unlit_children);
        }
    }
}

fn corners_gizmos(
    mut gizmos: Gizmos,
    enabled: Option<Single<&CheckboxShowCheckerboardCornerGizmos, With<Checked>>>,
    checkerboards: Query<(&Children, &Transform), With<CheckerboardSettings>>,
    corners: Query<(&GlobalTransform, &CornerId)>,
) {
    if enabled.is_none() {
        return;
    }

    for (checkerboard_children, checkerboard_transform) in &checkerboards {
        let num_corners = checkerboard_children.len();

        let green = palettes::basic::GREEN;
        let red = palettes::basic::RED;

        for child in checkerboard_children.iter() {
            if let Ok((corner_transform, corner_id)) = corners.get(child) {
                let color = green.mix(&red, corner_id.0 as f32 / num_corners as f32);
                gizmos.sphere(
                    corner_transform.translation(),
                    checkerboard_transform.scale[0] / 10.,
                    color,
                );
            }
        }
    }
}

fn enable_gizmos(
    mut gizmos: ResMut<GizmoConfigStore>,
    // Oh this is very broken, the "false" here is so wrong?
    show_gizmos: Option<Single<&CheckboxShowGizmos, With<Checked>>>,
) {
    let (config, _) = gizmos.config_mut::<DefaultGizmoConfigGroup>();
    config.enabled = show_gizmos.is_some();
}

fn enable_fps_overlay(
    mut config: ResMut<FpsOverlayConfig>,
    show_fps: Option<Single<&CheckboxShowFpsOverlay, With<Checked>>>,
) {
    config.enabled = show_fps.is_some();
    config.frame_time_graph_config.enabled = show_fps.is_some();
}

fn enable_vsync(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    use_vsync: Option<Single<&CheckboxUseVsync, With<Checked>>>,
) {
    window.present_mode = if use_vsync.is_some() {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    }
}

fn update_point_lights_from_sliders(
    slider_point_x: Single<&SliderValue, With<SliderLightPointX>>,
    slider_point_y: Single<&SliderValue, With<SliderLightPointY>>,
    slider_point_z: Single<&SliderValue, With<SliderLightPointZ>>,

    slider_point_r: Single<&SliderValue, With<SliderLightPointColorR>>,
    slider_point_g: Single<&SliderValue, With<SliderLightPointColorG>>,
    slider_point_b: Single<&SliderValue, With<SliderLightPointColorB>>,

    mut q_point_light: Query<(&mut Transform, &mut PointLight)>,
) {
    for (mut transform, mut point_light) in &mut q_point_light {
        transform.translation = Vec3::new(slider_point_x.0, slider_point_y.0, slider_point_z.0);

        point_light.color =
            Color::srgb_from_array([slider_point_r.0, slider_point_g.0, slider_point_b.0]);
    }
}

fn update_directional_lights_from_sliders(
    slider_dir_x: Single<&SliderValue, With<SliderLightDirectionX>>,
    slider_dir_y: Single<&SliderValue, With<SliderLightDirectionY>>,
    slider_dir_z: Single<&SliderValue, With<SliderLightDirectionZ>>,

    slider_dir_r: Single<&SliderValue, With<SliderLightDirectionColorR>>,
    slider_dir_g: Single<&SliderValue, With<SliderLightDirectionColorG>>,
    slider_dir_b: Single<&SliderValue, With<SliderLightDirectionColorB>>,

    mut q_directional_light: Query<(&mut Transform, &mut DirectionalLight)>,
) {
    for (mut transform, mut dir_light) in &mut q_directional_light {
        let direction =
            Vec3::new(slider_dir_x.0, slider_dir_y.0, slider_dir_z.0).normalize_or_zero();
        transform.rotation = Quat::from_rotation_arc(Vec3::Y, direction);

        dir_light.color = Color::srgb_from_array([slider_dir_r.0, slider_dir_g.0, slider_dir_b.0]);
    }
}

fn set_ui_debug_options(
    mut opts: ResMut<UiDebugOptions>,
    checkbox_ui_debug_enabled: Option<Single<&CheckboxUiDebugEnabled, With<Checked>>>,
    checkbox_ui_debug_show_hidden: Option<Single<&CheckboxUiDebugShowHidden, With<Checked>>>,
    checkbox_ui_debug_show_clipped: Option<Single<&CheckboxUiDebugShowClipped, With<Checked>>>,
) {
    opts.enabled = checkbox_ui_debug_enabled.is_some();
    opts.show_clipped = checkbox_ui_debug_show_clipped.is_some();
    opts.show_hidden = checkbox_ui_debug_show_hidden.is_some();
}

fn set_picking_debug(
    mut mode: ResMut<DebugPickingMode>,
    radio_disabled: Option<Single<&RadioPickingDebugDisabled, With<Checked>>>,
    radio_normal: Option<Single<&RadioPickingDebugNormal, With<Checked>>>,
    radio_noisy: Option<Single<&RadioPickingDebugNoisy, With<Checked>>>,
) {
    if radio_disabled.is_some() {
        *mode = DebugPickingMode::Disabled;
    } else if radio_normal.is_some() {
        *mode = DebugPickingMode::Normal;
    } else if radio_noisy.is_some() {
        *mode = DebugPickingMode::Noisy;
    }
}

fn update_decal_from_sliders(
    slider_r: Single<&SliderValue, With<SliderDecalColorR>>,
    slider_g: Single<&SliderValue, With<SliderDecalColorG>>,
    slider_b: Single<&SliderValue, With<SliderDecalColorB>>,
    slider_a: Single<&SliderValue, With<SliderDecalColorA>>,
    slider_scale: Single<&SliderValue, With<SliderDecalScale>>,
    slider_angle: Single<&SliderValue, With<SliderDecalAngle>>,
    mut decal: Single<
        (
            &mut Transform,
            &MeshMaterial3d<ForwardDecalMaterial<StandardMaterial>>,
        ),
        With<ForwardDecal>,
    >,
    mut materials: ResMut<Assets<ForwardDecalMaterial<StandardMaterial>>>,
) {
    let color = Color::srgb_from_array([slider_r.0, slider_g.0, slider_b.0]).with_alpha(slider_a.0);

    let (_transform, material_handle) = decal.deref_mut();

    if let Some(material) = materials.get_mut(&material_handle.clone()) {
        material.base.base_color = color;
        material.base.uv_transform = Affine2::from_scale_angle_translation(
            Vec2::splat(slider_scale.0),
            slider_angle.0.to_radians(),
            Vec2::ONE,
        )
    }
}

// Make the decal always cover the entire checkerboard
fn update_decal_transform(
    mut decal: Single<&mut Transform, (With<ForwardDecal>, Without<CheckerboardSettings>)>,
    checkerboard: Single<&Transform, With<CheckerboardSettings>>,
    settings: Res<CheckerboardSettings>,
) {
    **decal = Transform {
        scale: Vec3::new(
            settings.square_size * settings.cols as f32,
            1.0,
            settings.square_size * settings.rows as f32,
        ),
        ..**checkerboard
    };
}

fn set_decal_texture(
    radio_fingerprints: Option<Single<&RadioPickingDecalTextureFingerprints, With<Checked>>>,
    radio_raindrops: Option<Single<&RadioPickingDecalTextureRaindrops, With<Checked>>>,
    radio_chewing_gum: Option<Single<&RadioPickingDecalTextureChewingGum, With<Checked>>>,
    decal: Single<&MeshMaterial3d<ForwardDecalMaterial<StandardMaterial>>, With<ForwardDecal>>,
    decals: Res<Decals>,
    mut materials: ResMut<Assets<ForwardDecalMaterial<StandardMaterial>>>,
) {
    if let Some(material) = materials.get_mut(&decal.clone()) {
        if radio_fingerprints.is_some() {
            material.base.base_color_texture = Some(decals.fingerprints.clone());
        } else if radio_raindrops.is_some() {
            material.base.base_color_texture = Some(decals.raindrops.clone());
        } else if radio_chewing_gum.is_some() {
            material.base.base_color_texture = Some(decals.chewing_gum.clone());
        }
    }
}

fn unlit_corner_spheres(
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    enabled: Option<Single<&CheckboxShowCheckerboardCornerViewportSpheres, With<Checked>>>,
    checkerboards: Query<&Children, With<CheckerboardSettings>>,
    mut corners: Query<(&GlobalTransform, &CornerId), Without<Gizmo>>,
    mut unlits: Query<(&mut Transform, &mut Visibility, &CornerId), With<Mesh3d>>,
) {
    for checkerboard_children in &checkerboards {
        let num_corners = checkerboard_children.len();

        let green = palettes::basic::GREEN;
        let red = palettes::basic::RED;

        for (child, (mut unlit_transform, mut unlit_visibility, _gizmo_id)) in
            zip(checkerboard_children.iter(), unlits.iter_mut())
        {
            *unlit_visibility = if enabled.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };

            if let Ok((corner_global_transform, corner_id)) = corners.get_mut(child) {
                let _color = green.mix(&red, corner_id.0 as f32 / num_corners as f32);

                let (camera, camera_global_transform) = *camera;

                let corner_world_pos = corner_global_transform.translation();

                let Ok(viewport_coordinate) =
                    camera.world_to_viewport(camera_global_transform, corner_world_pos)
                else {
                    continue;
                };

                let ray_at_near_plane = camera
                    .viewport_to_world(camera_global_transform, viewport_coordinate)
                    .expect("unsure?")
                    .origin;

                let Transform {
                    translation,
                    rotation,
                    scale: _,
                } = Transform::from_translation(ray_at_near_plane).looking_to(
                    camera_global_transform.forward(),
                    camera_global_transform.up(),
                );

                let dist = ray_at_near_plane.distance(camera_global_transform.translation());

                unlit_transform.translation = translation;
                unlit_transform.rotation = rotation;
                unlit_transform.scale = Vec3::splat(0.15 / dist);
            }
        }
    }
}
