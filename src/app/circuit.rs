use super::component::*;
use super::locale::*;
use super::viewport::BASE_ZOOM;
use crate::app::math::*;
use crate::HashSet;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

const MIN_LINEAR_ZOOM: f32 = 0.0;
const MAX_LINEAR_ZOOM: f32 = 1.0;
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 4.0;
pub const DEFAULT_ZOOM: f32 = 1.0;

// Note: these should be constants but `ln` and `exp` are not constant functions
fn zoom_fn_a() -> f32 {
    static ZOOM_FN_A: OnceCell<f32> = OnceCell::new();
    *ZOOM_FN_A.get_or_init(|| MIN_ZOOM * (-zoom_fn_b() * MIN_LINEAR_ZOOM).exp())
}
fn zoom_fn_b() -> f32 {
    static ZOOM_FN_B: OnceCell<f32> = OnceCell::new();
    *ZOOM_FN_B.get_or_init(|| (MAX_ZOOM / MIN_ZOOM).ln() / (MAX_LINEAR_ZOOM - MIN_LINEAR_ZOOM))
}

#[inline]
fn zoom_to_linear(zoom: f32) -> f32 {
    (zoom / zoom_fn_a()).ln() / zoom_fn_b()
}

#[inline]
fn linear_to_zoom(linear: f32) -> f32 {
    zoom_fn_a() * (zoom_fn_b() * linear).exp()
}

#[derive(Serialize, Deserialize)]
pub struct WireSegment {
    pub point_a: Vec2i,
    pub point_b: Vec2i,
}

impl WireSegment {
    pub fn contains(&self, p: Vec2f) -> bool {
        false // TODO:
    }
}

#[derive(Default)]
pub enum Selection {
    #[default]
    None,
    Component(usize),
    WireSegment(usize),
    Multi {
        components: HashSet<usize>,
        wire_segments: HashSet<usize>,
    },
}

impl Selection {
    pub fn contains_component(&self, component: usize) -> bool {
        match self {
            Selection::None => false,
            &Selection::Component(c) => c == component,
            Selection::WireSegment(_) => false,
            Selection::Multi { components, .. } => components.contains(&component),
        }
    }

    pub fn contains_wire_segment(&self, segment: usize) -> bool {
        match self {
            Selection::None => false,
            Selection::Component(_) => false,
            &Selection::WireSegment(s) => s == segment,
            Selection::Multi { wire_segments, .. } => wire_segments.contains(&segment),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Circuit {
    name: String,
    offset: Vec2f,
    linear_zoom: f32,
    zoom: f32,
    components: Vec<Component>,
    wire_segments: Vec<WireSegment>,
    #[serde(skip)]
    selection: Selection,
    #[serde(skip)]
    drag_start: Vec2i,
    #[serde(skip)]
    drag_delta: Vec2f,
    #[serde(skip)]
    create_wire: Option<usize>,
}

impl Circuit {
    pub fn new() -> Self {
        Self {
            name: "New Circuit".to_owned(),
            offset: Vec2f::default(),
            linear_zoom: zoom_to_linear(DEFAULT_ZOOM),
            zoom: DEFAULT_ZOOM,
            components: vec![],
            wire_segments: vec![],
            selection: Selection::None,
            drag_start: Vec2i::default(),
            drag_delta: Vec2f::default(),
            create_wire: None,
        }
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn offset(&self) -> Vec2f {
        self.offset
    }

    #[inline]
    pub fn set_offset(&mut self, offset: Vec2f) {
        self.offset = offset;
    }

    #[inline]
    pub fn linear_zoom(&self) -> f32 {
        self.linear_zoom
    }

    pub fn set_linear_zoom(&mut self, zoom: f32) {
        self.linear_zoom = zoom.clamp(MIN_LINEAR_ZOOM, MAX_LINEAR_ZOOM);
        self.zoom = linear_to_zoom(self.linear_zoom);
    }

    #[inline]
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    #[inline]
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    pub fn add_component(&mut self, kind: ComponentKind) {
        self.selection = Selection::Component(self.components.len());
        self.components.push(Component::new(kind));
    }

    #[inline]
    pub fn wire_segments(&self) -> &[WireSegment] {
        &self.wire_segments
    }

    #[inline]
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn update_selection(&mut self, pos: Vec2f) {
        let logical_pos = pos / (self.zoom * BASE_ZOOM) + self.offset;

        self.selection = Selection::None;
        self.drag_start = logical_pos.round().to_vec2i();
        self.drag_delta = Vec2f::default();
        self.create_wire = None;

        for (i, component) in self.components.iter().enumerate() {
            if component.bounding_box().contains(logical_pos) {
                self.selection = Selection::Component(i);
                self.drag_start = component.position;
                break;
            }
        }

        for (i, wire_segment) in self.wire_segments.iter().enumerate() {
            if wire_segment.contains(logical_pos) {
                self.selection = Selection::Component(i);
                self.drag_start = wire_segment.point_a;
                break;
            }
        }
    }

    pub fn rotate_selection(&mut self) {
        match &self.selection {
            Selection::None => {}
            &Selection::Component(selected_component) => {
                let component = &mut self.components[selected_component];
                component.rotation = component.rotation.next();
            }
            Selection::WireSegment(_) => {}
            Selection::Multi { .. } => { /* TODO: */ }
        }
    }

    pub fn mirror_selection(&mut self) {
        match &self.selection {
            Selection::None => {}
            &Selection::Component(selected_component) => {
                let component = &mut self.components[selected_component];
                component.mirrored = !component.mirrored;
            }
            Selection::WireSegment(_) => {}
            Selection::Multi { .. } => { /* TODO: */ }
        }
    }

    pub fn drag_selection(&mut self, delta: Vec2f) {
        self.drag_delta += delta;

        match &self.selection {
            Selection::None => {
                let create_wire = if let Some(create_wire) = self.create_wire {
                    &mut self.wire_segments[create_wire]
                } else {
                    if (self.drag_delta.x.abs() < 1.0) && (self.drag_delta.y.abs() < 1.0) {
                        return;
                    }

                    self.create_wire = Some(self.wire_segments.len());
                    println!("Created wire at {:?}", self.drag_start);

                    self.wire_segments.push(WireSegment {
                        point_a: self.drag_start,
                        point_b: self.drag_start,
                    });

                    self.wire_segments.last_mut().unwrap()
                };

                create_wire.point_b = self.drag_start + self.drag_delta.round().to_vec2i();
            }
            &Selection::Component(selected_component) => {
                let component = &mut self.components[selected_component];
                component.position = self.drag_start + self.drag_delta.round().to_vec2i();
            }
            &Selection::WireSegment(selected_wire_segment) => {
                let wire_segment = &mut self.wire_segments[selected_wire_segment];
                let diff = wire_segment.point_b - wire_segment.point_a;
                wire_segment.point_a = self.drag_start + self.drag_delta.round().to_vec2i();
                wire_segment.point_b = wire_segment.point_a + diff;
            }
            Selection::Multi { .. } => {}
        }
    }

    pub fn end_drag(&mut self) {
        self.drag_start = Vec2i::default();
        self.drag_delta = Vec2f::default();
        self.create_wire = None;
    }

    pub fn update_component_properties<'a>(
        &mut self,
        ui: &mut egui::Ui,
        locale_manager: &LocaleManager,
        lang: &LangId,
    ) {
        match &self.selection {
            Selection::None => {}
            &Selection::Component(selected_component) => {
                ui.heading(locale_manager.get(lang, "properties-header"));
                self.components[selected_component].update_properties(ui, locale_manager, lang);
            }
            &Selection::WireSegment(selected_segment) => {
                ui.heading(locale_manager.get(lang, "properties-header"));

                let segment = &mut self.wire_segments[selected_segment];

                ui.horizontal(|ui| {
                    ui.label("X1:");

                    let mut x1_text = format!("{}", segment.point_a.x);
                    ui.text_edit_singleline(&mut x1_text);
                    if let Ok(new_x1) = x1_text.parse() {
                        segment.point_a.x = new_x1;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Y1:");

                    let mut y1_text = format!("{}", segment.point_a.y);
                    ui.text_edit_singleline(&mut y1_text);
                    if let Ok(new_y1) = y1_text.parse() {
                        segment.point_a.y = new_y1;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("X2:");

                    let mut x2_text = format!("{}", segment.point_b.x);
                    ui.text_edit_singleline(&mut x2_text);
                    if let Ok(new_x2) = x2_text.parse() {
                        segment.point_b.x = new_x2;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Y2:");

                    let mut y2_text = format!("{}", segment.point_b.y);
                    ui.text_edit_singleline(&mut y2_text);
                    if let Ok(new_y2) = y2_text.parse() {
                        segment.point_b.y = new_y2;
                    }
                });
            }
            Selection::Multi { .. } => {}
        }
    }
}
