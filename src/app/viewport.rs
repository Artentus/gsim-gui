mod buffer;
mod pass;

mod geometry;
use geometry::*;

mod text;
use text::*;

mod selection_box;
use selection_box::*;

use super::circuit::*;
use crate::app::math::Vec2f;
use eframe::egui_wgpu::RenderState;
use egui::TextureId;
use vello::kurbo::*;
use vello::peniko::*;
use wgpu::{FilterMode, Texture, TextureView};

pub use vello::peniko::Color;

struct RenderTarget {
    texture: Texture,
    view: TextureView,
}

fn create_render_target(render_state: &RenderState, width: u32, height: u32) -> RenderTarget {
    use wgpu::*;

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
        usage: TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    };

    let texture = render_state.device.create_texture(&desc);
    let view = texture.create_view(&TextureViewDescriptor::default());

    RenderTarget { texture, view }
}

pub const BASE_ZOOM: f32 = 10.0; // Logical pixels per unit
pub const LOGICAL_PIXEL_SIZE: f32 = 1.0 / BASE_ZOOM;

pub struct ViewportColors {
    pub background_color: Color,
    pub grid_color: Color,
    pub component_color: Color,
    pub selected_component_color: Color,
}

pub struct Viewport {
    render_target: RenderTarget,
    texture_id: TextureId,
    renderer: vello::Renderer,
    scene: vello::Scene,
    geometry: GeometryStore,
    text_pass: TextPass,
    selection_box_pass: SelectionBoxPass,
}

impl Viewport {
    pub fn create(render_state: &RenderState, width: u32, height: u32) -> Self {
        let render_target = create_render_target(render_state, width, height);

        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &render_target.view,
            FilterMode::Nearest,
        );

        let renderer = vello::Renderer::new(
            &render_state.device,
            &vello::RendererOptions {
                surface_format: None,
                timestamp_period: render_state.queue.get_timestamp_period(),
                use_cpu: false,
            },
        )
        .unwrap();

        Self {
            render_target,
            texture_id,
            renderer,
            scene: vello::Scene::new(),
            geometry: GeometryStore::new(),
            text_pass: TextPass::create(render_state),
            selection_box_pass: SelectionBoxPass::create(render_state),
        }
    }

    pub fn resize(&mut self, render_state: &RenderState, width: u32, height: u32) -> bool {
        if (self.render_target.texture.width() == width)
            && (self.render_target.texture.height() == height)
        {
            return false;
        }

        self.render_target = create_render_target(render_state, width, height);

        render_state
            .renderer
            .write()
            .update_egui_texture_from_wgpu_texture(
                &render_state.device,
                &self.render_target.view,
                FilterMode::Nearest,
                self.texture_id,
            );

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
        let width = self.render_target.texture.width();
        let height = self.render_target.texture.height();
        let resolution = Vec2f::new(width as f32, height as f32);

        let (offset, zoom) = circuit
            .map(|c| (c.offset(), c.zoom()))
            .unwrap_or((Vec2f::default(), DEFAULT_ZOOM));

        let mut fragment = vello::SceneFragment::new();
        let mut builder = vello::SceneBuilder::for_fragment(&mut fragment);
        draw_grid(&mut builder, resolution, offset, zoom, colors.grid_color);
        if let Some(circuit) = circuit {
            draw_wires(&mut builder, circuit);
            draw_components(&mut builder, circuit, colors, &self.geometry);
        }

        let mut builder = vello::SceneBuilder::for_scene(&mut self.scene);

        // Draw a dummy rectangle to prevent a crash in case there is no other geometry
        builder.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            colors.background_color,
            None,
            &Rect::ZERO,
        );

        let transform = Affine::FLIP_Y
            .then_translate((-offset.x as f64, offset.y as f64).into())
            .then_scale((zoom * BASE_ZOOM) as f64)
            .then_translate(((width as f64) * 0.5, (height as f64) * 0.5).into());
        builder.append(&fragment, Some(transform));

        self.renderer
            .render_to_texture(
                &render_state.device,
                &render_state.queue,
                &self.scene,
                &self.render_target.view,
                &vello::RenderParams {
                    base_color: colors.background_color,
                    width,
                    height,
                },
            )
            .unwrap();

        if let Some(circuit) = circuit {
            self.text_pass.draw(
                render_state,
                &self.render_target.view,
                circuit,
                resolution,
                offset,
                zoom,
                colors,
            );

            if let Some((box_a, box_b)) = circuit.selection_box() {
                self.selection_box_pass.draw(
                    render_state,
                    &self.render_target.view,
                    resolution,
                    offset,
                    zoom,
                    box_a,
                    box_b,
                    colors.selected_component_color,
                );
            }
        }
    }
}

fn draw_grid(
    builder: &mut vello::SceneBuilder,
    resolution: Vec2f,
    offset: Vec2f,
    zoom: f32,
    color: Color,
) {
    if zoom > 0.99 {
        let step = if zoom > 1.99 { 1 } else { 2 };

        let grid_width = resolution.x / (zoom * BASE_ZOOM);
        let grid_height = resolution.y / (zoom * BASE_ZOOM);

        let left = (offset.x - (grid_width * 0.5)).floor() as i32;
        let right = (offset.x + (grid_width * 0.5)).ceil() as i32;
        let bottom = (offset.y - (grid_height * 0.5)).floor() as i32;
        let top = (offset.y + (grid_height * 0.5)).ceil() as i32;

        let rect = Rect {
            x0: ((-LOGICAL_PIXEL_SIZE as f64) / 2.0) * (step as f64),
            x1: ((LOGICAL_PIXEL_SIZE as f64) / 2.0) * (step as f64),
            y0: ((-LOGICAL_PIXEL_SIZE as f64) / 2.0) * (step as f64),
            y1: ((LOGICAL_PIXEL_SIZE as f64) / 2.0) * (step as f64),
        };

        for y in (bottom..=top).filter(|&y| (y % step) == 0) {
            for x in (left..=right).filter(|&x| (x % step) == 0) {
                builder.fill(
                    Fill::NonZero,
                    Affine::translate((x as f64, y as f64)),
                    color,
                    None,
                    &rect,
                );
            }
        }
    }
}

fn draw_wires(builder: &mut vello::SceneBuilder, circuit: &Circuit) {
    let stroke = Stroke::new((2.0 * LOGICAL_PIXEL_SIZE) as f64)
        .with_join(Join::Miter)
        .with_caps(Cap::Round);

    for (i, segment) in circuit.wire_segments().iter().enumerate() {
        let stroke_color = if circuit.selection().contains_wire_segment(i) {
            Color::rgb8(80, 80, 255)
        } else {
            Color::BLUE
        };

        let mut path = BezPath::new();
        path.move_to((segment.endpoint_a.x as f64, segment.endpoint_a.y as f64));
        for midpoint in &segment.midpoints {
            path.line_to((midpoint.x as f64, midpoint.y as f64));
        }
        path.line_to((segment.endpoint_b.x as f64, segment.endpoint_b.y as f64));

        builder.stroke(&stroke, Affine::IDENTITY, stroke_color, None, &path);
    }
}

fn draw_components(
    builder: &mut vello::SceneBuilder,
    circuit: &Circuit,
    colors: &ViewportColors,
    geometry: &GeometryStore,
) {
    use crate::app::component::*;

    let stroke = Stroke::new((2.0 * LOGICAL_PIXEL_SIZE) as f64)
        .with_join(Join::Miter)
        .with_caps(Cap::Butt);

    for (i, component) in circuit.components().iter().enumerate() {
        let transform = Affine::scale_non_uniform(if component.mirrored { -1.0 } else { 1.0 }, 1.0)
            .then_rotate(component.rotation.radians())
            .then_translate((component.position.x as f64, component.position.y as f64).into());

        let stroke_color = if circuit.selection().contains_component(i) {
            colors.selected_component_color
        } else {
            colors.component_color
        };

        let geometry = match component.kind {
            ComponentKind::AndGate { .. } => &geometry.and_gate_geometry,
            ComponentKind::OrGate { .. } => &geometry.or_gate_geometry,
            ComponentKind::XorGate { .. } => &geometry.xor_gate_geometry,
            ComponentKind::NandGate { .. } => &geometry.nand_gate_geometry,
            ComponentKind::NorGate { .. } => &geometry.nor_gate_geometry,
            ComponentKind::XnorGate { .. } => &geometry.xnor_gate_geometry,
        };

        builder.fill(
            Fill::NonZero,
            transform,
            colors.background_color,
            None,
            geometry.fill_path(),
        );
        builder.stroke(
            &stroke,
            transform,
            stroke_color,
            None,
            geometry.stroke_path(),
        );

        for anchor in component.anchors() {
            let color = match anchor.kind {
                AnchorKind::Input => Color::LIME,
                AnchorKind::Output => Color::RED,
                AnchorKind::BiDirectional => Color::YELLOW,
                AnchorKind::Passive => Color::BLUE,
            };

            let shape = Circle::new(
                (anchor.position.x as f64, anchor.position.y as f64),
                (LOGICAL_PIXEL_SIZE * 2.0) as f64,
            );

            builder.fill(Fill::NonZero, Affine::IDENTITY, color, None, &shape);
        }
    }
}
