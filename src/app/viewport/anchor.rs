use super::buffer::*;
use super::{shader, RenderStateEx, BASE_ZOOM, LOGICAL_PIXEL_SIZE};
use crate::app::circuit::Circuit;
use crate::app::component::AnchorKind;
use crate::app::math::*;
use crate::{size_of, HashSet};
use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::RenderState;
use wgpu::*;

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Globals {
    input_color: [f32; 4],
    output_color: [f32; 4],
    bidirectional_color: [f32; 4],
    passive_color: [f32; 4],
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
    kind: u32,
    size: f32,
}

const VERTEX_COUNT: usize = 24;

fn vertices() -> &'static [Vertex; VERTEX_COUNT + 1] {
    use std::sync::OnceLock;

    static VERTICES: OnceLock<[Vertex; VERTEX_COUNT + 1]> = OnceLock::new();
    VERTICES.get_or_init(|| {
        let mut vertices = [Vertex {
            position: Vec2f::default(),
        }; VERTEX_COUNT + 1];
        for i in 0..VERTEX_COUNT {
            let angle = ((i as f32) / (VERTEX_COUNT as f32)) * std::f32::consts::TAU;
            let (y, x) = angle.sin_cos();
            vertices[i] = Vertex {
                position: Vec2f::new(x, y),
            };
        }
        vertices
    })
}

const INDICES: [u16; VERTEX_COUNT * 3] = {
    let mut indices = [0; VERTEX_COUNT * 3];
    let mut i = 0;
    while i < VERTEX_COUNT {
        indices[i * 3 + 0] = VERTEX_COUNT as u16;
        indices[i * 3 + 1] = ((i + 0) % VERTEX_COUNT) as u16;
        indices[i * 3 + 2] = ((i + 1) % VERTEX_COUNT) as u16;
        i += 1;
    }
    indices
};

pub struct ViewportAnchors {
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

impl ViewportAnchors {
    pub fn create(render_state: &RenderState) -> Self {
        let shader = shader!(render_state.device, "anchor");

        let global_buffer = StaticBuffer::create(
            &render_state.device,
            Some("Viewport anchor globals"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            1,
        );

        let vertex_buffer = StaticBuffer::create_init(
            &render_state.device,
            Some("Viewport anchor vertices"),
            BufferUsages::VERTEX,
            vertices(),
        );

        let instance_buffer = DynamicBuffer::create(
            &render_state.device,
            Some("Viewport anchor instances"),
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            128 * 1024,
        );

        let index_buffer = StaticBuffer::create_init(
            &render_state.device,
            Some("Viewport anchor indices"),
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
                    label: Some("Viewport anchor pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = render_state
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Viewport anchor pipeline"),
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
                            attributes: &vertex_attr_array![1 => Float32x2, 2 => Uint32, 3 => Float32],
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
        circuit: &Circuit,
        resolution: Vec2f,
        offset: Vec2f,
        zoom: f32,
    ) {
        let mut segment_end_points = HashSet::default();
        for segment in circuit.wire_segments() {
            segment_end_points.insert(segment.endpoint_a);
            segment_end_points.insert(segment.endpoint_b);
        }

        let mut instances = Vec::new();
        for point in segment_end_points {
            instances.push(Instance {
                offset: point.to_vec2f(),
                kind: AnchorKind::Passive as u32,
                size: LOGICAL_PIXEL_SIZE,
            });
        }
        for component in circuit.components() {
            for anchor in component.anchors() {
                instances.push(Instance {
                    offset: anchor.position.to_vec2f(),
                    kind: anchor.kind as u32,
                    size: LOGICAL_PIXEL_SIZE * 2.0,
                });
            }
        }

        if instances.len() > 0 {
            self.global_buffer.write(
                &render_state.queue,
                &[Globals {
                    input_color: [0.0, 1.0, 0.0, 1.0],
                    output_color: [1.0, 0.0, 0.0, 1.0],
                    bidirectional_color: [1.0, 1.0, 0.0, 1.0],
                    passive_color: [0.0, 0.0, 1.0, 1.0],
                    resolution,
                    offset,
                    zoom: zoom * BASE_ZOOM,
                }],
            );

            self.instance_buffer
                .write(&render_state.device, &render_state.queue, &instances);

            render_state.render_pass(texture_view, None, None, |pass, _| {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice());
                pass.set_vertex_buffer(1, self.instance_buffer.slice());
                pass.set_index_buffer(self.index_buffer.slice(), IndexFormat::Uint16);

                pass.draw_indexed(0..(INDICES.len() as u32), 0, 0..(instances.len() as u32));
            });
        }
    }
}
