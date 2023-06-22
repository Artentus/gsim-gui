use super::super::buffer::*;
use super::super::{RenderStateEx, BASE_ZOOM, LOGICAL_PIXEL_SIZE};
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

vs_input!(Vertex { position: Vec2f });

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

pub struct SelectionBoxPass {
    _shader: ShaderModule,
    global_buffer: StaticBuffer<Globals>,
    _bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    vertex_buffer: StaticBuffer<Vertex>,
    index_buffer: StaticBuffer<u16>,
    _pipeline_layout: PipelineLayout,
    pipeline: RenderPipeline,
}

impl SelectionBoxPass {
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
            "selection box",
            &shader,
            &bind_group_layout,
            &[Vertex::BUFFER_LAYOUT],
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

    #[allow(clippy::too_many_arguments)]
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
