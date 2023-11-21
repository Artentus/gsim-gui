use super::component::*;
use super::locale::*;
use super::viewport::{BASE_ZOOM, LOGICAL_PIXEL_SIZE};
use crate::app::math::*;
use crate::{is_discriminant, HashSet};
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use std::num::NonZeroU8;
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
    pub fn contains(&self, p: Vec2f) -> Option<usize> {
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
            return None;
        }

        // Triangle test
        let midpoints = self.midpoints.iter().copied();
        let endpoint_b = std::iter::once(self.endpoint_b);

        let mut a = self.endpoint_a.to_vec2f();
        for (i, b) in midpoints.chain(endpoint_b).map(Vec2i::to_vec2f).enumerate() {
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
                return Some(i);
            }

            a = b;
        }

        None
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

    fn split_at(&mut self, index: usize, p: Vec2i) -> WireSegment {
        let (mut left, mut right) = self.midpoints.split_at(index);

        if Some(p) == left.last().copied() {
            left = &left[..(left.len() - 1)];
        }

        if Some(p) == right.first().copied() {
            right = &right[1..];
        }

        let new = WireSegment {
            endpoint_a: p,
            midpoints: right.into(),
            endpoint_b: self.endpoint_b,
            sim_wires: self.sim_wires.clone(),
        };

        self.midpoints = left.into();
        self.endpoint_b = p;

        new
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

enum HitTestResult {
    None,
    Component(usize),
    WireSegment(usize, usize),
    ComponentAnchor(usize),
    WirePointA(usize),
    WirePointB(usize),
}

#[derive(Default)]
pub enum SimState {
    #[default]
    None,
    Active {
        sim: gsim::Simulator,
        clock_state: bool,
    },
    Conflict {
        sim: gsim::Simulator,
        conflict_segments: HashSet<usize>,
    },
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
    sim_state: SimState,
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
            sim_state: SimState::None,
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

    #[inline]
    pub fn sim_state(&self) -> &SimState {
        &self.sim_state
    }

    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap()
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, serde_json::Error> {
        let mut circuit: Circuit = serde_json::from_slice(data)?;
        circuit.linear_zoom = zoom_to_linear(circuit.zoom);
        Ok(circuit)
    }

    fn hit_test(&self, logical_pos: Vec2f, exclude_wire: Option<usize>) -> HitTestResult {
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
            if Some(i) == exclude_wire {
                continue;
            }

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

            if let Some(split_point) = wire_segment.contains(logical_pos) {
                return HitTestResult::WireSegment(i, split_point);
            }
        }

        HitTestResult::None
    }

    pub fn primary_button_pressed(
        &mut self,
        pos: Vec2f,
        drag_mode: DragMode,
        max_steps: u64,
    ) -> bool {
        assert!(
            is_discriminant!(self.drag_state, DragState::None),
            "invalid drag state"
        );

        let logical_pos = pos / (self.zoom * BASE_ZOOM) + self.offset;
        let hit = self.hit_test(logical_pos, None);

        let mut sim_state = SimState::None;
        std::mem::swap(&mut sim_state, &mut self.sim_state);

        let requires_redraw = if let SimState::Active {
            mut sim,
            clock_state,
        } = sim_state
        {
            match hit {
                HitTestResult::Component(component) | HitTestResult::ComponentAnchor(component) => {
                    let component = &mut self.components[component];
                    match &mut component.kind {
                        ComponentKind::Input {
                            value,
                            width,
                            sim_wire,
                            ..
                        } if width.value.get() == 1 => {
                            *value = !*value;
                            sim.set_wire_drive(*sim_wire, &gsim::LogicState::from_int(*value))
                                .unwrap();

                            self.advance_simulation(sim, clock_state, max_steps);

                            true
                        }
                        _ => {
                            self.sim_state = SimState::Active { sim, clock_state };
                            false
                        }
                    }
                }
                _ => {
                    self.sim_state = SimState::Active { sim, clock_state };
                    false
                }
            }
        } else {
            self.sim_state = sim_state;

            self.drag_state = DragState::Deadzone {
                drag_start: logical_pos,
                drag_delta: Vec2f::default(),
            };

            match (hit, drag_mode) {
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
                (HitTestResult::WireSegment(wire_segment, _), DragMode::BoxSelection)
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
                | (HitTestResult::WireSegment(_, _), DragMode::DrawWire)
                | (HitTestResult::WirePointA(_), DragMode::DrawWire)
                | (HitTestResult::WirePointB(_), DragMode::DrawWire) => false,
            }
        };

        self.primary_button_down = true;
        requires_redraw
    }

    pub fn primary_button_released(&mut self, pos: Vec2f) -> bool {
        let mut requires_redraw = false;

        if self.primary_button_down {
            if is_discriminant!(self.drag_state, DragState::None) {
                let logical_pos = pos / (self.zoom * BASE_ZOOM) + self.offset;
                let hit = self.hit_test(logical_pos, None);

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
                    HitTestResult::WireSegment(wire_segment, _)
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

            //   If we were drawing a wire segment we want to split an existing
            //   segment that exactly intersects with the new segments endpoints.
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
            let dragged = match self.drag_state {
                DragState::DraggingWirePointA { wire_segment, .. } => {
                    Some((wire_segment, self.wire_segments[wire_segment].endpoint_a))
                }
                DragState::DraggingWirePointB { wire_segment, .. } => {
                    Some((wire_segment, self.wire_segments[wire_segment].endpoint_b))
                }
                _ => None,
            };
            if let Some((dragged_wire, dragged_endpoint)) = dragged {
                if let HitTestResult::WireSegment(split_segment, split_index) =
                    self.hit_test(dragged_endpoint.to_vec2f(), Some(dragged_wire))
                {
                    let old_split_segment = &mut self.wire_segments[split_segment];
                    let new_split_segment =
                        old_split_segment.split_at(split_index, dragged_endpoint);
                    self.wire_segments.push(new_split_segment);
                }
            }

            self.drag_state = DragState::None;
        }

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
            let hit = self.hit_test(logical_pos, None);

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
                HitTestResult::WireSegment(wire_segment, _)
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
        const DEADZONE_RANGE: f32 = 0.8;

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

                    if drag_delta.len() >= DEADZONE_RANGE {
                        let hit = self.hit_test(drag_start, None);

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
                            (
                                HitTestResult::WireSegment(wire_segment, _),
                                DragMode::BoxSelection,
                            ) => {
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
                            (
                                HitTestResult::WireSegment(wire_segment, split_index),
                                DragMode::DrawWire,
                            ) => {
                                let endpoint_a = drag_start.round().to_vec2i();
                                let endpoint_b = (drag_start + drag_delta).round().to_vec2i();

                                let old_split_segment = &mut self.wire_segments[wire_segment];
                                let new_split_segment =
                                    old_split_segment.split_at(split_index, endpoint_a);
                                self.wire_segments.push(new_split_segment);

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

    pub fn delete_selection(&mut self) {
        let mut i = 0;
        self.components.retain(|_| {
            let in_selection = self.selection.contains_component(i);
            i += 1;
            !in_selection
        });

        let mut i = 0;
        self.wire_segments.retain(|_| {
            let in_selection = self.selection.contains_wire_segment(i);
            i += 1;
            !in_selection
        });

        self.selection = Selection::None;
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

    fn find_wire_groups(&self) -> (Vec<Vec<usize>>, Vec<usize>) {
        fn segments_connect(a: &WireSegment, b: &WireSegment) -> bool {
            (a.endpoint_a == b.endpoint_a)
                || (a.endpoint_a == b.endpoint_b)
                || (a.endpoint_b == b.endpoint_a)
                || (a.endpoint_b == b.endpoint_b)
        }

        fn find_adjacent(
            segments: &[WireSegment],
            segment: &WireSegment,
            group: &mut Vec<usize>,
            group_map: &mut Vec<Option<usize>>,
            group_index: usize,
        ) {
            for (i, other_segment) in segments.iter().enumerate() {
                if group_map[i].is_none() && segments_connect(segment, other_segment) {
                    group_map[i] = Some(group_index);

                    group.push(i);
                    find_adjacent(segments, other_segment, group, group_map, group_index);
                }
            }
        }

        let mut groups = Vec::new();
        let mut group_map = vec![None; self.wire_segments.len()];
        for (i, segment) in self.wire_segments.iter().enumerate() {
            if group_map[i].is_none() {
                let group_index = groups.len();
                group_map[i] = Some(group_index);

                let mut group = vec![i];
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

        let group_map = group_map
            .into_iter()
            .map(|i| i.expect("wire with no group"))
            .collect();

        (groups, group_map)
    }

    fn find_wire_group_widths(&self, groups: &[Vec<usize>]) -> Result<Vec<NonZeroU8>, ()> {
        fn find_segment_width(
            segment: &WireSegment,
            components: &[Component],
        ) -> Result<Option<NonZeroU8>, ()> {
            let mut segment_width = None;
            for anchor in components.iter().flat_map(Component::anchors) {
                if (anchor.position == segment.endpoint_a)
                    || (anchor.position == segment.endpoint_b)
                {
                    if let Some(segment_width) = segment_width {
                        if anchor.width != segment_width {
                            return Err(());
                        }
                    } else {
                        segment_width = Some(anchor.width);
                    }
                }
            }

            Ok(segment_width)
        }

        groups
            .iter()
            .map(|group| {
                let mut group_width = None;
                for segment in group.iter().map(|&i| &self.wire_segments[i]) {
                    let segment_width = find_segment_width(segment, &self.components)?;

                    match (group_width, segment_width) {
                        (_, None) => (),
                        (None, Some(segment_width)) => group_width = Some(segment_width),
                        (Some(group_width), Some(segment_width)) => {
                            if segment_width != group_width {
                                return Err(());
                            }
                        }
                    }
                }

                Ok(group_width.unwrap_or(NonZeroU8::MIN))
            })
            .collect()
    }

    fn advance_simulation(&mut self, mut sim: gsim::Simulator, clock_state: bool, max_steps: u64) {
        use gsim::*;

        self.sim_state = match sim.run_sim(max_steps) {
            SimulationRunResult::Ok => SimState::Active { sim, clock_state },
            SimulationRunResult::MaxStepsReached => todo!(),
            SimulationRunResult::Err(err) => {
                let mut conflict_segments = HashSet::new();
                for (i, segment) in self.wire_segments.iter().enumerate() {
                    for sim_wire in &segment.sim_wires {
                        if err.conflicts.contains(sim_wire) {
                            conflict_segments.insert(i);
                        }
                    }
                }

                SimState::Conflict {
                    sim,
                    conflict_segments,
                }
            }
        };
    }

    pub fn start_simulation(&mut self, max_steps: u64) {
        use gsim::*;

        let mut builder = SimulatorBuilder::default();

        // TODO: build simulation graph
        //
        //  1. Find connected nets of wire segments
        //  2. Create wire(s) in simulation graph for each net
        //  3. Create component(s) in simulation graph for each editor component

        // TODO: optimize all of this, because we are doing work multiple times

        // connected nets of wire segments
        let (groups, group_map) = self.find_wire_groups();
        let Ok(group_widths) = self.find_wire_group_widths(&groups) else {
            todo!() // TODO: display wire width conflict
        };

        // TODO: find connected nets of wire segments _and_ splitters

        // TODO: depending on splitter configuration, potentially create more than one sim wire per group
        for (group, &group_width) in groups.iter().zip(group_widths.iter()) {
            let sim_wire = builder.add_wire(group_width).unwrap();

            for &i in group {
                let segment = &mut self.wire_segments[i];
                segment.sim_wires = smallvec![sim_wire];
            }
        }

        // TODO: find some general solution to associate anchors with wires instead of hardcoding indices
        // TODO: create dummy wires for unconnected anchors
        for component in &mut self.components {
            let anchors = component.anchors();

            match &mut component.kind {
                ComponentKind::Input {
                    name,
                    value,
                    width,
                    sim_wire,
                } => {
                    let mut wire = None;
                    for segment in &self.wire_segments {
                        if (segment.endpoint_a == anchors[0].position)
                            || (segment.endpoint_b == anchors[0].position)
                        {
                            wire = Some(segment.sim_wires[0]);
                            break;
                        }
                    }
                    *sim_wire = wire.unwrap();
                }
                ComponentKind::ClockInput { name, sim_wire } => todo!(),
                ComponentKind::Output {
                    name,
                    width,
                    sim_wire,
                } => {
                    let mut wire = None;
                    for segment in &self.wire_segments {
                        if (segment.endpoint_a == anchors[0].position)
                            || (segment.endpoint_b == anchors[0].position)
                        {
                            wire = Some(segment.sim_wires[0]);
                            break;
                        }
                    }
                    *sim_wire = wire.unwrap();
                }
                ComponentKind::Splitter { width, ranges } => todo!(),
                ComponentKind::AndGate {
                    width,
                    sim_component,
                } => {
                    let mut wires = vec![];
                    for anchor in anchors {
                        for segment in &self.wire_segments {
                            if (segment.endpoint_a == anchor.position)
                                || (segment.endpoint_b == anchor.position)
                            {
                                wires.push(segment.sim_wires[0]);
                                break;
                            }
                        }
                    }

                    let output = wires.pop().unwrap();
                    *sim_component = builder.add_and_gate(&wires, output).unwrap();
                }
                ComponentKind::OrGate {
                    width,
                    sim_component,
                } => {
                    let mut wires = vec![];
                    for anchor in anchors {
                        for segment in &self.wire_segments {
                            if (segment.endpoint_a == anchor.position)
                                || (segment.endpoint_b == anchor.position)
                            {
                                wires.push(segment.sim_wires[0]);
                                break;
                            }
                        }
                    }

                    let output = wires.pop().unwrap();
                    *sim_component = builder.add_or_gate(&wires, output).unwrap();
                }
                ComponentKind::XorGate {
                    width,
                    sim_component,
                } => {
                    let mut wires = vec![];
                    for anchor in anchors {
                        for segment in &self.wire_segments {
                            if (segment.endpoint_a == anchor.position)
                                || (segment.endpoint_b == anchor.position)
                            {
                                wires.push(segment.sim_wires[0]);
                                break;
                            }
                        }
                    }

                    let output = wires.pop().unwrap();
                    *sim_component = builder.add_xor_gate(&wires, output).unwrap();
                }
                ComponentKind::NandGate {
                    width,
                    sim_component,
                } => {
                    let mut wires = vec![];
                    for anchor in anchors {
                        for segment in &self.wire_segments {
                            if (segment.endpoint_a == anchor.position)
                                || (segment.endpoint_b == anchor.position)
                            {
                                wires.push(segment.sim_wires[0]);
                                break;
                            }
                        }
                    }

                    let output = wires.pop().unwrap();
                    *sim_component = builder.add_nand_gate(&wires, output).unwrap();
                }
                ComponentKind::NorGate {
                    width,
                    sim_component,
                } => {
                    let mut wires = vec![];
                    for anchor in anchors {
                        for segment in &self.wire_segments {
                            if (segment.endpoint_a == anchor.position)
                                || (segment.endpoint_b == anchor.position)
                            {
                                wires.push(segment.sim_wires[0]);
                                break;
                            }
                        }
                    }

                    let output = wires.pop().unwrap();
                    *sim_component = builder.add_nor_gate(&wires, output).unwrap();
                }
                ComponentKind::XnorGate {
                    width,
                    sim_component,
                } => {
                    let mut wires = vec![];
                    for anchor in anchors {
                        for segment in &self.wire_segments {
                            if (segment.endpoint_a == anchor.position)
                                || (segment.endpoint_b == anchor.position)
                            {
                                wires.push(segment.sim_wires[0]);
                                break;
                            }
                        }
                    }

                    let output = wires.pop().unwrap();
                    *sim_component = builder.add_xnor_gate(&wires, output).unwrap();
                }
            }
        }

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

        let sim = builder.build();
        self.advance_simulation(sim, false, max_steps);
    }

    pub fn step_simulation(&mut self, max_steps: u64) {
        use gsim::*;

        let mut sim_state = SimState::None;
        std::mem::swap(&mut sim_state, &mut self.sim_state);

        let SimState::Active {
            mut sim,
            clock_state,
        } = sim_state
        else {
            panic!("simulation is not running");
        };

        let clock_state = !clock_state;
        let clk = LogicState::from_bool(clock_state);
        for component in &self.components {
            if let ComponentKind::ClockInput { sim_wire, .. } = component.kind {
                sim.set_wire_drive(sim_wire, &clk).unwrap();
            }
        }

        self.advance_simulation(sim, clock_state, max_steps);
    }

    pub fn stop_simulation(&mut self) {
        self.sim_state = SimState::None;

        for component in &mut self.components {
            component.kind.reset_sim_ids();
        }

        for wire_segment in &mut self.wire_segments {
            wire_segment.sim_wires.clear();
        }
    }
}
