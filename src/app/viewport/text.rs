mod atlas;
use atlas::*;

use super::buffer::*;
use super::pass::*;
use super::{ViewportColors, BASE_ZOOM};
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
    px_range: f32,
}

vs_input!(Vertex {
    position: Vec2f,
    uv: Vec2f,
    selected: u32,
});

const MAX_VERTEX_COUNT: usize = (u16::MAX as usize) + 1;
const BATCH_SIZE: usize = MAX_VERTEX_COUNT / 4;

#[allow(clippy::identity_op)]
const INDICES: [u16; BATCH_SIZE * 6] = {
    let mut indices = [0; BATCH_SIZE * 6];
    let mut i = 0;
    while i < BATCH_SIZE {
        indices[i * 6 + 0] = (i as u16) * 4 + 0;
        indices[i * 6 + 1] = (i as u16) * 4 + 1;
        indices[i * 6 + 2] = (i as u16) * 4 + 2;
        indices[i * 6 + 3] = (i as u16) * 4 + 0;
        indices[i * 6 + 4] = (i as u16) * 4 + 2;
        indices[i * 6 + 5] = (i as u16) * 4 + 3;
        i += 1;
    }
    indices
};

const ATLAS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/fonts/Inter/Inter-Regular.json"
));

const ATLAS_TEXTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/fonts/Inter/Inter-Regular.png"
));

pub struct TextPass {
    _shader: ShaderModule,
    atlas: FontAtlas,
    _atlas_texture: Texture,
    _atlas_view: TextureView,
    _sampler: Sampler,
    global_buffer: StaticBuffer<Globals>,
    _bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    vertex_buffer: StaticBuffer<Vertex>,
    index_buffer: StaticBuffer<u16>,
    _pipeline_layout: PipelineLayout,
    pipeline: RenderPipeline,
    vertices: Vec<Vertex>,
}

impl TextPass {
    pub fn create(render_state: &RenderState) -> Self {
        let shader = shader!(render_state.device, "text");

        let atlas = FontAtlas::load(ATLAS).unwrap();

        let atlas_texture_reader = std::io::Cursor::new(ATLAS_TEXTURE);
        let atlas_texture =
            render_state.create_texture(atlas_texture_reader, Some("Viewport text atlas"), false);
        let atlas_view = atlas_texture.create_view(&TextureViewDescriptor::default());

        let sampler = render_state.device.create_sampler(&SamplerDescriptor {
            label: Some("Viewport text sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        let global_buffer = StaticBuffer::create(
            &render_state.device,
            Some("Viewport text globals"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            1,
        );

        let vertex_buffer = StaticBuffer::create(
            &render_state.device,
            Some("Viewport text vertices"),
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            MAX_VERTEX_COUNT,
        );

        let index_buffer = StaticBuffer::create_init(
            &render_state.device,
            Some("Viewport text indices"),
            BufferUsages::INDEX,
            &INDICES,
        );

        let bind_group_layout =
            render_state
                .device
                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[
                        BindGroupLayoutEntry {
                            binding: 0,
                            visibility: ShaderStages::VERTEX_FRAGMENT,
                            ty: BindingType::Buffer {
                                ty: BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: Some(global_buffer.byte_size()),
                            },
                            count: None,
                        },
                        BindGroupLayoutEntry {
                            binding: 1,
                            visibility: ShaderStages::FRAGMENT,
                            ty: BindingType::Texture {
                                sample_type: TextureSampleType::Float { filterable: true },
                                view_dimension: TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        BindGroupLayoutEntry {
                            binding: 2,
                            visibility: ShaderStages::FRAGMENT,
                            ty: BindingType::Sampler(SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let bind_group = render_state.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: global_buffer.as_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&atlas_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        let (pipeline_layout, pipeline) = create_pipeline(
            &render_state.device,
            "text",
            &shader,
            &bind_group_layout,
            &[Vertex::BUFFER_LAYOUT],
            Some(BlendState::ALPHA_BLENDING),
        );

        Self {
            _shader: shader,
            atlas,
            _atlas_texture: atlas_texture,
            _atlas_view: atlas_view,
            _sampler: sampler,
            global_buffer,
            _bind_group_layout: bind_group_layout,
            bind_group,
            vertex_buffer,
            index_buffer,
            _pipeline_layout: pipeline_layout,
            pipeline,
            vertices: Vec::with_capacity(MAX_VERTEX_COUNT),
        }
    }

    fn draw_batch(&mut self, render_state: &RenderState, texture_view: &TextureView) {
        self.vertex_buffer
            .write(&render_state.queue, &self.vertices);

        render_state.render_pass(texture_view, None, None, |pass, _| {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice());
            pass.set_index_buffer(self.index_buffer.slice(), IndexFormat::Uint16);

            let index_count = ((self.vertices.len() / 4) * 6) as u32;
            pass.draw_indexed(0..index_count, 0, 0..1);
        });

        self.vertices.clear();
    }

    fn draw_text(
        &mut self,
        render_state: &RenderState,
        texture_view: &TextureView,
        text: &str,
        selected: bool,
        position: Vec2f,
        font_size: f32, // in grid units
    ) {
        let mut rel_x = 0.0;

        let mut prev: Option<char> = None;
        for c in text.chars() {
            if let Some(glyph) = self.atlas.get_glyph(c) {
                let kerning = self.atlas.get_kerning(prev, c);

                if let Some(sprite) = &glyph.sprite {
                    let top = sprite.bounds.top;
                    let bottom = sprite.bounds.bottom;
                    let left = rel_x + sprite.bounds.left + kerning;
                    let right = rel_x + sprite.bounds.right + kerning;

                    self.vertices.push(Vertex {
                        position: Vec2f::new(left, top) * font_size + position,
                        uv: Vec2f::new(sprite.uv_bounds.left, sprite.uv_bounds.top),
                        selected: selected as u32,
                    });
                    self.vertices.push(Vertex {
                        position: Vec2f::new(right, top) * font_size + position,
                        uv: Vec2f::new(sprite.uv_bounds.right, sprite.uv_bounds.top),
                        selected: selected as u32,
                    });
                    self.vertices.push(Vertex {
                        position: Vec2f::new(right, bottom) * font_size + position,
                        uv: Vec2f::new(sprite.uv_bounds.right, sprite.uv_bounds.bottom),
                        selected: selected as u32,
                    });
                    self.vertices.push(Vertex {
                        position: Vec2f::new(left, bottom) * font_size + position,
                        uv: Vec2f::new(sprite.uv_bounds.left, sprite.uv_bounds.bottom),
                        selected: selected as u32,
                    });
                }

                rel_x += glyph.x_advance + kerning;
                prev = Some(c);

                if self.vertices.len() >= MAX_VERTEX_COUNT {
                    self.draw_batch(render_state, texture_view);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        render_state: &RenderState,
        render_target: &TextureView,
        circuit: &Circuit,
        resolution: Vec2f,
        offset: Vec2f,
        zoom: f32,
        colors: &ViewportColors,
    ) {
        // TODO: cull the text to the visible area
        // TODO: don't draw text that is unreadably small

        self.global_buffer.write(
            &render_state.queue,
            &[Globals {
                color: convert_color(colors.component_color),
                selected_color: convert_color(colors.selected_component_color),
                resolution,
                offset,
                zoom: zoom * BASE_ZOOM,
                px_range: self.atlas.get_distance_range(zoom * BASE_ZOOM),
            }],
        );

        // Font sizes are in grid units
        const NAME_FONT_SIZE: f32 = 1.0;

        for (i, component) in circuit.components().iter().enumerate() {
            let name = component.kind.name();

            if !name.is_empty() {
                let selected = circuit.selection().contains_component(i);
                let name_width = self.atlas.measure_text(&name);
                let name_offset =
                    Vec2f::new(name_width, self.atlas.line_height) * NAME_FONT_SIZE * 0.5;

                self.draw_text(
                    render_state,
                    render_target,
                    &name,
                    selected,
                    component.position.to_vec2f() - name_offset,
                    NAME_FONT_SIZE,
                );
            }
        }

        if !self.vertices.is_empty() {
            self.draw_batch(render_state, render_target);
        }
    }
}
