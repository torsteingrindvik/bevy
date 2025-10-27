//! TODO

use std::f32::consts::PI;

use bevy::{
    core_pipeline::core_3d::graph::Node3d, math::Affine2,
    post_process::lens_distortion::LensDistortion, prelude::*, render::view::Hdr,
};
use bevy_asset::RenderAssetUsages;
use bevy_image::{ImageAddressMode, ImageSamplerDescriptor};
use bevy_render::{
    render_graph::IntoRenderNodeArray,
    render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// The number of units per frame to add to or subtract from intensity when the
/// arrow keys are held.
const ADJUSTMENT_SPEED: f32 = 0.002;

/// The maximum supported chromatic aberration intensity level.
const MAX_VALUE: f32 = 10.4;

/// The settings that the user can control.
#[derive(Resource, Default, Deref, DerefMut)]
struct AppSettings {
    lens_distortion: LensDistortion,
}

#[derive(Component)]
struct ExampleText;

#[derive(Debug, Component)]
struct Pattern;

#[derive(Debug, Component)]
struct Spin;

// trait OrderedNode<const B: usize, const A: usize> {
//     type BeforeNodeLabels: IntoRenderNodeArray<B>;
//     type AfterNodeLabels: IntoRenderNodeArray<A>;
// }

// struct MyNode;

// impl OrderedNode<2, 1> for MyNode {
//     type BeforeNodeLabels = (Node3d::MsaaWriteback);
//     type AfterNodeLabels = ();
// }
//

pub trait IntoRenderNodeArrayV {
    fn into_array(self) -> Vec<bevy_render::render_graph::InternedRenderLabel>;
}

fn ordering() -> (impl IntoRenderNodeArrayV, impl IntoRenderNodeArrayV) {
    (
        (Node3d::MsaaWriteback, Node3d::Bloom),
        (Node3d::DepthOfField),
    )
}

/// The entry point.
fn main() {
    App::new()
        .init_resource::<AppSettings>()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Bevy Optics Distortion Example".into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin {
                    default_sampler: ImageSamplerDescriptor {
                        // Repeat the texture when sampling outside UVs in the [0., 1.] range
                        address_mode_u: ImageAddressMode::Repeat,
                        address_mode_v: ImageAddressMode::Repeat,
                        // Using nearest sampling of the colorful UV debut texture makes it look sharp,
                        // using linear would interpolate the texture making it appear "smudged".
                        ..ImageSamplerDescriptor::nearest()
                    },
                }),
        )
        .add_systems(Startup, (setup_camera, setup_text, setup_scene))
        .add_systems(Update, handle_keyboard_input)
        .add_systems(Update, scroll_uv)
        .add_systems(Update, spin)
        .add_systems(
            Update,
            (apply_settings, update_settings_and_text)
                .run_if(resource_changed::<AppSettings>)
                .after(handle_keyboard_input),
        )
        .run();
}

fn scroll_uv(
    pattern: Single<&MeshMaterial3d<StandardMaterial>, With<Pattern>>,
    time: Res<Time>,
    mut images: ResMut<Assets<StandardMaterial>>,
) {
    let Some(pattern_material) = images.get_mut(pattern.id()) else {
        return;
    };

    // Negate such that the pattern appears to scroll toward the right which
    // feels more natural.
    pattern_material.uv_transform.translation.x = -time.elapsed_secs_wrapped();
}

fn spin(mut to_spin: Single<&mut Transform, With<Spin>>, time: Res<Time>) {
    to_spin.rotate_axis(Dir3::new_unchecked(Vec3::Y), time.delta_secs() / 5.);
}

fn setup_camera(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Transform::from_xyz(0.0, 1.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
        DistanceFog {
            color: Color::srgb_u8(43, 44, 47),
            falloff: FogFalloff::Linear {
                start: 1.0,
                end: 8.0,
            },
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
            specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            intensity: 2000.0,
            ..default()
        },
        // Include the `ChromaticAberration` component.
        // ChromaticAberration::default(),
        LensDistortion::default(),
    ));
}

fn setup_text(mut commands: Commands) {
    commands.spawn((
        Text::default(),
        Node {
            position_type: PositionType::Absolute,
            top: px(4),
            left: px(4),
            ..default()
        },
        ExampleText,
    ));
}

fn setup_scene(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(5.0, 5.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
    ));

    const UV_EXTRA_SCALE: f32 = 4.0;

    // Pattern plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Z, Vec2::ONE).mesh().size(5.0, 2.0))),
        // This is the default color, but note that vertex colors are
        // multiplied by the base color, so you'll likely want this to be
        // white if using vertex colors.
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(images.add(uv_debug_texture())),
            uv_transform: Affine2::from_scale(UV_EXTRA_SCALE * Vec2::new(5., 2.)),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Pattern,
    ));

    // Helmet
    commands.spawn((
        SceneRoot(
            asset_server
                .load(GltfAssetLabel::Scene(0).from_asset("models/FlightHelmet/FlightHelmet.gltf")),
        ),
        Transform::from_xyz(0.0, 0.0, 1.0),
        Spin,
    ));
}

/// Handles requests from the user to change the chromatic aberration intensity.
fn handle_keyboard_input(mut app_settings: ResMut<AppSettings>, input: Res<ButtonInput<KeyCode>>) {
    let mut delta = 0.0;
    if input.pressed(KeyCode::ArrowLeft) {
        delta -= ADJUSTMENT_SPEED;
    } else if input.pressed(KeyCode::ArrowRight) {
        delta += ADJUSTMENT_SPEED;
    }

    // If no arrow key was pressed, just bail out.
    if delta == 0.0 {
        return;
    }

    app_settings.p1 = (app_settings.p1 + delta).clamp(-MAX_VALUE, MAX_VALUE);
}

// Propagate global settings to per-camera settings
fn apply_settings(
    mut lens_distortions: Query<&mut LensDistortion>,
    app_settings: Res<AppSettings>,
) {
    for mut distortion in &mut lens_distortions {
        *distortion = **app_settings;
    }
}

/// Creates a colorful test pattern
fn uv_debug_texture() -> Image {
    const TEXTURE_SIZE: usize = 8;

    let mut palette: [u8; 32] = [
        255, 102, 159, 255, 255, 159, 102, 255, 236, 255, 102, 255, 121, 255, 102, 255, 102, 255,
        198, 255, 102, 198, 255, 255, 121, 102, 255, 255, 236, 102, 255, 255,
    ];

    let mut texture_data = [0; TEXTURE_SIZE * TEXTURE_SIZE * 4];
    for y in 0..TEXTURE_SIZE {
        let offset = TEXTURE_SIZE * y * 4;
        texture_data[offset..(offset + TEXTURE_SIZE * 4)].copy_from_slice(&palette);
        palette.rotate_right(4);
    }

    Image::new_fill(
        Extent3d {
            width: TEXTURE_SIZE as u32,
            height: TEXTURE_SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texture_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn update_settings_and_text(
    mut settings: ResMut<AppSettings>,
    input: Res<ButtonInput<KeyCode>>,
    mut display: Single<&mut Text, With<ExampleText>>,
) {
    let up_down = |key_up, key_down, target: &mut f32| {
        let mut delta = 0.0;

        if input.pressed(key_down) {
            delta -= ADJUSTMENT_SPEED;
        } else if input.pressed(key_up) {
            delta += ADJUSTMENT_SPEED;
        }

        *target = (*target + delta).clamp(-MAX_VALUE, MAX_VALUE);
    };

    up_down(KeyCode::Digit2, KeyCode::Digit1, &mut settings.k1);
    up_down(KeyCode::KeyW, KeyCode::KeyQ, &mut settings.k2);
    up_down(KeyCode::KeyS, KeyCode::KeyA, &mut settings.k3);
    up_down(KeyCode::KeyX, KeyCode::KeyZ, &mut settings.p1);
    up_down(KeyCode::KeyR, KeyCode::KeyE, &mut settings.p2);

    if input.just_pressed(KeyCode::Space) {
        **settings = LensDistortion::default();
    }

    let LensDistortion { k1, k2, k3, p1, p2 } = **settings;

    display.0 = format!(
        r#"
        Brown-Conrady lens distortion.
        Space to reset.

        Radial distortions:
            1 / 2  K1: {k1:.2},
            Q / W  K2: {k2:.2},
            A / S  K3: {k3:.2},

        Tangential distortions:
            Z / X  P1: {p1:.2},
            E / R  P2: {p2:.2}
        "#,
    );
}
