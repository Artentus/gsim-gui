use super::locale::*;
use crate::app::math::*;
use egui::*;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AnchorKind {
    Input = 0,
    Output = 1,
    BiDirectional = 2,
    Passive = 3,
}

#[derive(Clone, Copy)]
pub struct Anchor {
    pub position: Vec2i,
    pub kind: AnchorKind,
}

macro_rules! anchors {
    ($($kind:ident($x:literal, $y:literal)),* $(,)?) => {
        smallvec::smallvec![$(
            Anchor {
                position: Vec2i::new($x, $y),
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
    fn anchors(&self) -> SmallVec<[Anchor; 3]> {
        match self {
            ComponentKind::AndGate { .. } => anchors![Input(-1, -2), Input(1, -2), Output(0, 2)],
        }
    }

    fn bounding_box(&self) -> BoundingBox {
        match self {
            ComponentKind::AndGate { .. } => BoundingBox {
                top: 2.0,
                bottom: -2.0,
                left: -2.0,
                right: 2.0,
            },
        }
    }

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
            }
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

    pub fn next(self) -> Self {
        match self {
            Rotation::Deg0 => Rotation::Deg90,
            Rotation::Deg90 => Rotation::Deg180,
            Rotation::Deg180 => Rotation::Deg270,
            Rotation::Deg270 => Rotation::Deg0,
        }
    }

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
    pub position: Vec2i,
    pub rotation: Rotation,
    pub mirrored: bool,
}

impl Component {
    pub fn new(kind: ComponentKind) -> Self {
        Self {
            kind,
            position: Vec2i::default(),
            rotation: Rotation::default(),
            mirrored: false,
        }
    }

    pub fn anchors(&self) -> SmallVec<[Anchor; 3]> {
        let mut anchors = self.kind.anchors();
        for anchor in anchors.iter_mut() {
            anchor.position = match self.rotation {
                Rotation::Deg0 => anchor.position,
                Rotation::Deg90 => {
                    if self.mirrored {
                        Vec2i::new(-anchor.position.y, anchor.position.x)
                    } else {
                        Vec2i::new(anchor.position.y, -anchor.position.x)
                    }
                }
                Rotation::Deg180 => -anchor.position,
                Rotation::Deg270 => {
                    if self.mirrored {
                        Vec2i::new(anchor.position.y, -anchor.position.x)
                    } else {
                        Vec2i::new(-anchor.position.y, anchor.position.x)
                    }
                }
            };

            if self.mirrored {
                anchor.position.x = -anchor.position.x;
            }

            anchor.position += self.position;
        }
        anchors
    }

    pub fn bounding_box(&self) -> BoundingBox {
        let mut bb = self.kind.bounding_box();

        bb = match self.rotation {
            Rotation::Deg0 => bb,
            Rotation::Deg90 => {
                if self.mirrored {
                    BoundingBox {
                        top: bb.right,
                        bottom: bb.left,
                        left: -bb.top,
                        right: -bb.bottom,
                    }
                } else {
                    BoundingBox {
                        top: -bb.left,
                        bottom: -bb.right,
                        left: bb.bottom,
                        right: bb.top,
                    }
                }
            }
            Rotation::Deg180 => BoundingBox {
                top: -bb.bottom,
                bottom: -bb.top,
                left: -bb.right,
                right: -bb.left,
            },
            Rotation::Deg270 => {
                if self.mirrored {
                    BoundingBox {
                        top: -bb.left,
                        bottom: -bb.right,
                        left: bb.bottom,
                        right: bb.top,
                    }
                } else {
                    BoundingBox {
                        top: bb.right,
                        bottom: bb.left,
                        left: -bb.top,
                        right: -bb.bottom,
                    }
                }
            }
        };

        if self.mirrored {
            std::mem::swap(&mut bb.left, &mut bb.right);
            bb.left = -bb.left;
            bb.right = -bb.right;
        }

        bb.top += self.position.y as f32;
        bb.bottom += self.position.y as f32;
        bb.left += self.position.x as f32;
        bb.right += self.position.x as f32;

        bb
    }

    pub fn update_properties(
        &mut self,
        ui: &mut Ui,
        locale_manager: &LocaleManager,
        lang: &LangId,
    ) {
        self.kind.update_properties(ui, locale_manager, lang);

        ui.horizontal(|ui| {
            ui.label("X:");

            let mut x_text = format!("{}", self.position.x);
            ui.text_edit_singleline(&mut x_text);
            if let Ok(new_x) = x_text.parse() {
                self.position.x = new_x;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Y:");

            let mut y_text = format!("{}", self.position.y);
            ui.text_edit_singleline(&mut y_text);
            if let Ok(new_y) = y_text.parse() {
                self.position.y = new_y;
            }
        });

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
