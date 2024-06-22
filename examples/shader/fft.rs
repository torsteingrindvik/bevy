//! Hey

#[path = "../helpers/camera_controller.rs"]
mod camera_controller;

use std::f32::consts::PI;

use bevy::{color::palettes, prelude::*};
use camera_controller::{CameraController, CameraControllerPlugin};

fn main() {
    App::new()
        .insert_resource(DisplaySettings {
            time_scaling: 3.0,
            amplitude_scaling: 3.0,
        })
        .add_plugins((DefaultPlugins, CameraControllerPlugin))
        .add_systems(Startup, (setup, spawn_signals))
        .add_systems(
            Update,
            (
                show_grid,
                update_true_signal,
                resample,
                calc_fft,
                show_signals,
                spawn_bins,
                show_ffts,
            )
                .chain(),
        )
        .run();
}

#[derive(Debug, Component)]
struct FftSettings {
    bins: usize,
    max_freq: f32,

    display_width: f32,
    normalized_height: f32,
    xy_offset: Vec2,
}

#[derive(Debug, Component)]
struct FftResult {
    results: Vec<f32>,
}

#[derive(Debug, Component)]
struct DisplayColor {
    color: Color,
}

#[derive(Debug, Component)]
struct FftBin;

fn spawn_bins(
    mut commands: Commands,
    has_changed_fft: Query<(Entity, &FftSettings, &DisplayColor), Changed<FftSettings>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = mesh
        .get_or_insert_with(|| {
            meshes.add(
                Cylinder {
                    radius: 8.0,
                    half_height: 0.5,
                }
                .mesh(),
            )
        })
        .clone_weak();

    for (entity, settings, display) in &has_changed_fft {
        info!("Resetting");
        let mut sm = StandardMaterial::from(display.color);
        sm.unlit = true;
        let material = materials.add(sm);

        // Reset
        commands
            .entity(entity)
            .despawn_descendants()
            .with_children(|parent| {
                for _ in 0..settings.bins {
                    parent.spawn((
                        PbrBundle {
                            mesh: mesh.clone_weak(),
                            material: material.clone(),
                            ..default()
                        },
                        FftBin,
                    ));
                }
            });
    }
}

fn show_ffts(
    time: Res<Time>,
    ffts: Query<(Entity, &FftResult, &FftSettings)>,
    children: Query<&Children>,
    mut transforms: Query<&mut Transform, With<FftBin>>,
) {
    for (
        entity,
        fft,
        &FftSettings {
            bins,
            display_width,
            normalized_height,
            xy_offset,
            ..
        },
    ) in &ffts
    {
        let num_results = fft.results.len();
        assert_eq!(num_results, bins);

        let max_height = fft
            .results
            .iter()
            .copied()
            // .map(|v| v.log10())
            .reduce(f32::max)
            .unwrap();
        let scaling = 1. / max_height * normalized_height;

        for (index, (bin, &result)) in children
            .iter_descendants(entity)
            .zip(&fft.results)
            .enumerate()
        {
            let x_pos = (index as f32) / (bins as f32) * display_width;

            // Do an average using next and prev bin to smooth things out a bit
            let this = result;
            let prev = fft.results[index.saturating_sub(1)];
            let next = fft.results[(index + 1).min(bins - 1)];
            let height = ((this + prev + next) / 3.).max(0.0) * scaling;

            let edge_length = 1. / bins as f32;
            let translation =
                (xy_offset + Vec2::new(x_pos, 1.0 + height / 2.)).extend(-edge_length / 2.);

            let scale = Vec3::new(edge_length, height, edge_length);

            let mut transform = transforms.get_mut(bin).unwrap();

            let mut new_transform = transform.with_translation(translation).with_scale(scale);
            new_transform.rotate_axis(Dir3::Y, time.delta_seconds() * 1.0);

            *transform = new_transform;
        }
    }
}

fn spawn_signals(mut commands: Commands) {
    let true_signal_sample_time = 10.0;
    let true_signal_frequency = 2000.0;

    commands.spawn((
        SignalSettings {
            frequency: true_signal_frequency,
            sample_time: true_signal_sample_time,
            time_offset: 0.0,
        },
        SignalDisplay {
            depth_bias: 0.0,
            style: DisplayStyle::Shortest,
            amplitude_offset: 0.0,
        },
        DisplayColor {
            color: palettes::tailwind::INDIGO_500.with_alpha(0.3).into(),
        },
    ));

    use palettes::tailwind as colors;
    let colors = [
        colors::LIME_200,
        colors::GREEN_500,
        colors::RED_600,
        colors::BLUE_200,
        colors::ORANGE_500,
        colors::PINK_400,
    ];

    for (index, color) in colors.iter().enumerate() {
        let mut cmds = commands.spawn((
            SpatialBundle::default(),
            SignalSettings {
                frequency: true_signal_frequency / (2.0f32).powi((index + 1) as i32),
                sample_time: true_signal_sample_time / 2.,
                time_offset: true_signal_sample_time / 4.,
            },
            SignalDisplay {
                depth_bias: 0.1 * ((index + 1) as f32),
                style: DisplayStyle::Flat,
                amplitude_offset: -0.1 * (index as f32),
            },
            DisplayColor {
                color: (*color).into(),
            },
        ));

        if index == 0 {
            // Use this one for the Fft calc
            cmds.insert(FftSettings {
                bins: 300,
                display_width: 20.0,
                normalized_height: 7.5,
                max_freq: 128.0,
                xy_offset: Vec2::new(2.0, 1.0),
            });
        }
    }
}

fn resample(
    mut commands: Commands,
    changed_signals: Query<(Entity, &SignalSettings)>,
    input_signal: Res<TrueSignal>,
) {
    for (entity, settings) in &changed_signals {
        let num_samples = settings.num_samples();

        let samples = (0..num_samples)
            .into_iter()
            .map(|sample| (sample as f32 / num_samples as f32) * settings.sample_time)
            .map(|t| (input_signal.f)(t + settings.time_offset))
            .collect();

        commands.entity(entity).insert(Samples { samples });
    }
}

fn calc_fft(mut commands: Commands, signal: Query<(Entity, &Samples, &FftSettings)>) {
    let (entity, signal, fft_settings) = signal.single();
    let num_samples = signal.samples.len();

    let mut results = vec![];

    // E.g.:
    //  * 300 max freq + 100 bins -> each bin strides 3 Hz
    //  * 100 max freq + 250 bins -> each bin strides 0.4 Hz
    let per_bin_freq = fft_settings.max_freq / fft_settings.bins as f32;

    for k in 0..fft_settings.bins {
        let f = k as f32 * per_bin_freq;
        let mut sum = Vec2::ZERO;
        for n in 0..num_samples {
            // p ∈ [0.0, 1.0)
            let p = n as f32 / num_samples as f32;
            let (re, im) = (2.0 * PI * f * p).sin_cos();
            let test_signal = Vec2::new(-im, re);

            sum += test_signal * signal.samples[n];
        }
        results.push(sum.length());
    }

    commands.entity(entity).insert(FftResult { results });
}

fn show_signals(
    mut gizmos: Gizmos,
    signals: Query<(&Samples, &SignalSettings, &SignalDisplay, &DisplayColor)>,
    display: Res<DisplaySettings>,
) {
    for (signal, settings, signal_display, display_color) in &signals {
        let positions = signal.samples.iter().enumerate().map(|(n, &amplitude)| {
            Vec3::new(
                ((n as f32 / settings.frequency) + settings.time_offset) * display.time_scaling,
                (amplitude + signal_display.amplitude_offset) * display.amplitude_scaling,
                signal_display.depth_bias, // Avoids Z fighting
            )
        });

        match signal_display.style {
            DisplayStyle::Shortest => {
                gizmos.linestrip(positions, display_color.color);
            }
            DisplayStyle::Flat => {
                let lines = positions.collect::<Vec<_>>();
                let last = lines.last().unwrap();
                let lines = lines
                    .windows(2)
                    .map::<&[Vec3; 2], _>(|w| w.try_into().unwrap())
                    .flat_map(|[sample0, sample1]| {
                        [
                            *sample0,
                            Vec3::new(sample1.x, sample0.y, signal_display.depth_bias),
                        ]
                    })
                    .chain([*last]);

                gizmos.linestrip(lines, display_color.color);
            }
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

    // commands.spawn(PointLightBundle {
    //     point_light: PointLight {
    //         shadows_enabled: true,
    //         intensity: 10_000_000.,
    //         range: 1000.0,
    //         shadow_depth_bias: 0.2,
    //         ..default()
    //     },
    //     transform: Transform::from_xyz(8.0, 16.0, 8.0),
    //     ..default()
    // });
}

#[derive(Resource)]
struct TrueSignal {
    f: Box<dyn Fn(f32) -> f32 + 'static + Send + Sync>,
}

fn update_true_signal(mut commands: Commands, time: Res<Time>) {
    let sin = |t, f| {
        let t: f32 = 2.0 * PI * t * f;
        t.sin()
    };

    let varying = 3.0 + (time.elapsed_seconds() / 4.).sin();

    let hifreq = 20.0 + (time.elapsed_seconds()).sin();

    commands.insert_resource(TrueSignal {
        f: Box::new(move |t| {
            (sin(t, 1.0) * 2.0 + sin(t, varying) * 0.5 + sin(t, 7.0) * 0.25 + sin(t, hifreq) * 0.1)
                / 6.0
        }),
    });
}

#[derive(Debug, Component)]
struct Samples {
    samples: Vec<f32>,
}

#[derive(Debug, Component)]
struct SignalSettings {
    frequency: f32,
    sample_time: f32,
    time_offset: f32,
}

impl SignalSettings {
    fn num_samples(&self) -> usize {
        // E.g. 500 Hz * 2.0 secs -> 1000 samples
        (self.frequency * self.sample_time) as usize
    }
}

#[derive(Debug)]
enum DisplayStyle {
    /// Linestrip directly connecting each point
    Shortest,

    /// Linestrip but only horizontal or vertical line pieces
    Flat,
}

#[derive(Debug, Component)]
struct SignalDisplay {
    depth_bias: f32,
    style: DisplayStyle,
    amplitude_offset: f32,
}

fn show_grid(mut gizmos: Gizmos) {
    gizmos.grid_2d(
        Vec2::ZERO,
        0.0,
        UVec2::new(100, 100),
        Vec2::ONE,
        palettes::tailwind::BLUE_200.with_alpha(0.01),
    );
}

#[derive(Debug, Resource)]
struct DisplaySettings {
    /// How wide should a signal's time axis be.
    /// E.g. 1000 samples at 1 kHz is 1 second,
    /// and the width of that is 1 second times scaling.
    time_scaling: f32,

    /// Scales height of amplitudes
    amplitude_scaling: f32,
}
