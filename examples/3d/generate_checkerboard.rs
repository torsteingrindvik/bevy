//! Example that generates a checkerboard mesh procedurally

// TODO:
// - UVs are not working with clearcoat normal map, investigate
//   - Is there a way to debug view it?
// - Post-processing:
//   - Noise
// - Expose corner positions in world space
//   - Optionally visualize them with gizmos
// - Try using world-space corner positions, display in viewport via gizmos
// - Checkerboard color options
// - Checkerboard controls
// - "Settled" component: For all things with transforms, mark as settled when not moving for e.g. 3 frames
// - Decals!

use std::ops::Deref;

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::core_widgets::{Activate, Callback};
use bevy::feathers::controls::{button, ButtonProps, ButtonVariant};
use bevy::feathers::theme::ThemedText;
use bevy::platform::collections::HashMap;
use bevy::post_process::bloom::Bloom;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy::{
    asset::RenderAssetUsages, color::palettes, core_pipeline::Skybox, core_widgets::CoreSlider,
    mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use bevy::{
    core_widgets::{CoreWidgetsPlugins, SliderPrecision, SliderValue},
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
};
use bevy_image::{ImageLoaderSettings, ImageSampler};
use bevy_render::render_resource::TextureFormat;
use bevy_render::view::Hdr;

const UI_TEXT_SMALL: f32 = 12.0;
const UI_TEXT_BIG: f32 = 16.0;

const UI_ROW_GAP_PER_TAB: f32 = 8.0;

// Define a "marker" component to mark the custom mesh. Marker components are often used in Bevy for
// filtering entities in queries with `With`, they're usually not queried directly since they don't
// contain information within them.
#[derive(Component)]
struct CustomUV;

#[derive(Component)]
struct UpDown;

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

#[derive(Component, Clone, Copy, Hash, PartialEq, Eq, Debug)]
enum UiTabVariant {
    Geometry,
    Material,
    Environment,
    Camera,
}

#[derive(Component)]
struct UiTabNode;

#[derive(Component)]
struct CornerId(usize);

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            CoreWidgetsPlugins,
            InputDispatchPlugin,
            TabNavigationPlugin,
            FeathersPlugin,
        ))
        .insert_resource(UiTheme(create_dark_theme()))
        .add_systems(Startup, setup)
        .add_systems(Update, input_handler)
        .add_systems(Update, up_down)
        .add_systems(Update, draw_axes)
        .add_systems(Update, update_square_size_from_slider)
        .add_systems(Update, update_checkerboard_transform_from_settings)
        .add_systems(Update, update_checkerboard_color_from_sliders)
        .add_systems(Update, update_camera_transform_from_sliders)
        .add_systems(Update, update_camera_projection_from_sliders)
        .add_systems(Update, update_slider_height)
        .add_systems(Update, update_slider_font_size)
        .add_systems(Update, update_checkerboard_material_from_sliders)
        .add_systems(Update, update_environment_from_sliders)
        .add_systems(Update, update_depth_of_field_from_sliders)
        .add_systems(Update, update_node_visibility_from_ui_tab_variant)
        .add_systems(
            Update,
            (
                update_rows_cols_from_sliders,
                generate_checkerboard,
                maintain_corners_as_checkerboard_children,
                corners_gizmos,
            )
                .chain(),
        )
        .add_systems(Last, corners_gizmos)
        .run();
}

fn up_down(time: Res<Time>, mut query: Query<&mut Transform, With<UpDown>>) {
    for mut transform in &mut query {
        let new_y = (time.elapsed_secs().sin() + 1.0) / 2.0;
        transform.translation.y = new_y * 0.2;
    }
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

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
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
                "textures/ScratchedGold-Normal.png",
                |settings: &mut ImageLoaderSettings| settings.is_srgb = false,
            )),
            metallic: 0.9,
            perceptual_roughness: 0.1,
            base_color: palettes::css::GOLD.into(),
            ..default()
        })),
        UpDown,
        ShowAxes,
        checkerboard,
    ));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_and_light_transform =
        Transform::from_xyz(3.8, 3.8, 1.8).looking_at(Vec3::ZERO, Vec3::Y);

    // let scene_image = image_render_target(&asset_server);
    let scene_image = image_render_target(&mut images);

    // Camera in 3D space.
    commands
        .spawn((
            Camera3d::default(),
            Hdr,
            Camera {
                clear_color: ClearColorConfig::Custom(palettes::tailwind::PINK_600.into()),
                target: bevy::camera::RenderTarget::Image(scene_image.clone().into()),
                ..default()
            },
            camera_and_light_transform,
            Tonemapping::TonyMcMapface,
            Bloom::NATURAL,
            DepthOfField {
                mode: DepthOfFieldMode::Bokeh,
                focal_distance: 1.0,
                ..default()
            },
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

    // Light up the scene.
    commands.spawn((PointLight::default(), camera_and_light_transform));

    let root = root_node(&mut commands, ui_camera, &scene_image);
    commands.spawn(root);

    // commands.spawn(Sprite::from_image(asset_server.load("branding/icon.png")));
}

// System to receive input from the user,
// check out examples/input/ for more examples about user input.
fn input_handler(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<CustomUV>>,
    time: Res<Time>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {}
    if keyboard_input.pressed(KeyCode::KeyX) {
        for mut transform in &mut query {
            transform.rotate_x(time.delta_secs() / 1.2);
        }
    }
    if keyboard_input.pressed(KeyCode::KeyY) {
        for mut transform in &mut query {
            transform.rotate_y(time.delta_secs() / 1.2);
        }
    }
    if keyboard_input.pressed(KeyCode::KeyZ) {
        for mut transform in &mut query {
            transform.rotate_z(time.delta_secs() / 1.2);
        }
    }
    if keyboard_input.pressed(KeyCode::KeyR) {
        for mut transform in &mut query {
            transform.look_to(Vec3::NEG_Z, Vec3::Y);
        }
    }
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

fn tabs_node(commands: &mut Commands) -> impl Bundle {
    let tabs_callback = commands.register_system(button_selector);

    // Tabs
    (
        Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Start,
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
                UiTabVariant::Camera,
                Spawn((Text::new("Camera"), ThemedText))
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
                    value: 9.0,
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
                    value: 16.0,
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
                    value: 34.0,
                    max: 100.0,
                    ..default()
                },
                (SliderPrecision(0), SliderCheckerboardSquareSizeMillimeters),
            ),
        ],
    )
}

fn material_node() -> impl Bundle {
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
                        Text("R".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.0,
                            value: 1.0,
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
                            value: 1.0,
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
                            value: 1.0,
                            max: 1.0,
                            ..default()
                        },
                        (SliderPrecision(2), SliderColorB),
                    ),
                ]
            ),
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
                    value: 5000.0,
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
                    value: 2000.0,
                    max: 10000.0,
                    ..default()
                },
                (SliderPrecision(-2), SliderEnvironmentIntensity),
            ),
        ],
    )
}

fn camera_node() -> impl Bundle {
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
                            value: 0.5,
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
                            value: 0.5,
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
                            value: 0.5,
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
                    // Focal distance node
                    (
                        Text("Focal Distance".to_owned()),
                        TextFont::from_font_size(UI_TEXT_SMALL)
                    ),
                    slider(
                        SliderProps {
                            min: 0.3,
                            value: 1.0,
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
                            value: 18.66,
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
                            value: 1.0,
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

fn root_node(
    commands: &mut Commands,
    camera_entity: Entity,
    scene_image: &Handle<Image>,
) -> impl Bundle {
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
                    width: percent(30),
                    min_width: px(400),
                    ..default()
                },
                children![
                    tabs_node(commands),
                    geometry_node(),
                    material_node(),
                    environment_node(),
                    camera_node()
                ]
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

fn update_slider_height(mut sliders: Query<&mut Node, With<CoreSlider>>) {
    for mut node in &mut sliders {
        node.height = px(12);
    }
}

fn update_slider_font_size(
    mut q_sliders: Query<Entity, With<CoreSlider>>,
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
    mut q_dof: Query<&mut DepthOfField>,
) {
    for mut dof in &mut q_dof {
        dof.focal_distance = slider_focal_distance.0;
        dof.sensor_height = slider_sensor_height.0 * 1e-3; // mm to meters
        dof.aperture_f_stops = slider_f_stops.0;
    }
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
    checkerboard_res: Res<CheckerboardSettings>,
    checkerboards: Query<(Entity, &CheckerboardSettings)>,
) {
    if checkerboard_res.is_changed() {
        for (checkerboard, settings) in &checkerboards {
            let CheckerboardSettings { rows, cols, .. } = *settings;

            info!("making new corner position kids for {checkerboard}, settings: {rows}x{cols} vs {:?}", checkerboard_res.deref());

            // Start over
            commands.entity(checkerboard).despawn_children();

            let mut children = vec![];

            let half = Vec3::new(cols as f32, 0.0, rows as f32) / 2.0;

            let mut id = 0;
            for row in 1..rows {
                for col in 1..cols {
                    let position = Vec3::new(col as f32, 0.0, row as f32) - half;

                    children.push(
                        commands
                            .spawn((Transform::from_translation(position), CornerId(id)))
                            .id(),
                    );
                    id += 1;
                }
            }

            commands.entity(checkerboard).add_children(&children);
        }
    }
}

fn corners_gizmos(
    mut gizmos: Gizmos,
    checkerboards: Query<(&Children, &Transform), With<CheckerboardSettings>>,
    corners: Query<(&GlobalTransform, &CornerId)>,
) {
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
