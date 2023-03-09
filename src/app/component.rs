use super::locale::*;
use egui::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum ComponentKind {
    AndGate { width: u8 },
}

impl ComponentKind {
    pub fn update_properties(
        &mut self,
        ui: &mut Ui,
        locale_manager: &LocaleManager,
        lang: &LangId,
    ) {
        ui.heading(locale_manager.get(lang, "properties-header"));

        match self {
            ComponentKind::AndGate { width } => {
                ui.horizontal(|ui| {
                    ui.label(locale_manager.get(lang, "bit-width-property-name"));

                    let mut width_text = format!("{width}");
                    ui.text_edit_singleline(&mut width_text);
                    if let Ok(new_width) = width_text.parse::<u8>() {
                        *width = new_width;
                    }
                });

                //ui.with_layout(Layout::top_down(Align::Max), |ui| {
                //    if ui
                //        .button(locale_manager.get(lang, "reset-to-default-action"))
                //        .clicked()
                //    {
                //        *width = 1;
                //    }
                //});
            }
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rotation {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl Rotation {
    pub fn to_radians(self) -> f32 {
        match self {
            Rotation::Deg0 => 0.0,
            Rotation::Deg90 => std::f32::consts::FRAC_PI_2,
            Rotation::Deg180 => std::f32::consts::PI,
            Rotation::Deg270 => 3.0 * std::f32::consts::FRAC_PI_2,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Component {
    pub kind: ComponentKind,
    pub position: [i32; 2],
    pub rotation: Rotation,
    pub mirrored: bool,
}

impl Component {
    pub fn new(kind: ComponentKind) -> Self {
        Self {
            kind,
            position: [0; 2],
            rotation: Rotation::default(),
            mirrored: false,
        }
    }
}
