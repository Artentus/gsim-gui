mod grid;
pub(super) use grid::*;

mod component;
pub(super) use component::*;

mod wire;
pub(super) use wire::*;

mod anchor;
pub(super) use anchor::*;

mod text;
pub(super) use text::*;

mod selection_box;
pub(super) use selection_box::*;

use crate::app::math::{Vec2f, Vec2i};
use wgpu::*;

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

use shader;

trait VsInputFieldType {
    const FORMAT: VertexFormat;
}

macro_rules! impl_vertex_field_type {
    ($($t:ty: $attr:ident),+ $(,)?) => {
        $(
            impl VsInputFieldType for $t {
                const FORMAT: VertexFormat = VertexFormat::$attr;
            }
        )+
    };
}

impl_vertex_field_type!(
    u32: Uint32,
    [u32; 2]: Uint32x2,
    [u32; 4]: Uint32x4,
    f32: Float32,
    [f32; 2]: Float32x2,
    [f32; 4]: Float32x4,
    Vec2i: Uint32x2,
    Vec2f: Float32x2,
);

trait VsInput: Sized {
    const ATTRIBUTES: &'static [VertexAttribute];
    const BUFFER_LAYOUT: VertexBufferLayout<'static>;
}

const fn attrs<const N: usize>(
    formats: [VertexFormat; N],
    base_location: usize,
) -> [VertexAttribute; N] {
    let mut attrs = [VertexAttribute {
        format: VertexFormat::Float32,
        offset: 0,
        shader_location: 0,
    }; N];

    let mut offset = 0;
    let mut i = 0;
    while i < N {
        let format = formats[i];

        attrs[i] = VertexAttribute {
            format,
            offset,
            shader_location: (base_location + i) as u32,
        };

        offset += format.size();
        i += 1;
    }

    attrs
}

macro_rules! vs_input {
    (
        $vertex_name:ident {
            $($vertex_field:ident : $vertex_field_ty:ty),+ $(,)?
        }

        $($instance_name:ident {
            $($instance_field:ident : $instance_field_ty:ty),+ $(,)?
        })?
    ) => {
        #[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
        #[repr(C)]
        struct $vertex_name {
            $($vertex_field: $vertex_field_ty,)+
        }

        impl $crate::app::viewport::pass::VsInput for $vertex_name {
            const ATTRIBUTES: &'static [wgpu::VertexAttribute] = {
                let formats = [$(
                    <$vertex_field_ty as $crate::app::viewport::pass::VsInputFieldType>::FORMAT,
                )+];
                &$crate::app::viewport::pass::attrs(formats, 0)
            };

            const BUFFER_LAYOUT: wgpu::VertexBufferLayout<'static> = VertexBufferLayout {
                array_stride: crate::size_of!(Self) as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Self::ATTRIBUTES,
            };
        }

        $(
            #[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
            #[repr(C)]
            struct $instance_name {
                $($instance_field: $instance_field_ty,)+
            }

            impl $crate::app::viewport::pass::VsInput for $instance_name {
                const ATTRIBUTES: &'static [wgpu::VertexAttribute] = {
                    let formats = [$(
                        <$instance_field_ty as $crate::app::viewport::pass::VsInputFieldType>::FORMAT,
                    )+];

                    &$crate::app::viewport::pass::attrs(
                        formats,
                        <$vertex_name as $crate::app::viewport::pass::VsInput>::ATTRIBUTES.len(),
                    )
                };

                const BUFFER_LAYOUT: wgpu::VertexBufferLayout<'static> = VertexBufferLayout {
                    array_stride: crate::size_of!(Self) as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: Self::ATTRIBUTES,
                };
            }
        )?
    };
}

use vs_input;

fn create_pipeline(
    device: &Device,
    name: &str,
    shader: &ShaderModule,
    bind_group_layout: &BindGroupLayout,
    vs_input_layout: &[VertexBufferLayout<'_>],
    blend: Option<BlendState>,
) -> (PipelineLayout, RenderPipeline) {
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some(&format!("Viewport {name} pipeline layout")),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some(&format!("Viewport {name} pipeline")),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: shader,
            entry_point: "vs_main",
            buffers: vs_input_layout,
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
            module: shader,
            entry_point: "fs_main",
            targets: &[Some(ColorTargetState {
                format: TextureFormat::Rgba8Unorm,
                blend,
                write_mask: ColorWrites::all(),
            })],
        }),
        multiview: None,
    });

    (pipeline_layout, pipeline)
}
