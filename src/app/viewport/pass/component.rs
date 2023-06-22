mod geometry;
use geometry::*;

use super::super::buffer::*;
use super::super::{RenderStateEx, ViewportColors, BASE_ZOOM, LOGICAL_PIXEL_SIZE};
use super::*;
use crate::app::circuit::Circuit;
use crate::app::component::*;
use crate::app::math::*;
use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::RenderState;
use wgpu::*;

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Globals {
    resolution: Vec2f,
    offset: Vec2f,
    zoom: f32,
}

vs_input!(
    Vertex { position: Vec2f }

    Instance {
        offset: Vec2f,
        rotation: u32,
        mirrored: u32,
        color: [f32; 4],
    }
);

pub struct ComponentPass {
    _shader: ShaderModule,
    global_buffer: StaticBuffer<Globals>,
    _bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    instance_buffer: DynamicBuffer<Instance>,
    _pipeline_layout: PipelineLayout,
    pipeline: RenderPipeline,
}

impl ComponentPass {
    pub fn create(render_state: &RenderState) -> Self {
        let shader = shader!(render_state.device, "component");

        let global_buffer = StaticBuffer::create(
            &render_state.device,
            Some("Viewport component globals"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            1,
        );

        let instance_buffer = DynamicBuffer::create(
            &render_state.device,
            Some("Viewport component instances"),
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

        let (pipeline_layout, pipeline) = create_pipeline(
            &render_state.device,
            "component",
            &shader,
            &bind_group_layout,
            &[Vertex::BUFFER_LAYOUT, Instance::BUFFER_LAYOUT],
        );

        ComponentPass {
            _shader: shader,
            global_buffer,
            _bind_group_layout: bind_group_layout,
            bind_group,
            instance_buffer,
            _pipeline_layout: pipeline_layout,
            pipeline,
        }
    }

    fn draw_primitives(
        &mut self,
        render_state: &RenderState,
        texture_view: &TextureView,
        vertices: &StaticBuffer<Vertex>,
        instances: &[Instance],
        indices: &StaticBuffer<u16>,
    ) {
        assert!(instances.len() < (u32::MAX as usize));

        if !instances.is_empty() {
            self.instance_buffer
                .write(&render_state.device, &render_state.queue, instances);
        }

        render_state.render_pass(texture_view, None, None, |pass, _| {
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
        texture_view: &TextureView,
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
                texture_view,
                geometry.1.vertices(),
                &fill_instances,
                geometry.1.indices(),
            );
        }

        if !stroke_instances.is_empty() {
            self.draw_primitives(
                render_state,
                texture_view,
                geometry.0.vertices(),
                &stroke_instances,
                geometry.0.indices(),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        render_state: &RenderState,
        texture_view: &TextureView,
        circuit: &Circuit,
        resolution: Vec2f,
        offset: Vec2f,
        zoom: f32,
        colors: &ViewportColors,
    ) {
        self.global_buffer.write(
            &render_state.queue,
            &[Globals {
                resolution,
                offset,
                zoom: zoom * BASE_ZOOM,
            }],
        );

        macro_rules! draw_components {
            ($($component:ident : $geometry:ident),* $(,)?) => {
                const _: () = match { ComponentKind::AndGate { width: 1 } } {
                    $(ComponentKind::$component { .. } => (),)*
                };

                $(
                    self.draw_component_instances(
                        render_state,
                        texture_view,
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
    }
}
