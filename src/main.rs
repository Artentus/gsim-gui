#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn wgpu_config() -> eframe::egui_wgpu::WgpuConfiguration {
    eframe::egui_wgpu::WgpuConfiguration {
        supported_backends: wgpu::Backends::PRIMARY, // No GL because we need compute
        power_preference: wgpu::PowerPreference::LowPower, // An editor is expected to not eat through your battery
        ..Default::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions {
        wgpu_options: wgpu_config(),
        ..Default::default()
    };

    eframe::run_native(
        "Gsim",
        native_options,
        Box::new(|cc| Box::new(gsim_gui::App::new(cc))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    let web_options = eframe::WebOptions {
        wgpu_options: wgpu_config(),
        ..Default::default()
    };

    wasm_bindgen_futures::spawn_local(async {
        let runner = eframe::WebRunner::new();
        runner
            .start(
                "app_canvas",
                web_options,
                Box::new(|cc| Box::new(gsim_gui::App::new(cc))),
            )
            .await
            .expect("failed to start eframe");
    });
}
