use super::super::buffer::*;
use super::super::{RenderStateEx, BASE_ZOOM, LOGICAL_PIXEL_SIZE};
use super::*;
use crate::app::circuit::Circuit;
use crate::app::math::*;
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

vs_input!(Vertex {
    position: Vec2f,
    selected: u32,
});

const BATCH_SIZE: usize = ((u16::MAX as usize) + 1) / 4;
#[allow(clippy::identity_op)]
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

pub struct WirePass {
    _shader: ShaderModule,
    global_buffer: StaticBuffer<Globals>,
    _bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    vertex_buffer: StaticBuffer<Vertex>,
    index_buffer: StaticBuffer<u16>,
    _pipeline_layout: PipelineLayout,
    pipeline: RenderPipeline,
}

impl WirePass {
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
            "wire",
            &shader,
            &bind_group_layout,
            &[Vertex::BUFFER_LAYOUT],
            None,
        );

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
        // TODO: cull the wire segments to the visible area

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
            let midpoints = segment.midpoints.iter().copied();
            let endpoint_b = std::iter::once(segment.endpoint_b);

            let selected = circuit.selection().contains_wire_segment(i);

            // TODO: correctly join the corners of the path if the segment has midpoints
            let mut a = segment.endpoint_a.to_vec2f();
            for b in midpoints.chain(endpoint_b).map(Vec2i::to_vec2f) {
                let dir = (b - a).normalized();
                let left = Vec2f::new(dir.y, -dir.x) * LOGICAL_PIXEL_SIZE;
                let right = Vec2f::new(-dir.y, dir.x) * LOGICAL_PIXEL_SIZE;

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

                a = b;
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
