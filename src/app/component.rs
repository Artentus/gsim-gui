use crate::app::locale::*;
use crate::app::math::*;
use crate::app::UiExt;
use egui::*;
use gsim::Id;
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};
use std::num::NonZeroU8;

use super::NumericTextValue;

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
        smallvec![$(
            Anchor {
                position: Vec2i::new($x, $y),
                kind: AnchorKind::$kind,
            },
        )*]
    };
}

#[allow(clippy::enum_variant_names)]
#[derive(Serialize, Deserialize)]
pub enum ComponentKind {
    AndGate { width: NumericTextValue<NonZeroU8> },
    OrGate { width: NumericTextValue<NonZeroU8> },
    XorGate { width: NumericTextValue<NonZeroU8> },
    NandGate { width: NumericTextValue<NonZeroU8> },
    NorGate { width: NumericTextValue<NonZeroU8> },
    XnorGate { width: NumericTextValue<NonZeroU8> },
}

impl ComponentKind {
    pub fn new_and_gate() -> Self {
        Self::AndGate {
            width: NumericTextValue::new(NonZeroU8::MIN),
        }
    }

    pub fn new_or_gate() -> Self {
        Self::OrGate {
            width: NumericTextValue::new(NonZeroU8::MIN),
        }
    }

    pub fn new_xor_gate() -> Self {
        Self::XorGate {
            width: NumericTextValue::new(NonZeroU8::MIN),
        }
    }

    pub fn new_nand_gate() -> Self {
        Self::NandGate {
            width: NumericTextValue::new(NonZeroU8::MIN),
        }
    }

    pub fn new_nor_gate() -> Self {
        Self::NorGate {
            width: NumericTextValue::new(NonZeroU8::MIN),
        }
    }

    pub fn new_xnor_gate() -> Self {
        Self::XnorGate {
            width: NumericTextValue::new(NonZeroU8::MIN),
        }
    }

    fn anchors(&self) -> SmallVec<[Anchor; 3]> {
        match self {
            ComponentKind::AndGate { .. }
            | ComponentKind::OrGate { .. }
            | ComponentKind::XorGate { .. } => {
                anchors![Input(-1, -2), Input(1, -2), Output(0, 2)]
            }
            ComponentKind::NandGate { .. }
            | ComponentKind::NorGate { .. }
            | ComponentKind::XnorGate { .. } => {
                anchors![Input(-1, -2), Input(1, -2), Output(0, 3)]
            }
        }
    }

    fn bounding_box(&self) -> Rectangle {
        match self {
            ComponentKind::AndGate { .. }
            | ComponentKind::OrGate { .. }
            | ComponentKind::XorGate { .. }
            | ComponentKind::NandGate { .. }
            | ComponentKind::NorGate { .. }
            | ComponentKind::XnorGate { .. } => Rectangle {
                top: 2.0,
                bottom: -2.0,
                left: -2.0,
                right: 2.0,
            },
        }
    }

    fn update_properties(
        &mut self,
        ui: &mut Ui,
        locale_manager: &LocaleManager,
        lang: &LangId,
    ) -> bool {
        match self {
            ComponentKind::AndGate { width }
            | ComponentKind::OrGate { width }
            | ComponentKind::XorGate { width }
            | ComponentKind::NandGate { width }
            | ComponentKind::NorGate { width }
            | ComponentKind::XnorGate { width } => {
                ui.horizontal(|ui| {
                    ui.label(locale_manager.get(lang, "bit-width-property-name"));
                    ui.numeric_text_edit(width).lost_focus()
                })
                .inner
            }
        }
    }

    pub fn name(&self) -> std::borrow::Cow<'static, str> {
        match self {
            ComponentKind::AndGate { .. } => "AND".into(),
            ComponentKind::OrGate { .. } => "OR".into(),
            ComponentKind::XorGate { .. } => "XOR".into(),
            ComponentKind::NandGate { .. } => "NAND".into(),
            ComponentKind::NorGate { .. } => "NOR".into(),
            ComponentKind::XnorGate { .. } => "XNOR".into(),
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

    pub fn mirror(self) -> Self {
        match self {
            Rotation::Deg0 => Rotation::Deg0,
            Rotation::Deg90 => Rotation::Deg270,
            Rotation::Deg180 => Rotation::Deg180,
            Rotation::Deg270 => Rotation::Deg90,
        }
    }

    pub fn radians(self) -> f64 {
        match self {
            Rotation::Deg0 => 0.0,
            Rotation::Deg90 => std::f64::consts::FRAC_PI_2,
            Rotation::Deg180 => std::f64::consts::PI,
            Rotation::Deg270 => 3.0 * std::f64::consts::FRAC_PI_2,
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
    pub position_x: NumericTextValue<i32>,
    pub position_y: NumericTextValue<i32>,
    pub rotation: Rotation,
    pub mirrored: bool,
    #[serde(skip)]
    pub sim_component: gsim::ComponentId,
}

impl Component {
    pub fn new(kind: ComponentKind) -> Self {
        Self {
            kind,
            position_x: NumericTextValue::new(0),
            position_y: NumericTextValue::new(0),
            rotation: Rotation::default(),
            mirrored: false,
            sim_component: gsim::ComponentId::INVALID,
        }
    }

    #[inline]
    pub fn position(&self) -> Vec2i {
        Vec2i::new(*self.position_x.get(), *self.position_y.get())
    }

    #[inline]
    pub fn set_position(&mut self, new_position: Vec2i) {
        self.position_x.set(new_position.x);
        self.position_y.set(new_position.y);
    }

    pub fn anchors(&self) -> SmallVec<[Anchor; 3]> {
        let mut anchors = self.kind.anchors();
        for anchor in anchors.iter_mut() {
            if self.mirrored {
                anchor.position.x = -anchor.position.x;
            }

            anchor.position = match self.rotation {
                Rotation::Deg0 => anchor.position,
                Rotation::Deg90 => Vec2i::new(-anchor.position.y, anchor.position.x),
                Rotation::Deg180 => -anchor.position,
                Rotation::Deg270 => Vec2i::new(anchor.position.y, -anchor.position.x),
            };

            anchor.position.x += *self.position_x.get();
            anchor.position.y += *self.position_y.get();
        }
        anchors
    }

    pub fn bounding_box(&self) -> Rectangle {
        let mut bb = self.kind.bounding_box();

        if self.mirrored {
            std::mem::swap(&mut bb.left, &mut bb.right);
            bb.left = -bb.left;
            bb.right = -bb.right;
        }

        bb = match self.rotation {
            Rotation::Deg0 => bb,
            Rotation::Deg90 => Rectangle {
                top: bb.right,
                bottom: bb.left,
                left: -bb.top,
                right: -bb.bottom,
            },
            Rotation::Deg180 => Rectangle {
                top: -bb.bottom,
                bottom: -bb.top,
                left: -bb.right,
                right: -bb.left,
            },
            Rotation::Deg270 => Rectangle {
                top: -bb.left,
                bottom: -bb.right,
                left: bb.bottom,
                right: bb.top,
            },
        };

        bb.top += self.position().y as f32;
        bb.bottom += self.position().y as f32;
        bb.left += self.position().x as f32;
        bb.right += self.position().x as f32;

        bb
    }

    pub fn update_properties(
        &mut self,
        ui: &mut Ui,
        locale_manager: &LocaleManager,
        lang: &LangId,
    ) -> bool {
        let mut requires_redraw = self.kind.update_properties(ui, locale_manager, lang);

        ui.horizontal(|ui| {
            ui.label("X:");
            requires_redraw |= ui.numeric_text_edit(&mut self.position_x).lost_focus();
        });

        ui.horizontal(|ui| {
            ui.label("Y:");
            requires_redraw |= ui.numeric_text_edit(&mut self.position_y).lost_focus();
        });

        ui.horizontal(|ui| {
            ui.label(locale_manager.get(lang, "rotation-property-name"));

            ComboBox::from_id_source("rotation_property")
                .selected_text(self.rotation.as_str())
                .show_ui(ui, |ui| {
                    let mut rotation = self.rotation;

                    for rot in Rotation::ALL {
                        ui.selectable_value(&mut rotation, rot, rot.as_str());
                    }

                    if rotation != self.rotation {
                        self.rotation = rotation;
                        requires_redraw = true;
                    }
                });
        });

        let mut mirrored = self.mirrored;
        ui.checkbox(
            &mut mirrored,
            locale_manager.get(lang, "mirrored-property-name"),
        );

        if mirrored != self.mirrored {
            self.mirrored = mirrored;
            self.rotation = self.rotation.mirror();
            requires_redraw = true;
        }

        requires_redraw
    }
}
