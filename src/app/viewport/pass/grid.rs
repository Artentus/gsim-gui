use super::super::buffer::*;
use super::super::{RenderStateEx, ViewportColors, BASE_ZOOM, LOGICAL_PIXEL_SIZE};
use super::*;
use crate::app::math::*;
use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::RenderState;
use wgpu::*;

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Globals {
    color: [f32; 4],
    resolution: Vec2f,
    offset: Vec2f,
    zoom: f32,
}

vs_input!(
    Vertex { position: Vec2f }

    Instance { offset: Vec2f }
);

const VERTICES: [Vertex; 8] = [
    // size 1px
    Vertex {
        position: Vec2f::new(-LOGICAL_PIXEL_SIZE / 2.0, -LOGICAL_PIXEL_SIZE / 2.0),
    },
    Vertex {
        position: Vec2f::new(-LOGICAL_PIXEL_SIZE / 2.0, LOGICAL_PIXEL_SIZE / 2.0),
    },
    Vertex {
        position: Vec2f::new(LOGICAL_PIXEL_SIZE / 2.0, -LOGICAL_PIXEL_SIZE / 2.0),
    },
    Vertex {
        position: Vec2f::new(LOGICAL_PIXEL_SIZE / 2.0, LOGICAL_PIXEL_SIZE / 2.0),
    },
    // size 2px
    Vertex {
        position: Vec2f::new(-LOGICAL_PIXEL_SIZE, -LOGICAL_PIXEL_SIZE),
    },
    Vertex {
        position: Vec2f::new(-LOGICAL_PIXEL_SIZE, LOGICAL_PIXEL_SIZE),
    },
    Vertex {
        position: Vec2f::new(LOGICAL_PIXEL_SIZE, -LOGICAL_PIXEL_SIZE),
    },
    Vertex {
        position: Vec2f::new(LOGICAL_PIXEL_SIZE, LOGICAL_PIXEL_SIZE),
    },
];

const INDICES: [u16; 6] = [0, 1, 2, 1, 3, 2];

pub struct GridPass {
    _shader: ShaderModule,
    global_buffer: StaticBuffer<Globals>,
    _bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    vertex_buffer: StaticBuffer<Vertex>,
    instance_buffer: DynamicBuffer<Instance>,
    index_buffer: StaticBuffer<u16>,
    _pipeline_layout: PipelineLayout,
    pipeline: RenderPipeline,
}

impl GridPass {
    pub fn create(render_state: &RenderState) -> Self {
        let shader = shader!(render_state.device, "grid");

        let global_buffer = StaticBuffer::create(
            &render_state.device,
            Some("Viewport grid globals"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            1,
        );

        let vertex_buffer = StaticBuffer::create_init(
            &render_state.device,
            Some("Viewport grid vertices"),
            BufferUsages::VERTEX,
            &VERTICES,
        );

        let instance_buffer = DynamicBuffer::create(
            &render_state.device,
            Some("Viewport grid instances"),
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            128 * 1024,
        );

        let index_buffer = StaticBuffer::create_init(
            &render_state.device,
            Some("Viewport grid indices"),
            BufferUsages::INDEX,
            &INDICES,
        );

        let bind_group_layout =
            render_state
                .device
                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX_FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: Some(global_buffer.byte_size()),
                        },
                        count: None,
                    }],
                });

        let bind_group = render_state.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: global_buffer.as_binding(),
            }],
        });

        let (pipeline_layout, pipeline) = create_pipeline(
            &render_state.device,
            "grid",
            &shader,
            &bind_group_layout,
            &[Vertex::BUFFER_LAYOUT, Instance::BUFFER_LAYOUT],
            None,
        );

        Self {
            _shader: shader,
            global_buffer,
            _bind_group_layout: bind_group_layout,
            bind_group,
            vertex_buffer,
            instance_buffer,
            index_buffer,
            _pipeline_layout: pipeline_layout,
            pipeline,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        render_state: &RenderState,
        texture_view: &TextureView,
        resolution: Vec2f,
        offset: Vec2f,
        zoom: f32,
        colors: &ViewportColors,
    ) {
        let clear_color = Color {
            r: colors.background_color[0] as f64,
            g: colors.background_color[1] as f64,
            b: colors.background_color[2] as f64,
            a: colors.background_color[3] as f64,
        };

        if zoom < 0.99 {
            render_state.clear_pass(texture_view, clear_color);
            return;
        }

        self.global_buffer.write(
            &render_state.queue,
            &[Globals {
                color: colors.grid_color,
                resolution,
                offset,
                zoom: zoom * BASE_ZOOM,
            }],
        );

        let (step, base_vertex) = if zoom > 1.99 { (1, 0) } else { (2, 4) };

        let width = resolution.x / (zoom * BASE_ZOOM);
        let height = resolution.y / (zoom * BASE_ZOOM);

        let left = (offset.x - (width * 0.5)).floor() as i32;
        let right = (offset.x + (width * 0.5)).ceil() as i32;
        let bottom = (offset.y - (height * 0.5)).floor() as i32;
        let top = (offset.y + (height * 0.5)).ceil() as i32;

        let x_points = (right - left + 1) as usize;
        let y_points = (top - bottom + 1) as usize;

        let mut instances = Vec::with_capacity(x_points * y_points);
        for y in (bottom..=top).filter(|&y| (y % step) == 0) {
            for x in (left..=right).filter(|&x| (x % step) == 0) {
                instances.push(Instance {
                    offset: Vec2i::new(x, y).to_vec2f(),
                });
            }
        }

        self.instance_buffer
            .write(&render_state.device, &render_state.queue, &instances);

        render_state.render_pass(texture_view, None, Some(clear_color), |pass, _| {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice());
            pass.set_vertex_buffer(1, self.instance_buffer.slice());
            pass.set_index_buffer(self.index_buffer.slice(), IndexFormat::Uint16);

            pass.draw_indexed(
                0..(INDICES.len() as u32),
                base_vertex,
                0..(instances.len() as u32),
            );
        });
    }
}
