use super::component::*;
use super::locale::*;
use super::viewport::BASE_ZOOM;
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
    pub point_a: [i32; 2],
    pub point_b: [i32; 2],
}

#[derive(Default)]
enum Selection {
    #[default]
    None,
    Component(usize),
    WireSegment(usize),
}

#[derive(Serialize, Deserialize)]
pub struct Circuit {
    name: String,
    offset: [f32; 2],
    linear_zoom: f32,
    zoom: f32,
    components: Vec<Component>,
    wire_segments: Vec<WireSegment>,
    #[serde(skip)]
    selection: Selection,
    #[serde(skip)]
    drag_start: [i32; 2],
    #[serde(skip)]
    drag_delta: [f32; 2],
    #[serde(skip)]
    create_wire: Option<usize>,
}

impl Circuit {
    pub fn new() -> Self {
        Self {
            name: "New Circuit".to_owned(),
            offset: [0.0; 2],
            linear_zoom: zoom_to_linear(DEFAULT_ZOOM),
            zoom: DEFAULT_ZOOM,
            components: vec![],
            wire_segments: vec![],
            selection: Selection::None,
            drag_start: [0; 2],
            drag_delta: [0.0; 2],
            create_wire: None,
        }
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn offset(&self) -> [f32; 2] {
        self.offset
    }

    #[inline]
    pub fn set_offset(&mut self, offset: [f32; 2]) {
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

    pub fn update_selection(&mut self, pos: [f32; 2]) {
        let logical_pos = [
            pos[0] / (self.zoom * BASE_ZOOM) - self.offset[0],
            pos[1] / (self.zoom * BASE_ZOOM) - self.offset[1],
        ];

        self.selection = Selection::None;
        self.drag_start = [logical_pos[0].round() as i32, logical_pos[1].round() as i32];
        self.drag_delta = [0.0; 2];
        self.create_wire = None;

        for (i, component) in self.components.iter().enumerate() {
            if component.bounding_box().contains(logical_pos) {
                self.selection = Selection::Component(i);
                self.drag_start = component.position;
                break;
            }
        }
    }

    pub fn drag_selection(&mut self, delta: [f32; 2]) {
        self.drag_delta[0] += delta[0];
        self.drag_delta[1] += delta[1];

        match self.selection {
            Selection::None => {
                let create_wire = if let Some(create_wire) = self.create_wire {
                    &mut self.wire_segments[create_wire]
                } else {
                    self.create_wire = Some(self.wire_segments.len());
                    println!("Created wire at {:?}", self.drag_start);

                    self.wire_segments.push(WireSegment {
                        point_a: self.drag_start,
                        point_b: self.drag_start,
                    });

                    self.wire_segments.last_mut().unwrap()
                };

                create_wire.point_b[0] = self.drag_start[0] + (self.drag_delta[0].round() as i32);
                create_wire.point_b[1] = self.drag_start[1] + (self.drag_delta[1].round() as i32);
            }
            Selection::Component(selected_component) => {
                let component = &mut self.components[selected_component];
                component.position[0] = self.drag_start[0] + (self.drag_delta[0].round() as i32);
                component.position[1] = self.drag_start[1] + (self.drag_delta[1].round() as i32);
            }
            Selection::WireSegment(_) => { /* TODO: */ }
        }
    }

    pub fn end_drag(&mut self) {
        self.drag_start = [0; 2];
        self.drag_delta = [0.0; 2];
        self.create_wire = None;
    }

    pub fn update_component_properties<'a>(
        &mut self,
        ui: &mut egui::Ui,
        locale_manager: &LocaleManager,
        lang: &LangId,
    ) {
        match self.selection {
            Selection::None => {}
            Selection::Component(selected_component) => {
                ui.heading(locale_manager.get(lang, "properties-header"));
                self.components[selected_component].update_properties(ui, locale_manager, lang);
            }
            Selection::WireSegment(selected_segment) => {
                ui.heading(locale_manager.get(lang, "properties-header"));

                let segment = &mut self.wire_segments[selected_segment];

                ui.horizontal(|ui| {
                    ui.label("X1:");

                    let mut x1_text = format!("{}", segment.point_a[0]);
                    ui.text_edit_singleline(&mut x1_text);
                    if let Ok(new_x1) = x1_text.parse() {
                        segment.point_a[0] = new_x1;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Y1:");

                    let mut y1_text = format!("{}", segment.point_a[1]);
                    ui.text_edit_singleline(&mut y1_text);
                    if let Ok(new_y1) = y1_text.parse() {
                        segment.point_a[1] = new_y1;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("X2:");

                    let mut x2_text = format!("{}", segment.point_b[0]);
                    ui.text_edit_singleline(&mut x2_text);
                    if let Ok(new_x2) = x2_text.parse() {
                        segment.point_b[0] = new_x2;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Y2:");

                    let mut y2_text = format!("{}", segment.point_b[1]);
                    ui.text_edit_singleline(&mut y2_text);
                    if let Ok(new_y2) = y2_text.parse() {
                        segment.point_b[1] = new_y2;
                    }
                });
            }
        }
    }
}
