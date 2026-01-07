//! Toolza Sender GUI - графический интерфейс для передачи файлов

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ui;

use app::App;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Toolza Sender - Передача файлов",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
