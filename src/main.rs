use fyrox::{
    core::{
        algebra::{Matrix4, UnitQuaternion, Vector3},
        color::Color,
        parking_lot::Mutex,
        pool::Handle,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
    },
    engine::{
        executor::Executor, resource_manager::ResourceManager, Engine, EngineInitParams,
        SerializationContext,
    },
    event::{DeviceEvent, ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    material::{self, Material},
    plugin::{Plugin, PluginConstructor, PluginContext},
    resource::texture::TextureWrapMode,
    scene::{
        base::BaseBuilder,
        camera::{Camera, CameraBuilder, SkyBox, SkyBoxBuilder},
        collider::{ColliderBuilder, ColliderShape},
        light::{point::PointLightBuilder, BaseLightBuilder},
        mesh::{
            surface::{SurfaceBuilder, SurfaceData},
            MeshBuilder,
        },
        node::{Node, TypeUuidProvider},
        rigidbody::RigidBodyBuilder,
        transform::TransformBuilder,
        Scene,
    },
    window::WindowBuilder,
};
use std::{borrow::BorrowMut, sync::Arc, time};

mod circuit;

// Our game logic will be updated at 60 Hz rate.
const TIMESTEP: f32 = 1.0 / 60.0;

#[derive(Default)]
struct InputController {
    strafe_left: bool,
    strafe_right: bool,
    forward: bool,
    backward: bool,
    up: bool,
    down: bool,
}

struct Game {
    scene: Handle<Scene>,
    camera: Handle<Node>,
    input_controller: InputController,
}

impl Plugin for Game {
    fn id(&self) -> Uuid {
        GameConstructor::type_uuid()
    }

    fn update(&mut self, context: &mut PluginContext, _control_flow: &mut ControlFlow) {
        let scene = &mut context.scenes[self.scene];

        fn bool_to_float(b: bool) -> f32 {
            if b {
                1.0
            } else {
                0.0
            }
        }

        let offset_x: f32 = 0.1
            * (bool_to_float(self.input_controller.strafe_left)
                - bool_to_float(self.input_controller.strafe_right));
        let offset_z: f32 = 0.1
            * (bool_to_float(self.input_controller.forward)
                - bool_to_float(self.input_controller.backward));
        let offset_y: f32 = 0.1
            * (bool_to_float(self.input_controller.up) - bool_to_float(self.input_controller.down));

        scene.graph[self.camera]
            .as_camera_mut()
            .local_transform_mut()
            .offset(Vector3::new(offset_x, offset_y, offset_z));
    }

    fn on_os_event(
        &mut self,
        event: &Event<()>,
        _context: PluginContext,
        _control_flow: &mut ControlFlow,
    ) {
        if let Event::WindowEvent {
            event: WindowEvent::KeyboardInput { input, .. },
            ..
        } = event
        {
            if let Some(key_code) = input.virtual_keycode {
                match key_code {
                    VirtualKeyCode::A => {
                        self.input_controller.strafe_left = input.state == ElementState::Pressed
                    }
                    VirtualKeyCode::D => {
                        self.input_controller.strafe_right = input.state == ElementState::Pressed
                    }
                    VirtualKeyCode::W => {
                        self.input_controller.forward = input.state == ElementState::Pressed
                    }
                    VirtualKeyCode::S => {
                        self.input_controller.backward = input.state == ElementState::Pressed
                    }
                    VirtualKeyCode::R => {
                        self.input_controller.up = input.state == ElementState::Pressed
                    }
                    VirtualKeyCode::F => {
                        self.input_controller.down = input.state == ElementState::Pressed
                    }
                    _ => (),
                }
            }
        }
    }
}

impl Game {
    pub fn new(context: &mut PluginContext) -> Self {
        let mut scene = Scene::new();

        // Next create a camera, it is our "eyes" in the world.
        // This can also be made in editor, but for educational purpose we'll made it by hand.
        let camera = CameraBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, 6.0, -12.0))
                    .build(),
            ),
        )
        .build(&mut scene.graph);

        // Set ambient light.
        scene.ambient_lighting_color = Color::opaque(200, 200, 200);

        // Add some light.
        PointLightBuilder::new(BaseLightBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, 12.0, 0.0))
                    .build(),
            ),
        ))
        .with_radius(20.0)
        .build(&mut scene.graph);

        let mut material = Material::standard();
        material
            .set_property(
                &ImmutableString::new("diffuseColor"),
                material::PropertyValue::Color(Color::opaque(200, 20, 20)),
            )
            .unwrap();

        // Add floor.
        MeshBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, -0.25, 0.0))
                    .build(),
            ),
        )
        .with_surfaces(vec![SurfaceBuilder::new(Arc::new(Mutex::new(
            SurfaceData::make_cube(Matrix4::new_nonuniform_scaling(&Vector3::new(
                25.0, 0.25, 25.0,
            ))),
        )))
        .with_material(Arc::new(Mutex::new(material)))
        .build()])
        .build(&mut scene.graph);

        Self {
            camera,
            scene: context.scenes.add(scene),
            input_controller: InputController::default(),
        }
    }
}

struct GameConstructor;

impl TypeUuidProvider for GameConstructor {
    fn type_uuid() -> Uuid {
        uuid!("f615ac42-b259-4a23-bb44-407d753ac178")
    }
}

impl PluginConstructor for GameConstructor {
    fn create_instance(
        &self,
        _override_scene: Handle<Scene>,
        mut context: PluginContext,
    ) -> Box<dyn Plugin> {
        Box::new(Game::new(&mut context))
    }
}

// fn _main() {
//     // Configure main window first.
//     let window_builder = WindowBuilder::new().with_title("3D Shooter Tutorial");
//     // Create event loop that will be used to "listen" events from the OS.
//     let event_loop = EventLoop::new();

//     // Finally create an instance of the engine.
//     let serialization_context = Arc::new(SerializationContext::new());
//     let mut engine = Engine::new(EngineInitParams {
//         window_builder,
//         resource_manager: ResourceManager::new(serialization_context.clone()),
//         serialization_context,
//         events_loop: &event_loop,
//         vsync: false,
//     })
//     .unwrap();

//     // Initialize game instance. It is empty for now.
//     let mut game = Game::new(&mut engine);

//     // Run the event loop of the main window. which will respond to OS and window events and update
//     // engine's state accordingly. Engine lets you to decide which event should be handled,
//     // this is a minimal working example of how it should be.
//     let clock = time::Instant::now();

//     let mut elapsed_time = 0.0;
//     event_loop.run(move |event, _, control_flow| {
//         match event {
//             Event::MainEventsCleared => {
//                 // This main game loop - it has fixed time step which means that game
//                 // code will run at fixed speed even if renderer can't give you desired
//                 // 60 fps.
//                 let mut dt = clock.elapsed().as_secs_f32() - elapsed_time;
//                 while dt >= TIMESTEP {
//                     dt -= TIMESTEP;
//                     elapsed_time += TIMESTEP;

//                     // // Run our game's logic.
//                     // game.update();

//                     // Update engine each frame.
//                     engine.update(TIMESTEP, control_flow);
//                 }

//                 // Rendering must be explicitly requested and handled after RedrawRequested event is received.
//                 engine.get_window().request_redraw();
//             }
//             Event::RedrawRequested(_) => {
//                 // Render at max speed - it is not tied to the game code.
//                 engine.render().unwrap();
//             }
//             Event::WindowEvent { event, .. } => match event {
//                 WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
//                 WindowEvent::KeyboardInput { input, .. } => {
//                     // Exit game by hitting Escape.
//                     if let Some(VirtualKeyCode::Escape) = input.virtual_keycode {
//                         *control_flow = ControlFlow::Exit
//                     }
//                 }
//                 WindowEvent::Resized(size) => {
//                     // It is very important to handle Resized event from window, because
//                     // renderer knows nothing about window size - it must be notified
//                     // directly when window size has changed.
//                     engine.set_frame_size(size.into()).unwrap();
//                 }
//                 _ => (),
//             },
//             _ => *control_flow = ControlFlow::Poll,
//         }
//     });
// }

fn main() {
    let mut executor = Executor::new();
    executor.get_window().set_title("Transist");
    executor.add_plugin_constructor(GameConstructor);
    executor.run()
}
