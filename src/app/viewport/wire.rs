use super::buffer::*;
use super::{shader, RenderStateEx, BASE_ZOOM, LOGICAL_PIXEL_SIZE};
use crate::app::circuit::Circuit;
use crate::app::math::*;
use crate::size_of;
use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::RenderState;
use wgpu::*;

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Globals {
    color: [f32; 4],
    selected_color: [f32; 4],
    resolution: Vec2f,
    offset: Vec2f,
    zoom: f32,
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct Vertex {
    position: Vec2f,
    selected: u32,
}

const BATCH_SIZE: usize = ((u16::MAX as usize) + 1) / 4;
const INDICES: [u16; BATCH_SIZE * 6] = {
    let mut indices = [0; BATCH_SIZE * 6];
    let mut i = 0;
    while i < BATCH_SIZE {
        indices[i * 6 + 0] = (i as u16) * 4 + 0;
        indices[i * 6 + 1] = (i as u16) * 4 + 1;
        indices[i * 6 + 2] = (i as u16) * 4 + 2;
        indices[i * 6 + 3] = (i as u16) * 4 + 1;
        indices[i * 6 + 4] = (i as u16) * 4 + 3;
        indices[i * 6 + 5] = (i as u16) * 4 + 2;
        i += 1;
    }
    indices
};

pub struct ViewportWires {
    _shader: ShaderModule,
    global_buffer: StaticBuffer<Globals>,
    _bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    vertex_buffer: StaticBuffer<Vertex>,
    index_buffer: StaticBuffer<u16>,
    _pipeline_layout: PipelineLayout,
    pipeline: RenderPipeline,
}

impl ViewportWires {
    pub fn create(render_state: &RenderState) -> Self {
        let shader = shader!(render_state.device, "wire");

        let global_buffer = StaticBuffer::create(
            &render_state.device,
            Some("Viewport wire globals"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            1,
        );

        let vertex_buffer = StaticBuffer::create(
            &render_state.device,
            Some("Viewport wire vertices"),
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            BATCH_SIZE * 4,
        );

        let index_buffer = StaticBuffer::create_init(
            &render_state.device,
            Some("Viewport wire indices"),
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
                        visibility: ShaderStages::VERTEX,
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
                    label: Some("Viewport wire pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = render_state
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Viewport wire pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[VertexBufferLayout {
                        array_stride: size_of!(Vertex) as BufferAddress,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &vertex_attr_array![0 => Float32x2, 1 => Uint32],
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
        circuit: &Circuit,
        resolution: Vec2f,
        offset: Vec2f,
        zoom: f32,
    ) {
        self.global_buffer.write(
            &render_state.queue,
            &[Globals {
                color: [0.0, 0.0, 1.0, 1.0],
                selected_color: [0.3, 0.3, 1.0, 1.0],
                resolution,
                offset,
                zoom: zoom * BASE_ZOOM,
            }],
        );

        let mut count = 0;
        let mut vertices = Vec::with_capacity(BATCH_SIZE * 4);
        for (i, segment) in circuit.wire_segments().iter().enumerate() {
            let a = segment.point_a.to_vec2f();
            let b = segment.point_b.to_vec2f();
            let dir = (b - a).normalized();
            let left = Vec2f::new(dir.y, -dir.x) * LOGICAL_PIXEL_SIZE;
            let right = Vec2f::new(-dir.y, dir.x) * LOGICAL_PIXEL_SIZE;

            let selected = circuit.selection().contains_wire_segment(i);

            vertices.push(Vertex {
                position: a + left,
                selected: selected as u32,
            });
            vertices.push(Vertex {
                position: a + right,
                selected: selected as u32,
            });
            vertices.push(Vertex {
                position: b + left,
                selected: selected as u32,
            });
            vertices.push(Vertex {
                position: b + right,
                selected: selected as u32,
            });

            count += 1;
            if count >= BATCH_SIZE {
                self.vertex_buffer.write(&render_state.queue, &vertices);

                render_state.render_pass(texture_view, None, None, |pass, _| {
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.bind_group, &[]);
                    pass.set_vertex_buffer(0, self.vertex_buffer.slice());
                    pass.set_index_buffer(self.index_buffer.slice(), IndexFormat::Uint16);

                    pass.draw_indexed(0..((BATCH_SIZE * 6) as u32), 0, 0..1);
                });

                count = 0;
                vertices.clear();
            }
        }

        if count > 0 {
            self.vertex_buffer.write(&render_state.queue, &vertices);

            render_state.render_pass(texture_view, None, None, |pass, _| {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice());
                pass.set_index_buffer(self.index_buffer.slice(), IndexFormat::Uint16);

                pass.draw_indexed(0..((count * 6) as u32), 0, 0..1);
            });
        }
    }
}
