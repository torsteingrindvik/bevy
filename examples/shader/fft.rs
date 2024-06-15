//! Hey

use bevy::{color::palettes, prelude::*};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (update, move_balls, despawn_balls).chain())
        .observe(spinny_boy)
        .run();
}

#[derive(Debug, Event)]
struct FourierEval {
    pos: Vec3,
}

#[derive(Debug, Component)]
struct Ballz;

fn spinny_boy(
    trigger: Trigger<FourierEval>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
) {
    let FourierEval { pos } = trigger.event();

    let mesh_handle = mesh
        .get_or_insert(meshes.add(Sphere::default().mesh().ico(5).unwrap()))
        .clone_weak();

    commands.spawn((
        PbrBundle {
            mesh: mesh_handle,
            material: Default::default(),
            transform: Transform::from_translation(*pos).with_scale(Vec3::splat(0.005)),
            ..default()
        },
        Ballz,
    ));
}

fn move_balls(t: Res<Time>, mut q: Query<&mut Transform, With<Ballz>>) {
    let dt = t.delta_seconds();

    q.par_iter_mut().for_each(|mut transform| {
        transform.translation += dt * Vec3::NEG_Z * 0.1;
        // transform.scale *= Vec3::splat(0.99);
    });
}

fn despawn_balls(mut commands: Commands, q: Query<(Entity, &Transform), With<Ballz>>) {
    for (entity, transform) in &q {
        if transform.translation.z < -1.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn setup(mut commands: Commands) {
    // camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
}

fn update(mut commands: Commands, mut gizmos: Gizmos, t: Res<Time>) {
    let v0 = Vec3::ZERO;

    let (im, re) = t.elapsed_seconds().sin_cos();
    let pos = Vec3::new(re, im, 0.0);

    gizmos.arrow(v0, pos, palettes::tailwind::AMBER_100);

    commands.trigger(FourierEval { pos });
}
