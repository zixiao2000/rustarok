use nalgebra::{Matrix4, Vector3, Rotation3, Point3, Vector2, Point2, Matrix3};
use crate::video::{VertexArray, draw_lines_inefficiently, draw_circle_inefficiently, draw_lines_inefficiently2, VIDEO_HEIGHT, VIDEO_WIDTH};
use crate::video::VertexAttribDefinition;
use crate::components::{BrowserClient, PhysicsComponent, SimpleSpriteComponent, DummyAiComponent, ControllerComponent, ComponentRadius, ExtraSpriteComponent};
use specs::prelude::*;
use crate::systems::{SystemVariables, SystemFrameDurations};
use crate::{Shaders, MapRenderData, SpriteResource, Tick};
use std::collections::HashMap;

// the values that should be added to the sprite direction based on the camera
// direction (the index is the camera direction, which is floor(angle/45)
pub const DIRECTION_TABLE: [usize; 8] = [6, 5, 4, 3, 2, 1, 0, 7];

pub struct RenderBrowserClientsSystem;

impl<'a> specs::System<'a> for RenderBrowserClientsSystem {
    type SystemData = (
        specs::ReadStorage<'a, ControllerComponent>,
        specs::WriteStorage<'a, BrowserClient>,
        specs::ReadExpect<'a, SystemVariables>,
    );

    fn run(&mut self, (
        controller_storage,
        mut browser_client_storage,
        system_vars,
    ): Self::SystemData) {
        for (controller, browser) in (&controller_storage, &mut browser_client_storage).join() {
            let view = controller.camera.create_view_matrix();
//            render_client(
//                &view,
//                &system_vars.shaders.ground_shader_program,
//                &system_vars.shaders.model_shader_program,
//                &system_vars.shaders.sprite_shader_program,
//                &system_vars.matrices.projection,
//                &system_vars.map_render_data,
//            );
            // now the back buffer contains the rendered image for this client
            unsafe {
                gl::ReadBuffer(gl::BACK);
                gl::ReadPixels(0, 0, VIDEO_WIDTH as i32, VIDEO_HEIGHT as i32,
                               gl::RGBA,
                               gl::UNSIGNED_BYTE, browser.offscreen.as_mut_ptr() as *mut gl::types::GLvoid);
            }
        }
    }
}

pub struct RenderDesktopClientSystem;

impl<'a> specs::System<'a> for RenderDesktopClientSystem {
    type SystemData = (
        specs::Entities<'a>,
        specs::ReadStorage<'a, ControllerComponent>,
        specs::ReadStorage<'a, BrowserClient>,
        specs::ReadStorage<'a, PhysicsComponent>,
        specs::ReadStorage<'a, SimpleSpriteComponent>,
        specs::ReadStorage<'a, ExtraSpriteComponent>,
        specs::ReadStorage<'a, DummyAiComponent>,
        specs::ReadExpect<'a, SystemVariables>,
        specs::WriteExpect<'a, SystemFrameDurations>,
    );

    fn run(&mut self, (
        entities,
        input_storage,
        browser_client_storage,
        physics_storage,
        animated_sprite_storage,
        extra_sprite_storage,
        ai_storage,
        system_vars,
        mut system_benchmark,
    ): Self::SystemData) {
        let stopwatch = system_benchmark.start_measurement("RenderDesktopClientSystem");
        for (controller, _not_browser) in (&input_storage, !&browser_client_storage).join() {
            let view = controller.camera.create_view_matrix();
            render_client(
                &view,
                &system_vars.shaders,
                &system_vars.matrices.projection,
                &system_vars.map_render_data,
            );


            for (entity, physics, animated_sprite, ai) in (&entities, &physics_storage,
                        &animated_sprite_storage,
                        &ai_storage).join() {
                let pos = {
                    let body = system_vars.map_render_data.physics_world.rigid_body(physics.handle).unwrap();
                    body.position().translation.vector
                };
                let tick = system_vars.tick;
                let body_res = if animated_sprite.is_monster {
                    &system_vars.monster_sprites[animated_sprite.file_index]
                } else {
                    &system_vars.sprite_resources[animated_sprite.file_index]
                };
                let pos_offset = render_sprite(&system_vars,
                                               tick,
                                               &animated_sprite,
                                               body_res,
                                               &view,
                                               &controller,
                                               &pos,
                                               [0, 0],
                                               true);
                if let Some(extra_sprite) = extra_sprite_storage.get(entity) {
                    let head_res = &system_vars.head_sprites[extra_sprite.head_index];
                    render_sprite(&system_vars,
                                  tick,
                                  &animated_sprite,
                                  head_res,
                                  &view,
                                  &controller,
                                  &pos,
                                  pos_offset,
                                  false);
                }
            }
        }
    }
}

fn render_sprite(system_vars: &SystemVariables,
                 tick: Tick,
                 animated_sprite: &SimpleSpriteComponent,
                 sprite_res: &SpriteResource,
                 view: &Matrix4<f32>,
                 controller: &ControllerComponent,
                 pos: &Vector2<f32>,
                 pos_offset: [i32; 2],
                 is_main: bool,
) -> [i32; 2] {
    system_vars.shaders.sprite_shader.gl_use();
    system_vars.shaders.sprite_shader.set_mat4("projection", &system_vars.matrices.projection);
    system_vars.shaders.sprite_shader.set_mat4("view", &view);
    system_vars.shaders.sprite_shader.set_int("model_texture", 0);
    system_vars.shaders.sprite_shader.set_f32("alpha", 1.0);
    let binded_sprite_vertex_array = system_vars.map_render_data.sprite_vertex_array.bind();

    // draw layer
    let animation_elapsed_tick = tick.0 - animated_sprite.animation_start.0;
    let cam_dir = (((controller.yaw / 45.0) + 0.5) as usize) % 8;
    let idx = animated_sprite.action_index + (animated_sprite.direction + DIRECTION_TABLE[cam_dir]) % 8;

    let delay = sprite_res.action.actions[idx].delay;
    let frame_count = sprite_res.action.actions[idx].frames.len();
    let frame_index = ((animation_elapsed_tick / (delay / 20) as u64) % frame_count as u64) as usize;
    let animation = &sprite_res.action.actions[idx].frames[frame_index];
    for layer in &animation.layers {
        if layer.sprite_frame_index < 0 {
            continue;
        }
        let sprite_frame = &sprite_res.frames[layer.sprite_frame_index as usize];

        let width = sprite_frame.original_width as f32 * layer.scale[0];
        let height = sprite_frame.original_height as f32 * layer.scale[1];
        sprite_frame.texture.bind(gl::TEXTURE0);

        let mut offset = if !animation.positions.is_empty() && !is_main {
            [
                pos_offset[0] - animation.positions[0][0],
                pos_offset[1] - animation.positions[0][1]
            ]
        } else {
            [0, 0]
        };
        let offset = [layer.pos[0] + offset[0], layer.pos[1] + offset[1]];
        let offset = [
            offset[0] as f32 / 175.0 * 5.0,
            offset[1] as f32 / 175.0 * 5.0 - 0.5
        ];
        system_vars.shaders.sprite_shader.set_vec2("offset", &offset);

        let mut matrix = Matrix4::<f32>::identity();
        let mut pos = Vector3::new(pos.x, 1.0, pos.y);
        matrix.prepend_translation_mut(&pos);
        system_vars.shaders.sprite_shader.set_mat4("model", &matrix);

        let width = width as f32 / 175.0 * 5.0;
        let width = if layer.is_mirror { -width } else { width };
        system_vars.shaders.sprite_shader.set_vec3("size", &[
            width,
            height as f32 / 175.0 * 5.0,
            0.0
        ]);

        system_vars.shaders.sprite_shader.set_f32("alpha", 1.0);

        binded_sprite_vertex_array.draw();
    }
    return animation.positions.get(0).map(|it| it.clone()).unwrap_or([0, 0]);
}

fn render_client(view: &Matrix4<f32>,
                 shaders: &Shaders,
                 projection_matrix: &Matrix4<f32>,
                 map_render_data: &MapRenderData) {
    unsafe {
        gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
    }

    let model = Matrix4::<f32>::identity();
    let model_view = view * model;
    let normal_matrix = {
        let inverted = model_view.try_inverse().unwrap();
        let m3x3 = inverted.fixed_slice::<nalgebra::base::U3, nalgebra::base::U3>(0, 0);
        m3x3.transpose()
    };

    render_ground(shaders, projection_matrix, map_render_data, &model_view, &normal_matrix);

//    shaders.trimesh_shader.gl_use();
//    shaders.trimesh_shader.set_mat4("projection", &projection_matrix);
//    shaders.trimesh_shader.set_mat4("view", view);
//    shaders.trimesh_shader.set_vec4("color", &[0.0, 1.0, 0.0, 0.5]);
//    shaders.trimesh_shader.set_mat4("model", &Matrix4::identity());
//    map_render_data.ground_walkability_mesh.bind().draw();
//
//    shaders.trimesh_shader.set_vec4("color", &[1.0, 0.0, 0.0, 0.5]);
//    map_render_data.ground_walkability_mesh2.bind().draw();
//
//    shaders.trimesh_shader.set_vec4("color", &[0.0, 0.0, 1.0, 0.5]);
//    map_render_data.ground_walkability_mesh3.bind().draw();

    shaders.model_shader.gl_use();
    shaders.model_shader.set_mat4("projection", &projection_matrix);
    shaders.model_shader.set_mat4("view", &view);
    shaders.model_shader.set_mat3("normal_matrix", &normal_matrix);
    shaders.model_shader.set_int("model_texture", 0);

    shaders.model_shader.set_vec3("light_dir", &map_render_data.rsw.light.direction);
    shaders.model_shader.set_vec3("light_ambient", &map_render_data.rsw.light.ambient);
    shaders.model_shader.set_vec3("light_diffuse", &map_render_data.rsw.light.diffuse);
    shaders.model_shader.set_f32("light_opacity", map_render_data.rsw.light.opacity);

    shaders.model_shader.set_int("use_lighting", if map_render_data.use_lighting { 1 } else { 0 });

    if map_render_data.draw_models {
        for (i, (model_name, matrix)) in map_render_data.model_instances.iter().enumerate() {
            shaders.model_shader.set_mat4("model", &matrix);
            let model_render_data = &map_render_data.models[&model_name];
            shaders.model_shader.set_f32("alpha", model_render_data.alpha);
            for node_render_data in &model_render_data.model {
                // TODO: optimize this
                for face_render_data in node_render_data {
                    face_render_data.texture.bind(gl::TEXTURE0);
                    face_render_data.vao.bind().draw();
                }
            }
        }
    }
}

fn render_ground(shaders: &Shaders,
                 projection_matrix: &Matrix4<f32>,
                 map_render_data: &MapRenderData,
                 model_view: &Matrix4<f32>,
                 normal_matrix: &Matrix3<f32>) {
    shaders.ground_shader.gl_use();
    shaders.ground_shader.set_mat4("projection", &projection_matrix);
    shaders.ground_shader.set_mat4("model_view", &model_view);
    shaders.ground_shader.set_mat3("normal_matrix", &normal_matrix);
    shaders.ground_shader.set_vec3("light_dir", &map_render_data.rsw.light.direction);
    shaders.ground_shader.set_vec3("light_ambient", &map_render_data.rsw.light.ambient);
    shaders.ground_shader.set_vec3("light_diffuse", &map_render_data.rsw.light.diffuse);
    shaders.ground_shader.set_f32("light_opacity", map_render_data.rsw.light.opacity);
    shaders.ground_shader.set_vec3("in_lightWheight", &map_render_data.light_wheight);
    map_render_data.texture_atlas.bind(gl::TEXTURE0);
    shaders.ground_shader.set_int("gnd_texture_atlas", 0);
    map_render_data.tile_color_texture.bind(gl::TEXTURE1);
    shaders.ground_shader.set_int("tile_color_texture", 1);
    map_render_data.lightmap_texture.bind(gl::TEXTURE2);
    shaders.ground_shader.set_int("lightmap_texture", 2);
    shaders.ground_shader.set_int("use_tile_color", if map_render_data.use_tile_colors { 1 } else { 0 });
    shaders.ground_shader.set_int("use_lightmap", if map_render_data.use_lightmaps { 1 } else { 0 });
    shaders.ground_shader.set_int("use_lighting", if map_render_data.use_lighting { 1 } else { 0 });
    map_render_data.ground_vertex_array.bind().draw();
}


pub struct PhysicsDebugDrawingSystem {
    capsule_vertex_arrays: HashMap<ComponentRadius, VertexArray>, // radius to vertexArray
}

impl PhysicsDebugDrawingSystem {
    pub fn new() -> PhysicsDebugDrawingSystem {
        let capsule_vertex_arrays: HashMap<ComponentRadius, VertexArray> = [1, 2, 3, 4].iter().map(|radius| {
            let mut capsule_mesh = ncollide2d::procedural::circle(
                &(*radius as f32 * 0.5 * 2.0),
                32,
            );

            let coords = capsule_mesh.coords();
            (ComponentRadius(*radius), VertexArray::new(
                gl::LINE_LOOP,
                coords,
                coords.len(),
                None,
                vec![
                    VertexAttribDefinition {
                        number_of_components: 2,
                        offset_of_first_element: 0,
                    }
                ])
            )
        }).collect();
        PhysicsDebugDrawingSystem {
            capsule_vertex_arrays,
        }
    }
}

impl<'a> specs::System<'a> for PhysicsDebugDrawingSystem {
    type SystemData = (
        specs::ReadStorage<'a, ControllerComponent>,
        specs::ReadStorage<'a, BrowserClient>,
        specs::ReadStorage<'a, PhysicsComponent>,
        specs::ReadExpect<'a, SystemVariables>,
        specs::WriteExpect<'a, SystemFrameDurations>,
    );

    fn run(&mut self, (
        controller_storage,
        browser_client_storage,
        physics_storage,
        system_vars,
        mut system_benchmark,
    ): Self::SystemData) {
        let stopwatch = system_benchmark.start_measurement("PhysicsDebugDrawingSystem");
        for (controller, _not_browser) in (&controller_storage, !&browser_client_storage).join() {
            let view = controller.camera.create_view_matrix();

            for physics in (&physics_storage).join() {
                let mut matrix = Matrix4::<f32>::identity();
                let body = system_vars.map_render_data.physics_world.rigid_body(physics.handle).unwrap();
                let pos = body.position().translation.vector;
                let pos = Vector3::new(pos.x, 1.0, pos.y);
                matrix.prepend_translation_mut(&pos);
                let rotation = Rotation3::from_axis_angle(&nalgebra::Unit::new_normalize(Vector3::x()), std::f32::consts::FRAC_PI_2).to_homogeneous();
                matrix = matrix * rotation;

                system_vars.shaders.trimesh_shader.gl_use();
                system_vars.shaders.trimesh_shader.set_mat4("projection", &system_vars.matrices.projection);
                system_vars.shaders.trimesh_shader.set_mat4("view", &view);
                system_vars.shaders.trimesh_shader.set_f32("alpha", 1.0);
                system_vars.shaders.trimesh_shader.set_mat4("model", &matrix);
                system_vars.shaders.trimesh_shader.set_vec4("color", &[1.0, 0.0, 1.0, 1.0]);
                self.capsule_vertex_arrays[&physics.radius].bind().draw();
            }
        }
    }
}


pub struct RenderStreamingSystem;

impl<'a> specs::System<'a> for RenderStreamingSystem {
    type SystemData = (
        specs::WriteStorage<'a, BrowserClient>,
    );

    fn run(&mut self, (
        browser_client_storage,
    ): Self::SystemData) {
        for browser in (&browser_client_storage).join() {
            let message = websocket::Message::binary(browser.offscreen.as_slice());
//                sent_bytes_per_second_counter += client.offscreen.len();
            // it is ok if it fails, the client might have disconnected but
            // ecs_world.maintain has not executed yet to remove it from the world
            let _result = browser.websocket.lock().unwrap().send_message(&message);
        }
    }
}