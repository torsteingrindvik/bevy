//! This example demonstrates how to create a custom mesh,
//! assign a custom UV mapping for a custom texture,
//! and how to change the UV mapping at run-time.

use bevy::{
    asset::RenderAssetUsages, color::palettes, core_pipeline::Skybox, core_widgets::CoreSlider,
    mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use bevy::{
    core_widgets::{CoreWidgetsPlugins, SliderPrecision, SliderStep, SliderValue},
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
use bevy_image::ImageLoaderSettings;
use bevy_render::view::Hdr;

const UI_TEXT_SMALL: f32 = 12.0;
const UI_TEXT_BIG: f32 = 16.0;

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
        .add_systems(Update, generate_checkerboard)
        .add_systems(Update, update_rows_cols_from_sliders)
        .add_systems(Update, update_square_size_from_slider)
        .add_systems(Update, update_checkerboard_transform_from_settings)
        .add_systems(Update, update_camera_transform_from_sliders)
        .add_systems(Update, update_slider_height)
        .add_systems(Update, update_slider_font_size)
        .add_systems(Update, update_checkerboard_material_from_sliders)
        .add_systems(Update, update_environment_from_sliders)
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

#[derive(Resource, Component, PartialEq, Clone, Copy, Debug)]
struct CheckerboardSettings {
    rows: usize,
    cols: usize,
    /// Metric size of each square
    square_size: f32,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Import the custom texture.
    // let custom_texture_handle: Handle<Image> = asset_server.load("textures/array_texture.png");

    // Create and save a handle to the mesh.
    // let cube_mesh_handle: Handle<Mesh> = meshes.add(create_cube_mesh());

    // Render the mesh with the custom texture, and add the marker.
    // commands.spawn((
    //     Mesh3d(cube_mesh_handle),
    //     MeshMaterial3d(materials.add(StandardMaterial {
    //         // base_color_texture: Some(custom_texture_handle),
    //         ..default()
    //     })),
    //     CustomUV,
    // ));

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

    // Camera in 3D space.
    commands
        .spawn((
            Camera3d::default(),
            Hdr,
            Camera {
                clear_color: ClearColorConfig::Custom(palettes::tailwind::PINK_600.into()),
                ..default()
            },
            camera_and_light_transform,
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

    // Light up the scene.
    commands.spawn((PointLight::default(), camera_and_light_transform));

    commands.spawn(demo_root());

    commands.spawn(Sprite::from_image(asset_server.load("branding/icon.png")));
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

fn demo_root() -> impl Bundle {
    (
        Node {
            width: percent(30),
            height: percent(100),
            align_items: AlignItems::Start,
            justify_content: JustifyContent::Start,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: px(10),
            ..default()
        },
        TabGroup::default(),
        ThemeBackgroundColor(tokens::WINDOW_BG),
        children![(
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Start,
                padding: UiRect::all(px(8)),
                row_gap: px(16),
                width: percent(35),
                min_width: px(200),
                ..default()
            },
            children![
                // Checkerboard settings node
                (
                    Node {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        row_gap: px(4.),
                        ..default()
                    },
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
                            (SliderStep(1.), SliderPrecision(0), SliderCheckerboardRows),
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
                            (SliderStep(1.), SliderPrecision(0), SliderCheckerboardCols),
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
                            (
                                SliderStep(1.),
                                SliderPrecision(0),
                                SliderCheckerboardSquareSizeMillimeters
                            ),
                        ),
                    ],
                ),
                // Checkerboard material
                (
                    Node {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        row_gap: px(4.),
                        ..default()
                    },
                    children![
                        (
                            Text("Material".to_owned()),
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
                            (SliderStep(0.01), SliderPrecision(2), SliderMetallic),
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
                            (SliderStep(0.01), SliderPrecision(2), SliderRoughness),
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
                            (SliderStep(0.01), SliderPrecision(2), SliderClearcoat),
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
                            (
                                SliderStep(0.01),
                                SliderPrecision(2),
                                SliderClearcoatRoughness
                            ),
                        ),
                    ],
                ),
                // Environtment settings node
                (
                    Node {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        row_gap: px(4.),
                        ..default()
                    },
                    children![
                        (
                            Text("Environment".to_owned()),
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
                                    (
                                        SliderStep(1000.),
                                        SliderPrecision(3),
                                        SliderSkyboxBrightness
                                    ),
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
                                    (
                                        SliderStep(100.),
                                        SliderPrecision(3),
                                        SliderEnvironmentIntensity
                                    ),
                                ),
                            ]
                        ),
                    ],
                ),
                // Camera settings node
                (
                    Node {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        row_gap: px(4.),
                        ..default()
                    },
                    children![
                        (
                            Text("Camera Settings".to_owned()),
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
                                    (SliderStep(1.), SliderPrecision(2), SliderCameraX),
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
                                    (SliderStep(1.), SliderPrecision(2), SliderCameraY),
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
                                    (SliderStep(1.), SliderPrecision(2), SliderCameraZ),
                                ),
                            ]
                        ),
                    ],
                ),
            ]
        ),],
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
    checkerboard: Single<(&mut Mesh3d, &CheckerboardSettings)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if settings.is_changed() {
        let (mut mesh, mesh_settings) = checkerboard.into_inner();

        if *settings == *mesh_settings {
            // don't recreate mesh if settings are the same
            return;
        }

        info!("changed to {:?}", *settings);

        **mesh = meshes.add(create_checkerboard(*settings));
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
