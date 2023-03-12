use super::locale::*;
use egui::*;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AnchorKind {
    Input = 0,
    Output = 1,
    BiDirectional = 2,
}

#[derive(Clone, Copy)]
pub struct Anchor {
    pub position: [i32; 2],
    pub kind: AnchorKind,
}

macro_rules! anchors {
    ($($kind:ident($x:literal, $y:literal)),* $(,)?) => {
        smallvec::smallvec![$(
            Anchor {
                position: [$x, $y],
                kind: AnchorKind::$kind,
            },
        )*]
    };
}

#[derive(Serialize, Deserialize)]
pub enum ComponentKind {
    AndGate { width: u8 },
}

impl ComponentKind {
    fn update_properties(&mut self, ui: &mut Ui, locale_manager: &LocaleManager, lang: &LangId) {
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

    fn anchors(&self) -> SmallVec<[Anchor; 3]> {
        match self {
            ComponentKind::AndGate { .. } => anchors![Input(-1, -2), Input(1, -2), Output(0, 2)],
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum Rotation {
    #[default]
    Deg0 = 0,
    Deg90 = 1,
    Deg180 = 2,
    Deg270 = 3,
}

impl Rotation {
    const ALL: [Rotation; 4] = [
        Rotation::Deg0,
        Rotation::Deg90,
        Rotation::Deg180,
        Rotation::Deg270,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Rotation::Deg0 => "0°",
            Rotation::Deg90 => "90°",
            Rotation::Deg180 => "180°",
            Rotation::Deg270 => "270°",
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

    pub fn anchors(&self) -> SmallVec<[Anchor; 3]> {
        let mut anchors = self.kind.anchors();
        for anchor in anchors.iter_mut() {
            if self.mirrored {
                anchor.position[0] = -anchor.position[0];
            }

            anchor.position = match self.rotation {
                Rotation::Deg0 => anchor.position,
                Rotation::Deg90 => [anchor.position[1], -anchor.position[0]],
                Rotation::Deg180 => [-anchor.position[0], -anchor.position[1]],
                Rotation::Deg270 => [-anchor.position[1], anchor.position[0]],
            };

            anchor.position[0] += self.position[0];
            anchor.position[1] += self.position[1];
        }
        anchors
    }

    pub fn update_properties(
        &mut self,
        ui: &mut Ui,
        locale_manager: &LocaleManager,
        lang: &LangId,
    ) {
        ui.heading(locale_manager.get(lang, "properties-header"));
        self.kind.update_properties(ui, locale_manager, lang);

        ui.horizontal(|ui| {
            ui.label(locale_manager.get(lang, "rotation-property-name"));

            ComboBox::from_id_source("rotation_property")
                .selected_text(self.rotation.as_str())
                .show_ui(ui, |ui| {
                    for rot in Rotation::ALL {
                        ui.selectable_value(&mut self.rotation, rot, rot.as_str());
                    }
                });
        });

        ui.checkbox(
            &mut self.mirrored,
            locale_manager.get(lang, "mirrored-property-name"),
        );
    }
}
