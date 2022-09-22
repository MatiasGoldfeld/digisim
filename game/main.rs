use std::{cell::Cell, sync::Arc};

use bevy::{
    input::mouse::MouseMotion,
    prelude::*,
    utils::{Duration, HashMap},
};
use bevy_rapier3d::prelude::*;
use digisim::{
    circuit::{self, Circuit},
    circuit_fast::CircuitFast as UsedCircuit,
};

#[derive(Default)]
struct CameraState {
    pitch: f32,
    yaw: f32,
}

impl CameraState {
    fn camera_movement(
        windows: Res<Windows>,
        mut camera_state: ResMut<CameraState>,
        keyboard_input: Res<Input<KeyCode>>,
        mut mouse_motion_events: EventReader<MouseMotion>,
        mut transforms: Query<&mut Transform, With<Camera3d>>,
    ) {
        let window = windows.get_primary().unwrap();
        if !window.cursor_locked() {
            return;
        }

        let mut transform = transforms.get_single_mut().unwrap();

        let boost = if keyboard_input.pressed(KeyCode::LShift) {
            2.0
        } else {
            1.0
        };
        let move_speed = 0.1 * boost;
        let rotate_speed = 0.000003;

        let input = |key_code: KeyCode| {
            if keyboard_input.pressed(key_code) {
                move_speed
            } else {
                0.0
            }
        };

        let x = input(KeyCode::D) - input(KeyCode::A);
        let z = input(KeyCode::S) - input(KeyCode::W);
        let y = input(KeyCode::R) - input(KeyCode::F);

        let mut translation = transform.rotation * Vec3::new(x, 0.0, z);
        translation.y += y;
        transform.translation += translation;

        let window_scale = window.width().min(window.height());
        for mouse_motion in mouse_motion_events.iter() {
            camera_state.pitch = (camera_state.pitch
                - rotate_speed * mouse_motion.delta.y * window_scale)
                .clamp(-85f32.to_radians(), 85f32.to_radians());
            camera_state.yaw -= rotate_speed * mouse_motion.delta.x * window_scale;
            transform.rotation =
                Quat::from_rotation_y(camera_state.yaw) * Quat::from_rotation_x(camera_state.pitch);
        }
    }
}

#[derive(Clone)]
struct Coord {
    x: i32,
    y: i32,
    z: i32,
}

enum Side {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    fn opposite(&self) -> Self {
        use Side::*;
        match self {
            Front => Back,
            Back => Front,
            Left => Right,
            Right => Left,
            Top => Bottom,
            Bottom => Top,
        }
    }
}

enum CircuitNodeType {
    Wire {
        active: Arc<Cell<bool>>,
        wires: HashMap<Coord, Entity>,
    },
    Inverter,
}

struct CircuitNode {
    node_id: <UsedCircuit as Circuit>::NodeId,
    contents: CircuitNodeType,
}

impl CircuitNode {
    fn connect(&self, side: Side, other: &Self, circuit: &mut UsedCircuit) {}
}

struct Game {
    circuit: UsedCircuit,
    block_mesh: Handle<Mesh>,
    blocks: HashMap<Coord, Entity>,
    last_tick: Duration,
}

impl Game {
    fn setup(
        mut commands: Commands,
        mut materials: ResMut<Assets<StandardMaterial>>,
        mut meshes: ResMut<Assets<Mesh>>,
        time: Res<Time>,
    ) {
        let game = Game {
            circuit: circuit::Circuit::new(),
            block_mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
            blocks: HashMap::new(),
            last_tick: time.time_since_startup(),
        };
        Voxels::setup(&game, &mut commands, materials);
        commands.insert_resource(game);
    }

    fn spawn_block(
        &self,
        commands: &mut Commands,
        materials: &mut Assets<StandardMaterial>,
        x: i32,
        y: i32,
        z: i32,
        color: Color,
    ) {
        commands
            .spawn_bundle(PbrBundle {
                mesh: self.block_mesh.clone(),
                material: materials.add(StandardMaterial {
                    emissive: color,
                    // perceptual_roughness: 1.0,
                    // reflectance: 0.0,
                    ..Default::default()
                }),
                transform: Transform::from_xyz(x as f32, y as f32, z as f32),
                ..default()
            })
            .insert(Collider::cuboid(0.5, 0.5, 0.5));
    }

    fn tick(mut game: ResMut<Game>, time: Res<Time>) {
        let now = time.time_since_startup();
        if now - game.last_tick >= Duration::from_secs(1) {
            // TODO: This better
            game.circuit.run(1);
            game.last_tick = now;
        }
    }
}

pub struct Voxels;

impl Voxels {
    fn setup(
        game: &Game,
        commands: &mut Commands,
        mut materials: ResMut<Assets<StandardMaterial>>,
    ) {
        let size = 20;

        use rand::Rng;
        let mut rng = rand::thread_rng();
        for x in 0..size {
            for z in 0..size {
                game.spawn_block(
                    commands,
                    &mut materials,
                    x,
                    0,
                    z,
                    Color::rgb_u8(
                        rng.gen_range(0..=255),
                        rng.gen_range(0..=255),
                        rng.gen_range(0..=255),
                    ),
                )
            }
        }

        let middle_block = Vec3::new(10.0, 1.0, 10.0);
        game.spawn_block(
            commands,
            &mut materials,
            10,
            1,
            10,
            Color::rgb_u8(30, 180, 60),
        );

        // let scale = 1.0 / 10.0;
        // let proj_size = 100.0;
        // commands.spawn_bundle(DirectionalLightBundle {
        //     directional_light: DirectionalLight {
        //         illuminance: 20000.0,
        //         shadows_enabled: true,
        //         // shadow_depth_bias: 0.07,
        //         shadow_projection: OrthographicProjection {
        //             left: -proj_size,
        //             right: proj_size,
        //             bottom: -proj_size,
        //             top: proj_size,
        //             near: -proj_size,
        //             far: proj_size,
        //             scale,
        //             ..Default::default()
        //         },
        //         ..default()
        //     },
        //     transform: Transform::from_xyz(5.0, 10.0, 3.0).looking_at(middle_block, Vec3::Y),
        //     ..default()
        // });
        commands.insert_resource(AmbientLight {
            brightness: 0.05,
            ..Default::default()
        });

        // camera
        {
            let transform = Transform::from_xyz(0.0, 2.5, 0.0).looking_at(middle_block, Vec3::Y);
            commands.spawn_bundle(Camera3dBundle {
                transform,
                ..default()
            });

            let (pitch, yaw, _) = transform.rotation.to_euler(EulerRot::XYZ);
            commands.insert_resource(CameraState { pitch, yaw })
        }

        commands
            .spawn_bundle(NodeBundle {
                style: Style {
                    size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                    position_type: PositionType::Absolute,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..Default::default()
                },
                color: Color::rgba(0.0, 0.0, 0.0, 0.0).into(),
                ..Default::default()
            })
            .with_children(|parent| {
                parent.spawn_bundle(ImageBundle {
                    color: Color::WHITE.into(),
                    style: Style {
                        size: Size::new(Val::Px(4.0), Val::Px(4.0)),
                        ..default()
                    },
                    ..default()
                });
            });
    }

    fn grab_mouse(
        mut windows: ResMut<Windows>,
        mouse: Res<Input<MouseButton>>,
        key: Res<Input<KeyCode>>,
    ) {
        let window = windows.get_primary_mut().unwrap();
        if mouse.just_pressed(MouseButton::Left) {
            window.set_cursor_visibility(false);
            window.set_cursor_lock_mode(true);
        }
        if key.just_pressed(KeyCode::Escape) {
            window.set_cursor_visibility(true);
            window.set_cursor_lock_mode(false);
        }
    }

    fn cursor_ray(
        game: Res<Game>,
        mut commands: Commands,
        mut materials: ResMut<Assets<StandardMaterial>>,
        mouse: Res<Input<MouseButton>>,
        camera_transforms: Query<&Transform, With<Camera3d>>,
        rapier_context: Res<RapierContext>,
        transform_query: Query<&Transform>,
    ) {
        let build = mouse.just_pressed(MouseButton::Right);
        let destroy = mouse.just_pressed(MouseButton::Left);
        if build || destroy {
            let camera_transform = camera_transforms.get_single().unwrap();
            // TODO: Doesn't need normal for destroy
            if let Some((entity, intersection)) = rapier_context.cast_ray_and_get_normal(
                camera_transform.translation,
                camera_transform.rotation * Vec3::NEG_Z,
                15.0,
                false,
                QueryFilter::only_fixed(),
            ) {
                if build {
                    if let Ok(transform) = transform_query.get(entity) {
                        let Vec3 { x, y, z } = transform.translation + intersection.normal;
                        info!("Block created at {x}, {y}, {z}");
                        game.spawn_block(
                            &mut commands,
                            &mut materials,
                            x as i32,
                            y as i32,
                            z as i32,
                            Color::WHITE,
                        );
                    }
                } else if destroy {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

impl Plugin for Voxels {
    fn build(&self, app: &mut App) {
        app.add_startup_system(Game::setup)
            .add_system(Game::tick)
            .add_system(Voxels::grab_mouse)
            .add_system(Voxels::cursor_ray)
            .add_system(CameraState::camera_movement);
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugin(RapierDebugRenderPlugin::default())
        .add_plugin(Voxels)
        .add_plugin(bevy::diagnostic::LogDiagnosticsPlugin::default())
        .add_plugin(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .run();
}
