use egui::*;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[macro_use]
mod theme;
use theme::*;

mod locale;
use locale::*;

#[inline]
fn show_themed_image_button(
    image: &ThemedImage,
    ctx: &Context,
    theme: Theme,
    ui: &mut Ui,
) -> Response {
    ImageButton::new(image.texture_id(ctx, theme), image.size_vec2()).ui(ui)
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct AppState {
    theme: Theme,
    lang: LangId,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            lang: DEFAULT_LANG,
        }
    }
}

pub struct App {
    state: AppState,
    locale_manager: LocaleManager,
    next_visuals: Option<Visuals>,
    theme_image: ThemedImage,
    and_gate_image: ThemedImage,
    nand_gate_image: ThemedImage,
    or_gate_image: ThemedImage,
    nor_gate_image: ThemedImage,
    xor_gate_image: ThemedImage,
    xnor_gate_image: ThemedImage,
    not_gate_image: ThemedImage,
    buffer_image: ThemedImage,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let state: AppState = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        match state.theme {
            Theme::Light => cc.egui_ctx.set_visuals(Visuals::light()),
            Theme::Dark => cc.egui_ctx.set_visuals(Visuals::dark()),
        }

        Self {
            state,
            locale_manager: LocaleManager::init(),
            next_visuals: None,
            theme_image: themed_image!(SwitchTheme.svg),
            and_gate_image: themed_image!(AndGate.svg),
            nand_gate_image: themed_image!(NandGate.svg),
            or_gate_image: themed_image!(OrGate.svg),
            nor_gate_image: themed_image!(NorGate.svg),
            xor_gate_image: themed_image!(XorGate.svg),
            xnor_gate_image: themed_image!(XnorGate.svg),
            not_gate_image: themed_image!(NotGate.svg),
            buffer_image: themed_image!(Buffer.svg),
        }
    }

    #[inline]
    fn get_localized<'a>(&'a self, key: &'static str) -> Cow<'a, str> {
        self.locale_manager.get(&self.state.lang, key)
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if let Some(visuals) = self.next_visuals.take() {
            ctx.set_visuals(visuals);
        }

        TopBottomPanel::top("main_menu").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.menu_button(self.get_localized("file-menu-item"), |ui| {
                    if ui.button(self.get_localized("new-menu-item")).clicked() {}

                    if ui.button(self.get_localized("open-menu-item")).clicked() {}
                });

                let lang_item_title = self.get_localized("language-menu-item").into_owned();
                ui.menu_button(lang_item_title, |ui| {
                    for lang in self.locale_manager.langs() {
                        let english_name = self.locale_manager.get(lang, "english-lang-name");
                        let native_name = self.locale_manager.get(lang, "native-lang-name");

                        ui.radio_value(
                            &mut self.state.lang,
                            lang.clone(),
                            format!("{native_name} ({english_name})"),
                        );
                    }
                });
            });
        });

        TopBottomPanel::top("tool_bar").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let target_theme_name = match self.state.theme {
                        Theme::Light => self.get_localized("dark-theme-name"),
                        Theme::Dark => self.get_localized("light-theme-name"),
                    };

                    if show_themed_image_button(&self.theme_image, ctx, self.state.theme, ui)
                        .on_hover_text(target_theme_name)
                        .clicked()
                    {
                        match self.state.theme {
                            Theme::Light => {
                                self.state.theme = Theme::Dark;
                                self.next_visuals = Some(Visuals::dark());
                            }
                            Theme::Dark => {
                                self.state.theme = Theme::Light;
                                self.next_visuals = Some(Visuals::light());
                            }
                        }
                    }
                });
            });
        });

        SidePanel::left("component_picker").show(ctx, |ui| {
            ui.heading(self.get_localized("logic-header"));

            ui.horizontal(|ui| {
                if show_themed_image_button(&self.and_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(self.get_localized("and-gate-tool-tip"))
                    .clicked()
                {}

                if show_themed_image_button(&self.nand_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(self.get_localized("nand-gate-tool-tip"))
                    .clicked()
                {}
            });

            ui.horizontal(|ui| {
                if show_themed_image_button(&self.or_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(self.get_localized("or-gate-tool-tip"))
                    .clicked()
                {}

                if show_themed_image_button(&self.nor_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(self.get_localized("nor-gate-tool-tip"))
                    .clicked()
                {}
            });

            ui.horizontal(|ui| {
                if show_themed_image_button(&self.xor_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(self.get_localized("xor-gate-tool-tip"))
                    .clicked()
                {}

                if show_themed_image_button(&self.xnor_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(self.get_localized("xnor-gate-tool-tip"))
                    .clicked()
                {}
            });

            ui.horizontal(|ui| {
                if show_themed_image_button(&self.buffer_image, ctx, self.state.theme, ui)
                    .on_hover_text(self.get_localized("buffer-tool-tip"))
                    .clicked()
                {}

                if show_themed_image_button(&self.not_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(self.get_localized("not-gate-tool-tip"))
                    .clicked()
                {}
            });
        });

        SidePanel::right("property_view").show(ctx, |ui| {
            ui.heading(self.get_localized("properties-header"));

            ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                warn_if_debug_build(ui);
            })
        });

        CentralPanel::default().show(ctx, |ui| {});
    }
}
