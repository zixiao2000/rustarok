extern crate byteorder;
extern crate sdl2;
extern crate gl;
extern crate nalgebra;
extern crate encoding;
#[macro_use]
extern crate imgui;
extern crate imgui_sdl2;
extern crate imgui_opengl_renderer;
extern crate websocket;
#[macro_use]
extern crate log;
extern crate specs;
#[macro_use]
extern crate specs_derive;

use strum::IntoEnumIterator;

use std::io::ErrorKind;
use crate::common::BinaryReader;
use crate::rsw::Rsw;
use crate::gnd::Gnd;
use crate::gat::{Gat, CellType};

use imgui::ImString;
use nalgebra::{Vector3, Matrix4, Point3, Unit, Rotation3, Vector2, Point2, Isometry, Isometry2};
use crate::video::{Shader, ShaderProgram, VertexArray, VertexAttribDefinition, GlTexture, Video, VIDEO_HEIGHT, VIDEO_WIDTH, ortho};
use std::time::{Duration, SystemTime, Instant};
use std::collections::{HashMap, HashSet};
use crate::rsm::{Rsm, BoundingBox};
use sdl2::keyboard::{Keycode, Scancode};
use crate::act::ActionFile;
use crate::spr::SpriteFile;
use rand::Rng;
use websocket::stream::sync::TcpStream;
use websocket::{OwnedMessage, WebSocketError};
use log::LevelFilter;
use std::sync::Mutex;
use specs::Builder;
use specs::Join;
use specs::prelude::*;
use std::path::Path;
use crate::consts::{job_name_table, JobId, MonsterId};
use crate::systems::{SystemStopwatch, SystemVariables, SystemFrameDurations, EffectSprites, Sprites, Sex};
use crate::systems::render::{PhysicsDebugDrawingSystem, OpenGlInitializerFor3D, RenderStreamingSystem, RenderDesktopClientSystem, DamageRenderSystem};
use crate::systems::input::{InputConsumerSystem, BrowserInputProducerSystem};
use crate::systems::phys::{PhysicsSystem, FrictionSystem};
use rand::prelude::ThreadRng;
use ncollide2d::shape::ShapeHandle;
use nphysics2d::object::{ColliderDesc, Collider, BodyHandle};
use std::ops::{Bound, Div};
use ncollide2d::world::CollisionGroups;
use crate::systems::ui::RenderUI;
use crate::systems::control::CharacterControlSystem;
use nphysics2d::solver::SignoriniModel;
use crate::components::char::{PhysicsComponent, CharacterStateComponent, PlayerSpriteComponent, MonsterSpriteComponent, ComponentRadius};
use crate::components::controller::ControllerComponent;
use crate::components::{BrowserClient, FlyingNumberComponent};
use crate::components::skill::{PushBackWallSkill, SkillManifestationComponent};
use crate::systems::skill_sys::SkillSystem;
use crate::systems::char_state_sys::CharacterStateUpdateSystem;

mod common;
mod cursor;
mod cam;
mod video;
mod gat;
mod str;
mod rsw;
mod gnd;
mod rsm;
mod act;
mod spr;
mod consts;

mod components;
mod systems;

pub type PhysicsWorld = nphysics2d::world::World<f32>;

pub const TICKS_PER_SECOND: u64 = 1000 / 30;

#[derive(Clone, Copy)]
pub enum ActionIndex {
    Idle = 0,
    Walking = 8,
    Sitting = 16,
    PickingItem = 24,
    StandBy = 32,
    Attacking1 = 40,
    ReceivingDamage = 48,
    Freeze1 = 56,
    Dead = 65,
    Freeze2 = 72,
    Attacking2 = 80,
    Attacking3 = 88,
    CastingSpell = 96,
}

#[derive(Clone, Copy)]
pub enum MonsterActionIndex {
    Idle = 0,
    Walking = 8,
    Attack = 16,
    ReceivingDamage = 24,
    Die = 32,
}

const STATIC_MODELS_COLLISION_GROUP: usize = 1;
const LIVING_COLLISION_GROUP: usize = 2;
const SKILL_AREA_COLLISION_GROUP: usize = 3;

#[derive(Clone)]
pub struct SpriteResource {
    action: ActionFile,
    textures: Vec<spr::SpriteTexture>,
}

impl SpriteResource {
    pub fn new(path: &str) -> SpriteResource {
        trace!("Loading {}", path);
        let frames: Vec<spr::SpriteTexture> = SpriteFile::load(
            BinaryReader::new(format!("{}.spr", path))
        ).frames
            .into_iter()
            .map(|frame| spr::SpriteTexture::from(frame))
            .collect();
        let action = ActionFile::load(
            BinaryReader::new(format!("{}.act", path))
        );
        SpriteResource {
            action,
            textures: frames,
        }
    }
}


pub struct Shaders {
    pub ground_shader: ShaderProgram,
    pub model_shader: ShaderProgram,
    pub sprite_shader: ShaderProgram,
    pub player_shader: ShaderProgram,
    pub sprite2d_shader: ShaderProgram,
    pub trimesh_shader: ShaderProgram,
    pub trimesh2d_shader: ShaderProgram,
}

//áttetsző modellek
//  csak a camera felé néző falak rajzolódjanak ilyenkor ki
//  a modelleket z sorrendben növekvőleg rajzold ki
//jobIDt tartalmazzon ne indexet a sprite
// guild_vs4.rsw
// implement attack range check with proximity events
//3xos gyorsitás = 1 frame alatt 3x annyi minden történik (3 physics etc
// tick helyett idő mértékgeységgel számolj
// legyen egy központi abstract renderer, és neki külkdjenek a rendszerek
//  render commandokat, ő pedig hatékonyan csoportositva rajzolja ki azokat


pub struct RenderMatrices {
    pub projection: Matrix4<f32>,
    pub ortho: Matrix4<f32>,
    pub view: Matrix4<f32>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tick(u64);

#[derive(Copy, Clone, Debug)]
pub struct DeltaTime(pub f32);

#[derive(Debug, Copy, Clone)]
pub struct ElapsedTime(f32);

impl PartialEq  for ElapsedTime {
    fn eq(&self, other: &Self) -> bool {
        (self.0 * 1000.0) as u32 == (other.0 * 1000.0) as u32
    }
}

impl Eq for ElapsedTime {}

impl ElapsedTime {
    pub fn add_seconds(&self, seconds: f32) -> ElapsedTime {
        ElapsedTime(self.0 + seconds as f32)
    }

    pub fn diff(&self, other: &ElapsedTime) -> ElapsedTime {
        ElapsedTime(other.0 - self.0)
    }

    pub fn percentage_between(&self, from: &ElapsedTime, to: &ElapsedTime) -> f32 {
        let current = self.0 - from.0;
        let end = to.0 - from.0;
        return current / end;
    }

    pub fn add(&self, other: &ElapsedTime) -> ElapsedTime {
        ElapsedTime(self.0 + other.0)
    }

    pub fn elapsed_since(&self, other: &ElapsedTime) -> ElapsedTime {
        ElapsedTime(self.0 - other.0)
    }

    pub fn div(&self, other: f32) -> f32 {
        self.0 / other
    }

    pub fn run_at_least_until_seconds(&mut self, system_time: &ElapsedTime, seconds: i32) {
        self.0 = self.0.max(system_time.0 + seconds as f32);
    }

    pub fn has_passed(&self, system_time: &ElapsedTime) -> bool {
        self.0 <= system_time.0
    }

    pub fn has_not_passed(&self, system_time: &ElapsedTime) -> bool {
        self.0 > system_time.0
    }
}

fn main() {
    simple_logging::log_to_stderr(LevelFilter::Info);


    let mut video = Video::init();

    let shaders = Shaders {
        ground_shader: ShaderProgram::from_shaders(
            &[
                Shader::from_source(
                    include_str!("shaders/ground.vert"),
                    gl::VERTEX_SHADER,
                ).unwrap(),
                Shader::from_source(
                    include_str!("shaders/ground.frag"),
                    gl::FRAGMENT_SHADER,
                ).unwrap()
            ]
        ).unwrap(),
        model_shader: ShaderProgram::from_shaders(
            &[
                Shader::from_source(
                    include_str!("shaders/model.vert"),
                    gl::VERTEX_SHADER,
                ).unwrap(),
                Shader::from_source(
                    include_str!("shaders/model.frag"),
                    gl::FRAGMENT_SHADER,
                ).unwrap()
            ]
        ).unwrap(),
        sprite_shader: ShaderProgram::from_shaders(
            &[
                Shader::from_source(
                    include_str!("shaders/sprite.vert"),
                    gl::VERTEX_SHADER,
                ).unwrap(),
                Shader::from_source(
                    include_str!("shaders/sprite.frag"),
                    gl::FRAGMENT_SHADER,
                ).unwrap()
            ]
        ).unwrap(),
        player_shader: ShaderProgram::from_shaders(
            &[
                Shader::from_source(
                    include_str!("shaders/player.vert"),
                    gl::VERTEX_SHADER,
                ).unwrap(),
                Shader::from_source(
                    include_str!("shaders/player.frag"),
                    gl::FRAGMENT_SHADER,
                ).unwrap()
            ]
        ).unwrap(),
        sprite2d_shader: ShaderProgram::from_shaders(
            &[
                Shader::from_source(
                    include_str!("shaders/sprite2d.vert"),
                    gl::VERTEX_SHADER,
                ).unwrap(),
                Shader::from_source(
                    include_str!("shaders/sprite2d.frag"),
                    gl::FRAGMENT_SHADER,
                ).unwrap()
            ]
        ).unwrap(),
        trimesh_shader: ShaderProgram::from_shaders(
            &[
                Shader::from_source(
                    include_str!("shaders/trimesh.vert"),
                    gl::VERTEX_SHADER,
                ).unwrap(),
                Shader::from_source(
                    include_str!("shaders/trimesh.frag"),
                    gl::FRAGMENT_SHADER,
                ).unwrap()
            ]
        ).unwrap(),
        trimesh2d_shader: ShaderProgram::from_shaders(
            &[
                Shader::from_source(
                    include_str!("shaders/trimesh2d.vert"),
                    gl::VERTEX_SHADER,
                ).unwrap(),
                Shader::from_source(
                    include_str!("shaders/trimesh2d.frag"),
                    gl::FRAGMENT_SHADER,
                ).unwrap()
            ]
        ).unwrap(),
    };

    let mut ecs_world = specs::World::new();
    ecs_world.register::<BrowserClient>();
    ecs_world.register::<ControllerComponent>();
    ecs_world.register::<PlayerSpriteComponent>();
    ecs_world.register::<MonsterSpriteComponent>();
    ecs_world.register::<CharacterStateComponent>();
    ecs_world.register::<PhysicsComponent>();
    ecs_world.register::<FlyingNumberComponent>();

    ecs_world.register::<SkillManifestationComponent>();


    let mut ecs_dispatcher = specs::DispatcherBuilder::new()
        .with(BrowserInputProducerSystem, "browser_input_processor", &[])
        .with(InputConsumerSystem, "input_handler", &["browser_input_processor"])
        .with(FrictionSystem, "friction_sys", &[])
        .with(SkillSystem, "skill_sys", &[])
        .with(CharacterControlSystem, "char_control", &["friction_sys", "input_handler", "browser_input_processor"])
        .with(CharacterStateUpdateSystem, "char_state_update", &["char_control"])
        .with(PhysicsSystem, "physics", &["char_state_update"])
        .with_thread_local(OpenGlInitializerFor3D)
        .with_thread_local(RenderStreamingSystem)
        .with_thread_local(RenderDesktopClientSystem::new())
        .with_thread_local(PhysicsDebugDrawingSystem::new())
        .with_thread_local(DamageRenderSystem::new())
        .with_thread_local(RenderUI::new())
        .build();

    fn grf(str: &str) -> String {
        format!("d:\\Games\\TalonRO\\grf\\data\\{}", str)
    }

    let mut rng = rand::thread_rng();

    let (elapsed, sprites) = measure_time(|| {
        let job_name_table = job_name_table();
        Sprites {
            cursors: SpriteResource::new(&grf("sprite\\cursors")),
            numbers: GlTexture::from_file("damage.bmp"),
            character_sprites: JobId::iter().take(25).map(|job_id| {
                let job_file_name = &job_name_table[&job_id];
                let male_file_name = grf("sprite\\ÀÎ°£Á·\\¸öÅë\\³²\\") + &job_file_name + "_³²";
                let female_file_name = grf("sprite\\ÀÎ°£Á·\\¸öÅë\\¿©\\") + &job_file_name + "_¿©";
                let (male, female) = if !Path::new(&(female_file_name.clone() + ".act")).exists() {
                    let male = SpriteResource::new(&male_file_name);
                    let female = male.clone();
                    (male, female)
                } else if !Path::new(&(male_file_name.clone() + ".act")).exists() {
                    let female = SpriteResource::new(&female_file_name);
                    let male = female.clone();
                    (male, female)
                } else {
                    (SpriteResource::new(&male_file_name), SpriteResource::new(&female_file_name))
                };
                (job_id, [male, female])
            }).collect::<HashMap<JobId, [SpriteResource; 2]>>(),
            head_sprites: [
                (1..=26).map(|i| {
                    let male_file_name = grf("sprite\\ÀÎ°£Á·\\¸Ó¸®Åë\\³²\\") + &i.to_string() + "_³²";
                    let male = if Path::new(&(male_file_name.clone() + ".act")).exists() {
                        Some(SpriteResource::new(&male_file_name))
                    } else { None };
                    male
                }).filter_map(|it| it).collect::<Vec<SpriteResource>>(),
                (1..=26).map(|i| {
                    let female_file_name = grf("sprite\\ÀÎ°£Á·\\¸Ó¸®Åë\\¿©\\") + &i.to_string() + "_¿©";
                    let female = if Path::new(&(female_file_name.clone() + ".act")).exists() {
                        Some(SpriteResource::new(&female_file_name))
                    } else { None };
                    female
                }).filter_map(|it| it).collect::<Vec<SpriteResource>>()
            ],
            monster_sprites: MonsterId::iter().map(|monster_id| {
                let file_name = grf("sprite\\npc\\") + &monster_id.to_string().to_lowercase();
                (monster_id, SpriteResource::new(&file_name))
            }).collect::<HashMap<MonsterId, SpriteResource>>(),
            effect_sprites: EffectSprites {
                torch: SpriteResource::new(&grf("sprite\\ÀÌÆÑÆ®\\torch_01")),
                fire_wall: SpriteResource::new(&grf("sprite\\ÀÌÆÑÆ®\\firewall")),
                fire_ball: SpriteResource::new(&grf("sprite\\ÀÌÆÑÆ®\\fireball")),
            },
        }
    });

    info!("act and spr files loaded[{}]: {}ms",
          (sprites.character_sprites.len() * 2) +
              sprites.head_sprites[0].len() + sprites.head_sprites[1].len() +
              sprites.monster_sprites.len(), elapsed.as_millis());

    let mut map_name_filter = ImString::new("prontera");
    let all_map_names = std::fs::read_dir("d:\\Games\\TalonRO\\grf\\data").unwrap().map(|entry| {
        let dir_entry = entry.unwrap();
        if dir_entry.file_name().into_string().unwrap().ends_with("rsw") {
            let mut sstr = dir_entry.file_name().into_string().unwrap();
            let len = sstr.len();
            sstr.truncate(len - 4); // remove extension
            Some(sstr)
        } else { None }
    }).filter_map(|x| x).collect::<Vec<String>>();

    let render_matrices = RenderMatrices {
        projection: Matrix4::new_perspective(
            std::f32::consts::FRAC_PI_4,
            VIDEO_WIDTH as f32 / VIDEO_HEIGHT as f32,
            0.1f32,
            1000.0f32,
        ),
        view: Matrix4::identity(), // it is filled before every frame
        ortho: ortho(0.0, VIDEO_WIDTH as f32, VIDEO_HEIGHT as f32, 0.0, -1.0, 1.0),
    };


    let (map_render_data, physics_world) = load_map("prontera");
    ecs_world.add_resource(SystemVariables {
        shaders,
        sprites,
        tick: Tick(0),
        entity_below_cursor: None,
        cell_below_cursor_walkable: false,
        dt: DeltaTime(0.0),
        time: ElapsedTime(0.0),
        matrices: render_matrices,
        map_render_data,
    });

    ecs_world.add_resource(physics_world);
    ecs_world.add_resource(SystemFrameDurations(HashMap::new()));
    let mut desktop_client_entity = {
        let desktop_client_char = components::char::create_char(
            &mut ecs_world,
            Point2::new(250.0, -200.0),
            Sex::Male,
            JobId::ROGUE,
            1,
            1,
        );
        ecs_world
            .create_entity()
            .with(ControllerComponent::new(desktop_client_char, 250.0, -180.0))
            .build()
    };

    let mut next_second: SystemTime = std::time::SystemTime::now().checked_add(Duration::from_secs(1)).unwrap();
    let mut last_tick_time: u64 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as u64;
    let mut fps_counter: u64 = 0;
    let mut fps: u64 = 0;


    let mut sent_bytes_per_second: usize = 0;
    let mut sent_bytes_per_second_counter: usize = 0;
    let mut websocket_server = websocket::sync::Server::bind("127.0.0.1:6969").unwrap();
    websocket_server.set_nonblocking(true).unwrap();

    let mut other_entities: Vec<Entity> = vec![];

    let mut entity_count = 0;
    'running: loop {
        match websocket_server.accept() {
            Ok(wsupgrade) => {
                let browser_client = wsupgrade.accept().unwrap();
                browser_client.set_nonblocking(true).unwrap();
                info!("Client connected");
//                ecs_world
//                    .create_entity()
//                    .with(ControllerComponent::new(250.0, -180.0))
//                    .with(BrowserClient {
//                        websocket: Mutex::new(browser_client),
//                        offscreen: vec![0; (VIDEO_WIDTH * VIDEO_HEIGHT * 4) as usize],
//                        ping: 0,
//                    })
//                    .build();
            }
            _ => { /* Nobody tried to connect, move on.*/ }
        };

        {
            let mut storage = ecs_world.write_storage::<ControllerComponent>();
            let inputs = storage.get_mut(desktop_client_entity).unwrap();

            for event in video.event_pump.poll_iter() {
                trace!("SDL event: {:?}", event);
                video.imgui_sdl2.handle_event(&mut video.imgui, &event);
                match event {
                    sdl2::event::Event::Quit { .. } | sdl2::event::Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                        break 'running;
                    }
                    _ => {
                        inputs.inputs.push(event);
                    }
                }
            }
        }

        {
            let mut storage = ecs_world.write_storage::<ControllerComponent>();
            let controller = storage.get_mut(desktop_client_entity).unwrap();
            ecs_world.write_resource::<SystemVariables>().matrices.view = controller.camera.create_view_matrix();
        }
        ecs_dispatcher.dispatch(&mut ecs_world.res);
        ecs_world.maintain();

        if let Some(new_map_name) = imgui_frame(
            desktop_client_entity,
            &mut video,
            &mut ecs_world,
            rng.clone(),
            sent_bytes_per_second,
            &mut entity_count,
            &mut map_name_filter,
            &all_map_names,
            fps,
            &mut other_entities,
        ) {
            ecs_world.delete_all();
            let (map_render_data, physics_world) = load_map(&new_map_name);
            ecs_world.write_resource::<SystemVariables>().map_render_data = map_render_data;
            ecs_world.add_resource(physics_world);

            desktop_client_entity = {
                let desktop_client_char = components::char::create_char(
                    &mut ecs_world,
                    Point2::new(250.0, -200.0),
                    Sex::Male,
                    JobId::ROGUE,
                    1,
                    1,
                );
                ecs_world
                    .create_entity()
                    .with(ControllerComponent::new(desktop_client_char, 250.0, -180.0))
                    .build()
            };
        }

        video.gl_swap_window();

        let now = std::time::SystemTime::now();
        let now_ms = now.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as u64;
        let dt = (now_ms - last_tick_time) as f32 / 1000.0;
        last_tick_time = now_ms;
        if now >= next_second {
            fps = fps_counter;
            fps_counter = 0;
            sent_bytes_per_second = sent_bytes_per_second_counter;
            sent_bytes_per_second_counter = 0;
            next_second = std::time::SystemTime::now().checked_add(Duration::from_secs(1)).unwrap();

            video.set_title(&format!("Rustarok {} FPS", fps));

            // send a ping packet every second
            let now_ms = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
            let data = now_ms.to_le_bytes();
            let browser_storage = ecs_world.write_storage::<BrowserClient>();
            for browser_client in browser_storage.join() {
                let message = websocket::Message::ping(&data[..]);
                browser_client.websocket.lock().unwrap().send_message(&message).expect("Sending a ping message");
            }
        }
        fps_counter += 1;
        ecs_world.write_resource::<SystemVariables>().tick.0 += 1;
        ecs_world.write_resource::<SystemVariables>().dt.0 = dt;
        ecs_world.write_resource::<SystemVariables>().time.0 += dt;
    }
}

fn imgui_frame(desktop_client_entity: Entity,
               video: &mut Video,
               mut ecs_world: &mut specs::world::World,
               mut rng: ThreadRng,
               sent_bytes_per_second: usize,
               entity_count: &mut i32,
               mut map_name_filter: &mut ImString,
               all_map_names: &Vec<String>,
               fps: u64,
               other_entities: &mut Vec<Entity>) -> Option<String> {
    let ui = video.imgui_sdl2.frame(&video.window,
                                    &mut video.imgui,
                                    &video.event_pump.mouse_state());
    extern crate sublime_fuzzy;
    let mut ret = None;
    { // IMGUI
        ui.window(im_str!("Graphic opsions"))
            .position((0.0, 0.0), imgui::ImGuiCond::FirstUseEver)
            .size((300.0, 600.0), imgui::ImGuiCond::FirstUseEver)
            .build(|| {
                let map_name_filter_clone = map_name_filter.clone();
                let filtered_map_names: Vec<&String> = all_map_names.iter()
                    .filter(|map_name| {
                        let matc = sublime_fuzzy::best_match(map_name_filter_clone.to_str(), map_name);
                        matc.is_some()
                    }).collect();
                if ui.input_text(im_str!("Map name:"), &mut map_name_filter)
                    .enter_returns_true(true)
                    .build() {
                    if let Some(&map_name) = filtered_map_names.get(0) {
                        ret = Some(map_name.to_owned());
                    }
                }
                for &map_name in filtered_map_names.iter() {
                    if ui.small_button(&ImString::new(map_name.as_str())) {
                        ret = Some(map_name.to_owned());
                    }
                }

                let mut map_render_data = &mut ecs_world.write_resource::<SystemVariables>().map_render_data;
                ui.checkbox(im_str!("Use tile_colors"), &mut map_render_data.use_tile_colors);
                if ui.checkbox(im_str!("Use use_lighting"), &mut map_render_data.use_lighting) {
                    map_render_data.use_lightmaps = map_render_data.use_lighting && map_render_data.use_lightmaps;
                }
                if ui.checkbox(im_str!("Use lightmaps"), &mut map_render_data.use_lightmaps) {
                    map_render_data.use_lighting = map_render_data.use_lighting || map_render_data.use_lightmaps;
                }
                ui.checkbox(im_str!("Models"), &mut map_render_data.draw_models);

                ui.slider_int(im_str!("Entities"), entity_count, 0, 20)
                    .build();

                let mut storage = ecs_world.write_storage::<ControllerComponent>();
                let controller = storage.get(desktop_client_entity).unwrap();
                {
                    let mut char_state_storage = ecs_world.write_storage::<CharacterStateComponent>();
                    let mut char_state = char_state_storage.get_mut(controller.char).unwrap();
                    ui.slider_float(im_str!("Attack Speed"), &mut char_state.attack_speed, 1.0, 5.0)
                        .build();
                }

                ui.drag_float3(im_str!("light_dir"), &mut map_render_data.rsw.light.direction)
                    .min(-1.0).max(1.0).speed(0.05).build();
                ui.color_edit(im_str!("light_ambient"), &mut map_render_data.rsw.light.ambient)
                    .inputs(false)
                    .format(imgui::ColorFormat::Float)
                    .build();
                ui.color_edit(im_str!("light_diffuse"), &mut map_render_data.rsw.light.diffuse)
                    .inputs(false)
                    .format(imgui::ColorFormat::Float)
                    .build();
                ui.drag_float(im_str!("light_opacity"), &mut map_render_data.rsw.light.opacity)
                    .min(0.0).max(1.0).speed(0.05).build();

                ui.text(im_str!("Maps: {},{},{}", controller.camera.pos().x, controller.camera.pos().y, controller.camera.pos().z));
                ui.text(im_str!("yaw: {}, pitch: {}", controller.yaw, controller.pitch));
                ui.text(im_str!("FPS: {}", fps));
                let (traffic, unit) = if sent_bytes_per_second > 1024 * 1024 {
                    (sent_bytes_per_second / 1024 / 1024, "Mb")
                } else if sent_bytes_per_second > 1024 {
                    (sent_bytes_per_second / 1024, "Kb")
                } else {
                    (sent_bytes_per_second, "bytes")
                };

                let system_frame_durations = &mut ecs_world.write_resource::<SystemFrameDurations>().0;
                ui.text(im_str!("Systems: "));
                for (sys_name, duration) in system_frame_durations.iter() {
                    let color = if *duration < 5 {
                        (0.0, 1.0, 0.0, 1.0)
                    } else if *duration < 10 {
                        (1.0, 0.8, 0.0, 1.0)
                    } else if *duration < 15 {
                        (1.0, 0.5, 0.0, 1.0)
                    } else if *duration < 20 {
                        (1.0, 0.2, 0.0, 1.0)
                    } else {
                        (1.0, 0.0, 0.0, 1.0)
                    };
                    ui.text_colored(color, im_str!("{}: {} ms", sys_name, duration));
                }
//                ui.text(im_str!("Traffic: {} {}", traffic, unit));
//
//                for browser_client in clients.iter() {
//                    ui.bullet_text(im_str!("Ping: {} ms", browser_client.ping));
//                }
            });
    }
    {
        let current_entity_count = ecs_world.read_storage::<PlayerSpriteComponent>().join().count() as i32;
        if current_entity_count < *entity_count {
            let count_to_add = *entity_count - current_entity_count;
            for _i in 0..count_to_add / 2 {
                let pos = {
                    let hero_pos = {
                        let mut physics_world = &mut ecs_world.write_resource::<PhysicsWorld>();
                        let mut phys_storage = &mut ecs_world.read_storage::<PhysicsComponent>();
                        let mut storage = ecs_world.write_storage::<ControllerComponent>();
                        let controller = storage.get(desktop_client_entity).unwrap();
                        phys_storage.get(controller.char).unwrap().pos(&physics_world)
                    };
                    let map_render_data = &ecs_world.read_resource::<SystemVariables>().map_render_data;
                    let (x, y) = loop {
                        let x = rng.gen_range(hero_pos.x - 10.0, hero_pos.x + 10.0);
                        let y = rng.gen_range(hero_pos.y - 10.0, hero_pos.y + 10.0).abs();
                        let index = y as usize * map_render_data.gat.width as usize + x as usize;
                        let walkable = (map_render_data.gat.cells[index].cell_type & CellType::Walkable as u8) != 0;
                        if walkable {
                            break (x, y);
                        }
                    };
                    Point3::<f32>::new(x, 0.5, -y)
                };
                let pos2d = Point2::new(pos.x, pos.z);
                let mut rng = rand::thread_rng();
                let sprite_count = ecs_world.read_resource::<SystemVariables>().sprites.character_sprites.len();
                let sex = if rng.gen::<usize>() % 2 == 0 { Sex::Male } else { Sex::Female };
                let head_count = ecs_world.read_resource::<SystemVariables>().sprites.head_sprites[Sex::Male as usize].len();
                let entity_id = components::char::create_char(
                    &mut ecs_world,
                    pos2d,
                    sex,
                    JobId::SWORDMAN,
                    rng.gen::<usize>() % head_count,
                    rng.gen_range(1, 5),
                );

                other_entities.push(entity_id);
            }
            // add monsters
            for _i in 0..count_to_add / 2 {
                let pos = {
                    let map_render_data = &ecs_world.read_resource::<SystemVariables>().map_render_data;
                    // TODO: extract it
                    let hero_pos = {
                        let mut physics_world = &mut ecs_world.write_resource::<PhysicsWorld>();
                        let mut phys_storage = &mut ecs_world.read_storage::<PhysicsComponent>();
                        let mut storage = ecs_world.write_storage::<ControllerComponent>();
                        let controller = storage.get(desktop_client_entity).unwrap();
                        phys_storage.get(controller.char).unwrap().pos(&physics_world)
                    };
                    let (x, y) = loop {
                        let x: f32 = rng.gen_range(hero_pos.x - 10.0, hero_pos.x + 10.0);
                        let y: f32 = rng.gen_range(hero_pos.y - 10.0, hero_pos.y + 10.0).abs();
                        let index = y as usize * map_render_data.gat.width as usize + x as usize;
                        let walkable = (map_render_data.gat.cells[index].cell_type & CellType::Walkable as u8) != 0;
                        if walkable {
                            break (x, y);
                        }
                    };
                    Point3::<f32>::new(x, 0.5, -y)
                };
                let pos2d = Point2::new(pos.x, pos.z);
                let mut rng = rand::thread_rng();
                let sprite_count = ecs_world.read_resource::<SystemVariables>().sprites.monster_sprites.len();
                let entity_id = components::char::create_monster(
                    &mut ecs_world,
                    pos2d,
                    MonsterId::Poring,
                    rng.gen_range(1, 5),
                );
                other_entities.push(entity_id);
            }
        } else if current_entity_count - 1 > *entity_count { // -1 is the entity of the controller
            let to_remove = (current_entity_count - *entity_count) as usize;
            let entity_ids: Vec<Entity> = other_entities.drain(0..to_remove).collect();
            let body_handles: Vec<BodyHandle> = {
                let physic_storage = ecs_world.read_storage::<PhysicsComponent>();
                entity_ids.iter().map(|entity| {
                    physic_storage.get(*entity).unwrap().body_handle
                }).collect()
            };
            ecs_world.delete_entities(entity_ids.as_slice());

            // remove rigid bodies from the physic simulation
            let physics_world = &mut ecs_world.write_resource::<PhysicsWorld>();
            physics_world.remove_bodies(body_handles.as_slice());
        }
    }
    video.renderer.render(ui);
    return ret;
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ModelName(String);

pub struct MapRenderData {
    pub gat: Gat,
    pub gnd: Gnd,
    pub rsw: Rsw,
    pub light_wheight: [f32; 3],
    pub use_tile_colors: bool,
    pub use_lightmaps: bool,
    pub use_lighting: bool,
    pub ground_vertex_array: VertexArray,
    pub sprite_vertex_array: VertexArray,
    pub texture_atlas: GlTexture,
    pub tile_color_texture: GlTexture,
    pub lightmap_texture: GlTexture,
    pub models: HashMap<ModelName, ModelRenderData>,
    pub model_instances: Vec<(ModelName, Matrix4<f32>)>,
    pub draw_models: bool,
    pub ground_walkability_mesh: VertexArray,
    pub ground_walkability_mesh2: VertexArray,
    pub ground_walkability_mesh3: VertexArray,
}

pub struct ModelRenderData {
    pub bounding_box: BoundingBox,
    pub alpha: f32,
    pub model: Vec<DataForRenderingSingleNode>,
}

pub struct EntityRenderData {
    pub pos: Vector3<f32>,
//    pub texture: GlTexture,
}

pub type DataForRenderingSingleNode = Vec<SameTextureNodeFaces>;

pub struct SameTextureNodeFaces {
    pub vao: VertexArray,
    pub texture: GlTexture,
}

pub fn measure_time<T, F: FnOnce() -> T>(f: F) -> (Duration, T) {
    let start = Instant::now();
    let r = f();
    (start.elapsed(), r)
}

fn load_map(map_name: &str) -> (MapRenderData, PhysicsWorld) {
    let (elapsed, world) = measure_time(|| {
        Rsw::load(BinaryReader::new(format!("d:\\Games\\TalonRO\\grf\\data\\{}.rsw", map_name)))
    });
    info!("rsw loaded: {}ms", elapsed.as_millis());
    let (elapsed, gat) = measure_time(|| {
        Gat::load(BinaryReader::new(format!("d:\\Games\\TalonRO\\grf\\data\\{}.gat", map_name)), map_name)
    });
    let w = gat.width;
    let mut v = Vector3::<f32>::new(0.0, 0.0, 0.0);
    let rot = Rotation3::<f32>::new(Vector3::new(180f32.to_radians(), 0.0, 0.0));
    let mut rotate_around_x_axis = |mut pos: Point3<f32>| {
        v.x = pos[0];
        v.y = pos[1];
        v.z = pos[2];
        v = rot * v;
        pos[0] = v.x;
        pos[1] = v.y;
        pos[2] = v.z;
        pos
    };

    let vertices: Vec<Point3<f32>> = gat.rectangles.iter().map(|cell| {
        let x = cell.start_x as f32;
        let x2 = (cell.start_x + cell.width) as f32;
        let y = (cell.bottom - cell.height + 1) as f32;
        let y2 = (cell.bottom + 1) as f32;
        vec![rotate_around_x_axis(Point3::new(x, -2.0, y2)),
             rotate_around_x_axis(Point3::new(x2, -2.0, y2)),
             rotate_around_x_axis(Point3::new(x, -2.0, y)),
             rotate_around_x_axis(Point3::new(x, -2.0, y)),
             rotate_around_x_axis(Point3::new(x2, -2.0, y2)),
             rotate_around_x_axis(Point3::new(x2, -2.0, y))]
    }).flatten().collect();

    let vertices2: Vec<Point3<f32>> = gat.cells.iter().enumerate().map(|(i, cell)| {
        let x = (i as u32 % w) as f32;
        let y = (i as u32 / w) as f32;
        if cell.cell_type & CellType::Walkable as u8 == 0 {
            vec![rotate_around_x_axis(Point3::new(x + 0.0, -1.0, y + 1.0)),
                 rotate_around_x_axis(Point3::new(x + 1.0, -1.0, y + 1.0)),
                 rotate_around_x_axis(Point3::new(x + 0.0, -1.0, y + 0.0)),
                 rotate_around_x_axis(Point3::new(x + 0.0, -1.0, y + 0.0)),
                 rotate_around_x_axis(Point3::new(x + 1.0, -1.0, y + 1.0)),
                 rotate_around_x_axis(Point3::new(x + 1.0, -1.0, y + 0.0))]
        } else {
            vec![]
        }
    }).flatten().collect();
    let ground_walkability_mesh = VertexArray::new(
        gl::TRIANGLES,
        &vertices, vertices.len(), None, vec![
            VertexAttribDefinition {
                number_of_components: 3,
                offset_of_first_element: 0,
            }
        ]);
    let ground_walkability_mesh2 = VertexArray::new(
        gl::TRIANGLES,
        &vertices2, vertices2.len(), None, vec![
            VertexAttribDefinition {
                number_of_components: 3,
                offset_of_first_element: 0,
            }
        ]);
    info!("gat loaded: {}ms", elapsed.as_millis());
    let (elapsed, mut ground) = measure_time(|| {
        Gnd::load(BinaryReader::new(format!("d:\\Games\\TalonRO\\grf\\data\\{}.gnd", map_name)),
                  world.water.level,
                  world.water.wave_height)
    });
    info!("gnd loaded: {}ms", elapsed.as_millis());
    let (elapsed, models) = measure_time(|| {
        let model_names: HashSet<_> = world.models.iter().map(|m| m.filename.clone()).collect();
        Rsw::load_models(model_names)
    });
    info!("models[{}] loaded: {}ms", models.len(), elapsed.as_millis());

    let (elapsed, model_render_datas) = measure_time(|| {
        models.iter().map(|(name, rsm)| {
            let textures = Rsm::load_textures(&rsm.texture_names);
            let (data_for_rendering_full_model, bbox): (Vec<DataForRenderingSingleNode>, BoundingBox) = Rsm::generate_meshes_by_texture_id(
                &rsm.bounding_box,
                rsm.shade_type,
                rsm.nodes.len() == 1,
                &rsm.nodes,
                &textures,
            );
            (name.clone(), ModelRenderData {
                bounding_box: bbox,
                alpha: rsm.alpha,
                model: data_for_rendering_full_model,
            })
        }).collect::<HashMap<ModelName, ModelRenderData>>()
    });
    info!("model_render_datas loaded: {}ms", elapsed.as_millis());

    let model_instances: Vec<(ModelName, Matrix4<f32>)> = world.models.iter().map(|model_instance| {
        let mut instance_matrix = Matrix4::<f32>::identity();
        instance_matrix.prepend_translation_mut(&(model_instance.pos + Vector3::new(ground.width as f32, 0f32, ground.height as f32)));

// rot_z
        let rotation = Rotation3::from_axis_angle(&Unit::new_normalize(Vector3::z()), model_instance.rot.z.to_radians()).to_homogeneous();
        instance_matrix = instance_matrix * rotation;
// rot x
        let rotation = Rotation3::from_axis_angle(&Unit::new_normalize(Vector3::x()), model_instance.rot.x.to_radians()).to_homogeneous();
        instance_matrix = instance_matrix * rotation;
// rot y
        let rotation = Rotation3::from_axis_angle(&Unit::new_normalize(Vector3::y()), model_instance.rot.y.to_radians()).to_homogeneous();
        instance_matrix = instance_matrix * rotation;

        instance_matrix.prepend_nonuniform_scaling_mut(&model_instance.scale);

        let rotation = Rotation3::from_axis_angle(&Unit::new_normalize(Vector3::x()), 180f32.to_radians()).to_homogeneous();
        instance_matrix = rotation * instance_matrix;

        (model_instance.filename.clone(), instance_matrix)
    }).collect();

    let (elapsed, texture_atlas) = measure_time(|| {
        Gnd::create_gl_texture_atlas(&ground.texture_names)
    });
    info!("model texture_atlas loaded: {}ms", elapsed.as_millis());

    let tile_color_texture = Gnd::create_tile_color_texture(
        &mut ground.tiles_color_image,
        ground.width, ground.height,
    );
    let lightmap_texture = Gnd::create_lightmap_texture(&ground.lightmap_image, ground.lightmaps.count);

    let s: Vec<[f32; 4]> = vec![
        [-0.5, 0.5, 0.0, 0.0],
        [0.5, 0.5, 1.0, 0.0],
        [-0.5, -0.5, 0.0, 1.0],
        [0.5, -0.5, 1.0, 1.0]
    ];
    let sprite_vertex_array = VertexArray::new(
        gl::TRIANGLE_STRIP,
        &s, 4, None, vec![
            VertexAttribDefinition {
                number_of_components: 2,
                offset_of_first_element: 0,
            }, VertexAttribDefinition { // uv
                number_of_components: 2,
                offset_of_first_element: 2,
            }
        ]);

    let ground_vertex_array = VertexArray::new(
        gl::TRIANGLES,
        &ground.mesh, ground.mesh.len(), None, vec![
            VertexAttribDefinition {
                number_of_components: 3,
                offset_of_first_element: 0,
            }, VertexAttribDefinition { // normals
                number_of_components: 3,
                offset_of_first_element: 3,
            }, VertexAttribDefinition { // texcoords
                number_of_components: 2,
                offset_of_first_element: 6,
            }, VertexAttribDefinition { // lightmap_coord
                number_of_components: 2,
                offset_of_first_element: 8,
            }, VertexAttribDefinition { // tile color coordinate
                number_of_components: 2,
                offset_of_first_element: 10,
            }
        ]);
    let mut physics_world = nphysics2d::world::World::new();
    physics_world.set_contact_model(SignoriniModel::new());
    let colliders: Vec<(Vector2<f32>, Vector2<f32>)> = gat.rectangles.iter().map(|cell| {
        let rot = Rotation3::<f32>::new(Vector3::new(180f32.to_radians(), 0.0, 0.0));
        let half_w = cell.width as f32 / 2.0;
        let x = cell.start_x as f32 + half_w;
        let half_h = cell.height as f32 / 2.0;
        let y = (cell.bottom - cell.height) as f32 + 1.0 + half_h;
        let half_extents = Vector2::new(half_w, half_h);

        let cuboid = ShapeHandle::new(
            ncollide2d::shape::Cuboid::new(half_extents)
        );
        let v = rot * Vector3::new(x, 0.0, y);
        let v2 = Vector2::new(v.x, v.z);
        let shit = ColliderDesc::new(cuboid)
            .density(10.0)
            .translation(v2)
            .collision_groups(CollisionGroups::new()
                .with_membership(&[STATIC_MODELS_COLLISION_GROUP])
                .with_blacklist(&[STATIC_MODELS_COLLISION_GROUP])
            )
            .build(&mut physics_world);
        (half_extents, shit.position_wrt_body().translation.vector)
    }).collect();
    let vertices: Vec<Point3<f32>> = colliders.iter().map(|(extents, pos)| {
        let x = pos.x - extents.x;
        let x2 = pos.x + extents.x;
        let y = pos.y - extents.y;
        let y2 = pos.y + extents.y;
        vec![Point3::new(x, 3.0, y2),
             Point3::new(x2, 3.0, y2),
             Point3::new(x, 3.0, y),
             Point3::new(x, 3.0, y),
             Point3::new(x2, 3.0, y2),
             Point3::new(x2, 3.0, y)]
    }).flatten().collect();
    let ground_walkability_mesh3 = VertexArray::new(
        gl::TRIANGLES,
        &vertices, vertices.len(), None, vec![
            VertexAttribDefinition {
                number_of_components: 3,
                offset_of_first_element: 0,
            }
        ]);
    (MapRenderData {
        gat,
        gnd: ground,
        rsw: world,
        ground_vertex_array,
        models: model_render_datas,
        texture_atlas,
        tile_color_texture,
        lightmap_texture,
        model_instances,
        sprite_vertex_array,
        use_tile_colors: true,
        use_lightmaps: true,
        use_lighting: true,
        draw_models: true,
        ground_walkability_mesh,
        ground_walkability_mesh2,
        ground_walkability_mesh3,
        light_wheight: [0f32; 3],
    }, physics_world)
}