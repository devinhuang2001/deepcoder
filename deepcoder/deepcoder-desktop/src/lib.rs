pub mod app;
pub mod runner;
pub mod state;

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("DeepCoder")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DeepCoder",
        options,
        Box::new(|cc| Ok(Box::new(app::DesktopApp::new(cc)))),
    )
}
