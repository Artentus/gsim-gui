use super::super::buffer::*;
use super::super::{RenderStateEx, BASE_ZOOM, LOGICAL_PIXEL_SIZE};
use super::*;
use crate::app::circuit::Circuit;
use crate::app::component::AnchorKind;
use crate::app::math::*;
use crate::HashSet;
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

vs_input!(
    Vertex { position: Vec2f }

    Instance {
        offset: Vec2f,
        kind: u32,
        size: f32,
    }
);

const VERTEX_COUNT: usize = 24;

fn vertices() -> &'static [Vertex; VERTEX_COUNT + 1] {
    use std::sync::OnceLock;

    static VERTICES: OnceLock<[Vertex; VERTEX_COUNT + 1]> = OnceLock::new();
    VERTICES.get_or_init(|| {
        let mut vertices = [Vertex {
            position: Vec2f::default(),
        }; VERTEX_COUNT + 1];

        #[allow(clippy::needless_range_loop)]
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

#[allow(clippy::identity_op)]
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

pub struct AnchorPass {
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

impl AnchorPass {
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
            "anchor",
            &shader,
            &bind_group_layout,
            &[Vertex::BUFFER_LAYOUT, Instance::BUFFER_LAYOUT],
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

    pub fn draw(
        &mut self,
        render_state: &RenderState,
        texture_view: &TextureView,
        circuit: &Circuit,
        resolution: Vec2f,
        offset: Vec2f,
        zoom: f32,
    ) {
        // TODO: cull the anchors to the visible area

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

        if !instances.is_empty() {
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
