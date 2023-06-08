use super::buffer::*;
use super::{shader, RenderStateEx, BASE_ZOOM, LOGICAL_PIXEL_SIZE};
use crate::app::math::*;
use crate::size_of;
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

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct Vertex {
    position: Vec2f,
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Instance {
    offset: Vec2f,
}

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

pub struct ViewportGrid {
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

impl ViewportGrid {
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
                            min_binding_size: Some(global_buffer.byte_size().try_into().unwrap()),
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

        let pipeline_layout =
            render_state
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Viewport grid pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = render_state
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Viewport grid pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[
                        VertexBufferLayout {
                            array_stride: size_of!(Vertex) as BufferAddress,
                            step_mode: VertexStepMode::Vertex,
                            attributes: &vertex_attr_array![0 => Float32x2],
                        },
                        VertexBufferLayout {
                            array_stride: size_of!(Instance) as BufferAddress,
                            step_mode: VertexStepMode::Instance,
                            attributes: &vertex_attr_array![1 => Float32x2],
                        },
                    ],
                },
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: MultisampleState {
                    count: 4,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(TextureFormat::Rgba8Unorm.into())],
                }),
                multiview: None,
            });

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

    pub fn draw(
        &mut self,
        render_state: &RenderState,
        texture_view: &TextureView,
        resolution: Vec2f,
        offset: Vec2f,
        zoom: f32,
        background_color: [f32; 4],
        grid_color: [f32; 4],
    ) {
        let clear_color = Color {
            r: background_color[0] as f64,
            g: background_color[1] as f64,
            b: background_color[2] as f64,
            a: background_color[3] as f64,
        };

        if zoom < 0.99 {
            render_state.clear_pass(texture_view, clear_color);
            return;
        }

        self.global_buffer.write(
            &render_state.queue,
            &[Globals {
                color: grid_color,
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
