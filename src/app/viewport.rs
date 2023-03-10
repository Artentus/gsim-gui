use super::circuit::*;
use super::component::*;
use crate::size_of;
use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::RenderState;
use egui::TextureId;
use wgpu::*;

mod buffer;
use buffer::*;

mod grid;
use grid::*;

mod wire;
use wire::*;

mod anchor;
use anchor::*;

macro_rules! shader {
    ($device:expr, $name:literal) => {{
        const SOURCE: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/shaders/",
            $name,
            ".wgsl"
        ));

        const DESC: wgpu::ShaderModuleDescriptor = wgpu::ShaderModuleDescriptor {
            label: Some($name),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(SOURCE)),
        };

        $device.create_shader_module(DESC)
    }};
}
pub(self) use shader;

trait RenderStateEx {
    fn render_pass<'env, F>(
        &self,
        view: &TextureView,
        resolve_target: Option<&TextureView>,
        clear_color: Option<Color>,
        f: F,
    ) where
        // To restrict the lifetime of the closure in a way the compiler understands,
        // this weird double reference is necessary.
        for<'pass> F: FnOnce(&mut RenderPass<'pass>, &'pass &'env ());

    #[inline]
    fn clear_pass(&self, view: &TextureView, clear_color: Color) {
        self.render_pass(view, None, Some(clear_color), |_, _| {});
    }

    #[inline]
    fn resolve_pass(&self, view: &TextureView, resolve_target: &TextureView) {
        self.render_pass(view, Some(resolve_target), None, |_, _| {});
    }
}

impl RenderStateEx for RenderState {
    fn render_pass<'env, F>(
        &self,
        view: &TextureView,
        resolve_target: Option<&TextureView>,
        clear_color: Option<Color>,
        f: F,
    ) where
        for<'pass> F: FnOnce(&mut RenderPass<'pass>, &'pass &'env ()),
    {
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view,
                    resolve_target,
                    ops: Operations {
                        load: if let Some(clear_color) = clear_color {
                            LoadOp::Clear(clear_color)
                        } else {
                            LoadOp::Load
                        },
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            f(&mut pass, &&());
        }

        self.queue.submit([encoder.finish()]);
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Globals {
    resolution: [f32; 2],
    offset: [f32; 2],
    zoom: f32,
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Vertex {
    position: [f32; 2],
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Instance {
    offset: [f32; 2],
    rotation: u32,
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

pub const BASE_ZOOM: f32 = 10.0; // Logical pixels per unit
const LOGICAL_PIXEL_SIZE: f32 = 1.0 / BASE_ZOOM;
const GEOMETRY_TOLERANCE: f32 = LOGICAL_PIXEL_SIZE / 16.0;

struct Geometry {
    vertices: StaticBuffer<Vertex>,
    indices: StaticBuffer<u16>,
}

macro_rules! geometry {
    ($device:expr, $stroke:expr, $fill:expr, $label:literal) => {
        (
            Geometry {
                vertices: StaticBuffer::create_init(
                    $device,
                    Some(concat!($label, " stroke vertices")),
                    BufferUsages::VERTEX,
                    &$stroke.vertices,
                ),
                indices: StaticBuffer::create_init(
                    $device,
                    Some(concat!($label, " stroke indices")),
                    BufferUsages::INDEX,
                    &$stroke.indices,
                ),
            },
            Geometry {
                vertices: StaticBuffer::create_init(
                    $device,
                    Some(concat!($label, " fill vertices")),
                    BufferUsages::VERTEX,
                    &$fill.vertices,
                ),
                indices: StaticBuffer::create_init(
                    $device,
                    Some(concat!($label, " fill indices")),
                    BufferUsages::INDEX,
                    &$fill.indices,
                ),
            },
        )
    };
}

fn build_and_gate_geometry(device: &Device) -> (Geometry, Geometry) {
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

    let mut stroke_geometry = VertexBuffers::new();
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

    let mut fill_geometry = VertexBuffers::new();
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

    geometry!(device, stroke_geometry, fill_geometry, "AND gate")
}

struct GeometryStore {
    and_gate_geometry: (Geometry, Geometry),
}

impl GeometryStore {
    fn instance(device: &Device) -> &'static Self {
        use once_cell::sync::OnceCell;

        static INSTANCE: OnceCell<GeometryStore> = OnceCell::new();
        INSTANCE.get_or_init(|| GeometryStore {
            and_gate_geometry: build_and_gate_geometry(device),
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
    global_buffer: StaticBuffer<Globals>,
    _bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    instance_buffer: DynamicBuffer<Instance>,
    _pipeline_layout: PipelineLayout,
    pipeline: RenderPipeline,
    grid: ViewportGrid,
    wires: ViewportWires,
    anchors: ViewportAnchors,
}

impl Viewport {
    pub fn create(render_state: &RenderState, width: u32, height: u32) -> Self {
        let shader = shader!(render_state.device, "component");

        let (texture, texture_view, ms_texture, ms_texture_view) =
            create_viewport_texture(render_state, width, height);

        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &texture_view,
            FilterMode::Nearest,
        );

        let global_buffer = StaticBuffer::create(
            &render_state.device,
            Some("Viewport globals"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            1,
        );

        let instance_buffer = DynamicBuffer::create(
            &render_state.device,
            Some("Viewport instances"),
            BufferUsages::VERTEX | BufferUsages::COPY_DST,
            128,
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
                            array_stride: size_of!(Vertex) as BufferAddress,
                            step_mode: VertexStepMode::Vertex,
                            attributes: &vertex_attr_array![0 => Float32x2],
                        },
                        VertexBufferLayout {
                            array_stride: size_of!(Instance) as BufferAddress,
                            step_mode: VertexStepMode::Instance,
                            attributes: &vertex_attr_array![1 => Float32x2, 2 => Uint32, 3 => Uint32, 4 => Float32x4],
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

        let grid = ViewportGrid::create(render_state);
        let wires = ViewportWires::create(render_state);
        let anchors = ViewportAnchors::create(render_state);

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
            instance_buffer,
            _pipeline_layout: pipeline_layout,
            pipeline,
            grid,
            wires,
            anchors,
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
        vertices: &StaticBuffer<Vertex>,
        instances: &[Instance],
        indices: &StaticBuffer<u16>,
    ) {
        assert!(instances.len() < (u32::MAX as usize));

        if instances.len() > 0 {
            self.instance_buffer
                .write(&render_state.device, &render_state.queue, instances);
        }

        render_state.render_pass(&self.ms_texture_view, None, None, |pass, _| {
            if instances.len() > 0 {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, vertices.slice());
                pass.set_vertex_buffer(1, self.instance_buffer.slice());
                pass.set_index_buffer(indices.slice(), IndexFormat::Uint16);

                pass.draw_indexed(0..(indices.len() as u32), 0, 0..(instances.len() as u32));
            }
        });
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
                rotation: c.rotation as u32,
                mirrored: c.mirrored as u32,
                color: fill_color,
            })
            .collect();

        if instances.len() == 0 {
            return;
        }

        self.draw_primitives(
            render_state,
            &geometry.1.vertices,
            &instances,
            &geometry.1.indices,
        );

        for instance in instances.iter_mut() {
            instance.color = stroke_color;
        }

        self.draw_primitives(
            render_state,
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

        self.grid.draw(
            render_state,
            &self.ms_texture_view,
            [width, height],
            offset,
            zoom,
            colors.background_color,
            colors.grid_color,
        );

        self.global_buffer.write(
            &render_state.queue,
            &[Globals {
                resolution: [width, height],
                offset,
                zoom: zoom * BASE_ZOOM,
            }],
        );

        if let Some(circuit) = circuit {
            self.draw_component_instances(
                render_state,
                circuit,
                |c| matches!(c.kind, ComponentKind::AndGate { .. }),
                &GeometryStore::instance(&render_state.device).and_gate_geometry,
                colors.component_color,
                colors.background_color,
            );

            self.wires.draw(
                render_state,
                &self.ms_texture_view,
                circuit,
                [width, height],
                offset,
                zoom,
            );

            self.anchors.draw(
                render_state,
                &self.ms_texture_view,
                circuit,
                [width, height],
                offset,
                zoom,
            );
        }

        render_state.resolve_pass(&self.ms_texture_view, &self.texture_view);
    }
}
