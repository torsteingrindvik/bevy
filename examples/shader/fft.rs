//! Hey

#[path = "../helpers/camera_controller.rs"]
mod camera_controller;

use std::f32::consts::PI;

use bevy::{
    color::palettes,
    math::{NormedVectorSpace, VectorSpace},
    prelude::*,
    utils::HashMap,
};
use camera_controller::{CameraController, CameraControllerPlugin};

fn main() {
    App::new()
        .init_resource::<Colors>()
        .add_plugins((DefaultPlugins, CameraControllerPlugin))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (update, move_balls, despawn_balls, show_signal).chain(),
        )
        .observe(spinny_boy)
        .run();
}

#[derive(Debug, Event)]
struct FourierEval {
    index: usize,
    pos: Vec3,
}

#[derive(Debug, Component)]
struct Ballz;

#[derive(Debug, Resource, Deref)]
struct Colors(HashMap<usize, Color>);

impl Default for Colors {
    fn default() -> Self {
        Self(HashMap::from_iter(
            [
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
            ]
            .into_iter()
            .map(Into::into)
            .enumerate(),
        ))
    }
}

fn spinny_boy(
    trigger: Trigger<FourierEval>,
    colors: Res<Colors>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
    mut mat: Local<HashMap<usize, Handle<StandardMaterial>>>,
) {
    let FourierEval { index, pos } = trigger.event();

    let mesh_handle = mesh
        .get_or_insert(meshes.add(Sphere::default().mesh().ico(5).unwrap()))
        .clone_weak();
    let mat_handle = mat
        .entry(*index)
        .or_insert_with(|| {
            materials.add({
                let mut mat = StandardMaterial::from(colors[index]);
                mat.unlit = true;
                mat
            })
        })
        .clone_weak();

    commands.spawn((
        PbrBundle {
            mesh: mesh_handle,
            material: mat_handle,
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

fn x(t: f32, f: f32) -> f32 {
    ((2. * PI * f * t).sin()
        + (0.1 * (2. * PI * f * 7. * t).sin())
        + (0.1 * (2. * PI * f * 13. * t).cos()))
        / 1.20
}

fn show_signal(mut gizmos: Gizmos, t: Res<Time>) {
    let t = t.elapsed_seconds() * 25.0;

    gizmos.grid_2d(
        Vec2::ZERO, // Add some bias to avoid flicker
        0.0,
        UVec2::new(100, 100),
        Vec2::ONE,
        palettes::tailwind::BLUE_200.with_alpha(0.01),
    );

    let bias = 0.01;

    let freqs = [1.0, 2.0, 3.0, 4.0, 5.0, 10.0, 25.0];
    for (index, freq) in freqs.iter().enumerate() {
        // VARS
        let y_scaling = 2.5;
        let y_span = freqs.len() as f32 * y_scaling;
        let y_offset = index as f32 * y_scaling - (y_span / 2.);
        let width = 20.0;
        let num_ms = 1000;
        let dx = width / num_ms as f32;

        // DRAW SIGNAL
        fn ms_to_signal(ms: u64, freq: f32) -> f32 {
            x(ms as f32 / 1000.0, freq)
        }

        let positions = (0..num_ms).into_iter().map(|ms| {
            let x = ms as f32 * dx - width / 2.;
            Vec2::new(x, ms_to_signal(ms + t as u64, *freq) + y_offset).extend(bias)
        });

        gizmos.linestrip(positions, palettes::tailwind::INDIGO_500.with_alpha(0.3));

        // SAMPLE SIGNAL
        let samples = (0..num_ms)
            .into_iter()
            .step_by(10)
            .map(|ms| {
                let x = ms as f32 * dx - width / 2.;
                Vec2::new(x, ms_to_signal(ms + t as u64, *freq) + y_offset).extend(bias)
            })
            .collect::<Vec<_>>();

        for sample in &samples {
            gizmos
                .sphere(*sample, Quat::default(), 0.05, palettes::tailwind::RED_600)
                .resolution(3);
        }

        let lines = samples
            .windows(2)
            .map::<&[Vec3; 2], _>(|w| w.try_into().unwrap())
            .flat_map(|[sample0, sample1]| [*sample0, Vec3::new(sample1.x, sample0.y, bias)]);

        gizmos.linestrip(
            lines.chain([*samples.last().unwrap()]),
            palettes::tailwind::RED_600,
        );
    }
}

fn update(
    // mut commands: Commands,
    // mut gizmos: Gizmos,
    // t: Res<Time>,
    // colors: Res<Colors>,
    mut iter: Local<usize>,
) {
    return;

    let mut max_sum = -1.0;
    let mut max_k = usize::MAX;

    const N: usize = 10000;

    if *iter == 0 {
        info!("gooing for it");
    }

    for k in 1..=(N / 2) {
        let mut sum = 0.0;

        for n in 0..N {
            let n = n as f32;
            let k = k as f32;

            // sum += x(n) * (2. * PI * k * n / N as f32).cos();

            // let freq = index as f32 / 10.0;
            // // let start = Vec3::new(index as f32, 0.0, 0.0);

            // let mut sum = 0.0;
            // for t in 0..100_000 {
            //     let t = t as f32 / 1000.0;

            //     let re = (freq * t * std::f32::consts::TAU).cos();
            //     // sum += g(t) * Vec2::new(re, im).length();
            //     sum += g(t) * re;
            // }

            // // sum /= 1000.0;

            // // let pos = center + Vec3::new(re, im, 0.0) * amplitude * signal;

            // // let color = colors[&index];
            // let ampl = Vec3::Y * sum;
            // // gizmos.arrow(start, start + ampl, color);

            // // commands.trigger(FourierEval { pos: ampl, index });

            // if *iter == 0 {
            //     info!("f:{freq:.2}, index {index}, sum:{sum:.2}");
            // }
            // if sum > max_sum {
            //     max_sum = sum;
            //     max_index = index;
            // }
        }

        // sum *= 2.0;
        // sum /= N as f32;

        if sum > max_sum {
            max_sum = sum;
            max_k = k;
        }
    }

    if *iter == 0 {
        info!("max sum: {max_sum:.8} @ {max_k}");
    }

    *iter += 1;
}
