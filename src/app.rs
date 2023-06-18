use egui::*;
use serde::{Deserialize, Serialize};
use std::cell::OnceCell;

mod math;
use math::*;

#[macro_use]
mod theme;
use theme::*;

mod locale;
use locale::*;

mod component;
use component::*;

mod circuit;
use circuit::*;

mod viewport;
use viewport::*;

mod file_dialog;
use file_dialog::*;

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
    file_dialog: OnceCell<FileDialog>,

    theme_image: ThemedImage,
    and_gate_image: ThemedImage,
    nand_gate_image: ThemedImage,
    or_gate_image: ThemedImage,
    nor_gate_image: ThemedImage,
    xor_gate_image: ThemedImage,
    xnor_gate_image: ThemedImage,
    not_gate_image: ThemedImage,
    buffer_image: ThemedImage,

    viewport: Option<Viewport>,

    circuits: Vec<Circuit>,
    selected_circuit: Option<usize>,
    drag_mode: DragMode,
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
            file_dialog: OnceCell::new(),

            theme_image: themed_image!(SwitchTheme.svg),
            and_gate_image: themed_image!(AndGate.svg),
            nand_gate_image: themed_image!(NandGate.svg),
            or_gate_image: themed_image!(OrGate.svg),
            nor_gate_image: themed_image!(NorGate.svg),
            xor_gate_image: themed_image!(XorGate.svg),
            xnor_gate_image: themed_image!(XnorGate.svg),
            not_gate_image: themed_image!(NotGate.svg),
            buffer_image: themed_image!(Buffer.svg),

            viewport: None,

            circuits: vec![],
            selected_circuit: None,
            drag_mode: DragMode::default(),
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        if let Some(visuals) = self.next_visuals.take() {
            ctx.set_visuals(visuals);
        }

        let Some(file_dialog) = self.file_dialog.get_mut() else {
            if let Some(file_dialog) = FileDialog::new() {
                let _ = self.file_dialog.set(file_dialog);
            }
            return;
        };

        #[cfg(not(target_arch = "wasm32"))]
        if let Some((file_name, data)) = file_dialog.get() {
            let mut circuit = Circuit::deserialize(&data).expect("error opening file");
            circuit.set_file_name(file_name);

            self.selected_circuit = Some(self.circuits.len());
            self.circuits.push(circuit);
        }

        #[cfg(target_arch = "wasm32")]
        if let Some(data) = file_dialog.get() {
            let circuit = Circuit::deserialize(&data).expect("error opening file");

            self.selected_circuit = Some(self.circuits.len());
            self.circuits.push(circuit);
        }

        TopBottomPanel::top("main_menu").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.menu_button(
                    self.locale_manager.get(&self.state.lang, "file-menu-item"),
                    |ui| {
                        if ui
                            .button(self.locale_manager.get(&self.state.lang, "new-menu-item"))
                            .clicked()
                        {
                            self.selected_circuit = Some(self.circuits.len());
                            self.circuits.push(Circuit::new());
                        }

                        if ui
                            .button(self.locale_manager.get(&self.state.lang, "open-menu-item"))
                            .clicked()
                        {
                            file_dialog.open();
                        }

                        if let Some(circuit) = self.selected_circuit.map(|i| &mut self.circuits[i])
                        {
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                if ui
                                    .button(
                                        self.locale_manager.get(&self.state.lang, "save-menu-item"),
                                    )
                                    .clicked()
                                {
                                    if let Some(file_name) = circuit.file_name() {
                                        std::fs::write(file_name, &Circuit::serialize(circuit))
                                            .expect("error saving file");
                                        circuit.set_file_name(file_name.to_owned());
                                    } else {
                                        if let Some(file_name) = file_dialog
                                            .save(None, &Circuit::serialize(circuit))
                                            .expect("error saving file")
                                        {
                                            circuit.set_file_name(file_name);
                                        }
                                    }
                                }

                                if ui
                                    .button(
                                        self.locale_manager
                                            .get(&self.state.lang, "save-as-menu-item"),
                                    )
                                    .clicked()
                                {
                                    if let Some(file_name) = file_dialog
                                        .save(circuit.file_name(), &Circuit::serialize(circuit))
                                        .expect("error saving file")
                                    {
                                        circuit.set_file_name(file_name);
                                    }
                                }
                            }

                            #[cfg(target_arch = "wasm32")]
                            {
                                if ui
                                    .button(
                                        self.locale_manager.get(&self.state.lang, "save-menu-item"),
                                    )
                                    .clicked()
                                {
                                    file_dialog.save(circuit.name(), &Circuit::serialize(circuit));
                                }
                            }
                        }
                    },
                );

                ui.menu_button(
                    self.locale_manager
                        .get(&self.state.lang, "language-menu-item"),
                    |ui| {
                        for lang in self.locale_manager.langs() {
                            let english_name = self.locale_manager.get(lang, "english-lang-name");
                            let native_name = self.locale_manager.get(lang, "native-lang-name");

                            ui.radio_value(
                                &mut self.state.lang,
                                lang.clone(),
                                format!("{native_name} ({english_name})"),
                            );
                        }
                    },
                );
            });
        });

        TopBottomPanel::top("tool_bar").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let target_theme_name = match self.state.theme {
                        Theme::Light => {
                            self.locale_manager.get(&self.state.lang, "dark-theme-name")
                        }
                        Theme::Dark => self
                            .locale_manager
                            .get(&self.state.lang, "light-theme-name"),
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
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.drag_mode, DragMode::BoxSelection, "Select");
                ui.radio_value(&mut self.drag_mode, DragMode::DrawWire, "Draw Wires");
            });

            ui.heading(self.locale_manager.get(&self.state.lang, "logic-header"));

            ui.horizontal(|ui| {
                if show_themed_image_button(&self.and_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(
                        self.locale_manager
                            .get(&self.state.lang, "and-gate-tool-tip"),
                    )
                    .clicked()
                {
                    if let Some(selected_circuit) = self.selected_circuit {
                        self.circuits[selected_circuit]
                            .add_component(ComponentKind::AndGate { width: 1 });
                    }
                }

                if show_themed_image_button(&self.nand_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(
                        self.locale_manager
                            .get(&self.state.lang, "nand-gate-tool-tip"),
                    )
                    .clicked()
                {
                    if let Some(selected_circuit) = self.selected_circuit {
                        self.circuits[selected_circuit]
                            .add_component(ComponentKind::NandGate { width: 1 });
                    }
                }
            });

            ui.horizontal(|ui| {
                if show_themed_image_button(&self.or_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(
                        self.locale_manager
                            .get(&self.state.lang, "or-gate-tool-tip"),
                    )
                    .clicked()
                {
                    if let Some(selected_circuit) = self.selected_circuit {
                        self.circuits[selected_circuit]
                            .add_component(ComponentKind::OrGate { width: 1 });
                    }
                }

                if show_themed_image_button(&self.nor_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(
                        self.locale_manager
                            .get(&self.state.lang, "nor-gate-tool-tip"),
                    )
                    .clicked()
                {
                    if let Some(selected_circuit) = self.selected_circuit {
                        self.circuits[selected_circuit]
                            .add_component(ComponentKind::NorGate { width: 1 });
                    }
                }
            });

            ui.horizontal(|ui| {
                if show_themed_image_button(&self.xor_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(
                        self.locale_manager
                            .get(&self.state.lang, "xor-gate-tool-tip"),
                    )
                    .clicked()
                {
                    if let Some(selected_circuit) = self.selected_circuit {
                        self.circuits[selected_circuit]
                            .add_component(ComponentKind::XorGate { width: 1 });
                    }
                }

                if show_themed_image_button(&self.xnor_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(
                        self.locale_manager
                            .get(&self.state.lang, "xnor-gate-tool-tip"),
                    )
                    .clicked()
                {
                    if let Some(selected_circuit) = self.selected_circuit {
                        self.circuits[selected_circuit]
                            .add_component(ComponentKind::XnorGate { width: 1 });
                    }
                }
            });

            ui.horizontal(|ui| {
                if show_themed_image_button(&self.buffer_image, ctx, self.state.theme, ui)
                    .on_hover_text(self.locale_manager.get(&self.state.lang, "buffer-tool-tip"))
                    .clicked()
                {}

                if show_themed_image_button(&self.not_gate_image, ctx, self.state.theme, ui)
                    .on_hover_text(
                        self.locale_manager
                            .get(&self.state.lang, "not-gate-tool-tip"),
                    )
                    .clicked()
                {}
            });
        });

        SidePanel::right("property_view").show(ctx, |ui| {
            if let Some(selected_circuit) = self.selected_circuit {
                self.circuits[selected_circuit].update_component_properties(
                    ui,
                    &self.locale_manager,
                    &self.state.lang,
                );
            }

            ui.with_layout(Layout::bottom_up(Align::RIGHT), |ui| {
                warn_if_debug_build(ui);
            })
        });

        TopBottomPanel::top("tab_headers").show(ctx, |ui| {
            for (i, circuit) in self.circuits.iter().enumerate() {
                let mut selected = self.selected_circuit.map(|sc| i == sc).unwrap_or(false);

                ui.toggle_value(&mut selected, circuit.name());

                if selected {
                    self.selected_circuit = Some(i);
                }
            }
        });

        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let zoom = self
                    .selected_circuit
                    .map(|i| self.circuits[i].zoom())
                    .unwrap_or(DEFAULT_ZOOM);
                ui.label(format!("{:.0}%", zoom * 100.0));
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            let render_state = frame.wgpu_render_state().unwrap();
            let selected_circuit = self.selected_circuit.map(|i| &mut self.circuits[i]);

            let viewport_size = ui.available_size();
            let viewport_width = viewport_size.x.max(1.0) as u32;
            let viewport_height = viewport_size.y.max(1.0) as u32;

            let viewport = if let Some(viewport) = self.viewport.as_mut() {
                viewport.resize(render_state, viewport_width, viewport_height);
                viewport
            } else {
                let viewport = Viewport::create(render_state, viewport_width, viewport_height);
                self.viewport = Some(viewport);
                self.viewport.as_mut().unwrap()
            };

            let background_color: Rgba = ui.visuals().extreme_bg_color.into();
            let grid_color: Rgba = ui.visuals().weak_text_color().into();
            let component_color: Rgba = ui.visuals().text_color().into();
            let selected_component_color: Rgba = ui.visuals().strong_text_color().into();

            viewport.draw(
                render_state,
                selected_circuit.as_deref(),
                ViewportColors {
                    background_color: background_color.to_array(),
                    grid_color: grid_color.to_array(),
                    component_color: component_color.to_array(),
                    selected_component_color: selected_component_color.to_array(),
                },
            );

            let response = Image::new(
                viewport.texture_id(),
                Vec2::new(viewport_width as f32, viewport_height as f32),
            )
            .sense(Sense::click_and_drag())
            .ui(ui);

            if let Some(circuit) = selected_circuit {
                let viewport_rect = response.rect;

                if let Some(pos) = response.interact_pointer_pos() {
                    if viewport_rect.contains(pos) {
                        let mut rel_pos = pos - viewport_rect.min;
                        rel_pos.y = viewport_rect.height() - rel_pos.y;
                        rel_pos -= response.rect.size() * 0.5;

                        if ui.input(|state| state.pointer.button_pressed(PointerButton::Primary)) {
                            circuit.primary_button_pressed(rel_pos.into());
                        } else if ui
                            .input(|state| state.pointer.button_pressed(PointerButton::Secondary))
                        {
                            circuit.secondary_button_pressed(rel_pos.into());
                        }
                    }
                }

                if ui.input(|state| state.key_pressed(Key::R)) {
                    circuit.rotate_selection();
                }

                if ui.input(|state| state.key_pressed(Key::M)) {
                    circuit.mirror_selection();
                }

                if ui.input(|state| state.key_pressed(Key::ArrowUp)) {
                    circuit.move_selection(Vec2i::new(0, 1));
                }

                if ui.input(|state| state.key_pressed(Key::ArrowDown)) {
                    circuit.move_selection(Vec2i::new(0, -1));
                }

                if ui.input(|state| state.key_pressed(Key::ArrowLeft)) {
                    circuit.move_selection(Vec2i::new(-1, 0));
                }

                if ui.input(|state| state.key_pressed(Key::ArrowRight)) {
                    circuit.move_selection(Vec2i::new(1, 0));
                }

                const ZOOM_LEVELS: f32 = 10.0;
                let zoom_delta = ui.input(|state| state.scroll_delta.y) / 120.0;
                circuit.set_linear_zoom(circuit.linear_zoom() + (zoom_delta / ZOOM_LEVELS));

                let mouse_delta = ui.input(|state| state.pointer.delta());
                let mouse_delta = mouse_delta / (circuit.zoom() * BASE_ZOOM);
                let mouse_delta = Vec2f::new(mouse_delta.x, -mouse_delta.y);
                circuit.mouse_moved(mouse_delta, self.drag_mode);

                if response.dragged() {
                    if ui.input(|state| state.pointer.button_down(PointerButton::Middle)) {
                        let offset_delta = response.drag_delta() / (circuit.zoom() * BASE_ZOOM);
                        let new_offset = Vec2f::new(
                            circuit.offset().x - offset_delta.x,
                            circuit.offset().y + offset_delta.y,
                        );
                        circuit.set_offset(new_offset);
                    }
                }

                if let Some(pos) = response.interact_pointer_pos() {
                    if viewport_rect.contains(pos) {
                        let mut rel_pos = pos - viewport_rect.min;
                        rel_pos.y = viewport_rect.height() - rel_pos.y;
                        rel_pos -= response.rect.size() * 0.5;

                        if ui.input(|state| state.pointer.button_released(PointerButton::Primary)) {
                            circuit.primary_button_released(rel_pos.into());
                        } else if ui
                            .input(|state| state.pointer.button_released(PointerButton::Secondary))
                        {
                            circuit.secondary_button_released(rel_pos.into());
                        }
                    }
                }
            }
        });
    }
}
