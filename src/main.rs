
mod app;
mod hid;
mod mapping;
mod preview;
mod profiles;
mod settings;
mod theme;
mod widgets;

use app::ControllerApp;
use eframe::egui;
use std::path::PathBuf;
use theme::apply_style;

fn load_app_icon() -> egui::IconData {
    let image = image::load_from_memory(include_bytes!(
        "../assets/DualLink-icon.png"
    ))
    .expect("Failed to load DualLink application icon")
    .into_rgba8();

    let (width, height) = image.dimensions();

    egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}

fn main() -> eframe::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mut screenshot_path = None;

    while let Some(argument) = args.next() {
        if argument == "--screenshot" {
            let Some(path) = args.next() else {
                eprintln!("Expected a file name after --screenshot");
                std::process::exit(2);
            };

            screenshot_path = Some(PathBuf::from(path));
        }
    }

    let screenshot_mode = screenshot_path.is_some();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1050.0, 680.0])
        .with_icon(load_app_icon());

    if screenshot_mode {
        viewport = viewport.with_active(false);

        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(
                egui::viewport::WindowLevel::AlwaysOnBottom,
            );
        }
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "DualLink Controller Studio",
        options,
        Box::new(move |creation_context| {
            apply_style(&creation_context.egui_ctx);

            Ok(Box::new(ControllerApp::new(screenshot_path)))
        }),
    )
}
