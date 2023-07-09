use crate::app::math::{Vec2f, Vec2i};
use eframe::egui_wgpu::RenderState;
use std::io::{BufRead, Seek};
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

pub(super) use shader;

pub(super) trait VsInputFieldType {
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

pub(super) trait VsInput: Sized {
    const ATTRIBUTES: &'static [VertexAttribute];
    const BUFFER_LAYOUT: VertexBufferLayout<'static>;
}

pub(super) const fn attrs<const N: usize>(
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

pub(super) use vs_input;

pub(super) fn create_pipeline(
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
        multisample: MultisampleState::default(),
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

pub(super) trait RenderStateEx {
    fn create_texture<R: BufRead + Seek>(
        &self,
        reader: R,
        label: Option<&str>,
        srgb: bool,
    ) -> Texture;

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
    fn create_texture<R: BufRead + Seek>(
        &self,
        reader: R,
        label: Option<&str>,
        srgb: bool,
    ) -> Texture {
        use image::ImageFormat;
        use wgpu::util::DeviceExt;

        let img = image::load(reader, ImageFormat::Png).unwrap();
        let img = img.to_rgba8();

        let desc = TextureDescriptor {
            label,
            size: Extent3d {
                width: img.width(),
                height: img.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: if srgb {
                TextureFormat::Rgba8UnormSrgb
            } else {
                TextureFormat::Rgba8Unorm
            },
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        self.device
            .create_texture_with_data(&self.queue, &desc, img.as_raw())
    }

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

pub(super) fn convert_color(c: super::Color) -> [f32; 4] {
    #[inline]
    fn unorm_to_float(u: u8) -> f32 {
        (u as f32) / (u8::MAX as f32)
    }

    [
        unorm_to_float(c.r),
        unorm_to_float(c.g),
        unorm_to_float(c.b),
        unorm_to_float(c.a),
    ]
}
