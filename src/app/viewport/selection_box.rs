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
struct Vertex {
    position: Vec2f,
}

/*

Vertex order:

0-------------4
| 1---------5 |
| |         | |
| |         | |
| |         | |
| 3---------7 |
2-------------6

*/

const INDICES: [u16; 24] = [
    0, 1, 2, 1, 3, 2, // left
    0, 4, 1, 1, 4, 5, // top
    5, 4, 7, 7, 4, 6, // right
    3, 6, 2, 3, 7, 6, // bottom
];

pub struct ViewportSelectionBox {
    _shader: ShaderModule,
    global_buffer: StaticBuffer<Globals>,
    _bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    vertex_buffer: StaticBuffer<Vertex>,
    index_buffer: StaticBuffer<u16>,
    _pipeline_layout: PipelineLayout,
    pipeline: RenderPipeline,
}

impl ViewportSelectionBox {
    pub fn create(render_state: &RenderState) -> Self {
        let shader = shader!(render_state.device, "selection_box");

        let global_buffer = StaticBuffer::create(
            &render_state.device,
            Some("Viewport selection box globals"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            1,
        );

        let vertex_buffer = StaticBuffer::create(
            &render_state.device,
            Some("Viewport selection box vertices"),
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            8,
        );

        let index_buffer = StaticBuffer::create_init(
            &render_state.device,
            Some("Viewport selection box indices"),
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
                    label: Some("Viewport selection box pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = render_state
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Viewport selection box pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[VertexBufferLayout {
                        array_stride: size_of!(Vertex) as BufferAddress,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &vertex_attr_array![0 => Float32x2],
                    }],
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
        box_a: Vec2f,
        box_b: Vec2f,
        box_color: [f32; 4],
    ) {
        self.global_buffer.write(
            &render_state.queue,
            &[Globals {
                color: box_color,
                resolution,
                offset,
                zoom: zoom * BASE_ZOOM,
            }],
        );

        let min_x = box_a.x.min(box_b.x);
        let max_x = box_a.x.max(box_b.x);
        let min_y = box_a.y.min(box_b.y);
        let max_y = box_a.y.max(box_b.y);

        let top_left = Vec2f::new(min_x, max_y);
        let top_right = Vec2f::new(max_x, max_y);
        let bottom_left = Vec2f::new(min_x, min_y);
        let bottom_right = Vec2f::new(max_x, min_y);

        let offset_nw_se = Vec2f::new(-LOGICAL_PIXEL_SIZE, LOGICAL_PIXEL_SIZE) / zoom;
        let offset_ne_sw = Vec2f::new(LOGICAL_PIXEL_SIZE, LOGICAL_PIXEL_SIZE) / zoom;

        let top_left_outside = top_left + offset_nw_se;
        let top_left_inside = top_left - offset_nw_se;

        let top_right_outside = top_right + offset_ne_sw;
        let top_right_inside = top_right - offset_ne_sw;

        let bottom_left_outside = bottom_left - offset_ne_sw;
        let bottom_left_inside = bottom_left + offset_ne_sw;

        let bottom_right_outside = bottom_right - offset_nw_se;
        let bottom_right_inside = bottom_right + offset_nw_se;

        let vertices = [
            Vertex {
                position: top_left_outside,
            },
            Vertex {
                position: top_left_inside,
            },
            Vertex {
                position: bottom_left_outside,
            },
            Vertex {
                position: bottom_left_inside,
            },
            Vertex {
                position: top_right_outside,
            },
            Vertex {
                position: top_right_inside,
            },
            Vertex {
                position: bottom_right_outside,
            },
            Vertex {
                position: bottom_right_inside,
            },
        ];

        self.vertex_buffer.write(&render_state.queue, &vertices);

        render_state.render_pass(texture_view, None, None, |pass, _| {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice());
            pass.set_index_buffer(self.index_buffer.slice(), IndexFormat::Uint16);

            pass.draw_indexed(0..(INDICES.len() as u32), 0, 0..1);
        });
    }
}
