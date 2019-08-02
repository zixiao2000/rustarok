use crate::common::v2_to_v3;
use crate::components::char::SpriteBoundingRect;
use crate::systems::render_sys::ONE_SPRITE_PIXEL_SIZE_IN_3D;
use crate::video::{GlTexture, VertexArray, VIDEO_HEIGHT, VIDEO_WIDTH};
use nalgebra::{Matrix4, Rotation3, Vector2, Vector3, Vector4};
use specs::prelude::*;
use std::collections::HashMap;

fn create_2d_matrix(pos: &[f32; 2], rotation_rad: f32) -> Matrix4<f32> {
    let mut matrix = Matrix4::<f32>::identity();
    matrix.prepend_translation_mut(&v3!(pos[0], pos[1], 0));

    let rotation =
        Rotation3::from_axis_angle(&nalgebra::Unit::new_normalize(Vector3::z()), rotation_rad)
            .to_homogeneous();
    matrix * rotation
}

fn create_3d_matrix(pos: &Vector3<f32>, rotation_rad: &(Vector3<f32>, f32)) -> Matrix4<f32> {
    let mut matrix = Matrix4::<f32>::identity();
    matrix.prepend_translation_mut(&pos);
    let rotation = Rotation3::from_axis_angle(
        &nalgebra::Unit::new_normalize(rotation_rad.0),
        rotation_rad.1,
    )
    .to_homogeneous();
    return matrix * rotation;
}

#[derive(Hash, Eq, PartialEq, Clone)]
pub struct EffectFrameCacheKey {
    pub effect_name: String,
    pub layer_index: usize,
    pub key_index: i32,
}

#[derive(Component)]
pub struct RenderCommandCollectorComponent {
    pub(super) trimesh_2d_commands: Vec<Trimesh2dRenderCommand>,
    pub(super) texture_2d_commands: Vec<Texture2dRenderCommand>,
    pub(super) rectangle_3d_commands: Vec<Rectangle3dRenderCommand>,
    pub(super) rectangle_2d_commands: Vec<Common2DProperties>,
    pub(super) circle_3d_commands: Vec<Circle3dRenderCommand>,
    pub(super) billboard_commands: Vec<BillboardRenderCommand>,
    pub(super) number_3d_commands: Vec<Number3dRenderCommand>,
    pub(super) effect_commands: HashMap<EffectFrameCacheKey, Vec<Matrix4<f32>>>,
    //    pub(super) effect_commands: Vec<(EffectFrameCacheKey, Matrix4<f32>)>,
    pub(super) view_matrix: Matrix4<f32>,
}

impl<'a> RenderCommandCollectorComponent {
    pub fn new() -> RenderCommandCollectorComponent {
        RenderCommandCollectorComponent {
            trimesh_2d_commands: Vec::with_capacity(128),
            texture_2d_commands: Vec::with_capacity(128),
            rectangle_3d_commands: Vec::with_capacity(128),
            rectangle_2d_commands: Vec::with_capacity(128),
            circle_3d_commands: Vec::with_capacity(128),
            billboard_commands: Vec::with_capacity(128),
            number_3d_commands: Vec::with_capacity(128),
            effect_commands: HashMap::with_capacity(128),
            view_matrix: Matrix4::identity(),
        }
    }

    pub fn set_view_matrix(&mut self, view_matrix: &Matrix4<f32>) {
        self.view_matrix = *view_matrix;
    }

    pub fn clear(&mut self) {
        self.trimesh_2d_commands.clear();
        self.texture_2d_commands.clear();
        self.rectangle_3d_commands.clear();
        self.rectangle_2d_commands.clear();
        self.circle_3d_commands.clear();
        self.billboard_commands.clear();
        self.number_3d_commands.clear();
        self.effect_commands.clear();
    }

    pub fn prepare_for_3d(&'a mut self) -> Common3DPropBuilder {
        Common3DPropBuilder::new(self)
    }

    pub fn prepare_for_2d(&'a mut self) -> Common2DPropBuilder {
        Common2DPropBuilder::new(self)
    }

    pub fn get_last_billboard_command(&'a self) -> &'a BillboardRenderCommand {
        return &self.billboard_commands[self.billboard_commands.len() - 1];
    }
}

#[derive(Debug)]
pub struct Trimesh2dRenderCommand {
    pub(super) vao: VertexArray,
    pub(super) color: [f32; 4],
    pub(super) size: [f32; 2],
    pub(super) matrix: Matrix4<f32>,
    pub(super) layer: Layer2d,
}

pub struct Common2DProperties {
    pub(super) color: [f32; 4],
    pub(super) size: [f32; 2],
    pub(super) matrix: Matrix4<f32>,
    pub(super) layer: Layer2d,
}

pub struct Common2DPropBuilder<'a> {
    collector: &'a mut RenderCommandCollectorComponent,
    color: [f32; 4],
    screen_pos: [f32; 2],
    size: [f32; 2],
    rotation_rad: f32,
}

impl<'a> Common2DPropBuilder<'a> {
    pub fn new(collector: &mut RenderCommandCollectorComponent) -> Common2DPropBuilder {
        Common2DPropBuilder {
            collector,
            color: [1.0, 1.0, 1.0, 1.0],
            screen_pos: [0.0, 0.0],
            size: [1.0, 1.0],
            rotation_rad: 0.0,
        }
    }

    pub fn add_trimesh_command(&'a mut self, vao: &'a VertexArray, layer: Layer2d) {
        self.collector
            .trimesh_2d_commands
            .push(Trimesh2dRenderCommand {
                vao: vao.clone(),
                color: self.color,
                size: self.size,
                matrix: create_2d_matrix(&self.screen_pos, self.rotation_rad),
                layer,
            });
    }

    pub fn add_rectangle_command(&'a mut self, layer: Layer2d) {
        self.collector
            .rectangle_2d_commands
            .push(Common2DProperties {
                color: self.color,
                size: self.size,
                matrix: create_2d_matrix(&self.screen_pos, self.rotation_rad),
                layer,
            });
    }

    pub fn add_texture_command(&'a mut self, texture: &GlTexture, layer: Layer2d) {
        self.add_sprite_command(texture, [0.0, 0.0], false, layer);
    }

    pub fn add_sprite_command(
        &'a mut self,
        texture: &GlTexture,
        offset: [f32; 2],
        flip_vertically: bool,
        layer: Layer2d,
    ) {
        self.collector
            .texture_2d_commands
            .push(Texture2dRenderCommand {
                color: self.color,
                size: self.size[0],
                texture: texture.id(),
                offset,
                texture_width: (1 - flip_vertically as i32 * 2) * texture.width,
                texture_height: texture.height,
                matrix: create_2d_matrix(&self.screen_pos, self.rotation_rad),
                layer,
            });
    }

    pub fn color(&'a mut self, color: &[f32; 4]) -> &'a mut Common2DPropBuilder {
        self.color = *color;
        self
    }

    pub fn screen_pos(&'a mut self, x: f32, y: f32) -> &'a mut Common2DPropBuilder {
        self.screen_pos = [x, y];
        self
    }

    pub fn size2(&'a mut self, x: f32, y: f32) -> &'a mut Common2DPropBuilder {
        self.size = [x, y];
        self
    }

    pub fn size(&'a mut self, size: f32) -> &'a mut Common2DPropBuilder {
        self.size = [size, size];
        self
    }

    pub fn rotation_rad(&'a mut self, rotation_rad: f32) -> &'a mut Common2DPropBuilder {
        self.rotation_rad = rotation_rad;
        self
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Layer2d {
    Layer0,
    Layer1,
    Layer2,
    Layer3,
    Layer4,
    Layer5,
    Layer6,
    Layer7,
    Layer8,
    Layer9,
}

#[derive(Debug)]
pub struct Texture2dRenderCommand {
    pub(super) color: [f32; 4],
    pub(super) offset: [f32; 2],
    pub(super) size: f32,
    pub(super) matrix: Matrix4<f32>,
    pub(super) texture: gl::types::GLuint,
    pub(super) texture_width: i32,
    pub(super) texture_height: i32,
    pub(super) layer: Layer2d,
}

#[derive(Debug)]
pub struct Rectangle3dRenderCommand {
    pub(super) common: Common3DProperties,
    pub(super) size: [f32; 2],
}

#[derive(Debug)]
pub struct Circle3dRenderCommand {
    pub(super) common: Common3DProperties,
}

#[derive(Debug)]
pub struct BillboardRenderCommand {
    pub common: Common3DProperties,
    pub texture: gl::types::GLuint,
    pub texture_width: f32,
    pub texture_height: f32,
}

impl BillboardRenderCommand {
    pub fn project_to_screen(
        &self,
        view: &Matrix4<f32>,
        projection: &Matrix4<f32>,
    ) -> SpriteBoundingRect {
        let width = self.texture_width.abs();
        let height = self.texture_height;
        let mut top_right = Vector4::new(0.5 * width, 0.5 * height, 0.0, 1.0);
        top_right.x += self.common.offset[0];
        top_right.y -= self.common.offset[1];

        let mut bottom_left = Vector4::new(-0.5 * width, -0.5 * height, 0.0, 1.0);
        bottom_left.x += self.common.offset[0];
        bottom_left.y -= self.common.offset[1];

        let mut model_view = view * self.common.matrix;
        BillboardRenderCommand::set_spherical_billboard(&mut model_view);
        fn sh(v: Vector4<f32>) -> [i32; 2] {
            //        dbg!(&v);
            let s = if v[3] == 0.0 { 1.0 } else { 1.0 / v[3] };
            [
                ((v[0] * s / 2.0 + 0.5) * VIDEO_WIDTH as f32) as i32,
                VIDEO_HEIGHT as i32 - ((v[1] * s / 2.0 + 0.5) * VIDEO_HEIGHT as f32) as i32,
            ]
        }
        let bottom_left = sh(projection * model_view * bottom_left);
        let top_right = sh(projection * model_view * top_right);
        return SpriteBoundingRect {
            bottom_left,
            top_right,
        };
    }

    fn set_spherical_billboard(model_view: &mut Matrix4<f32>) {
        model_view[0] = 1.0;
        model_view[1] = 0.0;
        model_view[2] = 0.0;
        model_view[4] = 0.0;
        model_view[5] = 1.0;
        model_view[6] = 0.0;
        model_view[8] = 0.0;
        model_view[9] = 0.0;
        model_view[10] = 1.0;
    }
}

#[derive(Debug)]
pub struct Common3DProperties {
    pub color: [f32; 4],
    pub offset: [f32; 2],
    pub size: f32,
    pub matrix: Matrix4<f32>,
}

pub struct Common3DPropBuilder<'a> {
    collector: &'a mut RenderCommandCollectorComponent,
    color: [f32; 4],
    pos: Vector3<f32>,
    offset: [f32; 2],
    size: f32,
    rotation_rad: (Vector3<f32>, f32),
}

impl<'a> Common3DPropBuilder<'a> {
    fn new(collector: &mut RenderCommandCollectorComponent) -> Common3DPropBuilder {
        Common3DPropBuilder {
            collector,
            color: [1.0, 1.0, 1.0, 1.0],
            pos: Vector3::zeros(),
            offset: [0.0, 0.0],
            size: 1.0,
            rotation_rad: (Vector3::zeros(), 0.0),
        }
    }

    pub fn color(&mut self, color: &[f32; 4]) -> &'a mut Common3DPropBuilder {
        self.color = *color;
        self
    }

    pub fn color_rgb(&mut self, color: &[f32; 3]) -> &'a mut Common3DPropBuilder {
        self.color[0] = color[0];
        self.color[1] = color[1];
        self.color[2] = color[2];
        self
    }

    pub fn alpha(&mut self, a: f32) -> &'a mut Common3DPropBuilder {
        self.color[3] = a;
        self
    }

    pub fn offset(&mut self, offset: [f32; 2]) -> &'a mut Common3DPropBuilder {
        self.offset = offset;
        self
    }

    pub fn pos_2d(&mut self, pos: &Vector2<f32>) -> &'a mut Common3DPropBuilder {
        self.pos.x = pos.x;
        self.pos.z = pos.y;
        self
    }

    pub fn pos(&mut self, pos: &Vector3<f32>) -> &'a mut Common3DPropBuilder {
        self.pos = *pos;
        self
    }

    pub fn y(&mut self, y: f32) -> &'a mut Common3DPropBuilder {
        self.pos.y = y;
        self
    }

    pub fn scale(&'a mut self, size: f32) -> &'a mut Common3DPropBuilder {
        self.size = size;
        self
    }

    pub fn radius(&'a mut self, size: f32) -> &'a mut Common3DPropBuilder {
        self.size = size;
        self
    }

    pub fn rotation_rad(&mut self, axis: Vector3<f32>, angle: f32) -> &'a mut Common3DPropBuilder {
        self.rotation_rad = (axis, angle);
        self
    }

    fn build(&self) -> Common3DProperties {
        Common3DProperties {
            color: self.color,
            size: self.size,
            offset: [
                self.offset[0] * ONE_SPRITE_PIXEL_SIZE_IN_3D,
                self.offset[1] * ONE_SPRITE_PIXEL_SIZE_IN_3D,
            ],
            matrix: create_3d_matrix(&self.pos, &self.rotation_rad),
        }
    }

    pub fn add_effect_command(
        &'a mut self,
        pos: &Vector2<f32>,
        effect_name: &str,
        key_index: i32,
        layer_index: usize,
    ) {
        let frame_cache_key = EffectFrameCacheKey {
            effect_name: effect_name.to_owned(),
            layer_index,
            key_index,
        };
        self.collector
            .effect_commands
            .entry(frame_cache_key.clone())
            .or_insert(Vec::with_capacity(16))
            .push({
                let mut matrix = Matrix4::<f32>::identity();
                matrix.prepend_translation_mut(&v2_to_v3(pos));
                matrix
            });
    }

    pub fn add_number_command(&'a mut self, value: u32, digit_count: u8) {
        self.collector
            .number_3d_commands
            .push(Number3dRenderCommand {
                common: self.build(),
                value,
                digit_count,
            });
    }

    pub fn add_billboard_command(&'a mut self, texture: &GlTexture, flip_vertically: bool) {
        let flipped_width = (1 - flip_vertically as i32 * 2) * texture.width;
        let command = BillboardRenderCommand {
            common: self.build(),
            texture: texture.id(),
            texture_width: flipped_width as f32 * ONE_SPRITE_PIXEL_SIZE_IN_3D * self.size,
            texture_height: texture.height as f32 * ONE_SPRITE_PIXEL_SIZE_IN_3D * self.size,
        };
        self.collector.billboard_commands.push(command);
    }

    pub fn add_circle_command(&'a mut self) {
        self.collector
            .circle_3d_commands
            .push(Circle3dRenderCommand {
                common: self.build(),
            });
    }

    pub fn add_rectangle_command(&'a mut self, size: &Vector2<f32>) {
        self.collector
            .rectangle_3d_commands
            .push(Rectangle3dRenderCommand {
                common: self.build(),
                size: [size.x, size.y],
            });
    }
}

#[derive(Debug)]
pub struct Number3dRenderCommand {
    pub(super) common: Common3DProperties,
    pub(super) value: u32,
    pub(super) digit_count: u8,
}
