//! The checkmate!

#![allow(missing_docs)]

// TODO:
// - UVs are not working with clearcoat normal map, investigate
//   - Is there a way to debug view it?
// - Post-processing:
//   - Noise
//   - Solarisation (?)
//   - Color jittering (?)
// - "Settled" component: For all things with transforms, mark as settled when not moving for e.g. 3 frames
// - Host online wasm
// - Another render target: Shows the scene from afar such that we can see gizmos for lights etc., maybe orthographic?
// - Macro for creating marker component
// - More HDRIs, perhaps https://github.com/bytestring-net/bevy_skybox_cli
// - Some way to make a key of all settings such that we can go from image name to key and recreate images?
// - Intrinsics with distortion model
//  - Can custom projections help?
//  - How do we ensure our perfect information is still correct?
// - Decal scale aspect ratio independent of checkerboard aspect ratio
// - CI to publish to webpage
// - RGB use actual feathers color widgets
//   - There's no actual 2D picker

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

use std::iter::zip;

use bevy::anti_alias::fxaa::Fxaa;
use bevy::camera::RenderTarget;
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::dev_tools::picking_debug::{DebugPickingMode, DebugPickingPlugin};
use bevy::feathers::controls::{button, checkbox, radio, ButtonProps, ButtonVariant};
use bevy::feathers::theme::ThemedText;
use bevy::input::common_conditions::input_just_pressed;
use bevy::math::Affine2;
use bevy::pbr::decal::{ForwardDecal, ForwardDecalMaterial, ForwardDecalMaterialExt};
use bevy::pbr::ExtendedMaterial;
use bevy::platform::collections::HashMap;
use bevy::post_process::bloom::Bloom;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy::prelude::*;
use bevy::ui::widget::ImageNodeSize;
use bevy::ui::{Checkable, Checked};
use bevy::ui_widgets::{
    observe, Activate, AddObserver, RadioButton, RadioGroup, Slider, UiWidgetsPlugins, ValueChange,
};
use bevy::window::{PresentMode, PrimaryWindow};
use bevy::{
    asset::RenderAssetUsages, color::palettes, core_pipeline::Skybox, mesh::Indices,
    render::render_resource::PrimitiveTopology,
};
use bevy::{
    camera_controller::free_cam::{FreeCam, FreeCamPlugin},
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
use bevy::{
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin, FrameTimeGraphConfig},
    text::FontSmoothing,
};
use bevy_ecs::system::IntoObserverSystem;
use bevy_image::{ImageLoaderSettings, ImageSampler};
use bevy_render::render_resource::TextureFormat;
use bevy_render::view::Hdr;

const UI_TEXT_MINI: f32 = 10.0;
const UI_TEXT_SMALL: f32 = 12.0;
const UI_TEXT_BIG: f32 = 16.0;

const UI_ROW_GAP_PER_TAB: f32 = 8.0;

#[derive(Component)]
struct ShowAxes {
    enabled: bool,
}

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

#[derive(Resource, Default)]
struct ShowCheckerboardCornerGizmos(bool);

#[derive(Resource, Default)]
struct ShowCheckerboardCornerViewportSpheres(bool);

#[derive(Debug, Resource)]
struct Decals {
    fingerprints: Handle<Image>,
    raindrops: Handle<Image>,
    chewing_gum: Handle<Image>,
}

#[derive(Resource, Default)]
enum ChosenDecal {
    #[default]
    Fingerprints,
    Raindrops,
    ChewingGum,
}

#[derive(Component, Debug)]
struct Checkerboard {
    rows: usize,
    cols: usize,
    square_size: f32,
}

/// Marker component for the checkerboard corner gizmos.
/// Positions set by calculating from the checkerboard settings.
#[derive(Component, Debug)]
struct CheckerboardCornerGizmo;

/// Marker component for the checkerboard corner unlit spheres.
/// Positions set from projecting from viewport to world space.
#[derive(Component, Debug)]
struct CheckerboardCornerUnlitSphere;

#[derive(Debug, Resource)]
struct UnlitViewportSpheresParent {
    parent: Entity,
    mesh: Handle<Mesh>,
}

#[derive(Debug, Resource, Default, Copy, Clone)]
enum RenderResolution {
    Res4K,
    #[default]
    Res1080p,
    Res720p,
    Res480p,
}

#[derive(Debug, Component)]
struct SceneImageNodeMarker;

#[derive(Debug, Component)]
struct ZoomImageNodeMarker;

#[derive(Debug, Component)]
struct ZoomSize(f32);

#[derive(Debug, Component)]
struct CheckerboardDistanceToCamera;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Checkmate".into(),
                        present_mode: PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    filter: "bevy_dev_tools=trace".into(), // Show picking logs trace level and up
                    ..default()
                }),
            FreeCamPlugin,
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
                    enabled: false,
                    frame_time_graph_config: FrameTimeGraphConfig {
                        enabled: false,
                        // The minimum acceptable fps
                        min_fps: 100.0,
                        // The target fps
                        target_fps: 400.0,
                    },
                },
            },
        ))
        .init_resource::<RenderResolution>()
        .init_resource::<ChosenDecal>()
        .insert_resource(UiTheme(create_dark_theme()))
        .insert_resource(DebugPickingMode::Disabled)
        .init_resource::<ShowCheckerboardCornerGizmos>()
        .init_resource::<ShowCheckerboardCornerViewportSpheres>()
        .add_systems(Startup, setup)
        .add_systems(Update, draw_axes)
        .add_systems(Update, on_render_resolution_changed)
        .add_systems(
            Update,
            (update_decal_transform, unlit_corner_spheres).chain(),
        )
        .add_systems(Update, update_slider_height)
        .add_systems(Update, update_slider_font_size)
        .add_systems(Update, update_node_visibility_from_ui_tab_variant)
        .add_systems(
            Update,
            (
                generate_checkerboard,
                checkerboard_changed_add_corner_children,
            )
                .chain(),
        )
        .add_systems(Last, corners_gizmos)
        .add_systems(Update, update_distance_from_checkerboard_to_camera_text)
        .add_systems(
            Update,
            exit_success.run_if(input_just_pressed(KeyCode::Escape)),
        )
        .add_systems(
            Update,
            update_material_with_new_decal.run_if(resource_changed::<ChosenDecal>),
        )
        .add_observer(pointer_move_over_scene_image)
        .add_observer(pointer_scroll_over_scene_image)
        .add_observer(pointer_move_or_scroll_over_scene_image)
        .add_observer(observe_slider_updates)
        .add_observer(observe_radio_updates)
        .add_observer(observe_checkbox_updates)
        .add_observer(|ac: On<ValueChange<f32>>| info!("ac: {ac:#?}"))
        .run();
}

fn update_material_with_new_decal(
    chosen: Res<ChosenDecal>,
    decals: Res<Decals>,
    decal: Single<&MeshMaterial3d<ForwardDecalMaterial<StandardMaterial>>, With<ForwardDecal>>,
    mut materials: ResMut<Assets<ForwardDecalMaterial<StandardMaterial>>>,
) {
    let Some(mat) = materials.get_mut(decal.id()) else {
        return;
    };

    mat.base.base_color_texture = Some(match *chosen {
        ChosenDecal::Fingerprints => decals.fingerprints.clone(),
        ChosenDecal::Raindrops => decals.raindrops.clone(),
        ChosenDecal::ChewingGum => decals.chewing_gum.clone(),
    });
}

fn observe_slider_updates(
    change: On<ValueChange<f32>>,
    slider: Query<Entity, With<SliderValue>>,
    mut commands: Commands,
) {
    let source = change.source;
    let value = change.value;
    if let Ok(entity) = slider.get(source) {
        commands.entity(entity).insert(SliderValue(value));
    }
}

fn observe_radio_updates(
    activate: On<ValueChange<Entity>>,
    radio_parents: Query<&RadioGroup>,
    children: Query<&Children>,
    radio_button: Query<&RadioButton>,
    mut commands: Commands,
) {
    let source = activate.source;
    let target = activate.value;
    info!("Activate: {activate:?}");

    if radio_parents.get(source).is_ok() {
        for button_entity in children.iter_descendants(source) {
            if radio_button.get(button_entity).is_ok() {
                if button_entity == target {
                    commands.entity(button_entity).insert(Checked);
                } else {
                    commands.entity(button_entity).remove::<Checked>();
                }
            }
        }
    }
}

fn observe_checkbox_updates(
    change: On<ValueChange<bool>>,
    checkable: Query<&Checkable>,
    mut commands: Commands,
) {
    let source = change.source;
    let value = change.value;
    if checkable.get(source).is_ok() {
        if value {
            commands.entity(source).insert(Checked);
        } else {
            commands.entity(source).remove::<Checked>();
        }
    }
}

fn exit_success(mut commands: Commands) {
    info!("Exiting");
    commands.write_message(AppExit::default());
}

fn update_distance_from_checkerboard_to_camera_text(
    mut text: Single<&mut Text, With<CheckerboardDistanceToCamera>>,
    checkerboard: Single<&GlobalTransform, (With<Checkerboard>, With<Mesh3d>)>,
    camera: Single<&GlobalTransform, (With<Camera3d>, Without<Checkerboard>)>,
) {
    let distance = checkerboard.translation().distance(camera.translation());

    text.0 = format!(
        "Checkerboard center distance to camera (mm): {:.1}",
        distance * 1e3
    );
}

fn draw_axes(mut gizmos: Gizmos, query: Query<(&Transform, &ShowAxes)>) {
    for (&transform, &ShowAxes { enabled }) in &query {
        if enabled {
            gizmos.axes(transform, 1.0);
        }
    }
}

fn image_render_target(width: u32, height: u32) -> Image {
    let mut image = Image::new_target_texture(width, height, TextureFormat::bevy_default());
    image.sampler = ImageSampler::nearest();

    image
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
    let checkerboard = Checkerboard {
        rows: 11,
        cols: 10,
        square_size: 78. * 1e-3, // 78mm
    };
    let checkboard_handle: Handle<Mesh> =
        meshes.add(create_checkerboard(checkerboard.rows, checkerboard.cols));

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
        ShowAxes { enabled: false },
        checkerboard,
    ));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_and_light_transform =
        Transform::from_xyz(0.8, 0.8, 0.8).looking_at(Vec3::ZERO, Vec3::Y);

    let scene_image = images.add(image_render_target(1920, 1080));

    // Camera in 3D space.
    commands
        .spawn((
            Camera3d::default(),
            FreeCam {
                enabled: false,
                ..default()
            },
            Msaa::Off,
            Fxaa::default(), // Supports both decals and WebGPU at the same time
            Hdr,
            Camera {
                clear_color: ClearColorConfig::Custom(palettes::tailwind::PINK_600.into()),
                target: RenderTarget::Image(scene_image.clone().into()),
                ..default()
            },
            DepthPrepass,
            camera_and_light_transform,
            Tonemapping::AcesFitted,
            Bloom::NATURAL,
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

    let root = root_node(ui_camera, &scene_image);

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
fn create_checkerboard(rows: usize, cols: usize) -> Mesh {
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

fn button_selector(activate: On<Activate>, mut buttons: Query<(Entity, &mut ButtonVariant)>) {
    debug!("Clicked! {activate:?}");
    for (e, mut variant) in &mut buttons {
        if activate.entity == e {
            *variant = ButtonVariant::Primary;
        } else {
            *variant = ButtonVariant::Normal;
        }
    }
}

fn tabs_node() -> impl Bundle + use<> {
    let button = |primary, tab_variant, tab_name| {
        (
            button(
                ButtonProps {
                    variant: if primary {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Normal
                    },
                    ..default()
                },
                tab_variant,
                Spawn((Text::new(tab_name), ThemedText)),
            ),
            observe(button_selector),
        )
    };

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
            button(true, UiTabVariant::Geometry, "Geometry"),
            button(false, UiTabVariant::Material, "Material"),
            button(false, UiTabVariant::Environment, "Environment"),
            button(false, UiTabVariant::Light, "Light"),
            button(false, UiTabVariant::Camera, "Camera"),
            button(false, UiTabVariant::Debug, "Debug"),
        ],
    )
}

fn text(label: &str) -> impl Bundle {
    (
        Text(label.to_owned()),
        TextFont::from_font_size(UI_TEXT_SMALL),
    )
}

fn text_big(label: &str) -> impl Bundle {
    (
        Text(label.to_owned()),
        TextLayout::new_with_justify(Justify::Center),
        TextFont::from_font_size(UI_TEXT_BIG),
    )
}

fn myslider(min: f32, value: f32, max: f32, precision: i32) -> impl Bundle {
    slider(SliderProps { min, value, max }, SliderPrecision(precision))
}

fn tab_node(variant: UiTabVariant, bundle: impl Bundle) -> impl Bundle {
    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: px(UI_ROW_GAP_PER_TAB),
            ..default()
        },
        variant,
        UiTabNode,
        bundle,
    )
}

fn geometry_node() -> impl Bundle {
    tab_node(UiTabVariant::Geometry,
        children![
            text_big("Geometry"),
            text("Rows"),
            (
                myslider(2.0, 11.0, 20.0, 0),
                observe(
                    |change: On<ValueChange<f32>>, mut checkerboards: Query<&mut Checkerboard>| {
                        info!("updating rows {change:#?}");
                        for mut chk in &mut checkerboards {
                            chk.rows = change.value as usize;
                        }
                    }
                )
            ),
            text("Cols"),
            (
                myslider(2.0, 10.0, 20.0, 0),
                observe(
                    |change: On<ValueChange<f32>>, mut checkerboards: Query<&mut Checkerboard>| {
                        info!("updating cols {change:#?}");
                        for mut chk in &mut checkerboards {
                            chk.cols = change.value as usize;
                        }
                    }
                )
            ),
            text("Square Size (mm)"),
            (
                myslider(5.0, 78.0, 100.0, 0),
                observe(
                    |change: On<ValueChange<f32>>, mut checkerboards: Query<&mut Checkerboard>| {
                        info!("updating square size {change:#?}");
                        for mut chk in &mut checkerboards {
                            chk.square_size = change.value * 1e-3;
                        }
                    }
                )
            ),
            (
                vec3_node("Translation", ["X", "Y", "Z"], -2. * Vec3::ONE, Vec3::ZERO, Vec3::ONE*2., IVec3::ONE * 2),
                observe(
                    |vec3: On<Vec3Event>, mut checkerboard: Single<&mut Transform, With<Checkerboard>>| {
                        checkerboard.translation = vec3.value;
                    }
                ),
            ),
            (
              vec3_node("Rotation (XYZEx)", ["X", "Y", "Z"], -90. * Vec3::ONE, Vec3::ZERO, Vec3::ONE * 90., IVec3::ONE * 2),
              observe(
                  |vec3: On<Vec3Event>, mut checkerboard: Single<&mut Transform, With<Checkerboard>>| {
                      let [x,y,z] = vec3.value.to_array();
                      checkerboard.rotation = Quat::from_euler(EulerRot::XYZEx, x.to_radians(), y.to_radians(), z.to_radians());
                  }
              ),
            ),
            (
                Node {
                    padding: UiRect::top(px(8.)),
                    ..default()
                },
                Text("".into()),
                TextFont::from_font_size(UI_TEXT_MINI),
                CheckerboardDistanceToCamera
            )
        ],
    )
}

fn material_node() -> impl Bundle {
    fn use_material(
        use_material: impl Fn(&mut StandardMaterial, f32) + Send + Sync + 'static,
    ) -> impl Bundle {
        observe(
            move |change: On<ValueChange<f32>>,
                  material: Single<&mut MeshMaterial3d<StandardMaterial>, With<Checkerboard>>,
                  mut materials: ResMut<Assets<StandardMaterial>>| {
                let Some(material) = materials.get_mut(material.0.id()) else {
                    warn!("no material via {}", material.0.id());
                    return;
                };
                use_material(material, change.value);
            },
        )
    }

    fn use_decal_material(
        use_material: impl Fn(&mut ExtendedMaterial<StandardMaterial, ForwardDecalMaterialExt>, f32)
            + Send
            + Sync
            + 'static,
    ) -> impl Bundle {
        observe(
            move |change: On<ValueChange<f32>>,
                  material: Single<&mut MeshMaterial3d<ForwardDecalMaterial<StandardMaterial>>, With<ForwardDecal>>,
                  mut materials: ResMut<Assets<ForwardDecalMaterial<StandardMaterial>>>| {
                let Some(material) = materials.get_mut(material.0.id()) else {
                    warn!("no material via {}", material.0.id());
                    return;
                };
                use_material(material, change.value);
            },
        )
    }

    tab_node(
        UiTabVariant::Material,
        children![
            text_big("PBR"),
            text("Metallic"),
            (
                myslider(0.0, 0.9, 1.0, 2),
                use_material(|m, v| m.metallic = v),
            ),
            text("Roughness"),
            (
                myslider(0.089, 0.1, 1.0, 2),
                use_material(|m, v| m.perceptual_roughness = v),
            ),
            text("Clearcoat"),
            (
                myslider(0.0, 1.0, 1.0, 2),
                use_material(|m, v| m.clearcoat = v),
            ),
            text("Clearcoat Roughness"),
            (
                myslider(0.0, 0.5, 1.0, 2),
                use_material(|m, v| m.clearcoat_perceptual_roughness = v),
            ),
            (
                vec3_node(
                    "Color",
                    ["R", "G", "B"],
                    Vec3::ZERO,
                    Vec3::splat(0.88),
                    Vec3::ONE,
                    IVec3::ONE * 2
                ),
                observe(
                    |vec3: On<Vec3Event>,
                     material: Single<
                        &mut MeshMaterial3d<StandardMaterial>,
                        With<Checkerboard>,
                    >,
                     mut materials: ResMut<Assets<StandardMaterial>>| {
                        let Some(material) = materials.get_mut(material.0.id()) else {
                            warn!("no material via {}", material.0.id());
                            return;
                        };

                        material.base_color = Srgba::from_vec3(vec3.value).into();
                    }
                ),
            ),
            // Decal
            text_big("Decal"),
            text("Texture"),
            // Texture
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    column_gap: px(4),
                    ..default()
                },
                RadioGroup,
                children![
                    (
                        radio(Checked, Spawn((Text::new("Fingerprints"), ThemedText))),
                        observe(|_: On<Add, Checked>, mut decal: ResMut<ChosenDecal>| {
                            *decal = ChosenDecal::Fingerprints;
                        }),
                    ),
                    (
                        radio((), Spawn((Text::new("Raindrops"), ThemedText))),
                        observe(|_: On<Add, Checked>, mut decal: ResMut<ChosenDecal>| {
                            *decal = ChosenDecal::Raindrops;
                        }),
                    ),
                    (
                        radio((), Spawn((Text::new("Chewing Gum"), ThemedText))),
                        observe(|_: On<Add, Checked>, mut decal: ResMut<ChosenDecal>| {
                            *decal = ChosenDecal::ChewingGum;
                        }),
                    )
                ]
            ),
            text("Color"),
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
                    text("R"),
                    (
                        myslider(0.0, 0.28, 1.0, 2),
                        use_decal_material(|m, v| m.base.base_color =
                            m.base.base_color.to_linear().with_red(v).into()),
                    ),
                    text("G"),
                    (
                        myslider(0.0, 0.23, 1.0, 2),
                        use_decal_material(|m, v| m.base.base_color =
                            m.base.base_color.to_linear().with_green(v).into()),
                    ),
                    text("B"),
                    (
                        myslider(0.0, 0.11, 1.0, 2),
                        use_decal_material(|m, v| m.base.base_color =
                            m.base.base_color.to_linear().with_blue(v).into()),
                    ),
                    text("A"),
                    (
                        myslider(0.0, 0.11, 1.0, 2),
                        use_decal_material(|m, v| m.base.base_color =
                            m.base.base_color.to_linear().with_alpha(v).into()),
                    ),
                ]
            ),
            text("Scale"),
            (
                myslider(0.1, 1.86, 10.0, 2),
                use_decal_material(|m, v| {
                    let (_, angle, translation) = m.base.uv_transform.to_scale_angle_translation();
                    m.base.uv_transform =
                        Affine2::from_scale_angle_translation(Vec2::splat(v), angle, translation);
                }),
            ),
            text("Rotation (degrees"),
            (
                myslider(0.0, 86.0, 360.0, 1),
                use_decal_material(|m, v| {
                    let (scale, _, translation) = m.base.uv_transform.to_scale_angle_translation();
                    m.base.uv_transform =
                        Affine2::from_scale_angle_translation(scale, v.to_radians(), translation);
                }),
            ),
        ],
    )
}

fn environment_node() -> impl Bundle {
    tab_node(
        UiTabVariant::Environment,
        children![
            text_big("Environment"),
            text("Skybox Brightness"),
            (
                myslider(0.0, 5000.0, 10000.0, -3),
                observe(
                    |change: On<ValueChange<f32>>, mut skyboxes: Query<&mut Skybox>| {
                        for mut sb in &mut skyboxes {
                            sb.brightness = change.value;
                        }
                    }
                )
            ),
            text("Environment Intensity"),
            (
                myslider(0.0, 2000.0, 10000.0, -2),
                observe(
                    |change: On<ValueChange<f32>>, mut envmaps: Query<&mut EnvironmentMapLight>| {
                        for mut em in &mut envmaps {
                            em.intensity = change.value;
                        }
                    }
                )
            ),
        ],
    )
}

fn ui_flex_row() -> impl Bundle {
    Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        column_gap: px(4.),
        ..default()
    }
}

fn ui_flex_col() -> impl Bundle {
    Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::SpaceBetween,
        row_gap: px(UI_ROW_GAP_PER_TAB),
        ..default()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, EntityEvent)]
#[entity_event(auto_propagate)]
pub struct Vec3Event {
    #[event_target]
    pub source: Entity,
    pub value: Vec3,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Channel {
    X,
    Y,
    Z,
}

#[derive(Copy, Clone, Debug, PartialEq, EntityEvent)]
#[entity_event(auto_propagate)]
pub struct ChannelValueChange {
    #[event_target]
    pub source: Entity,
    pub channel: Channel,
    pub value: f32,
}

fn vec3_node(
    label: &str,
    sub_labels: [&str; 3],
    mins: Vec3,
    default_values: Vec3,
    maxs: Vec3,
    precisions: IVec3,
) -> impl Bundle {
    let [x, y, z] = sub_labels;
    let [min_x, min_y, min_z] = mins.to_array();
    let [def_x, def_y, def_z] = default_values.to_array();
    let [max_x, max_y, max_z] = maxs.to_array();
    let [prec_x, prec_y, prec_z] = precisions.to_array();

    (
        ui_flex_col(),
        observe(
            move |change: On<ChannelValueChange>,
                  mut commands: Commands,
                  mut cached: Local<Option<Vec3>>| {
                info!("Yayzah {change:?}");
                let ChannelValueChange {
                    channel,
                    value,
                    source,
                } = change.event();
                let cached = cached.get_or_insert(default_values);

                match channel {
                    Channel::X => cached.x = *value,
                    Channel::Y => cached.y = *value,
                    Channel::Z => cached.z = *value,
                }

                commands.trigger(Vec3Event {
                    source: *source,
                    value: *cached,
                });
            },
        ),
        children![
            text_big(label),
            (
                ui_flex_row(),
                children![
                    text(x),
                    (
                        myslider(min_x, def_x, max_x, prec_x),
                        observe(|change: On<ValueChange<f32>>, mut commands: Commands| {
                            commands.trigger(ChannelValueChange {
                                source: change.source,
                                value: change.value,
                                channel: Channel::X,
                            });
                        })
                    ),
                    text(y),
                    (
                        myslider(min_y, def_y, max_y, prec_y),
                        observe(|change: On<ValueChange<f32>>, mut commands: Commands| {
                            commands.trigger(ChannelValueChange {
                                source: change.source,
                                value: change.value,
                                channel: Channel::Y,
                            });
                        })
                    ),
                    text(z),
                    (
                        myslider(min_z, def_z, max_z, prec_z),
                        observe(|change: On<ValueChange<f32>>, mut commands: Commands| {
                            commands.trigger(ChannelValueChange {
                                source: change.source,
                                value: change.value,
                                channel: Channel::Z,
                            });
                        })
                    ),
                ],
            ),
        ],
    )
}

fn light_node() -> impl Bundle {
    tab_node(
        UiTabVariant::Light,
        children![
            text_big("Light"),
            // Direction
            (
              vec3_node("Direction", ["X", "Y", "Z"], Vec3::ZERO, Vec3::new(45., 0., 120.), Vec3::splat(360.), IVec3::splat(2)),
              observe(
                  |vec3: On<Vec3Event>, mut lights: Query<&mut Transform, With<DirectionalLight>>| {
                      let [x,y,z] = vec3.value.to_array();
                      let rotation = Quat::from_euler(EulerRot::XYZEx, x.to_radians(), y.to_radians(), z.to_radians());
                      for mut t in &mut lights {
                          t.rotation = rotation;
                      }
                  }
              ),
            ),
            // Direction RGB
            (
                vec3_node(
                    "Color",
                    ["R", "G", "B"],
                    Vec3::ZERO,
                    Vec3::splat(0.8),
                    Vec3::ONE,
                    IVec3::ONE * 2
                ),
                observe(
                    |vec3: On<Vec3Event>,
                    mut lights: Query<&mut DirectionalLight>| {
                        for mut l in &mut lights {
                            info!("dir light color {l:?}");
                            l.color = Srgba::from_vec3(vec3.value).into();
                        }
                    }
                ),
            ),
            // Point light direction
            (
              vec3_node("Translation (point)", ["X", "Y", "Z"], Vec3::splat(-3.), Vec3::ONE, Vec3::splat(3.), IVec3::splat(2)),
              observe(
                  |vec3: On<Vec3Event>, mut lights: Query<&mut Transform, With<PointLight>>| {
                      for mut t in &mut lights {
                          t.translation = vec3.value;
                      }
                  }
              ),
            ),
            // Point RGB
            (
                vec3_node(
                    "Color (point)",
                    ["R", "G", "B"],
                    Vec3::ZERO,
                    Vec3::splat(0.8),
                    Vec3::ONE,
                    IVec3::ONE * 2
                ),
                observe(
                    |vec3: On<Vec3Event>,
                    mut lights: Query<&mut PointLight>| {
                        for mut l in &mut lights {
                            l.color = Srgba::from_vec3(vec3.value).into();
                        }
                    }
                ),
            ),

        ],
    )
}

fn camera_node() -> impl Bundle {
    // Wrapper component to hold render resolution
    #[derive(Debug, Component)]
    struct RenderResolutionComponent(RenderResolution);

    tab_node(
        UiTabVariant::Camera,
        children![
            (
                vec3_node("Position", ["X", "Y", "Z"], Vec3::ZERO, Vec3::new(0.43, 0.81, 0.01), Vec3::splat(3.), IVec3::splat(2)),
                observe(
                    |pos: On<Vec3Event>, mut camera: Single<&mut Transform, With<Camera3d>>| {
                        camera.translation = pos.value;
                    }
                ),
            ),
            observed_checkbox((), "Freecam", |change: On<ValueChange<bool>>, mut freecam: Single<&mut FreeCam,  With<Camera3d>>| {
                info!("freecam changed: {change:?}");
                freecam.enabled = change.value;
            }),
            // Projection
            text_big("Projection"),
            (
                ui_flex_row(),
                children![
                    text("Field of View (degrees)"),
                    (
                        myslider(10.0, 45.0, 135.0, 1),
                            observe(
                                |change: On<ValueChange<f32>>, mut projections: Query<&mut Projection, With<Camera3d>>| {
                                    info!("updating fov {change:#?}");
                                    for mut proj in &mut projections {
                                        let perspective = match *proj {
                                            Projection::Perspective(ref mut p) => p,
                                            _ => {
                                                unimplemented!();
                                            }
                                        };

                                        perspective.fov = change.value.to_radians();
                                    }
                                }
                            ),
                    ),
                ]
            ),
            // DoF
            (
                ui_flex_col(),
                children![
                    text_big("Depth of Field"),
                    observed_checkbox((), "Enabled", |change: On<ValueChange<bool>>, mut commands: Commands, camera: Single<Entity, (With<Camera>, With<Camera3d>)>| {
                        info!("value changed: {change:?}");
                        let mut cmds = commands.entity(*camera);
                        if change.value {
                            cmds
                                .insert(camera_depth_of_field());
                        } else {
                            cmds.remove::<DepthOfField>();
                        }
                    }),
                    // Focal distance node
                    text("Focal Distance"),
                    (
                        myslider(0.3, 1.43, 10.0, 2),
                        observe(
                            |change: On<ValueChange<f32>>, mut dofs: Query<&mut DepthOfField>| {
                                info!("updating focal distance {change:#?}");
                                for mut dof in &mut dofs {
                                    dof.focal_distance = change.value;
                                }
                            }
                        )
                    ),
                    // Sensor height node
                    text("Sensor Height (mm)"),
                    (
                        myslider(5.0, 10.18, 50.0, 2),
                        observe(
                            |change: On<ValueChange<f32>>, mut dofs: Query<&mut DepthOfField>| {
                                    info!("updating sensor height {change:#?}");
                                    for mut dof in &mut dofs {
                                        dof.sensor_height = change.value * 1e-3; // mm to meters
                                    }
                                }
                            )
                    ),
                    // F-stops node
                    text("F-stops"),
                    (
                        myslider(0.1, 2.5, 3.0, 1),
                        observe(
                            |change: On<ValueChange<f32>>, mut dofs: Query<&mut DepthOfField>| {
                                    info!("updating F-stops {change:#?}");
                                    for mut dof in &mut dofs {
                                        dof.aperture_f_stops = change.value;
                                    }
                                }
                            )
                    ),
                ],
            ),
            // Render resolution
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    column_gap: px(4),
                    ..default()
                },
                RadioGroup,
                observe(|activate: On<ValueChange<Entity>>, mut button: Query<&RenderResolutionComponent, With<RadioButton>>, mut render_res:
                    ResMut<RenderResolution>| {
                    // RadioGroup emits ValueChange<Entity> with value the button that changed
                    *render_res = button.get_mut(activate.event().value).unwrap().0;
                }),
                children![
                    text("Resolution"),
                    radio(
                        RenderResolutionComponent(RenderResolution::Res4K),
                        Spawn((Text::new("4K"), ThemedText))
                    ),
                    radio(
                        (
                            Checked,
                            RenderResolutionComponent(RenderResolution::Res1080p)
                        ),
                        Spawn((Text::new("1080p"), ThemedText))
                    ),
                    radio(
                        RenderResolutionComponent(RenderResolution::Res720p),
                        Spawn((Text::new("720p"), ThemedText))
                    ),
                    radio(
                        RenderResolutionComponent(RenderResolution::Res480p),
                        Spawn((Text::new("480p"), ThemedText))
                    ),
                ]
            ),
        ],
    )
}

fn on_render_resolution_changed(
    render_res: Res<RenderResolution>,
    mut images: ResMut<Assets<Image>>,
    mut query: Query<&mut Camera, With<Camera3d>>,
) {
    if !render_res.is_changed() {
        return;
    }

    let (width, height) = match *render_res {
        RenderResolution::Res4K => (3840, 2160),
        RenderResolution::Res1080p => (1920, 1080),
        RenderResolution::Res720p => (1280, 720),
        RenderResolution::Res480p => (854, 480),
    };

    info!("Setting render resolution to {width}x{height}");

    for mut camera in &mut query {
        let RenderTarget::Image(target) = &mut camera.target else {
            unreachable!()
        };

        if let Some(image) = images.get_mut(target.handle.id()) {
            *image = image_render_target(width, height);
        } else {
            warn!("could not find image for render target");
        }
    }
}

fn observed_checkbox<B: Bundle, M, I: IntoObserverSystem<ValueChange<bool>, B, M>>(
    extras: impl Bundle,
    label: &str,
    obs: I,
) -> impl Bundle
where
    AddObserver<ValueChange<bool>, B, M, I>: Bundle,
{
    (
        checkbox(extras, Spawn((Text::new(label), ThemedText))),
        observe(obs),
    )
}

fn debug_node() -> impl Bundle {
    // Wrapper component to hold DebugPickingMode so we can query radios by that
    #[derive(Debug, Component)]
    struct DebugPickingModeComponent(DebugPickingMode);

    tab_node(
        UiTabVariant::Debug,
        children![
            text_big("Debug"),
            (
                checkbox(Checked, Spawn((Text::new("Gizmos enabled"), ThemedText))),
                observe(
                    |change: On<ValueChange<bool>>, mut config: ResMut<GizmoConfigStore>| {
                        let (config, _) = config.config_mut::<DefaultGizmoConfigGroup>();
                        config.enabled = change.value;
                    }
                ),
            ),
            (
                checkbox((), Spawn((Text::new("Draw Axes"), ThemedText))),
                observe(
                    |change: On<ValueChange<bool>>, mut show_axes: Query<&mut ShowAxes>| {
                        for mut show in &mut show_axes {
                            show.enabled = change.value;
                        }
                    }
                )
            ),
            (
                checkbox(
                    (),
                    Spawn((Text::new("Checkerboard corner world gizmos"), ThemedText))
                ),
                observe(
                    |change: On<ValueChange<bool>>,
                     mut show: ResMut<ShowCheckerboardCornerGizmos>| {
                        show.0 = change.value;
                    }
                )
            ),
            (
                checkbox(
                    (),
                    Spawn((
                        Text::new("Checkerboard corner viewport spheres"),
                        ThemedText
                    ))
                ),
                observe(
                    |change: On<ValueChange<bool>>,
                     mut show: ResMut<ShowCheckerboardCornerViewportSpheres>| {
                        show.0 = change.value;
                    }
                )
            ),
            (
                checkbox((), Spawn((Text::new("Show FPS"), ThemedText))),
                observe(
                    |change: On<ValueChange<bool>>, mut config: ResMut<FpsOverlayConfig>| {
                        config.enabled = change.value;
                        config.frame_time_graph_config.enabled = change.value;
                    }
                )
            ),
            (
                checkbox(Checked, Spawn((Text::new("Vsync"), ThemedText))),
                observe(
                    |change: On<ValueChange<bool>>,
                     mut window: Single<&mut Window, With<PrimaryWindow>>| {
                        window.present_mode = if change.value {
                            PresentMode::AutoVsync
                        } else {
                            PresentMode::AutoNoVsync
                        };
                    }
                )
            ),
            // Picking
            (
                ui_flex_col(),
                RadioGroup,
                observe(
                    |activate: On<ValueChange<Entity>>,
                     mut button: Query<&DebugPickingModeComponent, With<RadioButton>>,
                     mut picking_debug: ResMut<DebugPickingMode>| {
                        // RadioGroup emits ValueChange<Entity> with value the button that changed
                        *picking_debug = button.get_mut(activate.event().value).unwrap().0;
                    }
                ),
                children![
                    text("Picking"),
                    radio(
                        (
                            Checked,
                            DebugPickingModeComponent(DebugPickingMode::Disabled)
                        ),
                        Spawn((Text::new("Disabled"), ThemedText))
                    ),
                    radio(
                        DebugPickingModeComponent(DebugPickingMode::Normal),
                        Spawn((Text::new("Normal"), ThemedText))
                    ),
                    radio(
                        DebugPickingModeComponent(DebugPickingMode::Noisy),
                        Spawn((Text::new("Noisy"), ThemedText))
                    ),
                ]
            ),
            // UI debug
            text_big("UI debug"),
            (
                checkbox((), Spawn((Text::new("Enabled"), ThemedText))),
                observe(
                    |change: On<ValueChange<bool>>, mut options: ResMut<UiDebugOptions>| {
                        options.enabled = change.value;
                    }
                ),
            ),
            (
                checkbox((), Spawn((Text::new("Show hidden"), ThemedText))),
                observe(
                    |change: On<ValueChange<bool>>, mut options: ResMut<UiDebugOptions>| {
                        options.show_hidden = change.value;
                    }
                ),
            ),
            (
                checkbox((), Spawn((Text::new("Show clipped"), ThemedText))),
                observe(
                    |change: On<ValueChange<bool>>, mut options: ResMut<UiDebugOptions>| {
                        options.show_clipped = change.value;
                    }
                ),
            ),
        ],
    )
}

/// Event relating only to [`SceneImageNodeMarker`]
#[derive(Debug, Event)]
enum MoveOrScroll {
    Scroll {
        diff: f32,
    },

    Move {
        /// Moved to position over [`SceneImageNodeMarker`].
        /// In normalized [0., 1.] range
        pos: Vec2,
    },
}

fn pointer_move_or_scroll_over_scene_image(
    e: On<MoveOrScroll>,
    mut last_pos: Local<Vec2>,
    main: Single<&ImageNodeSize, With<SceneImageNodeMarker>>,
    zoomed: Single<
        (&mut ImageNode, &mut ZoomSize),
        (With<ZoomImageNodeMarker>, Without<SceneImageNodeMarker>),
    >,
) {
    let (mut zoom_node, mut zoom_size) = zoomed.into_inner();

    match e.event() {
        MoveOrScroll::Scroll { diff } => {
            zoom_size.0 += *diff;
            zoom_size.0 = zoom_size.0.clamp(5., 400.)
        }
        MoveOrScroll::Move { pos } => *last_pos = *pos,
    }

    // The desired px size of the zoomed area
    let zoom = Vec2::new(16. / 9., 1.) * zoom_size.0;

    // Px size of the main scene image
    let texture_size = main.size().as_vec2();

    // Get the pointer position in the scene image coordinates.
    // Clamp to within a border of half the desired zoom size to avoid
    // warp effects close to borders
    let pointer_pos = (texture_size * *last_pos).clamp(zoom / 2., texture_size - zoom / 2.);

    let rect = Rect {
        min: pointer_pos - zoom / 2.,
        max: pointer_pos + zoom / 2.,
    };

    debug!(
        "{:?} -> {pointer_pos:?} -> {rect:?}, zoom size = {:?}",
        *last_pos, zoom_size.0
    );

    zoom_node.rect = Some(rect);
}

fn pointer_move_over_scene_image(
    e: On<Pointer<Move>>,
    main: Single<Entity, With<SceneImageNodeMarker>>,
    mut commands: Commands,
) {
    if *main == e.event().entity {
        let normalized = (e.event().hit.position.unwrap_or_default().truncate() + Vec2::splat(0.5))
            .clamp(Vec2::ZERO, Vec2::ONE);

        commands.trigger(MoveOrScroll::Move { pos: normalized });
    }
}

fn pointer_scroll_over_scene_image(
    e: On<Pointer<Scroll>>,
    main: Single<Entity, With<SceneImageNodeMarker>>,
    mut commands: Commands,
) {
    if *main == e.event().entity {
        let diff = e.event().event.y * -5.;

        commands.trigger(MoveOrScroll::Scroll { diff });
    }
}

fn root_node(camera_entity: Entity, scene_image: &Handle<Image>) -> impl Bundle {
    let tabs = tabs_node();
    let geometry = geometry_node();
    let material = material_node();
    let environment = environment_node();
    let light = light_node();
    let camera = camera_node();
    let debug = debug_node();

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
                    aspect_ratio: Some(16. / 9.),
                    ..default()
                },
                SceneImageNodeMarker,
            ),
            (
                ImageNode::new(scene_image.clone()).with_rect(Rect {
                    min: Vec2::new(100., 200.),
                    max: Vec2::new(300., 300.)
                }),
                Node {
                    height: px(200.),
                    width: px(200. * 16. / 9.),
                    position_type: PositionType::Absolute,
                    right: px(16.),
                    bottom: px(16.),
                    aspect_ratio: Some(16. / 9.),
                    ..default()
                },
                BorderRadius::new(
                    // top left
                    Val::Px(20.),
                    // top right
                    Val::Px(20.),
                    // bottom right
                    Val::Px(20.),
                    // bottom left
                    Val::Px(20.),
                ),
                ZoomImageNodeMarker,
                ZoomSize(100.)
            )
        ],
    )
}

// Generate a new checkerboard mesh if the settings changed.
fn generate_checkerboard(
    mut checkerboards: Query<(&mut Mesh3d, &mut Transform, &Checkerboard), Changed<Checkerboard>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (mut mesh, mut transform, settings) in &mut checkerboards {
        info!("changed to {:?}", *settings);

        **mesh = meshes.add(create_checkerboard(settings.rows, settings.cols));
        transform.scale = Vec3::splat(settings.square_size);
    }
}

// Iterate over all sliders and set their height to 12px to make the UI less cluttered.
fn update_slider_height(mut sliders: Query<&mut Node, With<Slider>>) {
    for mut node in &mut sliders {
        node.height = px(12);
    }
}

// Iterate over all sliders and set their text font size to small to make
// the UI less cluttered.
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

// Show only the node that corresponds to the selected tab.
// The others get display: none and are hidden.
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

// When the checkerboard settings change, respawn new corner children to the checkerboard
fn checkerboard_changed_add_corner_children(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    unlit_parent: Res<UnlitViewportSpheresParent>,
    checkerboards: Query<(Entity, &Checkerboard), Changed<Checkerboard>>,
) {
    for (checkerboard, settings) in &checkerboards {
        let Checkerboard { rows, cols, .. } = *settings;

        debug!("making new corner position kids for {checkerboard}, settings: {rows}x{cols}",);

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
                        .spawn((
                            Transform::from_translation(position),
                            CornerId(id),
                            CheckerboardCornerGizmo,
                        ))
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
                            CheckerboardCornerUnlitSphere,
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

fn corners_gizmos(
    mut gizmos: Gizmos,
    enabled: Res<ShowCheckerboardCornerGizmos>,
    checkerboards: Query<(&Children, &Transform), With<Checkerboard>>,
    corners: Query<(&GlobalTransform, &CornerId), With<CheckerboardCornerGizmo>>,
) {
    if !enabled.0 {
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

// Make the decal always cover the entire checkerboard
fn update_decal_transform(
    mut decal: Single<&mut Transform, (With<ForwardDecal>, Without<Checkerboard>)>,
    checkerboard: Single<(&Transform, &Checkerboard)>,
) {
    let (transform, settings) = *checkerboard;

    **decal = Transform {
        scale: Vec3::new(
            settings.square_size * settings.cols as f32,
            1.0,
            settings.square_size * settings.rows as f32,
        ),
        ..*transform
    };
}

fn unlit_corner_spheres(
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    enabled: Res<ShowCheckerboardCornerViewportSpheres>,
    checkerboards: Query<&Children, With<Checkerboard>>,
    mut corners: Query<(&GlobalTransform, &CornerId), Without<Gizmo>>,
    mut unlits: Query<
        (&mut Transform, &mut Visibility, &CornerId),
        With<CheckerboardCornerUnlitSphere>,
    >,
) {
    for checkerboard_children in &checkerboards {
        let num_corners = checkerboard_children.len();

        let green = palettes::basic::GREEN;
        let red = palettes::basic::RED;

        for (child, (mut unlit_transform, mut unlit_visibility, _gizmo_id)) in
            zip(checkerboard_children.iter(), unlits.iter_mut())
        {
            *unlit_visibility = if enabled.0 {
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
