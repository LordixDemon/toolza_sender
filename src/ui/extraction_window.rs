//! Окно для отображения процесса распаковки на лету

use crate::app::App;
use toolza_sender::utils::format_size;
use eframe::egui;

impl App {
    /// Отобразить окно распаковки
    pub fn render_extraction_window(&mut self, ctx: &egui::Context) {
        if !self.extraction_window_open {
            return;
        }
        
        egui::Window::new("📦 Распаковка архива")
            .collapsible(false)
            .resizable(true)
            .default_size([500.0, 300.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.heading(&self.extraction_filename);
                    ui.add_space(10.0);
                    
                    ui.separator();
                    ui.add_space(10.0);
                    
                    // Текущий файл
                    if !self.extraction_current_file.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Текущий файл:");
                            ui.label(egui::RichText::new(&self.extraction_current_file).monospace());
                        });
                        ui.add_space(5.0);
                    }
                    
                    // Статистика
                    ui.horizontal(|ui| {
                        ui.label("Распаковано файлов:");
                        ui.label(egui::RichText::new(format!("{}", self.extraction_files_count)).strong());
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Общий размер:");
                        ui.label(egui::RichText::new(format_size(self.extraction_total_size)).strong());
                    });
                    
                    ui.add_space(10.0);
                    
                    // Индикатор прогресса
                    ui.spinner();
                    ui.label(egui::RichText::new("Распаковка в процессе...").color(egui::Color32::YELLOW));
                });
            });
    }
}
