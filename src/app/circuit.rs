use super::component::*;
use super::locale::*;
use super::viewport::{BASE_ZOOM, LOGICAL_PIXEL_SIZE};
use crate::app::math::*;
use crate::HashSet;
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use std::path::{Path, PathBuf};
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
    pub endpoint_a: Vec2i,
    pub midpoints: SmallVec<[Vec2i; 2]>,
    pub endpoint_b: Vec2i,
    #[serde(skip)]
    pub sim_wires: SmallVec<[gsim::WireId; 4]>,
}

impl WireSegment {
    pub fn contains(&self, p: Vec2f) -> bool {
        // Bounding box test
        let midpoints = self.midpoints.iter().copied();
        let endpoint_a = std::iter::once(self.endpoint_a);
        let endpoint_b = std::iter::once(self.endpoint_b);

        let (min, max) = midpoints
            .chain(endpoint_a)
            .chain(endpoint_b)
            .fold((Vec2i::MAX, Vec2i::MIN), |(min, max), v| {
                (min.min(v), max.max(v))
            });

        let bb = Rectangle {
            top: (max.y as f32) + LOGICAL_PIXEL_SIZE,
            bottom: (min.y as f32) - LOGICAL_PIXEL_SIZE,
            left: (min.x as f32) - LOGICAL_PIXEL_SIZE,
            right: (max.x as f32) + LOGICAL_PIXEL_SIZE,
        };

        if !bb.contains(p) {
            return false;
        }

        // Triangle test
        let midpoints = self.midpoints.iter().copied();
        let endpoint_b = std::iter::once(self.endpoint_b);

        let mut a = self.endpoint_a.to_vec2f();
        for b in midpoints.chain(endpoint_b).map(Vec2i::to_vec2f) {
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

            if t1.contains(p) || t2.contains(p) {
                return true;
            }

            a = b;
        }

        false
    }

    fn update_midpoints(&mut self) {
        self.midpoints.clear();

        let diff = (self.endpoint_b - self.endpoint_a).abs();
        if (diff.x == 0) || (diff.y == 0) || (diff.x == diff.y) {
            // Straight wire, no midpoints
        } else if diff.x > diff.y {
            // X direction further apart, midpoint horizontal

            let offset = if self.endpoint_a.x > self.endpoint_b.x {
                diff.x - diff.y
            } else {
                diff.y - diff.x
            };

            self.midpoints
                .push(Vec2i::new(self.endpoint_b.x + offset, self.endpoint_b.y));
        } else {
            // Y direction further apart, midpoint vertical

            let offset = if self.endpoint_a.y > self.endpoint_b.y {
                diff.y - diff.x
            } else {
                diff.x - diff.y
            };

            self.midpoints
                .push(Vec2i::new(self.endpoint_b.x, self.endpoint_b.y + offset));
        }

        if self.midpoints.len() <= self.midpoints.inline_size() {
            self.midpoints.shrink_to_fit();
        }
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
        center: Vec2f,
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
    DraggingWirePointA {
        wire_segment: usize,
        drag_start: Vec2f,
        drag_delta: Vec2f,
    },
    DraggingWirePointB {
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
    ComponentAnchor(usize),
    WirePointA(usize),
    WirePointB(usize),
}

#[derive(Serialize, Deserialize)]
pub struct Circuit {
    name: String,
    offset: Vec2f,
    #[serde(skip)]
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
    #[serde(skip)]
    file_name: Option<PathBuf>,
    #[serde(skip)]
    sim: Option<(gsim::Simulator, bool)>,
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
            file_name: None,
            sim: None,
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
    pub fn set_offset(&mut self, offset: Vec2f) -> bool {
        let old_offset = self.offset;
        self.offset = offset;
        old_offset != offset
    }

    #[inline]
    pub fn linear_zoom(&self) -> f32 {
        self.linear_zoom
    }

    pub fn set_linear_zoom(&mut self, zoom: f32) -> bool {
        let new_linear_zoom = zoom.clamp(MIN_LINEAR_ZOOM, MAX_LINEAR_ZOOM);
        if new_linear_zoom != self.linear_zoom {
            self.linear_zoom = new_linear_zoom;
            self.zoom = linear_to_zoom(self.linear_zoom);
            true
        } else {
            false
        }
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

    #[inline]
    pub fn file_name(&self) -> Option<&Path> {
        self.file_name.as_deref()
    }

    #[inline]
    pub fn set_file_name(&mut self, file_name: PathBuf) {
        self.file_name = Some(file_name);
    }

    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap()
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, serde_json::Error> {
        let mut circuit: Circuit = serde_json::from_slice(data)?;
        circuit.linear_zoom = zoom_to_linear(circuit.zoom);
        Ok(circuit)
    }

    fn hit_test(&self, logical_pos: Vec2f) -> HitTestResult {
        for (i, component) in self.components.iter().enumerate() {
            for anchor in component.anchors() {
                if (logical_pos - anchor.position.to_vec2f()).len() <= (LOGICAL_PIXEL_SIZE * 2.0) {
                    return HitTestResult::ComponentAnchor(i);
                }
            }

            if component.bounding_box().contains(logical_pos) {
                return HitTestResult::Component(i);
            }
        }

        for (i, wire_segment) in self.wire_segments.iter().enumerate() {
            if (logical_pos - wire_segment.endpoint_a.to_vec2f()).len()
                <= (LOGICAL_PIXEL_SIZE * 2.0)
            {
                return HitTestResult::WirePointA(i);
            }

            if (logical_pos - wire_segment.endpoint_b.to_vec2f()).len()
                <= (LOGICAL_PIXEL_SIZE * 2.0)
            {
                return HitTestResult::WirePointB(i);
            }

            if wire_segment.contains(logical_pos) {
                return HitTestResult::WireSegment(i);
            }
        }

        HitTestResult::None
    }

    pub fn primary_button_pressed(&mut self, pos: Vec2f, drag_mode: DragMode) -> bool {
        assert!(
            is_discriminant!(self.drag_state, DragState::None),
            "invalid drag state"
        );

        let logical_pos = pos / (self.zoom * BASE_ZOOM) + self.offset;
        let hit = self.hit_test(logical_pos);

        let requires_redraw = match (hit, drag_mode) {
            (HitTestResult::None, _) => {
                if !matches!(self.selection, Selection::None) {
                    self.selection = Selection::None;
                    true
                } else {
                    false
                }
            }
            (HitTestResult::Component(component), _)
            | (HitTestResult::ComponentAnchor(component), DragMode::BoxSelection) => {
                if !self.selection.contains_component(component) {
                    self.selection = Selection::Component(component);
                    true
                } else {
                    false
                }
            }
            (HitTestResult::WireSegment(wire_segment), DragMode::BoxSelection)
            | (HitTestResult::WirePointA(wire_segment), DragMode::BoxSelection)
            | (HitTestResult::WirePointB(wire_segment), DragMode::BoxSelection) => {
                if !self.selection.contains_wire_segment(wire_segment) {
                    self.selection = Selection::WireSegment(wire_segment);
                    true
                } else {
                    false
                }
            }
            (HitTestResult::ComponentAnchor(_), DragMode::DrawWire)
            | (HitTestResult::WireSegment(_), DragMode::DrawWire)
            | (HitTestResult::WirePointA(_), DragMode::DrawWire)
            | (HitTestResult::WirePointB(_), DragMode::DrawWire) => false,
        };

        self.drag_state = DragState::Deadzone {
            drag_start: logical_pos,
            drag_delta: Vec2f::default(),
        };

        self.primary_button_down = true;
        requires_redraw
    }

    pub fn primary_button_released(&mut self, pos: Vec2f) -> bool {
        let mut requires_redraw = false;

        if self.primary_button_down {
            if is_discriminant!(self.drag_state, DragState::None) {
                let logical_pos = pos / (self.zoom * BASE_ZOOM) + self.offset;
                let hit = self.hit_test(logical_pos);

                match hit {
                    HitTestResult::None => {
                        if !matches!(self.selection, Selection::None) {
                            self.selection = Selection::None;
                            requires_redraw = true;
                        }
                    }
                    HitTestResult::Component(component)
                    | HitTestResult::ComponentAnchor(component) => {
                        self.selection = Selection::Component(component);
                        requires_redraw = true;
                    }
                    HitTestResult::WireSegment(wire_segment)
                    | HitTestResult::WirePointA(wire_segment)
                    | HitTestResult::WirePointB(wire_segment) => {
                        self.selection = Selection::WireSegment(wire_segment);
                        requires_redraw = true;
                    }
                }
            }

            if let DragState::DrawingBoxSelection {
                drag_start,
                drag_delta,
            } = &self.drag_state
            {
                let selection_box = Rectangle {
                    top: drag_start.y.max(drag_start.y + drag_delta.y),
                    bottom: drag_start.y.min(drag_start.y + drag_delta.y),
                    left: drag_start.x.min(drag_start.x + drag_delta.x),
                    right: drag_start.x.max(drag_start.x + drag_delta.x),
                };

                let mut selected_components = HashSet::new();
                for (i, component) in self.components.iter().enumerate() {
                    if selection_box.contains(component.position().to_vec2f()) {
                        selected_components.insert(i);
                    }
                }

                let mut selected_wire_segments = HashSet::new();
                for (i, wire_segment) in self.wire_segments.iter().enumerate() {
                    if selection_box.contains(wire_segment.endpoint_a.to_vec2f())
                        || selection_box.contains(wire_segment.endpoint_b.to_vec2f())
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
                } else if !selected_components.is_empty() || !selected_wire_segments.is_empty() {
                    let bb = self
                        .find_selection_bounding_box(&selected_components, &selected_wire_segments);

                    self.selection = Selection::Multi {
                        components: selected_components,
                        wire_segments: selected_wire_segments,
                        center: bb.center(),
                    };
                }

                requires_redraw = true;
            }

            self.drag_state = DragState::None;
        }

        // TODO:
        //   If we were drawing a wire segment we want to potentially split existing
        //   segments if they exactly intersect with one of the new segments endpoints.
        //
        //   x-----------------------x
        //               ^ dragging on top
        //               x
        //               |
        //               |
        //
        //               v split existing segment here
        //   x-----------x-----------x
        //               |
        //               |

        self.primary_button_down = false;

        requires_redraw
    }

    pub fn secondary_button_pressed(&mut self, _pos: Vec2f) -> bool {
        self.secondary_button_down = true;
        false
    }

    pub fn secondary_button_released(&mut self, pos: Vec2f) -> bool {
        let mut requires_redraw = false;

        if self.secondary_button_down {
            let logical_pos = pos / (self.zoom * BASE_ZOOM) + self.offset;
            let hit = self.hit_test(logical_pos);

            match hit {
                HitTestResult::None => {
                    if !matches!(self.selection, Selection::None) {
                        self.selection = Selection::None;
                        requires_redraw = true;
                    }
                }
                HitTestResult::Component(component) | HitTestResult::ComponentAnchor(component) => {
                    if !self.selection.contains_component(component) {
                        self.selection = Selection::Component(component);
                        requires_redraw = true;
                    }

                    // TODO: show context menu
                }
                HitTestResult::WireSegment(wire_segment)
                | HitTestResult::WirePointA(wire_segment)
                | HitTestResult::WirePointB(wire_segment) => {
                    if !self.selection.contains_wire_segment(wire_segment) {
                        self.selection = Selection::WireSegment(wire_segment);
                        requires_redraw = true;
                    }

                    // TODO: show context menu
                }
            }

            self.drag_state = DragState::None;
        }

        self.secondary_button_down = false;

        requires_redraw
    }

    pub fn move_selection(&mut self, delta: Vec2i) {
        match self.selection {
            Selection::None => {}
            Selection::Component(component) => {
                let component = self
                    .components
                    .get_mut(component)
                    .expect("invalid selection");

                component.set_position(component.position() + delta);
            }
            Selection::WireSegment(wire_segment) => {
                let wire_segment = self
                    .wire_segments
                    .get_mut(wire_segment)
                    .expect("invalid selection");

                wire_segment.endpoint_a += delta;
                wire_segment.endpoint_b += delta;
                for p in wire_segment.midpoints.iter_mut() {
                    *p += delta;
                }
            }
            Selection::Multi {
                ref components,
                ref wire_segments,
                ref mut center,
            } => {
                for &component in components {
                    let component = self
                        .components
                        .get_mut(component)
                        .expect("invalid selection");

                    component.set_position(component.position() + delta);
                }

                for &wire_segment in wire_segments {
                    let wire_segment = self
                        .wire_segments
                        .get_mut(wire_segment)
                        .expect("invalid selection");

                    wire_segment.endpoint_a += delta;
                    wire_segment.endpoint_b += delta;
                    for p in wire_segment.midpoints.iter_mut() {
                        *p += delta;
                    }
                }

                *center += delta.to_vec2f();
            }
        }
    }

    pub fn mouse_moved(&mut self, delta: Vec2f, drag_mode: DragMode) -> bool {
        const DEADZONE_RANGE: f32 = 1.0;

        if self.primary_button_down && !self.secondary_button_down {
            match &mut self.drag_state {
                DragState::None => false,
                DragState::Deadzone {
                    drag_start,
                    drag_delta,
                } => {
                    *drag_delta += delta;

                    let drag_start = *drag_start;
                    let drag_delta = *drag_delta;

                    if (drag_delta.x.abs() >= DEADZONE_RANGE)
                        || (drag_delta.y.abs() >= DEADZONE_RANGE)
                    {
                        let hit = self.hit_test(drag_start);

                        self.drag_state = match (hit, drag_mode) {
                            (HitTestResult::None, DragMode::BoxSelection) => {
                                DragState::DrawingBoxSelection {
                                    drag_start,
                                    drag_delta,
                                }
                            }
                            (HitTestResult::None, DragMode::DrawWire)
                            | (HitTestResult::ComponentAnchor(_), DragMode::DrawWire) => {
                                let endpoint_a = drag_start.round().to_vec2i();
                                let endpoint_b = (drag_start + drag_delta).round().to_vec2i();

                                let mut segment = WireSegment {
                                    endpoint_a,
                                    midpoints: smallvec![],
                                    endpoint_b,
                                    sim_wires: smallvec![],
                                };
                                segment.update_midpoints();

                                let wire_segment = self.wire_segments.len();
                                self.wire_segments.push(segment);

                                DragState::DraggingWirePointB {
                                    wire_segment,
                                    drag_start,
                                    drag_delta,
                                }
                            }
                            (HitTestResult::Component(component), _)
                            | (HitTestResult::ComponentAnchor(component), DragMode::BoxSelection) =>
                            {
                                assert!(
                                    self.selection.contains_component(component),
                                    "invalid drag state"
                                );

                                // TODO: already drag whole part of delta
                                DragState::Dragging {
                                    fract_drag_delta: drag_delta,
                                }
                            }
                            (HitTestResult::WireSegment(wire_segment), DragMode::BoxSelection) => {
                                assert!(
                                    self.selection.contains_wire_segment(wire_segment),
                                    "invalid drag state"
                                );

                                // TODO: already drag whole part of delta
                                DragState::Dragging {
                                    fract_drag_delta: drag_delta,
                                }
                            }
                            (HitTestResult::WirePointA(wire_segment), DragMode::BoxSelection) => {
                                DragState::DraggingWirePointA {
                                    wire_segment,
                                    drag_start,
                                    drag_delta,
                                }
                            }
                            (HitTestResult::WirePointB(wire_segment), DragMode::BoxSelection) => {
                                DragState::DraggingWirePointB {
                                    wire_segment,
                                    drag_start,
                                    drag_delta,
                                }
                            }
                            (HitTestResult::WireSegment(_wire_segment), DragMode::DrawWire) => {
                                // TODO: split the existing segment at the new wires start point

                                let endpoint_a = drag_start.round().to_vec2i();
                                let endpoint_b = (drag_start + drag_delta).round().to_vec2i();

                                let mut segment = WireSegment {
                                    endpoint_a,
                                    midpoints: smallvec![],
                                    endpoint_b,
                                    sim_wires: smallvec![],
                                };
                                segment.update_midpoints();

                                let wire_segment = self.wire_segments.len();
                                self.wire_segments.push(segment);

                                DragState::DraggingWirePointB {
                                    wire_segment,
                                    drag_start,
                                    drag_delta,
                                }
                            }
                            (HitTestResult::WirePointA(_), DragMode::DrawWire)
                            | (HitTestResult::WirePointB(_), DragMode::DrawWire) => {
                                let endpoint_a = drag_start.round().to_vec2i();
                                let endpoint_b = (drag_start + drag_delta).round().to_vec2i();

                                let mut segment = WireSegment {
                                    endpoint_a,
                                    midpoints: smallvec![],
                                    endpoint_b,
                                    sim_wires: smallvec![],
                                };
                                segment.update_midpoints();

                                let wire_segment = self.wire_segments.len();
                                self.wire_segments.push(segment);

                                DragState::DraggingWirePointB {
                                    wire_segment,
                                    drag_start,
                                    drag_delta,
                                }
                            }
                        };

                        true
                    } else {
                        false
                    }
                }
                DragState::DrawingBoxSelection { drag_delta, .. } => {
                    *drag_delta += delta;
                    true
                }
                DragState::DraggingWirePointA {
                    wire_segment,
                    drag_start,
                    drag_delta,
                } => {
                    *drag_delta += delta;

                    let wire_segment = self
                        .wire_segments
                        .get_mut(*wire_segment)
                        .expect("invalid drag state");

                    let new_a = (*drag_start + *drag_delta).round().to_vec2i();
                    if wire_segment.endpoint_a != new_a {
                        wire_segment.endpoint_a = new_a;
                        wire_segment.update_midpoints();
                    }

                    true
                }
                DragState::DraggingWirePointB {
                    wire_segment,
                    drag_start,
                    drag_delta,
                } => {
                    *drag_delta += delta;

                    let wire_segment = self
                        .wire_segments
                        .get_mut(*wire_segment)
                        .expect("invalid drag state");

                    let new_b = (*drag_start + *drag_delta).round().to_vec2i();
                    if wire_segment.endpoint_b != new_b {
                        wire_segment.endpoint_b = new_b;
                        wire_segment.update_midpoints();
                    }

                    true
                }
                DragState::Dragging { fract_drag_delta } => {
                    assert!(
                        !is_discriminant!(self.selection, Selection::None),
                        "invalid drag state"
                    );

                    *fract_drag_delta += delta;
                    let whole_drag_delta = fract_drag_delta.round();
                    *fract_drag_delta -= whole_drag_delta;

                    let whole_drag_delta = whole_drag_delta.to_vec2i();
                    if whole_drag_delta != Vec2i::ZERO {
                        self.move_selection(whole_drag_delta);
                        true
                    } else {
                        false
                    }
                }
            }
        } else {
            false
        }
    }

    fn find_selection_bounding_box(
        &self,
        components: &HashSet<usize>,
        wire_segments: &HashSet<usize>,
    ) -> Rectangle {
        let mut min = Vec2i::new(i32::MAX, i32::MAX);
        let mut max = Vec2i::new(i32::MIN, i32::MIN);

        for &component in components {
            let component = self.components.get(component).expect("invalid selection");

            min = min.min(component.position());
            max = max.max(component.position());
        }

        for &wire_segment in wire_segments {
            let wire_segment = self
                .wire_segments
                .get(wire_segment)
                .expect("invalid selection");

            min = min.min(wire_segment.endpoint_a);
            max = max.max(wire_segment.endpoint_a);

            min = min.min(wire_segment.endpoint_b);
            max = max.max(wire_segment.endpoint_b);

            for p in wire_segment.midpoints.iter() {
                min = min.min(*p);
                max = max.max(*p);
            }
        }

        Rectangle {
            top: max.y as f32,
            bottom: min.y as f32,
            left: min.x as f32,
            right: max.x as f32,
        }
    }

    fn transform_selection(
        &mut self,
        apply_mirror: impl Fn(bool) -> bool,
        apply_rot: impl Fn(Rotation) -> Rotation,
        apply_pt: impl Fn(Vec2f) -> Vec2f,
    ) {
        match self.selection {
            Selection::None => {}
            Selection::Component(component) => {
                let component = self
                    .components
                    .get_mut(component)
                    .expect("invalid selection");

                component.mirrored = apply_mirror(component.mirrored);
                component.rotation = apply_rot(component.rotation);
            }
            Selection::WireSegment(wire_segment) => {
                let wire_segment = self
                    .wire_segments
                    .get_mut(wire_segment)
                    .expect("invalid selection");

                let center = (wire_segment.endpoint_a + wire_segment.endpoint_b).to_vec2f() * 0.5;

                let a = wire_segment.endpoint_a.to_vec2f() - center;
                let b = wire_segment.endpoint_b.to_vec2f() - center;
                wire_segment.endpoint_a = (apply_pt(a) + center).floor().to_vec2i();
                wire_segment.endpoint_b = (apply_pt(b) + center).floor().to_vec2i();

                for p in wire_segment.midpoints.iter_mut() {
                    let rp = p.to_vec2f() - center;
                    *p = (apply_pt(rp) + center).floor().to_vec2i();
                }
            }
            Selection::Multi {
                ref components,
                ref wire_segments,
                center,
            } => {
                for &component in components {
                    let component = self
                        .components
                        .get_mut(component)
                        .expect("invalid selection");

                    let pos = component.position().to_vec2f() - center;
                    component.set_position((apply_pt(pos) + center).floor().to_vec2i());
                    component.mirrored = apply_mirror(component.mirrored);
                    component.rotation = apply_rot(component.rotation);
                }

                for &wire_segment in wire_segments {
                    let wire_segment = self
                        .wire_segments
                        .get_mut(wire_segment)
                        .expect("invalid selection");

                    let a = wire_segment.endpoint_a.to_vec2f() - center;
                    let b = wire_segment.endpoint_b.to_vec2f() - center;
                    wire_segment.endpoint_a = (apply_pt(a) + center).floor().to_vec2i();
                    wire_segment.endpoint_b = (apply_pt(b) + center).floor().to_vec2i();

                    for p in wire_segment.midpoints.iter_mut() {
                        let rp = p.to_vec2f() - center;
                        *p = (apply_pt(rp) + center).floor().to_vec2i();
                    }
                }
            }
        }
    }

    pub fn counterclockwise_rotate_selection(&mut self) {
        self.transform_selection(std::convert::identity, Rotation::next, |v| {
            Vec2f::new(-v.y, v.x)
        });
    }

    pub fn clockwise_rotate_selection(&mut self) {
        self.transform_selection(std::convert::identity, Rotation::prev, |v| {
            Vec2f::new(v.y, -v.x)
        });
    }

    pub fn mirror_selection(&mut self) {
        self.transform_selection(std::ops::Not::not, Rotation::mirror, |v| {
            Vec2f::new(-v.x, v.y)
        });
    }

    pub fn update_component_properties(
        &mut self,
        ui: &mut egui::Ui,
        locale_manager: &LocaleManager,
        lang: &LangId,
    ) -> bool {
        match &self.selection {
            Selection::None => false,
            &Selection::Component(selected_component) => {
                ui.heading(locale_manager.get(lang, "properties-header"));
                self.components[selected_component].update_properties(ui, locale_manager, lang)
            }
            &Selection::WireSegment(selected_segment) => {
                ui.heading(locale_manager.get(lang, "properties-header"));

                let segment = &mut self.wire_segments[selected_segment];
                let mut needs_midpoint_update = false;

                ui.horizontal(|ui| {
                    ui.label("X1:");

                    let mut x1_text = format!("{}", segment.endpoint_a.x);
                    ui.text_edit_singleline(&mut x1_text);
                    if let Ok(new_x1) = x1_text.parse() {
                        if segment.endpoint_a.x != new_x1 {
                            segment.endpoint_a.x = new_x1;
                            needs_midpoint_update = true;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Y1:");

                    let mut y1_text = format!("{}", segment.endpoint_a.y);
                    ui.text_edit_singleline(&mut y1_text);
                    if let Ok(new_y1) = y1_text.parse() {
                        if segment.endpoint_a.y != new_y1 {
                            segment.endpoint_a.y = new_y1;
                            needs_midpoint_update = true;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("X2:");

                    let mut x2_text = format!("{}", segment.endpoint_b.x);
                    ui.text_edit_singleline(&mut x2_text);
                    if let Ok(new_x2) = x2_text.parse() {
                        if segment.endpoint_b.x != new_x2 {
                            segment.endpoint_b.x = new_x2;
                            needs_midpoint_update = true;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Y2:");

                    let mut y2_text = format!("{}", segment.endpoint_b.y);
                    ui.text_edit_singleline(&mut y2_text);
                    if let Ok(new_y2) = y2_text.parse() {
                        if segment.endpoint_b.y != new_y2 {
                            segment.endpoint_b.y = new_y2;
                            needs_midpoint_update = true;
                        }
                    }
                });

                if needs_midpoint_update {
                    segment.update_midpoints();
                }

                needs_midpoint_update
            }
            Selection::Multi { .. } => false,
        }
    }

    #[inline]
    pub fn is_simulating(&self) -> bool {
        self.sim.is_some()
    }

    pub fn start_simulation(&mut self, max_steps: u64) -> gsim::SimulationRunResult {
        use gsim::*;

        let mut builder = SimulatorBuilder::default();

        // TODO: build simulation graph
        //
        //  1. Find connected nets of wire segments
        //  2. Create wire(s) in simulation graph for each net
        //  3. Create component(s) in simulation graph for each editor component

        type WireSegmentIndex = usize;
        type WireGroupIndex = usize;

        fn segments_connect(a: &WireSegment, b: &WireSegment) -> bool {
            (a.endpoint_a == b.endpoint_a)
                || (a.endpoint_a == b.endpoint_b)
                || (a.endpoint_b == b.endpoint_a)
                || (a.endpoint_b == b.endpoint_b)
        }

        fn find_adjacent(
            segments: &[WireSegment],
            segment: &WireSegment,
            group: &mut Vec<WireSegmentIndex>,
            group_map: &mut Vec<Option<WireGroupIndex>>,
            group_index: WireGroupIndex,
        ) {
            for (i, other_segment) in segments.iter().enumerate() {
                if group_map[i].is_none() && segments_connect(segment, other_segment) {
                    group_map[i] = Some(group_index);

                    group.push(i);
                    find_adjacent(segments, other_segment, group, group_map, group_index);
                }
            }
        }

        let mut groups: Vec<Vec<WireSegmentIndex>> = Vec::new();
        let mut group_map: Vec<Option<WireGroupIndex>> = vec![None; self.wire_segments.len()];
        for (i, segment) in self.wire_segments.iter().enumerate() {
            if group_map[i].is_none() {
                let group_index: WireGroupIndex = groups.len();
                group_map[i] = Some(group_index);

                let mut group: Vec<WireSegmentIndex> = vec![i];
                find_adjacent(
                    &self.wire_segments,
                    segment,
                    &mut group,
                    &mut group_map,
                    group_index,
                );
                groups.push(group);
            }
        }

        println!("{groups:?}");
        println!("{group_map:?}");

        let clk_state = LogicState::LOGIC_0;
        for component in &self.components {
            match component.kind {
                ComponentKind::Input {
                    value, sim_wire, ..
                } => {
                    let state = LogicState::from_int(value);
                    builder.set_wire_drive(sim_wire, &state).unwrap()
                }
                ComponentKind::ClockInput { sim_wire, .. } => {
                    builder.set_wire_drive(sim_wire, &clk_state).unwrap()
                }
                _ => (),
            }
        }

        let mut sim = builder.build();
        let result = sim.run_sim(max_steps);
        if matches!(result, gsim::SimulationRunResult::Ok) {
            self.sim = Some((sim, false));
        }
        result
    }

    pub fn step_simulation(&mut self, max_steps: u64) -> gsim::SimulationRunResult {
        use gsim::*;

        let Some((sim, clk)) = &mut self.sim else {
            panic!("simulation is not running");
        };

        *clk = !*clk;
        let clk_state = LogicState::from_bool(*clk);
        for component in &self.components {
            if let ComponentKind::ClockInput { sim_wire, .. } = component.kind {
                sim.set_wire_drive(sim_wire, &clk_state).unwrap();
            }
        }

        let result = sim.run_sim(max_steps);
        if !matches!(result, gsim::SimulationRunResult::Ok) {
            self.sim = None;
        }
        result
    }

    pub fn stop_simulation(&mut self) {
        self.sim = None;

        for component in &mut self.components {
            component.kind.reset_sim_ids();
        }

        for wire_segment in &mut self.wire_segments {
            wire_segment.sim_wires.clear();
        }
    }
}
