//! A minimal example that outputs "hello world"

use bevy::prelude::*;

// fn main() {
//     let file_appender = tracing_appender::rolling::hourly(".", "my-funny-foo.log");
//     let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
//     bevy::log::tracing_subscriber::fmt()
//         .with_writer(non_blocking)
//         .init();

//     App::new()
//         .add_plugins(DefaultPlugins)
//         .add_systems(Startup, |mut commands: Commands| {
//             commands.spawn(Camera3dBundle {
//                 transform: Transform::from_xyz(0.0, 2.5, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
//                 ..default()
//             });
//         })
//         .add_systems(Update, |mut gizmos: Gizmos| {
//             gizmos.arrow(Vec3::ZERO, Vec3::ONE, Color::default());
//         })
//         .run();
// }

#[path = "helpers/camera_controller.rs"]
mod camera_controller;

use bevy::{color::palettes, prelude::*};
use camera_controller::{CameraController, CameraControllerPlugin};

fn main() {
    let file_appender = tracing_appender::rolling::hourly(".", "my-funny-foo.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    bevy::log::tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .init();

    App::new()
        .add_plugins((DefaultPlugins, CameraControllerPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (update, move_balls, despawn_balls).chain())
        .observe(spinny_boy)
        .run();
}

#[derive(Debug, Event)]
struct FourierEval {
    pos: Vec3,
    color: Color,
}

#[derive(Debug, Component)]
struct Ballz;

fn spinny_boy(
    trigger: Trigger<FourierEval>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
) {
    let FourierEval { pos, color } = trigger.event();

    let mesh_handle = mesh
        .get_or_insert(meshes.add(Sphere::default().mesh().ico(5).unwrap()))
        .clone_weak();
    let material = materials.add(StandardMaterial::from(*color));

    commands.spawn((
        PbrBundle {
            mesh: mesh_handle,
            material: Default::default(),
            transform: Transform::from_translation(*pos).with_scale(Vec3::splat(0.025)),
            ..default()
        },
        Ballz,
    ));
}

fn move_balls(t: Res<Time>, mut q: Query<&mut Transform, With<Ballz>>) {
    let dt = t.delta_seconds();

    q.par_iter_mut().for_each(|mut transform| {
        transform.translation += dt * Vec3::NEG_Z * 1.5;
    });
}

fn despawn_balls(mut commands: Commands, q: Query<(Entity, &Transform), With<Ballz>>) {
    for (entity, transform) in &q {
        if transform.translation.z < -10.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn setup(mut commands: Commands) {
    // camera
    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        CameraController::default(),
    ));
}

// fn signal(time: f32) -> f32 {}

fn update(mut commands: Commands, mut gizmos: Gizmos, t: Res<Time>) {
    let colors = [
        palettes::tailwind::CYAN_100,
        palettes::tailwind::EMERALD_300,
        palettes::tailwind::FUCHSIA_400,
        palettes::tailwind::GREEN_300,
        palettes::tailwind::NEUTRAL_500,
        palettes::tailwind::ORANGE_400,
        palettes::tailwind::ROSE_700,
        palettes::tailwind::ROSE_700,
        palettes::tailwind::SKY_800,
        palettes::tailwind::YELLOW_500,
    ];

    // I need to sum over time, not use elapsed time.
    // And likely lots of timesteps
    for index in 0..=9 {
        // From 0.1 to 1.0 Hz
        let freq = (index as f32 + 1.0) / 10.0 * std::f32::consts::TAU;
        let elapsed = t.elapsed_seconds();
        let center = Vec3::new(index as f32, 0.0, 0.0);

        let (im, re) = (elapsed * freq).sin_cos();
        let amplitude = 1. / 3.;
        let signal = elapsed.sin();

        let pos = center + Vec3::new(re, im, 0.0) * amplitude * signal;

        let color = colors[index];
        gizmos.arrow(center, pos, color);

        commands.trigger(FourierEval {
            pos,
            color: color.into(),
        });
    }
}
