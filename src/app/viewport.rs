mod buffer;

mod pass;
use pass::*;

use super::circuit::*;
use crate::app::math::*;
use eframe::egui_wgpu::RenderState;
use egui::TextureId;
use std::io::{BufRead, Seek};
use wgpu::*;

trait RenderStateEx {
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
    texture_id: TextureId,
    texture: Texture,
    texture_view: TextureView,
    ms_texture: Texture,
    ms_texture_view: TextureView,
    grid: GridPass,
    components: ComponentPass,
    wires: WirePass,
    anchors: AnchorPass,
    selection_box: SelectionBoxPass,
    text: TextPass,
}

impl Viewport {
    pub fn create(render_state: &RenderState, width: u32, height: u32) -> Self {
        let (texture, texture_view, ms_texture, ms_texture_view) =
            create_viewport_texture(render_state, width, height);

        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &texture_view,
            FilterMode::Nearest,
        );

        let grid = GridPass::create(render_state);
        let components = ComponentPass::create(render_state);
        let wires = WirePass::create(render_state);
        let anchors = AnchorPass::create(render_state);
        let selection_box = SelectionBoxPass::create(render_state);
        let text = TextPass::create(render_state);

        Self {
            texture_id,
            texture,
            texture_view,
            ms_texture,
            ms_texture_view,
            grid,
            components,
            wires,
            anchors,
            selection_box,
            text,
        }
    }

    pub fn resize(&mut self, render_state: &RenderState, width: u32, height: u32) -> bool {
        if (self.texture.width() == width) && (self.texture.height() == height) {
            return false;
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

        true
    }

    #[inline]
    pub fn texture_id(&self) -> TextureId {
        self.texture_id
    }

    pub fn draw(
        &mut self,
        render_state: &RenderState,
        circuit: Option<&Circuit>,
        colors: &ViewportColors,
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
            colors,
        );

        if let Some(circuit) = circuit {
            self.components.draw(
                render_state,
                &self.ms_texture_view,
                circuit,
                resolution,
                offset,
                zoom,
                colors,
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

            self.text.draw(
                render_state,
                &self.ms_texture_view,
                circuit,
                resolution,
                offset,
                zoom,
                colors,
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
