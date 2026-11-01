#![windows_subsystem = "windows"]

mod fsops;
mod settings;
mod sys;
mod ui;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Nexora Unlocker")
            .with_inner_size([1000.0, 650.0])
            .with_min_inner_size([760.0, 480.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Nexora Unlocker",
        options,
        Box::new(|cc| Ok(Box::new(ui::App::new(cc)))),
    )
}
