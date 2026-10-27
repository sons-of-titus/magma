use eframe::egui;
use magma::kernel::boot::{self, BootOptions};
use magma_gui::render::gui::GuiApp;
#[cfg(feature = "janet")]
use scripting;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "janet")]
    let rt = {
        let r = scripting::JanetRuntime::new();
        // init happens in bootstrap; construct here only
        Some(Box::new(r) as Box<dyn magma::kernel::script_runtime::ScriptRuntime>)
    };
    #[cfg(not(feature = "janet"))]
    let rt = None;

    let boot = boot::bootstrap(BootOptions {
        interactive: true,
        reopen_last_project: true,
    }, rt)?;

    let result = run_gui(boot.editor, boot.bg_receiver);
    boot::shutdown(boot.runtime);
    result
}

fn run_gui(
    editor: magma::kernel::state::Editor,
    bg_receiver: tokio::sync::mpsc::UnboundedReceiver<magma::kernel::runtime::BackgroundEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Magma")
            .with_inner_size([800.0, 640.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Magma",
        options,
        Box::new(|_cc| Ok(Box::new(GuiApp::new_with_bg(editor, bg_receiver)))),
    )
    .map_err(|e| -> Box<dyn std::error::Error> { format!("GUI error: {e}").into() })?;
    Ok(())
}
