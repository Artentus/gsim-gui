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

type Geometry = lyon::tessellation::VertexBuffers<Vertex, u32>;

fn build_and_gate_geometry() -> Geometry {
    use lyon::math::*;
    use lyon::path::*;
    use lyon::tessellation::*;

    const ARC_CTRL_POS: f32 = 0.552284749831 * 4.0;

    let mut builder = Path::builder();
    builder.begin(point(-4.0, -4.0));
    builder.line_to(point(-4.0, 0.0));
    builder.cubic_bezier_to(
        point(-4.0, ARC_CTRL_POS),
        point(-ARC_CTRL_POS, 4.0),
        point(0.0, 4.0),
    );
    builder.cubic_bezier_to(
        point(ARC_CTRL_POS, 4.0),
        point(4.0, ARC_CTRL_POS),
        point(4.0, 0.0),
    );
    builder.line_to(point(4.0, -4.0));
    builder.close();
    let path = builder.build();

    let mut geometry = Geometry::new();
    let mut tessellator = StrokeTessellator::new();
    tessellator
        .tessellate_path(
            &path,
            &StrokeOptions::DEFAULT
                .with_line_width(0.5)
                .with_tolerance(0.01),
            &mut BuffersBuilder::new(&mut geometry, |v: StrokeVertex| Vertex {
                position: v.position().to_array(),
            }),
        )
        .expect("failed to tessellate path");

    geometry
}

struct GeometryStore {
    and_gate_geometry: Geometry,
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

    fn draw_component_instances(
        &mut self,
        render_state: &RenderState,
        circuit: &Circuit,
        filter: impl Fn(&&Component) -> bool,
        geometry: &Geometry,
        color: [f32; 4],
    ) {
        let instances: Vec<_> = circuit
            .components()
            .iter()
            .filter(filter)
            .map(|c| Instance {
                offset: c.position.map(|x| x as f32),
                rotation: c.rotation.to_radians(),
                mirrored: c.mirrored as u32,
                color,
            })
            .collect();

        if instances.len() == 0 {
            return;
        }

        assert!(geometry.vertices.len() < (u32::MAX as usize));
        assert!(instances.len() < (u32::MAX as usize));
        assert!(geometry.indices.len() < (u32::MAX as usize));

        self.vertex_buffer.write_data(
            &render_state.device,
            &render_state.queue,
            &geometry.vertices,
        );

        self.instance_buffer
            .write_data(&render_state.device, &render_state.queue, &instances);

        self.index_buffer
            .write_data(&render_state.device, &render_state.queue, &geometry.indices);

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
                        load: LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.as_slice());
            pass.set_vertex_buffer(1, self.instance_buffer.as_slice());
            pass.set_index_buffer(self.index_buffer.as_slice(), IndexFormat::Uint32);

            pass.draw_indexed(
                0..(geometry.indices.len() as u32),
                0,
                0..(instances.len() as u32),
            );
        }

        render_state.queue.submit([encoder.finish()]);
    }

    pub fn draw(&mut self, render_state: &RenderState, circuit: Option<&Circuit>) {
        if let Some(circuit) = circuit {
            let globals = Globals {
                resolution: [self.texture.width() as f32, self.texture.height() as f32],
                offset: circuit.offset(),
                zoom: circuit.zoom(),
                _padding: [0; 3],
            };

            render_state
                .queue
                .write_buffer(&self.global_buffer, 0, bytemuck::bytes_of(&globals));
        }

        let mut encoder = render_state
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());
        {
            let _ = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &self.ms_texture_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::WHITE),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            // TODO: draw grid
        }
        render_state.queue.submit([encoder.finish()]);

        if let Some(circuit) = circuit {
            self.draw_component_instances(
                render_state,
                circuit,
                |c| matches!(c.kind, ComponentKind::AndGate { .. }),
                &GeometryStore::instance().and_gate_geometry,
                [0.0, 0.0, 0.0, 1.0],
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
