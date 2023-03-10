use super::component::*;
use super::locale::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Circuit {
    name: String,
    offset: [f32; 2],
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
            zoom: 1.0,
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
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    #[inline]
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.5, 4.0);
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
            self.components[selected_component]
                .kind
                .update_properties(ui, locale_manager, lang);
        }
    }

    pub fn draw(&self) {}
}
