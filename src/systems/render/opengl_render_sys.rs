use crate::asset::database::AssetDatabase;
use crate::asset::str::{KeyFrameType, StrFile, StrLayer};
use crate::common::{rotate_vec2, v2_to_v3};
use crate::components::controller::CameraComponent;
use crate::components::BrowserClient;
use crate::effect::StrEffectId;
use crate::my_gl::{Gl, MyGlBlendEnum, MyGlEnum};
use crate::runtime_assets::map::MapRenderData;
use crate::shaders::GroundShaderParameters;
use crate::systems::render::render_command::{
    create_2d_pos_rot_matrix, create_3d_pos_rot_matrix, create_3d_rot_matrix, EffectFrameCacheKey,
    RenderCommandCollector, UiLayer2d,
};
use crate::systems::render_sys::{DamageRenderSystem, ONE_SPRITE_PIXEL_SIZE_IN_3D};
use crate::systems::{SystemFrameDurations, SystemVariables};
use crate::video::{GlTexture, ShaderProgram, VertexArray, VertexAttribDefinition, Video};
use nalgebra::{Matrix3, Matrix4, Point2, Rotation3, Vector3};
use sdl2::ttf::Sdl2TtfContext;
use specs::prelude::*;
use std::collections::HashMap;

pub struct StrEffectCache {
    cache: HashMap<EffectFrameCacheKey, Option<EffectFrameCache>>,
}

impl StrEffectCache {
    pub fn new() -> StrEffectCache {
        StrEffectCache {
            cache: HashMap::with_capacity(32),
        }
    }

    pub fn precache_effect(&mut self, gl: &Gl, effect_id: StrEffectId, str_file: &StrFile) {
        for key_index in 0..str_file.max_key {
            for layer_index in 0..str_file.layers.len() {
                let layer = &str_file.layers[layer_index];
                let cached_effect_frame =
                    OpenGlRenderSystem::prepare_effect(&gl, layer, key_index as i32);
                let frame_cache_key = EffectFrameCacheKey {
                    effect_id,
                    layer_index,
                    key_index: key_index as i32,
                };

                self.cache.insert(frame_cache_key, cached_effect_frame);
            }
        }
    }
}

pub struct OpenGlRenderSystem<'a, 'b> {
    centered_rectangle_vao: VertexArray,
    circle_vao: VertexArray,
    points_vao: VertexArray,
    vaos: [VertexArray; 2],
    points_buffer: Vec<[f32; 7]>,
    // damage rendering
    single_digit_u_coord: f32,
    texture_u_coords: [f32; 10],

    str_effect_cache: StrEffectCache,
    text_cache: HashMap<String, GlTexture>,
    circle_vertex_arrays: Vec<VertexArray>,
    fonts: Fonts<'a, 'b>,

    identity_mat: Matrix4<f32>,
}

pub struct Fonts<'a, 'b> {
    small_font: sdl2::ttf::Font<'a, 'b>,
    normal_font: sdl2::ttf::Font<'a, 'b>,
    big_font: sdl2::ttf::Font<'a, 'b>,
    small_bold_font: sdl2::ttf::Font<'a, 'b>,
    normal_bold_font: sdl2::ttf::Font<'a, 'b>,
    big_bold_font: sdl2::ttf::Font<'a, 'b>,

    small_font_outline: sdl2::ttf::Font<'a, 'b>,
    normal_font_outline: sdl2::ttf::Font<'a, 'b>,
    big_font_outline: sdl2::ttf::Font<'a, 'b>,
    small_bold_font_outline: sdl2::ttf::Font<'a, 'b>,
    normal_bold_font_outline: sdl2::ttf::Font<'a, 'b>,
    big_bold_font_outline: sdl2::ttf::Font<'a, 'b>,
}

pub const SMALL_FONT_SIZE: i32 = 14;
pub const NORMAL_FONT_H: i32 = 20;
pub const NORMAL_FONT_W: i32 = 10;
pub const BIG_FONT_SIZE: i32 = 32;

impl<'a, 'b> Fonts<'a, 'b> {
    pub fn new(ttf_context: &'a Sdl2TtfContext) -> Fonts {
        let small_font = Video::load_font(
            &ttf_context,
            "assets/fonts/UbuntuMono-R.ttf",
            SMALL_FONT_SIZE as u16,
        )
        .unwrap();
        let mut small_font_outline = Video::load_font(
            &ttf_context,
            "assets/fonts/UbuntuMono-R.ttf",
            SMALL_FONT_SIZE as u16,
        )
        .unwrap();
        small_font_outline.set_outline_width(2);

        let normal_font = Video::load_font(
            &ttf_context,
            "assets/fonts/UbuntuMono-R.ttf",
            NORMAL_FONT_H as u16,
        )
        .unwrap();
        let mut normal_font_outline = Video::load_font(
            &ttf_context,
            "assets/fonts/UbuntuMono-R.ttf",
            NORMAL_FONT_H as u16,
        )
        .unwrap();
        normal_font_outline.set_outline_width(2);

        let big_font = Video::load_font(&ttf_context, "assets/fonts/UbuntuMono-R.ttf", 32).unwrap();
        let mut big_font_outline =
            Video::load_font(&ttf_context, "assets/fonts/UbuntuMono-R.ttf", 32).unwrap();
        big_font_outline.set_outline_width(2);

        let small_bold_font = Video::load_font(
            &ttf_context,
            "assets/fonts/UbuntuMono-B.ttf",
            SMALL_FONT_SIZE as u16,
        )
        .unwrap();
        let mut small_bold_font_outline = Video::load_font(
            &ttf_context,
            "assets/fonts/UbuntuMono-B.ttf",
            SMALL_FONT_SIZE as u16,
        )
        .unwrap();
        small_bold_font_outline.set_outline_width(2);

        let normal_bold_font = Video::load_font(
            &ttf_context,
            "assets/fonts/UbuntuMono-B.ttf",
            NORMAL_FONT_H as u16,
        )
        .unwrap();
        let mut normal_bold_font_outline = Video::load_font(
            &ttf_context,
            "assets/fonts/UbuntuMono-B.ttf",
            NORMAL_FONT_H as u16,
        )
        .unwrap();
        normal_bold_font_outline.set_outline_width(2);

        let big_bold_font = Video::load_font(
            &ttf_context,
            "assets/fonts/UbuntuMono-B.ttf",
            BIG_FONT_SIZE as u16,
        )
        .unwrap();
        let mut big_bold_font_outline = Video::load_font(
            &ttf_context,
            "assets/fonts/UbuntuMono-B.ttf",
            BIG_FONT_SIZE as u16,
        )
        .unwrap();
        big_bold_font_outline.set_outline_width(2);

        Fonts {
            small_font,
            normal_font,
            big_font,
            small_bold_font,
            normal_bold_font,
            big_bold_font,
            small_font_outline,
            normal_font_outline,
            big_font_outline,
            small_bold_font_outline,
            normal_bold_font_outline,
            big_bold_font_outline,
        }
    }
}

struct EffectFrameCache {
    pub pos_vao: VertexArray,
    pub offset: [f32; 2],
    pub color: [u8; 4],
    pub rotation_matrix: Matrix4<f32>,
    pub src_alpha: MyGlBlendEnum,
    pub dst_alpha: MyGlBlendEnum,
    pub texture_index: usize,
}

impl<'a, 'b> OpenGlRenderSystem<'a, 'b> {
    pub fn new(
        gl: Gl,
        ttf_context: &'a Sdl2TtfContext,
        str_effect_cache: StrEffectCache,
    ) -> OpenGlRenderSystem {
        let single_digit_width = 10.0;
        let texture_width = single_digit_width * 10.0;
        let single_digit_u_coord = single_digit_width / texture_width;

        let circle_vertex_arrays = (1..=100)
            .map(|i| {
                let nsubdivs = 100;
                let two_pi = std::f32::consts::PI * 2.0;
                let dtheta = two_pi / nsubdivs as f32;

                let mut pts: Vec<Point2<f32>> = Vec::with_capacity(i);
                let r = 12.0;
                ncollide2d::procedural::utils::push_xy_arc(r, i as u32, dtheta, &mut pts);
                let rotation_rad = std::f32::consts::FRAC_PI_2;

                pts.iter_mut().for_each(|it| {
                    let rotated = rotate_vec2(rotation_rad, &it.coords);
                    *it = Point2::new(rotated.x, rotated.y);
                });
                VertexArray::new_static(
                    &gl,
                    MyGlEnum::LINE_STRIP,
                    pts,
                    vec![VertexAttribDefinition {
                        number_of_components: 2,
                        offset_of_first_element: 0,
                    }],
                )
            })
            .collect();

        OpenGlRenderSystem {
            identity_mat: Matrix4::identity(),
            circle_vertex_arrays,
            fonts: Fonts::new(ttf_context),
            single_digit_u_coord,
            texture_u_coords: [
                single_digit_u_coord * 0.0,
                single_digit_u_coord * 1.0,
                single_digit_u_coord * 2.0,
                single_digit_u_coord * 3.0,
                single_digit_u_coord * 4.0,
                single_digit_u_coord * 5.0,
                single_digit_u_coord * 6.0,
                single_digit_u_coord * 7.0,
                single_digit_u_coord * 8.0,
                single_digit_u_coord * 9.0,
            ],
            vaos: {
                let single_cube: Vec<[f32; 4]> = vec![
                    // Front
                    [-0.5, 0.5, 0.5, 0.0],
                    [-0.5, -0.5, 0.5, 0.7],
                    [0.5, 0.5, 0.5, 0.0],
                    [0.5, 0.5, 0.5, 0.0],
                    [-0.5, -0.5, 0.5, 0.7],
                    [0.5, -0.5, 0.5, 0.7],
                    // Right
                    [0.5, 0.5, 0.5, 0.0],
                    [0.5, -0.5, 0.5, 0.7],
                    [0.5, 0.5, -0.5, 0.0],
                    [0.5, 0.5, -0.5, 0.0],
                    [0.5, -0.5, 0.5, 0.7],
                    [0.5, -0.5, -0.5, 0.7],
                    // Back
                    [0.5, 0.5, -0.5, 0.0],
                    [0.5, -0.5, -0.5, 0.7],
                    [-0.5, 0.5, -0.5, 0.0],
                    [-0.5, 0.5, -0.5, 0.0],
                    [0.5, -0.5, -0.5, 0.7],
                    [-0.5, -0.5, -0.5, 0.7],
                    // Let
                    [-0.5, 0.5, -0.5, 0.0],
                    [-0.5, -0.5, -0.5, 0.7],
                    [-0.5, 0.5, 0.5, 0.0],
                    [-0.5, 0.5, 0.5, 0.0],
                    [-0.5, -0.5, -0.5, 0.7],
                    [-0.5, -0.5, 0.5, 0.7],
                    // Top
                    //                    [-0.5, 0.5, -0.5, 0.0],
                    //                    [-0.5, 0.5, 0.5, 0.0],
                    //                    [0.5, 0.5, -0.5, 0.0],
                    //                    [0.5, 0.5, -0.5, 0.0],
                    //                    [-0.5, 0.5, 0.5, 0.0],
                    //                    [0.5, 0.5, 0.5, 0.0],
                    // Bottom
                    [-0.5, -0.5, 0.5, 0.3],
                    [-0.5, -0.5, -0.5, 0.3],
                    [0.5, -0.5, 0.5, 0.3],
                    [0.5, -0.5, 0.5, 0.3],
                    [-0.5, -0.5, -0.5, 0.3],
                    [0.5, -0.5, -0.5, 0.3],
                ];
                fn translate(vec: &Vec<[f32; 4]>, x: f32, z: f32) -> Vec<[f32; 4]> {
                    vec.iter()
                        .map(|v| [v[0] + x, v[1], v[2] + z, v[3]])
                        .collect()
                };
                let cubes = [
                    translate(&single_cube, -1.0, -2.0),
                    translate(&single_cube, 0.0, -2.0),
                    translate(&single_cube, 1.0, -2.0),
                    //
                    translate(&single_cube, -2.0, -1.0),
                    translate(&single_cube, -1.0, -1.0),
                    translate(&single_cube, 0.0, -1.0),
                    translate(&single_cube, 1.0, -1.0),
                    translate(&single_cube, 2.0, -1.0),
                    //
                    translate(&single_cube, -2.0, 0.0),
                    translate(&single_cube, -1.0, 0.0),
                    translate(&single_cube, 0.0, 0.0),
                    translate(&single_cube, 1.0, 0.0),
                    translate(&single_cube, 2.0, 0.0),
                    //
                    translate(&single_cube, -2.0, 1.0),
                    translate(&single_cube, -1.0, 1.0),
                    translate(&single_cube, 0.0, 1.0),
                    translate(&single_cube, 1.0, 1.0),
                    translate(&single_cube, 2.0, 1.0),
                    //
                    translate(&single_cube, -1.0, 2.0),
                    translate(&single_cube, 0.0, 2.0),
                    translate(&single_cube, 1.0, 2.0),
                ]
                .concat();
                let cubes: Vec<[f32; 7]> = cubes
                    .iter()
                    .map(|v| {
                        [
                            v[0] * 1.0,
                            (v[1] + 0.5) * 2.0,
                            v[2] * 1.0,
                            0.86,
                            0.99,
                            0.86,
                            v[3],
                        ]
                    })
                    .collect();
                let sanctuary_vao = VertexArray::new_static(
                    &gl,
                    MyGlEnum::TRIANGLES,
                    cubes,
                    vec![
                        VertexAttribDefinition {
                            number_of_components: 3,
                            offset_of_first_element: 0,
                        },
                        VertexAttribDefinition {
                            number_of_components: 4,
                            offset_of_first_element: 3,
                        },
                    ],
                );
                [sanctuary_vao.clone(), sanctuary_vao]
            },
            centered_rectangle_vao: {
                let bottom_left: [f32; 7] = [-0.5, 0.0, -0.5, 1.0, 1.0, 1.0, 1.0];
                let top_left: [f32; 7] = [-0.5, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
                let top_right: [f32; 7] = [0.5, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
                let bottom_right: [f32; 7] = [0.5, 0.0, -0.5, 1.0, 1.0, 1.0, 1.0];
                VertexArray::new_static(
                    &gl,
                    MyGlEnum::LINE_LOOP,
                    vec![bottom_left, top_left, top_right, bottom_right],
                    vec![
                        VertexAttribDefinition {
                            number_of_components: 3,
                            offset_of_first_element: 0,
                        },
                        VertexAttribDefinition {
                            number_of_components: 4,
                            offset_of_first_element: 3,
                        },
                    ],
                )
            },
            points_buffer: Vec::with_capacity(128),
            points_vao: VertexArray::new_dynamic(
                &gl,
                MyGlEnum::POINTS,
                Vec::<[f32; 7]>::new(),
                vec![
                    VertexAttribDefinition {
                        number_of_components: 3,
                        offset_of_first_element: 0,
                    },
                    VertexAttribDefinition {
                        number_of_components: 4,
                        offset_of_first_element: 3,
                    },
                ],
            ),
            circle_vao: {
                let capsule_mesh = ncollide2d::procedural::circle(&1.0, 32);
                let coords: Vec<[f32; 7]> = capsule_mesh
                    .coords()
                    .iter()
                    .map(|it| [it.x, 0.0, it.y, 1.0, 1.0, 1.0, 1.0])
                    .collect();
                VertexArray::new_static(
                    &gl,
                    MyGlEnum::LINE_LOOP,
                    coords,
                    vec![
                        VertexAttribDefinition {
                            number_of_components: 3,
                            offset_of_first_element: 0,
                        },
                        VertexAttribDefinition {
                            number_of_components: 4,
                            offset_of_first_element: 3,
                        },
                    ],
                )
            },
            str_effect_cache,
            text_cache: HashMap::with_capacity(1024),
        }
    }

    #[inline]
    fn create_translation_matrix_for_2d(screen_pos: &[i16; 2], layer: UiLayer2d) -> Matrix4<f32> {
        let t = Vector3::new(
            screen_pos[0] as f32,
            screen_pos[1] as f32,
            (layer as usize as f32) * 0.01,
        );
        return Matrix4::new_translation(&t);
    }

    fn render_ground(
        gl: &Gl,
        ground_shader: &ShaderProgram<GroundShaderParameters>,
        projection_matrix: &Matrix4<f32>,
        map_render_data: &MapRenderData,
        model_view: &Matrix4<f32>,
        normal_matrix: &Matrix3<f32>,
    ) {
        let shader = ground_shader.gl_use(gl);
        shader.params.projection_mat.set(gl, &projection_matrix);
        shader.params.model_view_mat.set(gl, &model_view);
        shader.params.normal_mat.set(gl, &normal_matrix);
        shader
            .params
            .light_dir
            .set(gl, &map_render_data.rsw.light.direction);
        shader
            .params
            .light_ambient
            .set(gl, &map_render_data.rsw.light.ambient);
        shader
            .params
            .light_diffuse
            .set(gl, &map_render_data.rsw.light.diffuse);
        shader
            .params
            .light_opacity
            .set(gl, map_render_data.rsw.light.opacity);
        shader.params.gnd_texture_atlas.set(gl, 0);
        shader.params.tile_color_texture.set(gl, 1);
        shader.params.lightmap_texture.set(gl, 2);

        shader.params.use_tile_color.set(
            gl,
            if map_render_data.use_tile_colors {
                1
            } else {
                0
            },
        );
        shader
            .params
            .use_lightmap
            .set(gl, if map_render_data.use_lightmaps { 1 } else { 0 });
        shader
            .params
            .use_lighting
            .set(gl, if map_render_data.use_lighting { 1 } else { 0 });

        map_render_data.texture_atlas.bind(&gl, MyGlEnum::TEXTURE0);
        map_render_data
            .tile_color_texture
            .bind(&gl, MyGlEnum::TEXTURE1);
        map_render_data
            .lightmap_texture
            .bind(&gl, MyGlEnum::TEXTURE2);
        map_render_data.ground_vertex_array.bind(&gl).draw(&gl);
    }

    pub fn create_number_vertex_array(&self, gl: &Gl, number: u32) -> VertexArray {
        let digits = DamageRenderSystem::get_digits(number);
        // create vbo based on the numbers
        let mut width = 0.0;
        let mut vertices = vec![];
        digits.iter().for_each(|&digit| {
            let digit = digit as usize;
            vertices.push([width - 0.5, 0.5, self.texture_u_coords[digit], 0.0]);
            vertices.push([
                width + 0.5,
                0.5,
                self.texture_u_coords[digit] + self.single_digit_u_coord,
                0.0,
            ]);
            vertices.push([width - 0.5, -0.5, self.texture_u_coords[digit], 1.0]);
            vertices.push([
                width + 0.5,
                0.5,
                self.texture_u_coords[digit] + self.single_digit_u_coord,
                0.0,
            ]);
            vertices.push([width - 0.5, -0.5, self.texture_u_coords[digit], 1.0]);
            vertices.push([
                width + 0.5,
                -0.5,
                self.texture_u_coords[digit] + self.single_digit_u_coord,
                1.0,
            ]);
            width += 1.0;
        });
        return VertexArray::new_static(
            &gl,
            MyGlEnum::TRIANGLES,
            vertices,
            vec![
                VertexAttribDefinition {
                    number_of_components: 2,
                    offset_of_first_element: 0,
                },
                VertexAttribDefinition {
                    // uv
                    number_of_components: 2,
                    offset_of_first_element: 2,
                },
            ],
        );
    }

    fn prepare_effect(gl: &Gl, layer: &StrLayer, key_index: i32) -> Option<EffectFrameCache> {
        let mut from_id = None;
        let mut to_id = None;
        let mut last_source_id = 0;
        let mut last_frame_id = 0;
        for (i, key_frame) in layer.key_frames.iter().enumerate() {
            if key_frame.frame <= key_index {
                match key_frame.typ {
                    KeyFrameType::Start => from_id = Some(i),
                    KeyFrameType::End => to_id = Some(i),
                };
            }
            last_frame_id = last_frame_id.max(key_frame.frame);
            if key_frame.typ == KeyFrameType::Start {
                last_source_id = last_source_id.max(key_frame.frame);
            }
        }
        if from_id.is_none() || to_id.is_none() || last_frame_id < key_index {
            return None;
        }
        let from_id = from_id.unwrap();
        let to_id = to_id.unwrap();
        if from_id >= layer.key_frames.len() || to_id >= layer.key_frames.len() {
            return None;
        }
        let from_frame = &layer.key_frames[from_id];
        let to_frame = &layer.key_frames[to_id];

        let (color, pos, xy, angle) = if to_id != from_id + 1 || to_frame.frame != from_frame.frame
        {
            // no other source
            if last_source_id <= from_frame.frame {
                return None;
            }
            (
                from_frame.color,
                from_frame.pos,
                from_frame.xy,
                from_frame.angle,
            )
        } else {
            let delta = key_index - from_frame.frame;

            // morphing
            let color = [
                (from_frame.color[0] as i32 + to_frame.color[0] as i32 * delta) as u8,
                (from_frame.color[1] as i32 + to_frame.color[1] as i32 * delta) as u8,
                (from_frame.color[2] as i32 + to_frame.color[2] as i32 * delta) as u8,
                (from_frame.color[3] as i32 + to_frame.color[3] as i32 * delta) as u8,
            ];
            let delta = delta as f32;
            let xy = [
                from_frame.xy[0] + to_frame.xy[0] * delta,
                from_frame.xy[1] + to_frame.xy[1] * delta,
                from_frame.xy[2] + to_frame.xy[2] * delta,
                from_frame.xy[3] + to_frame.xy[3] * delta,
                from_frame.xy[4] + to_frame.xy[4] * delta,
                from_frame.xy[5] + to_frame.xy[5] * delta,
                from_frame.xy[6] + to_frame.xy[6] * delta,
                from_frame.xy[7] + to_frame.xy[7] * delta,
            ];
            let angle = from_frame.angle + to_frame.angle * delta;
            let pos = [
                from_frame.pos[0] + to_frame.pos[0] * delta,
                from_frame.pos[1] + to_frame.pos[1] * delta,
            ];
            (color, pos, xy, angle)
        };

        let offset = [pos[0] - 320.0, pos[1] - 320.0];

        return Some(EffectFrameCache {
            pos_vao: VertexArray::new_static(
                &gl,
                MyGlEnum::TRIANGLE_STRIP,
                vec![
                    [xy[0], xy[4], 0.0, 0.0],
                    [xy[1], xy[5], 1.0, 0.0],
                    [xy[3], xy[7], 0.0, 1.0],
                    [xy[2], xy[6], 1.0, 1.0],
                ],
                vec![
                    VertexAttribDefinition {
                        // xy
                        number_of_components: 2,
                        offset_of_first_element: 0,
                    },
                    VertexAttribDefinition {
                        // uv
                        number_of_components: 2,
                        offset_of_first_element: 2,
                    },
                ],
            ),
            offset,
            color,
            rotation_matrix: Rotation3::from_axis_angle(
                &nalgebra::Unit::new_normalize(Vector3::z()),
                -angle,
            )
            .to_homogeneous(),
            src_alpha: from_frame.src_alpha,
            dst_alpha: from_frame.dst_alpha,
            texture_index: from_frame.texture_index,
        });
    }
}

impl<'a> specs::System<'a> for OpenGlRenderSystem<'_, '_> {
    type SystemData = (
        specs::ReadStorage<'a, RenderCommandCollector>,
        specs::ReadStorage<'a, BrowserClient>,
        specs::ReadStorage<'a, CameraComponent>,
        specs::WriteExpect<'a, SystemFrameDurations>,
        specs::ReadExpect<'a, SystemVariables>,
        specs::ReadExpect<'a, AssetDatabase>,
    );

    fn run(
        &mut self,
        (
            render_commands_storage,
            browser_client_storage,
            camera_storage,
            mut system_benchmark,
            system_vars,
            asset_database,
        ): Self::SystemData,
    ) {
        unsafe {
            system_vars
                .gl
                .Clear(MyGlEnum::COLOR_BUFFER_BIT as u32 | MyGlEnum::DEPTH_BUFFER_BIT as u32);
        }

        for (render_commands, camera, _not_browser) in (
            &render_commands_storage,
            &camera_storage,
            !&browser_client_storage,
        )
            .join()
        {
            let render_commands: &RenderCommandCollector = render_commands;

            let gl = &system_vars.gl;
            {
                let _stopwatch = system_benchmark.start_measurement("OpenGlRenderSystem.ground");
                if system_vars.map_render_data.draw_ground {
                    OpenGlRenderSystem::render_ground(
                        gl,
                        &system_vars.assets.shaders.ground_shader,
                        &system_vars.matrices.projection,
                        &system_vars.map_render_data,
                        &camera.view_matrix,
                        &camera.normal_matrix,
                    );
                }
            }

            {
                let shader = system_vars.assets.shaders.sprite_shader.gl_use(gl);
                shader
                    .params
                    .projection_mat
                    .set(gl, &system_vars.matrices.projection);
                shader.params.view_mat.set(gl, &render_commands.view_matrix);
                shader.params.texture.set(gl, 0);

                unsafe {
                    system_vars.gl.ActiveTexture(MyGlEnum::TEXTURE0);
                }
                /////////////////////////////////
                // 3D Sprites
                /////////////////////////////////
                {
                    let _stopwatch =
                        system_benchmark.start_measurement("OpenGlRenderSystem.sprite3d");
                    let vao_bind = system_vars
                        .map_render_data
                        .centered_sprite_vertex_array
                        .bind(&system_vars.gl);
                    for command in &render_commands.sprite_3d_commands {
                        let flipped_width = (1 - command.is_vertically_flipped as i16 * 2)
                            * command.texture_width as i16;

                        shader.params.size.set(
                            gl,
                            &[
                                flipped_width as f32 * ONE_SPRITE_PIXEL_SIZE_IN_3D * command.scale,
                                command.texture_height as f32
                                    * ONE_SPRITE_PIXEL_SIZE_IN_3D
                                    * command.scale,
                            ],
                        );
                        shader
                            .params
                            .model_mat
                            .set(gl, &Matrix4::new_translation(&command.pos));
                        shader.params.rot_mat.set(
                            gl,
                            &create_3d_rot_matrix(&(Vector3::z(), command.rot_radian)),
                        );
                        shader.params.color.set(gl, &command.color);
                        shader.params.offset.set(
                            gl,
                            &[
                                command.offset[0] as f32 * ONE_SPRITE_PIXEL_SIZE_IN_3D,
                                command.offset[1] as f32 * ONE_SPRITE_PIXEL_SIZE_IN_3D,
                            ],
                        );

                        unsafe {
                            system_vars
                                .gl
                                .BindTexture(MyGlEnum::TEXTURE_2D, command.texture.0);
                        }
                        vao_bind.draw(&system_vars.gl);
                    }
                }

                /////////////////////////////////
                // 3D NUMBERS
                /////////////////////////////////
                {
                    let _stopwatch =
                        system_benchmark.start_measurement("OpenGlRenderSystem.number3d");
                    unsafe {
                        gl.Disable(MyGlEnum::DEPTH_TEST);
                    }
                    system_vars
                        .assets
                        .sprites
                        .numbers
                        .bind(&system_vars.gl, MyGlEnum::TEXTURE0);
                    shader.params.offset.set(gl, &[0.0, 0.0]);
                    for command in &render_commands.number_3d_commands {
                        shader.params.size.set(gl, &[command.scale, command.scale]);
                        shader
                            .params
                            .model_mat
                            .set(gl, &Matrix4::new_translation(&command.pos));
                        shader.params.rot_mat.set(gl, &Matrix4::identity());
                        shader.params.color.set(gl, &command.color);

                        self.create_number_vertex_array(&system_vars.gl, command.value)
                            .bind(&system_vars.gl)
                            .draw(&system_vars.gl);
                    }
                    unsafe {
                        gl.Enable(MyGlEnum::DEPTH_TEST);
                    }
                }
            }
            /////////////////////////////////
            // 3D Horizontal textures
            /////////////////////////////////
            {
                let _stopwatch =
                    system_benchmark.start_measurement("OpenGlRenderSystem.horiz_texture");
                let shader = system_vars.assets.shaders.horiz_texture_shader.gl_use(gl);
                shader
                    .params
                    .projection_mat
                    .set(gl, &system_vars.matrices.projection);
                shader.params.view_mat.set(gl, &render_commands.view_matrix);
                shader.params.texture.set(gl, 0);

                unsafe {
                    system_vars.gl.ActiveTexture(MyGlEnum::TEXTURE0);
                }
                let vao_bind = system_vars
                    .map_render_data
                    .centered_sprite_vertex_array
                    .bind(&system_vars.gl);
                for command in &render_commands.horizontal_texture_3d_commands {
                    shader.params.size.set(gl, &[command.w, command.h]);

                    let model_matrix = create_3d_pos_rot_matrix(
                        &Vector3::new(command.pos.x, 0.2, command.pos.y),
                        &(Vector3::y(), command.rotation_rad),
                    );

                    shader.params.model_mat.set(gl, &model_matrix);
                    shader.params.color.set(gl, &command.color);

                    unsafe {
                        system_vars
                            .gl
                            .BindTexture(MyGlEnum::TEXTURE_2D, command.texture.0);
                    }
                    vao_bind.draw(&system_vars.gl);
                }
            }

            // TODO: measure the validity of effect caching
            /////////////////////////////////
            // 3D EFFECTS
            /////////////////////////////////
            {
                let _stopwatch = system_benchmark.start_measurement("OpenGlRenderSystem.effect3d");
                unsafe {
                    gl.Disable(MyGlEnum::DEPTH_TEST);
                }
                let shader = system_vars.assets.shaders.str_effect_shader.gl_use(gl);
                shader
                    .params
                    .projection_mat
                    .set(gl, &system_vars.matrices.projection);
                shader.params.view_mat.set(gl, &render_commands.view_matrix);
                shader.params.texture.set(gl, 0);
                unsafe {
                    system_vars.gl.ActiveTexture(MyGlEnum::TEXTURE0);
                }

                &render_commands
                    .effect_commands
                    .iter()
                    .filter(|(frame_cache_key, commands)| !commands.is_empty())
                    .for_each(|(frame_cache_key, commands)| {
                        let cached_frame = self.str_effect_cache.cache.get(&frame_cache_key);
                        let str_file = &system_vars.str_effects[frame_cache_key.effect_id.0];
                        match cached_frame {
                            None => {
                                let layer = &str_file.layers[frame_cache_key.layer_index];
                                let cached_effect_frame = OpenGlRenderSystem::prepare_effect(
                                    &system_vars.gl,
                                    layer,
                                    frame_cache_key.key_index,
                                );
                                self.str_effect_cache
                                    .cache
                                    .insert(frame_cache_key.clone(), cached_effect_frame);
                            }
                            Some(None) => {
                                // nothing
                            }
                            Some(Some(cached_frame)) => {
                                shader.params.offset.set(gl, &cached_frame.offset);
                                //                                shader.params.color.set(gl, &[255, 255, 255, 255]);
                                shader.params.color.set(gl, &cached_frame.color);
                                unsafe {
                                    gl.BlendFunc(cached_frame.src_alpha, cached_frame.dst_alpha);
                                }
                                // TODO: save the native_id into the command, so str_file should not be looked up
                                str_file.textures[cached_frame.texture_index]
                                    .bind(&system_vars.gl, MyGlEnum::TEXTURE0);
                                let bind = cached_frame.pos_vao.bind(&system_vars.gl);
                                for pos in commands {
                                    let mut matrix = cached_frame.rotation_matrix.clone();
                                    matrix.append_translation_mut(&v2_to_v3(&pos));
                                    //                                    let m = matrix.as_mut_slice();
                                    // ignore rotation for now
                                    //                                    m[12] = pos.x;
                                    //                                    m[13] = 0.0;
                                    //                                    m[14] = pos.y;
                                    //                                    m[15] = 1.0;
                                    shader.params.model_mat.set(gl, &matrix);
                                    bind.draw(&system_vars.gl);
                                }
                            }
                        }
                    });

                unsafe {
                    gl.BlendFunc(MyGlBlendEnum::SRC_ALPHA, MyGlBlendEnum::ONE_MINUS_SRC_ALPHA);
                    gl.Enable(MyGlEnum::DEPTH_TEST);
                }
            }

            /////////////////////////////////
            // 3D MODELS
            /////////////////////////////////
            {
                let _stopwatch = system_benchmark.start_measurement("OpenGlRenderSystem.models3d");
                let map_render_data = &system_vars.map_render_data;
                let shader = system_vars.assets.shaders.model_shader.gl_use(gl);
                shader
                    .params
                    .projection_mat
                    .set(gl, &system_vars.matrices.projection);
                shader.params.view_mat.set(gl, &render_commands.view_matrix);
                shader
                    .params
                    .normal_mat
                    .set(gl, &render_commands.normal_matrix);
                shader.params.texture.set(gl, 0);
                shader
                    .params
                    .light_dir
                    .set(gl, &map_render_data.rsw.light.direction);
                shader
                    .params
                    .light_ambient
                    .set(gl, &map_render_data.rsw.light.ambient);
                shader
                    .params
                    .light_diffuse
                    .set(gl, &map_render_data.rsw.light.diffuse);
                shader
                    .params
                    .light_opacity
                    .set(gl, map_render_data.rsw.light.opacity);
                shader
                    .params
                    .use_lighting
                    .set(gl, map_render_data.use_lighting as i32);

                for render_command in &render_commands.model_commands {
                    let matrix = &system_vars.map_render_data.model_instances
                        [render_command.model_instance_index]
                        .matrix;
                    shader.params.model_mat.set(gl, &matrix);
                    shader.params.alpha.set(
                        gl,
                        if render_command.is_transparent {
                            0.3
                        } else {
                            1.0
                        },
                    );
                    let asset_db_model_index = system_vars.map_render_data.model_instances
                        [render_command.model_instance_index]
                        .asset_db_model_index;
                    let model_render_data = asset_database.get_model(asset_db_model_index);
                    for node_render_data in &model_render_data.model {
                        // TODO: optimize this
                        for face_render_data in node_render_data {
                            face_render_data
                                .texture
                                .bind(&system_vars.gl, MyGlEnum::TEXTURE0);
                            face_render_data
                                .vao
                                .bind(&system_vars.gl)
                                .draw(&system_vars.gl);
                        }
                    }
                }
            }

            {
                let shader = system_vars.assets.shaders.trimesh3d_shader.gl_use(gl);
                shader
                    .params
                    .projection_mat
                    .set(gl, &system_vars.matrices.projection);
                shader.params.view_mat.set(gl, &render_commands.view_matrix);

                {
                    /////////////////////////////////
                    // 3D Trimesh
                    /////////////////////////////////
                    let _stopwatch =
                        system_benchmark.start_measurement("OpenGlRenderSystem.trimesh3d");

                    shader.params.scale.set(gl, &[1.0, 1.0, 1.0]);
                    shader.params.color.set_f32(gl, &[1.0, 1.0, 1.0, 1.0]);
                    for (i, commands) in render_commands.trimesh_3d_commands.iter().enumerate() {
                        let vao_bind = self.vaos[i].bind(&system_vars.gl);
                        for command in commands {
                            let mut matrix = Matrix4::<f32>::identity();
                            matrix.prepend_translation_mut(&command.pos);
                            shader.params.model_mat.set(gl, &matrix);
                            vao_bind.draw(&system_vars.gl);
                        }
                    }
                }
                {
                    let _stopwatch =
                        system_benchmark.start_measurement("OpenGlRenderSystem.rectangle3d");
                    let centered_rectangle_vao_bind =
                        self.centered_rectangle_vao.bind(&system_vars.gl);
                    for command in &render_commands.rectangle_3d_commands {
                        shader.params.color.set(gl, &command.color);
                        let mat = create_3d_pos_rot_matrix(
                            &command.pos,
                            &(Vector3::y(), command.rotation_rad),
                        );
                        shader.params.model_mat.set(gl, &mat);
                        shader
                            .params
                            .scale
                            .set(gl, &[command.width as f32, 1.0, command.height as f32]);
                        centered_rectangle_vao_bind.draw(&system_vars.gl);
                    }
                }

                /////////////////////////////////
                // 3D Circles
                /////////////////////////////////
                {
                    let _stopwatch =
                        system_benchmark.start_measurement("OpenGlRenderSystem.circle3d");
                    let vao_bind = self.circle_vao.bind(&system_vars.gl);
                    for command in &render_commands.circle_3d_commands {
                        shader.params.color.set(gl, &command.color);
                        shader
                            .params
                            .model_mat
                            .set(gl, &Matrix4::new_translation(&command.pos));
                        shader
                            .params
                            .scale
                            .set(gl, &[command.radius * 2.0, 1.0, command.radius * 2.0]);
                        vao_bind.draw(&system_vars.gl);
                    }
                }
            }

            /////////////////////////////////
            // 2D Trimesh
            /////////////////////////////////
            {
                let _stopwatch = system_benchmark.start_measurement("OpenGlRenderSystem.trimesh2d");
                let shader = system_vars.assets.shaders.trimesh2d_shader.gl_use(gl);
                shader
                    .params
                    .projection_mat
                    .set(gl, &system_vars.matrices.ortho);

                for command in &render_commands.partial_circle_2d_commands {
                    let matrix = OpenGlRenderSystem::create_translation_matrix_for_2d(
                        &command.screen_pos,
                        command.layer,
                    );
                    shader.params.model_mat.set(gl, &matrix);
                    shader.params.color.set(gl, &command.color);
                    shader.params.size.set(gl, &[1.0, 1.0]);

                    self.circle_vertex_arrays[command.circumference_index]
                        .bind(&system_vars.gl)
                        .draw(&system_vars.gl);
                }
            }

            /////////////////////////////////
            // 2D Points
            /////////////////////////////////
            {
                let _stopwatch = system_benchmark.start_measurement("OpenGlRenderSystem.points2d");
                let shader = system_vars.assets.shaders.point2d_shader.gl_use(gl);
                shader
                    .params
                    .projection_mat
                    .set(gl, &system_vars.matrices.ortho);

                self.points_buffer.clear();
                for command in &render_commands.point_2d_commands {
                    self.points_buffer.push([
                        command.screen_pos[0] as f32,
                        command.screen_pos[1] as f32,
                        (command.layer as usize as f32) * 0.01,
                        command.color[0] as f32 / 255.0,
                        command.color[1] as f32 / 255.0,
                        command.color[2] as f32 / 255.0,
                        command.color[3] as f32 / 255.0,
                    ]);
                }
                self.points_vao
                    .bind_dynamic(&system_vars.gl, self.points_buffer.as_slice())
                    .draw(&system_vars.gl)
            }

            /////////////////////////////////
            // 2D Texture
            /////////////////////////////////
            {
                let _stopwatch = system_benchmark.start_measurement("OpenGlRenderSystem.texture2d");
                let shader = system_vars.assets.shaders.sprite2d_shader.gl_use(gl);
                shader
                    .params
                    .projection_mat
                    .set(gl, &system_vars.matrices.ortho);
                shader.params.texture.set(gl, 0);

                let vertex_array_bind = system_vars
                    .map_render_data
                    .sprite_vertex_array
                    .bind(&system_vars.gl);
                unsafe {
                    system_vars.gl.ActiveTexture(MyGlEnum::TEXTURE0);
                }
                for command in &render_commands.texture_2d_commands {
                    let width = command.texture_width as f32;
                    let height = command.texture_height as f32;
                    unsafe {
                        system_vars
                            .gl
                            .BindTexture(MyGlEnum::TEXTURE_2D, command.texture.0);
                    }
                    let matrix =
                        create_2d_pos_rot_matrix(&command.screen_pos, command.rotation_rad as f32);
                    shader.params.model_mat.set(gl, &matrix);
                    shader
                        .params
                        .z
                        .set(gl, &0.01 * command.layer as usize as f32);
                    shader.params.offset.set(
                        gl,
                        command.offset[0] as i32,
                        command.offset[1] as i32,
                    );
                    shader
                        .params
                        .size
                        .set(gl, &[width * command.scale, height * command.scale]);
                    shader.params.color.set(gl, &command.color);
                    vertex_array_bind.draw(&system_vars.gl);
                }
            }

            /////////////////////////////////
            // 2D Rectangle
            /////////////////////////////////
            {
                let _stopwatch =
                    system_benchmark.start_measurement("OpenGlRenderSystem.rectangle2d");
                let vertex_array_bind = system_vars
                    .map_render_data
                    .sprite_vertex_array
                    .bind(&system_vars.gl);
                let shader = system_vars.assets.shaders.trimesh2d_shader.gl_use(gl);
                shader
                    .params
                    .projection_mat
                    .set(gl, &system_vars.matrices.ortho);

                for command in &render_commands.rectangle_2d_commands {
                    shader.params.color.set(gl, &command.color);
                    // TODO: is rotation necessary for 2d rect?
                    let matrix =
                        create_2d_pos_rot_matrix(&command.screen_pos, command.rotation_rad as f32);
                    shader.params.model_mat.set(gl, &matrix);
                    shader
                        .params
                        .z
                        .set(gl, 0.01 * command.layer as usize as f32);
                    shader
                        .params
                        .size
                        .set(gl, &[command.width as f32, command.height as f32]);

                    vertex_array_bind.draw(&system_vars.gl);
                }
            }

            /////////////////////////////////
            // 2D Text
            /////////////////////////////////
            {
                let _stopwatch = system_benchmark.start_measurement("OpenGlRenderSystem.text2d");
                let shader = system_vars.assets.shaders.sprite2d_shader.gl_use(gl);
                shader
                    .params
                    .projection_mat
                    .set(gl, &system_vars.matrices.ortho);
                shader.params.texture.set(gl, 0);

                let vertex_array_bind = system_vars
                    .map_render_data
                    .sprite_vertex_array
                    .bind(&system_vars.gl);
                unsafe {
                    system_vars.gl.ActiveTexture(MyGlEnum::TEXTURE0);
                }
                for command in &render_commands.text_2d_commands {
                    if let Some(texture) = self.text_cache.get(&command.text) {
                        let width = texture.width as f32;
                        let height = texture.height as f32;
                        unsafe {
                            system_vars
                                .gl
                                .BindTexture(MyGlEnum::TEXTURE_2D, texture.id().0);
                        }
                        shader
                            .params
                            .model_mat
                            .set(gl, &create_2d_pos_rot_matrix(&command.screen_pos, 0.0));
                        shader
                            .params
                            .z
                            .set(gl, 0.01 * command.layer as usize as f32);
                        shader.params.offset.set(gl, 0, 0);
                        shader.params.size.set(gl, &[width, height]);
                        shader.params.color.set(gl, &command.color);
                        vertex_array_bind.draw(&system_vars.gl);
                    } else {
                        let texture = Video::create_text_texture_inner(
                            &system_vars.gl,
                            &self.fonts.normal_font,
                            &command.text,
                        );
                        self.text_cache.insert(command.text.clone(), texture);
                    };
                }
            }
        }
    }
}