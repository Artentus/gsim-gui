use super::component::*;
use super::locale::*;
use super::viewport::{BASE_ZOOM, LOGICAL_PIXEL_SIZE};
use crate::app::math::*;
use crate::HashSet;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const MIN_LINEAR_ZOOM: f32 = 0.0;
const MAX_LINEAR_ZOOM: f32 = 1.0;
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 4.0;
pub const DEFAULT_ZOOM: f32 = 1.0;

// Note: these should be constants but `ln` and `exp` are not constant functions
fn zoom_fn_a() -> f32 {
    static ZOOM_FN_A: OnceLock<f32> = OnceLock::new();
    *ZOOM_FN_A.get_or_init(|| MIN_ZOOM * (-zoom_fn_b() * MIN_LINEAR_ZOOM).exp())
}
fn zoom_fn_b() -> f32 {
    static ZOOM_FN_B: OnceLock<f32> = OnceLock::new();
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
        // Bounding box test
        let bb = BoundingBox {
            top: self.point_a.y.max(self.point_b.y) as f32,
            bottom: self.point_a.y.min(self.point_b.y) as f32,
            left: self.point_a.x.min(self.point_b.x) as f32,
            right: self.point_a.x.max(self.point_b.x) as f32,
        };
        if !bb.contains(p) {
            return false;
        }

        // Triangle test
        let a = self.point_a.to_vec2f();
        let b = self.point_b.to_vec2f();
        let dir = (b - a).normalized();
        let left = Vec2f::new(dir.y, -dir.x) * LOGICAL_PIXEL_SIZE;
        let right = Vec2f::new(-dir.y, dir.x) * LOGICAL_PIXEL_SIZE;

        let a1 = a + left;
        let a2 = a + right;
        let b1 = b + left;
        let b2 = b + right;
        let t1 = Triangle {
            a: a1,
            b: a2,
            c: b2,
        };
        let t2 = Triangle {
            a: a1,
            b: b2,
            c: b1,
        };

        t1.contains(p) || t2.contains(p)
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DragMode {
    #[default]
    BoxSelection,
    DrawWire,
}

#[derive(Default, Debug)]
enum DragState {
    #[default]
    None,
    Deadzone {
        drag_start: Vec2f,
        drag_delta: Vec2f,
    },
    DrawingBoxSelection {
        drag_start: Vec2f,
        drag_delta: Vec2f,
    },
    DrawingWireSegment {
        wire_segment: usize,
        drag_start: Vec2f,
        drag_delta: Vec2f,
    },
    Dragging {
        fract_drag_delta: Vec2f,
    },
}

macro_rules! is_discriminant {
    ($value:expr, $discriminant:path) => {
        match &$value {
            $discriminant { .. } => true,
            _ => false,
        }
    };
}

enum HitTestResult {
    None,
    Component(usize),
    WireSegment(usize),
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
    drag_state: DragState,
    #[serde(skip)]
    primary_button_down: bool,
    #[serde(skip)]
    secondary_button_down: bool,
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
            drag_state: DragState::None,
            primary_button_down: false,
            secondary_button_down: false,
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
        self.drag_state = DragState::None;
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

    #[inline]
    pub fn selection_box(&self) -> Option<(Vec2f, Vec2f)> {
        match self.drag_state {
            DragState::DrawingBoxSelection {
                drag_start,
                drag_delta,
            } => Some((drag_start, drag_start + drag_delta)),
            _ => None,
        }
    }

    fn hit_test(&self, logical_pos: Vec2f) -> HitTestResult {
        for (i, component) in self.components.iter().enumerate() {
            if component.bounding_box().contains(logical_pos) {
                return HitTestResult::Component(i);
            }
        }

        for (i, wire_segment) in self.wire_segments.iter().enumerate() {
            if wire_segment.contains(logical_pos) {
                return HitTestResult::WireSegment(i);
            }
        }

        HitTestResult::None
    }

    pub fn primary_button_pressed(&mut self, pos: Vec2f) {
        assert!(
            is_discriminant!(self.drag_state, DragState::None),
            "invalid drag state"
        );

        let logical_pos = pos / (self.zoom * BASE_ZOOM) + self.offset;
        let hit = self.hit_test(logical_pos);

        match hit {
            HitTestResult::None => {
                self.selection = Selection::None;
            }
            HitTestResult::Component(component) => {
                if !self.selection.contains_component(component) {
                    self.selection = Selection::Component(component);
                }
            }
            HitTestResult::WireSegment(wire_segment) => {
                if !self.selection.contains_wire_segment(wire_segment) {
                    self.selection = Selection::WireSegment(wire_segment);
                }
            }
        }

        self.drag_state = DragState::Deadzone {
            drag_start: logical_pos,
            drag_delta: Vec2f::default(),
        };

        self.primary_button_down = true;
    }

    pub fn primary_button_released(&mut self, pos: Vec2f) {
        if self.primary_button_down {
            if is_discriminant!(self.drag_state, DragState::None) {
                let logical_pos = pos / (self.zoom * BASE_ZOOM) + self.offset;
                let hit = self.hit_test(logical_pos);

                match hit {
                    HitTestResult::None => {
                        self.selection = Selection::None;
                    }
                    HitTestResult::Component(component) => {
                        self.selection = Selection::Component(component);
                    }
                    HitTestResult::WireSegment(wire_segment) => {
                        self.selection = Selection::WireSegment(wire_segment);
                    }
                }
            }

            if let DragState::DrawingBoxSelection {
                drag_start,
                drag_delta,
            } = &self.drag_state
            {
                let selection_box = BoundingBox {
                    top: drag_start.y.max(drag_start.y + drag_delta.y),
                    bottom: drag_start.y.min(drag_start.y + drag_delta.y),
                    left: drag_start.x.min(drag_start.x + drag_delta.x),
                    right: drag_start.x.max(drag_start.x + drag_delta.x),
                };

                let mut selected_components = HashSet::new();
                for (i, component) in self.components.iter().enumerate() {
                    if selection_box.contains(component.position.to_vec2f()) {
                        selected_components.insert(i);
                    }
                }

                let mut selected_wire_segments = HashSet::new();
                for (i, wire_segment) in self.wire_segments.iter().enumerate() {
                    if selection_box.contains(wire_segment.point_a.to_vec2f())
                        || selection_box.contains(wire_segment.point_b.to_vec2f())
                    {
                        selected_wire_segments.insert(i);
                    }
                }

                if (selected_components.len() == 1) && selected_wire_segments.is_empty() {
                    self.selection =
                        Selection::Component(selected_components.into_iter().next().unwrap());
                } else if selected_components.is_empty() && (selected_wire_segments.len() == 1) {
                    self.selection =
                        Selection::WireSegment(selected_wire_segments.into_iter().next().unwrap());
                } else if !selected_components.is_empty() && !selected_wire_segments.is_empty() {
                    self.selection = Selection::Multi {
                        components: selected_components,
                        wire_segments: selected_wire_segments,
                    };
                }
            }

            self.drag_state = DragState::None;
        }

        self.primary_button_down = false;
    }

    pub fn secondary_button_pressed(&mut self, _pos: Vec2f) {
        self.secondary_button_down = true;
    }

    pub fn secondary_button_released(&mut self, pos: Vec2f) {
        if self.secondary_button_down {
            let logical_pos = pos / (self.zoom * BASE_ZOOM) + self.offset;
            let hit = self.hit_test(logical_pos);

            match hit {
                HitTestResult::None => {
                    self.selection = Selection::None;
                }
                HitTestResult::Component(component) => {
                    if !self.selection.contains_component(component) {
                        self.selection = Selection::Component(component);
                    }

                    // TODO: show context menu
                }
                HitTestResult::WireSegment(wire_segment) => {
                    if !self.selection.contains_wire_segment(wire_segment) {
                        self.selection = Selection::WireSegment(wire_segment);
                    }

                    // TODO: show context menu
                }
            }

            self.drag_state = DragState::None;
        }

        self.secondary_button_down = false;
    }

    fn move_selection(&mut self, delta: Vec2i) {
        match &self.selection {
            Selection::None => {}
            &Selection::Component(component) => {
                let component = self
                    .components
                    .get_mut(component)
                    .expect("invalid selection");

                component.position += delta;
            }
            &Selection::WireSegment(wire_segment) => {
                let wire_segment = self
                    .wire_segments
                    .get_mut(wire_segment)
                    .expect("invalid selection");

                wire_segment.point_a += delta;
                wire_segment.point_b += delta;
            }
            Selection::Multi {
                components,
                wire_segments,
            } => {
                for &component in components {
                    let component = self
                        .components
                        .get_mut(component)
                        .expect("invalid selection");

                    component.position += delta;
                }

                for &wire_segment in wire_segments {
                    let wire_segment = self
                        .wire_segments
                        .get_mut(wire_segment)
                        .expect("invalid selection");

                    wire_segment.point_a += delta;
                    wire_segment.point_b += delta;
                }
            }
        }
    }

    pub fn mouse_moved(&mut self, delta: Vec2f, drag_mode: DragMode) {
        if self.primary_button_down && !self.secondary_button_down {
            match &mut self.drag_state {
                DragState::None => {}
                DragState::Deadzone {
                    drag_start,
                    drag_delta,
                } => {
                    *drag_delta += delta;

                    let drag_start = *drag_start;
                    let drag_delta = *drag_delta;

                    const DEADZONE_RANGE: f32 = 1.0;
                    if (drag_delta.x.abs() >= DEADZONE_RANGE)
                        || (drag_delta.y.abs() >= DEADZONE_RANGE)
                    {
                        let hit = self.hit_test(drag_start);

                        self.drag_state = match hit {
                            HitTestResult::None => match drag_mode {
                                DragMode::BoxSelection => DragState::DrawingBoxSelection {
                                    drag_start,
                                    drag_delta,
                                },
                                DragMode::DrawWire => {
                                    let wire_segment = self.wire_segments.len();

                                    self.wire_segments.push(WireSegment {
                                        point_a: drag_start.to_vec2i(),
                                        point_b: (drag_start + drag_delta).round().to_vec2i(),
                                    });

                                    DragState::DrawingWireSegment {
                                        wire_segment,
                                        drag_start,
                                        drag_delta,
                                    }
                                }
                            },
                            HitTestResult::Component(component) => {
                                assert!(
                                    self.selection.contains_component(component),
                                    "invalid drag state"
                                );

                                // TODO: already drag whole part of delta
                                DragState::Dragging {
                                    fract_drag_delta: drag_delta,
                                }
                            }
                            HitTestResult::WireSegment(wire_segment) => {
                                assert!(
                                    self.selection.contains_wire_segment(wire_segment),
                                    "invalid drag state"
                                );

                                // TODO: already drag whole part of delta
                                DragState::Dragging {
                                    fract_drag_delta: drag_delta,
                                }
                            }
                        };
                    }
                }
                DragState::DrawingBoxSelection { drag_delta, .. } => {
                    *drag_delta += delta;
                }
                DragState::DrawingWireSegment {
                    wire_segment,
                    drag_start,
                    drag_delta,
                } => {
                    *drag_delta += delta;

                    let wire_segment = self
                        .wire_segments
                        .get_mut(*wire_segment)
                        .expect("invalid drag state");
                    wire_segment.point_b = (*drag_start + *drag_delta).round().to_vec2i();
                }
                DragState::Dragging { fract_drag_delta } => {
                    assert!(
                        !is_discriminant!(self.selection, Selection::None),
                        "invalid drag state"
                    );

                    *fract_drag_delta += delta;
                    let whole_drag_delta = fract_drag_delta.round();
                    *fract_drag_delta -= whole_drag_delta;

                    self.move_selection(whole_drag_delta.to_vec2i());
                }
            }
        }
    }

    pub fn rotate_selection(&mut self) {
        match &self.selection {
            Selection::None => {}
            &Selection::Component(component) => {
                let component = self
                    .components
                    .get_mut(component)
                    .expect("invalid selection");
                component.rotation = component.rotation.next();
            }
            &Selection::WireSegment(wire_segment) => {
                let wire_segment = self
                    .wire_segments
                    .get_mut(wire_segment)
                    .expect("invalid selection");

                let center = (wire_segment.point_a + wire_segment.point_b) / 2;
                let a = wire_segment.point_a - center;
                let b = wire_segment.point_b - center;

                wire_segment.point_a = Vec2i::new(-a.y, a.x) + center;
                wire_segment.point_b = Vec2i::new(-b.y, b.x) + center;
            }
            Selection::Multi { .. } => { /* TODO: */ }
        }
    }

    pub fn mirror_selection(&mut self) {
        match &self.selection {
            Selection::None => {}
            &Selection::Component(component) => {
                let component = &mut self.components[component];
                component.mirrored = !component.mirrored;
            }
            &Selection::WireSegment(wire_segment) => {
                let wire_segment = self
                    .wire_segments
                    .get_mut(wire_segment)
                    .expect("invalid selection");

                let center = (wire_segment.point_a + wire_segment.point_b) / 2;
                let a = wire_segment.point_a - center;
                let b = wire_segment.point_b - center;

                wire_segment.point_a = Vec2i::new(-a.x, a.y) + center;
                wire_segment.point_b = Vec2i::new(-b.x, b.y) + center;
            }
            Selection::Multi { .. } => { /* TODO: */ }
        }
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
