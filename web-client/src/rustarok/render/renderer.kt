package rustarok.render

import org.khronos.webgl.*
import rustarok.*

const val ONE_SPRITE_PIXEL_SIZE_IN_3D: Float = 1.0f / 35.0f;

class Renderer(gl: WebGL2RenderingContext) {

    var models: Array<ModelData> = emptyArray()
    var model_instances: ArrayList<ModelInstance> = arrayListOf()

    val centered_sprite_vertex_buffer = create_centered_sprite_buffer(gl)
    val sprite_vertex_buffer = create_sprite_buffer(gl)
    
    val trimesh_3d_renderer = Trimesh3dRenderer(gl)
    val ground_renderer = GroundRenderer(gl)
    val texture_2d_renderer = Texture2dRenderer(gl)
    val model_renderer = ModelRenderer(gl)
    val sprite_3d_renderer = Sprite3dRenderer(gl)


    val sprite_render_commands = arrayListOf<RenderCommand.Sprite3D>()
    val number_render_commands = arrayListOf<RenderCommand.Number3D>()
    val circle3d_render_commands = arrayListOf<RenderCommand.Circle3D>()
    val rectangle3d_render_commands = arrayListOf<RenderCommand.Rectangle3D>()
    val texture2d_render_commands = arrayListOf<RenderCommand.Texture2D>()
    val model3d_render_commands = arrayListOf<RenderCommand.Model3D>()

    fun clear() {
        sprite_render_commands.clear()
        number_render_commands.clear()
        model3d_render_commands.clear()
        texture2d_render_commands.clear()
        circle3d_render_commands.clear()
        rectangle3d_render_commands.clear()
    }

    fun render(gl: WebGL2RenderingContext) {

        ground_renderer.render_ground(gl, ground_render_command)

        sprite_3d_renderer.render_sprites(gl, sprite_render_commands, centered_sprite_vertex_buffer)

        sprite_3d_renderer.render_numbers(gl, number_render_commands)
        
        for (command in model3d_render_commands) {
            model_renderer.render_model(gl, command, ground_render_command, models, model_instances)
        }

        for (command in circle3d_render_commands) {
            trimesh_3d_renderer.render_circle(gl, command)
        }
        for (command in rectangle3d_render_commands) {
            trimesh_3d_renderer.render_rectangle(gl, command)
        }
        
        for (command in texture2d_render_commands) {
            texture_2d_renderer.render_texture_2d(gl, command, sprite_vertex_buffer)
        }
    }
}

fun create_sprite_buffer(gl: WebGL2RenderingContext): WebGLBuffer {
    val buffer = gl.createBuffer()!!
    gl.bindBuffer(WebGLRenderingContext.ARRAY_BUFFER, buffer)
    gl.bufferData(WebGLRenderingContext.ARRAY_BUFFER,
                  Float32Array(arrayOf(0.0f, 0.0f, 0.0f, 0.0f,
                                       1.0f, 0.0f, 1.0f, 0.0f,
                                       0.0f, 1.0f, 0.0f, 1.0f,
                                       1.0f, 1.0f, 1.0f, 1.0f)),
                  WebGLRenderingContext.STATIC_DRAW)
    return buffer
}


fun create_centered_sprite_buffer(gl: WebGL2RenderingContext): WebGLBuffer {
    val buffer = gl.createBuffer()!!
    gl.bindBuffer(WebGLRenderingContext.ARRAY_BUFFER, buffer)
    gl.bufferData(WebGLRenderingContext.ARRAY_BUFFER,
                  Float32Array(arrayOf(-0.5f, +0.5f, 0.0f, 0.0f,
                                       +0.5f, +0.5f, 1.0f, 0.0f,
                                       -0.5f, -0.5f, 0.0f, 1.0f,
                                       +0.5f, -0.5f, 1.0f, 1.0f)),
                  WebGLRenderingContext.STATIC_DRAW)
    return buffer
}