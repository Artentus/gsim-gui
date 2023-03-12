use super::component::*;
use super::locale::*;
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
pub struct Circuit {
    name: String,
    offset: [f32; 2],
    linear_zoom: f32,
    zoom: f32,
    components: Vec<Component>,
    #[serde(skip)]
    selected_component: Option<usize>,
}

impl Circuit {
    pub fn new() -> Self {
        Self {
            name: "New Circuit".to_owned(),
            offset: [0.0; 2],
            linear_zoom: zoom_to_linear(DEFAULT_ZOOM),
            zoom: DEFAULT_ZOOM,
            components: vec![],
            selected_component: None,
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
        self.selected_component = Some(self.components.len());
        self.components.push(Component::new(kind));
    }

    pub fn update_component_properties<'a>(
        &mut self,
        ui: &mut egui::Ui,
        locale_manager: &LocaleManager,
        lang: &LangId,
    ) {
        if let Some(selected_component) = self.selected_component {
            self.components[selected_component].update_properties(ui, locale_manager, lang);
        }
    }
}
