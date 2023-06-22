use super::circuit::*;
use super::component::*;
use crate::app::math::*;
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

mod selection_box;
use selection_box::*;

mod geometry;
use geometry::*;

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
    resolution: Vec2f,
    offset: Vec2f,
    zoom: f32,
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Instance {
    offset: Vec2f,
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
pub const LOGICAL_PIXEL_SIZE: f32 = 1.0 / BASE_ZOOM;

pub struct ViewportColors {
    pub background_color: [f32; 4],
    pub grid_color: [f32; 4],
    pub component_color: [f32; 4],
    pub selected_component_color: [f32; 4],
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
    selection_box: ViewportSelectionBox,
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
        let selection_box = ViewportSelectionBox::create(render_state);

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
            selection_box,
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

        if !instances.is_empty() {
            self.instance_buffer
                .write(&render_state.device, &render_state.queue, instances);
        }

        render_state.render_pass(&self.ms_texture_view, None, None, |pass, _| {
            if !instances.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, vertices.slice());
                pass.set_vertex_buffer(1, self.instance_buffer.slice());
                pass.set_index_buffer(indices.slice(), IndexFormat::Uint16);

                pass.draw_indexed(0..(indices.len() as u32), 0, 0..(instances.len() as u32));
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_component_instances(
        &mut self,
        render_state: &RenderState,
        circuit: &Circuit,
        filter: impl Fn(&Component) -> bool,
        geometry: &(Geometry, Geometry),
        stroke_color: [f32; 4],
        selected_stroke_color: [f32; 4],
        fill_color: [f32; 4],
    ) {
        // TODO: cull the components to the visible area

        let mut stroke_instances = Vec::new();
        let mut fill_instances = Vec::new();
        for (i, c) in circuit
            .components()
            .iter()
            .enumerate()
            .filter(|&(_, c)| filter(c))
        {
            let selected = circuit.selection().contains_component(i);

            stroke_instances.push(Instance {
                offset: c.position.to_vec2f(),
                rotation: c.rotation as u32,
                mirrored: c.mirrored as u32,
                color: if selected {
                    selected_stroke_color
                } else {
                    stroke_color
                },
            });

            fill_instances.push(Instance {
                offset: c.position.to_vec2f(),
                rotation: c.rotation as u32,
                mirrored: c.mirrored as u32,
                color: fill_color,
            });
        }

        if !fill_instances.is_empty() {
            self.draw_primitives(
                render_state,
                geometry.1.vertices(),
                &fill_instances,
                geometry.1.indices(),
            );
        }

        if !stroke_instances.is_empty() {
            self.draw_primitives(
                render_state,
                geometry.0.vertices(),
                &stroke_instances,
                geometry.0.indices(),
            );
        }
    }

    pub fn draw(
        &mut self,
        render_state: &RenderState,
        circuit: Option<&Circuit>,
        colors: ViewportColors,
    ) {
        let width = self.texture.width() as f32;
        let height = self.texture.height() as f32;
        let resolution = Vec2f::new(width, height);

        let (offset, zoom) = circuit
            .map(|c| (c.offset(), c.zoom()))
            .unwrap_or((Vec2f::default(), DEFAULT_ZOOM));

        self.grid.draw(
            render_state,
            &self.ms_texture_view,
            resolution,
            offset,
            zoom,
            colors.background_color,
            colors.grid_color,
        );

        self.global_buffer.write(
            &render_state.queue,
            &[Globals {
                resolution,
                offset,
                zoom: zoom * BASE_ZOOM,
            }],
        );

        if let Some(circuit) = circuit {
            macro_rules! draw_components {
                ($($component:ident : $geometry:ident),* $(,)?) => {
                    const _: () = match { ComponentKind::AndGate { width: 1 } } {
                        $(ComponentKind::$component { .. } => (),)*
                    };

                    $(
                        self.draw_component_instances(
                            render_state,
                            circuit,
                            |c| matches!(c.kind, ComponentKind::$component { .. }),
                            &GeometryStore::instance(&render_state.device).$geometry,
                            colors.component_color,
                            colors.selected_component_color,
                            colors.background_color,
                        );
                    )*
                };
            }

            draw_components!(
                AndGate: and_gate_geometry,
                OrGate: or_gate_geometry,
                XorGate: xor_gate_geometry,
                NandGate: nand_gate_geometry,
                NorGate: nor_gate_geometry,
                XnorGate: xnor_gate_geometry,
            );

            self.wires.draw(
                render_state,
                &self.ms_texture_view,
                circuit,
                resolution,
                offset,
                zoom,
            );

            self.anchors.draw(
                render_state,
                &self.ms_texture_view,
                circuit,
                resolution,
                offset,
                zoom,
            );

            if let Some((box_a, box_b)) = circuit.selection_box() {
                self.selection_box.draw(
                    render_state,
                    &self.ms_texture_view,
                    resolution,
                    offset,
                    zoom,
                    box_a,
                    box_b,
                    colors.selected_component_color,
                );
            }
        }

        render_state.resolve_pass(&self.ms_texture_view, &self.texture_view);
    }
}
