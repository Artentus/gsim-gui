use super::circuit::*;
use super::component::*;
use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::RenderState;
use egui::TextureId;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use wgpu::*;

macro_rules! size_of {
    ($t:ty) => {
        std::mem::size_of::<$t>()
    };
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Globals {
    resolution: [f32; 2],
    offset: [f32; 2],
    zoom: f32,
    _padding: [u32; 3],
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 2],
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Instance {
    offset: [f32; 2],
    rotation: f32,
    mirrored: u32,
    color: [f32; 4],
}

fn create_viewport_texture(
    render_state: &RenderState,
    width: u32,
    height: u32,
) -> (Texture, TextureView, Texture, TextureView) {
    let desc = TextureDescriptor {
        label: Some("Viewport"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[TextureFormat::Rgba8Unorm],
    };

    let texture = render_state.device.create_texture(&desc);
    let texture_view = texture.create_view(&TextureViewDescriptor::default());

    let ms_desc = TextureDescriptor {
        label: Some("ViewportMS"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 4,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[TextureFormat::Rgba8Unorm],
    };

    let ms_texture = render_state.device.create_texture(&ms_desc);
    let ms_texture_view = ms_texture.create_view(&TextureViewDescriptor::default());

    (texture, texture_view, ms_texture, ms_texture_view)
}

struct DynamicBuffer<T: Pod> {
    label: String,
    usage: BufferUsages,
    capacity: usize,
    len: usize,
    buffer: Buffer,
    _t: PhantomData<*mut T>,
}

impl<T: Pod> DynamicBuffer<T> {
    fn create(device: &Device, label: impl Into<String>, usage: BufferUsages) -> Self {
        const INITIAL_CAPACITY: usize = 1000;

        let label: String = label.into();

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(label.as_str()),
            size: (size_of!(T) * INITIAL_CAPACITY) as u64,
            usage: usage | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            label,
            usage,
            capacity: INITIAL_CAPACITY,
            len: 0,
            buffer,
            _t: PhantomData,
        }
    }

    fn write_data(&mut self, device: &Device, queue: &Queue, data: &[T]) {
        if data.len() > self.capacity {
            self.capacity = data.len() * 2;

            self.buffer = device.create_buffer(&BufferDescriptor {
                label: Some(self.label.as_str()),
                size: (size_of!(T) * self.capacity) as u64,
                usage: self.usage | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        self.len = data.len();
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
    }

    #[inline]
    fn as_slice(&self) -> BufferSlice<'_> {
        let slice_end = (size_of!(T) * self.len) as u64;
        self.buffer.slice(..slice_end)
    }
}

pub const BASE_ZOOM: f32 = 10.0; // Logical pixels per unit
const LOGICAL_PIXEL_SIZE: f32 = 1.0 / BASE_ZOOM;
const GEOMETRY_TOLERANCE: f32 = LOGICAL_PIXEL_SIZE / 16.0;

type Geometry = lyon::tessellation::VertexBuffers<Vertex, u32>;

fn build_and_gate_geometry() -> (Geometry, Geometry) {
    use lyon::math::*;
    use lyon::path::*;
    use lyon::tessellation::*;

    const ARC_CTRL_POS: f32 = 0.552284749831 * 2.0;

    let mut builder = Path::builder();
    builder.begin(point(-2.0, -2.0));
    builder.line_to(point(-2.0, 0.0));
    builder.cubic_bezier_to(
        point(-2.0, ARC_CTRL_POS),
        point(-ARC_CTRL_POS, 2.0),
        point(0.0, 2.0),
    );
    builder.cubic_bezier_to(
        point(ARC_CTRL_POS, 2.0),
        point(2.0, ARC_CTRL_POS),
        point(2.0, 0.0),
    );
    builder.line_to(point(2.0, -2.0));
    builder.close();
    let path = builder.build();

    let mut stroke_geometry = Geometry::new();
    let mut stroke_tessellator = StrokeTessellator::new();
    stroke_tessellator
        .tessellate_path(
            &path,
            &StrokeOptions::DEFAULT
                .with_line_width(2.0 * LOGICAL_PIXEL_SIZE)
                .with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut stroke_geometry, |v: StrokeVertex| Vertex {
                position: v.position().to_array(),
            }),
        )
        .expect("failed to tessellate path");

    let mut fill_geometry = Geometry::new();
    let mut fill_tessellator = FillTessellator::new();
    fill_tessellator
        .tessellate_path(
            &path,
            &FillOptions::DEFAULT.with_tolerance(GEOMETRY_TOLERANCE),
            &mut BuffersBuilder::new(&mut fill_geometry, |v: FillVertex| Vertex {
                position: v.position().to_array(),
            }),
        )
        .expect("failed to tessellate path");

    (stroke_geometry, fill_geometry)
}

struct GeometryStore {
    and_gate_geometry: (Geometry, Geometry),
}

impl GeometryStore {
    fn instance() -> &'static Self {
        use once_cell::sync::OnceCell;

        static INSTANCE: OnceCell<GeometryStore> = OnceCell::new();
        INSTANCE.get_or_init(|| GeometryStore {
            and_gate_geometry: build_and_gate_geometry(),
        })
    }
}

pub struct ViewportColors {
    pub background_color: [f32; 4],
    pub grid_color: [f32; 4],
    pub component_color: [f32; 4],
}

pub struct Viewport {
    _shader: ShaderModule,
    texture_id: TextureId,
    texture: Texture,
    texture_view: TextureView,
    ms_texture: Texture,
    ms_texture_view: TextureView,
    global_buffer: Buffer,
    _bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    vertex_buffer: DynamicBuffer<Vertex>,
    instance_buffer: DynamicBuffer<Instance>,
    index_buffer: DynamicBuffer<u32>,
    _pipeline_layout: PipelineLayout,
    pipeline: RenderPipeline,
}

impl Viewport {
    pub fn create(render_state: &RenderState, width: u32, height: u32) -> Self {
        let shader = render_state
            .device
            .create_shader_module(include_wgsl!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/shaders/viewport.wgsl"
            )));

        let (texture, texture_view, ms_texture, ms_texture_view) =
            create_viewport_texture(render_state, width, height);

        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &texture_view,
            FilterMode::Nearest,
        );

        let global_buffer = render_state.device.create_buffer(&BufferDescriptor {
            label: Some("Viewport globals"),
            size: size_of!(Globals) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let vertex_buffer = DynamicBuffer::create(
            &render_state.device,
            "Viewport vertices",
            BufferUsages::VERTEX,
        );

        let instance_buffer = DynamicBuffer::create(
            &render_state.device,
            "Viewport instances",
            BufferUsages::VERTEX,
        );

        let index_buffer = DynamicBuffer::create(
            &render_state.device,
            "Viewport indices",
            BufferUsages::INDEX,
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
                            min_binding_size: Some(
                                NonZeroU64::new(size_of!(Globals) as u64).unwrap(),
                            ),
                        },
                        count: None,
                    }],
                });

        let bind_group = render_state.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: global_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout =
            render_state
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Viewport pipeline layout"),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let pipeline = render_state
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Viewport pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[
                        VertexBufferLayout {
                            array_stride: size_of!(Vertex) as u64,
                            step_mode: VertexStepMode::Vertex,
                            attributes: &vertex_attr_array![0 => Float32x2],
                        },
                        VertexBufferLayout {
                            array_stride: size_of!(Instance) as u64,
                            step_mode: VertexStepMode::Instance,
                            attributes: &vertex_attr_array![1 => Float32x2, 2 => Float32, 3 => Uint32, 4 => Float32x4],
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
            texture_id,
            texture,
            texture_view,
            ms_texture,
            ms_texture_view,
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

    pub fn resize(&mut self, render_state: &RenderState, width: u32, height: u32) {
        if (self.texture.width() == width) && (self.texture.height() == height) {
            return;
        }

        let (texture, texture_view, ms_texture, ms_texture_view) =
            create_viewport_texture(render_state, width, height);

        render_state
            .renderer
            .write()
            .update_egui_texture_from_wgpu_texture(
                &render_state.device,
                &texture_view,
                FilterMode::Nearest,
                self.texture_id,
            );

        self.texture = texture;
        self.texture_view = texture_view;
        self.ms_texture = ms_texture;
        self.ms_texture_view = ms_texture_view;
    }

    #[inline]
    pub fn texture_id(&self) -> TextureId {
        self.texture_id
    }

    fn draw_primitives(
        &mut self,
        render_state: &RenderState,
        load_op: LoadOp<Color>,
        vertices: &[Vertex],
        instances: &[Instance],
        indices: &[u32],
    ) {
        assert!(vertices.len() < (u32::MAX as usize));
        assert!(instances.len() < (u32::MAX as usize));
        assert!(indices.len() < (u32::MAX as usize));

        if (instances.len() > 0) && (indices.len() > 0) {
            self.vertex_buffer
                .write_data(&render_state.device, &render_state.queue, vertices);

            self.instance_buffer
                .write_data(&render_state.device, &render_state.queue, instances);

            self.index_buffer
                .write_data(&render_state.device, &render_state.queue, indices);
        }

        let mut encoder = render_state
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.ms_texture_view,
                    resolve_target: None,
                    ops: Operations {
                        load: load_op,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            if (instances.len() > 0) && (indices.len() > 0) {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.as_slice());
                pass.set_vertex_buffer(1, self.instance_buffer.as_slice());
                pass.set_index_buffer(self.index_buffer.as_slice(), IndexFormat::Uint32);

                pass.draw_indexed(0..(indices.len() as u32), 0, 0..(instances.len() as u32));
            }
        }

        render_state.queue.submit([encoder.finish()]);
    }

    fn draw_grid(
        &mut self,
        render_state: &RenderState,
        offset: [f32; 2],
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
            self.draw_primitives(render_state, LoadOp::Clear(clear_color), &[], &[], &[]);
            return;
        }

        let step = if zoom > 1.99 { 1 } else { 2 };

        let width = (self.texture.width() as f32) / (zoom * BASE_ZOOM);
        let height = (self.texture.height() as f32) / (zoom * BASE_ZOOM);

        let left = (offset[0] - (width * 0.5)).ceil() as i32;
        let right = (offset[0] + (width * 0.5)).floor() as i32;
        let bottom = (offset[1] - (height * 0.5)).ceil() as i32;
        let top = (offset[1] + (height * 0.5)).floor() as i32;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for x in (left..=right).filter(|&x| (x % step) == 0) {
            let p_left = (x as f32) - ((LOGICAL_PIXEL_SIZE / 2.0) * (step as f32));
            let p_right = (x as f32) + ((LOGICAL_PIXEL_SIZE / 2.0) * (step as f32));

            indices.push((vertices.len() as u32) + 0);
            indices.push((vertices.len() as u32) + 1);
            indices.push((vertices.len() as u32) + 2);
            indices.push((vertices.len() as u32) + 1);
            indices.push((vertices.len() as u32) + 3);
            indices.push((vertices.len() as u32) + 2);

            vertices.push(Vertex {
                position: [p_left, (-LOGICAL_PIXEL_SIZE / 2.0) * (step as f32)],
            });
            vertices.push(Vertex {
                position: [p_left, (LOGICAL_PIXEL_SIZE / 2.0) * (step as f32)],
            });
            vertices.push(Vertex {
                position: [p_right, (-LOGICAL_PIXEL_SIZE / 2.0) * (step as f32)],
            });
            vertices.push(Vertex {
                position: [p_right, (LOGICAL_PIXEL_SIZE / 2.0) * (step as f32)],
            });
        }

        let instances: Vec<_> = (bottom..=top)
            .filter(|&y| (y % step) == 0)
            .map(|y| Instance {
                offset: [0.0, y as f32],
                rotation: 0.0,
                mirrored: 0,
                color: grid_color,
            })
            .collect();

        self.draw_primitives(
            render_state,
            LoadOp::Clear(clear_color),
            &vertices,
            &instances,
            &indices,
        );
    }

    fn draw_component_instances(
        &mut self,
        render_state: &RenderState,
        circuit: &Circuit,
        filter: impl Fn(&&Component) -> bool,
        geometry: &(Geometry, Geometry),
        stroke_color: [f32; 4],
        fill_color: [f32; 4],
    ) {
        let mut instances: Vec<_> = circuit
            .components()
            .iter()
            .filter(filter)
            .map(|c| Instance {
                offset: c.position.map(|x| x as f32),
                rotation: c.rotation.to_radians(),
                mirrored: c.mirrored as u32,
                color: fill_color,
            })
            .collect();

        if instances.len() == 0 {
            return;
        }

        self.draw_primitives(
            render_state,
            LoadOp::Load,
            &geometry.1.vertices,
            &instances,
            &geometry.1.indices,
        );

        for instance in instances.iter_mut() {
            instance.color = stroke_color;
        }

        self.draw_primitives(
            render_state,
            LoadOp::Load,
            &geometry.0.vertices,
            &instances,
            &geometry.0.indices,
        );
    }

    pub fn draw(
        &mut self,
        render_state: &RenderState,
        circuit: Option<&Circuit>,
        colors: ViewportColors,
    ) {
        let width = self.texture.width() as f32;
        let height = self.texture.height() as f32;

        let (offset, zoom) = circuit
            .map(|c| (c.offset(), c.zoom()))
            .unwrap_or(([0.0; 2], DEFAULT_ZOOM));

        let globals = Globals {
            resolution: [width, height],
            offset,
            zoom: zoom * BASE_ZOOM,
            _padding: [0; 3],
        };

        render_state
            .queue
            .write_buffer(&self.global_buffer, 0, bytemuck::bytes_of(&globals));

        self.draw_grid(
            render_state,
            offset,
            zoom,
            colors.background_color,
            colors.grid_color,
        );

        if let Some(circuit) = circuit {
            self.draw_component_instances(
                render_state,
                circuit,
                |c| matches!(c.kind, ComponentKind::AndGate { .. }),
                &GeometryStore::instance().and_gate_geometry,
                colors.component_color,
                colors.background_color,
            );
        }

        let mut encoder = render_state
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());
        {
            let _ = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.ms_texture_view,
                    resolve_target: Some(&self.texture_view),
                    ops: Operations {
                        load: LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        }
        render_state.queue.submit([encoder.finish()]);
    }
}
