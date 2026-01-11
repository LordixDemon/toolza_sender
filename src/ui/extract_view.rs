//! UI для локальной распаковки архивов

use crate::app::App;
use toolza_sender::utils::truncate_string;
use eframe::egui;

impl App {
    /// Рендерим режим распаковки
    pub fn render_extract_mode(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        
        ui.heading(t.extract_title);
        ui.add_space(10.0);
        
        // Информация о поддерживаемых форматах
        ui.label(egui::RichText::new(t.supported_formats).color(egui::Color32::GRAY));
        ui.add_space(10.0);
        
        ui.separator();
        ui.add_space(10.0);
        
        // === Выбор архива ===
        ui.horizontal(|ui| {
            ui.label(t.archive_path);
            
            if let Some(ref path) = self.extract_archive_path {
                let path_str = path.display().to_string();
                let short_path = truncate_string(&path_str, 50);
                ui.label(egui::RichText::new(short_path).monospace());
            } else {
                ui.label(egui::RichText::new(t.no_archive_selected).color(egui::Color32::GRAY).italics());
            }
        });
        
        ui.add_space(5.0);
        
        if ui.button(t.select_archive).clicked() && !self.extract_running {
            self.select_archive_dialog();
        }
        
        ui.add_space(15.0);
        
        // === Папка назначения ===
        ui.horizontal(|ui| {
            ui.label(t.extract_destination);
            
            let dest_str = self.extract_destination.display().to_string();
            let short_dest = truncate_string(&dest_str, 50);
            ui.label(egui::RichText::new(short_dest).monospace());
        });
        
        ui.add_space(5.0);
        
        if ui.button(t.choose).clicked() && !self.extract_running {
            self.select_extract_destination_dialog();
        }
        
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);
        
        // === Кнопки распаковки/остановки ===
        ui.horizontal(|ui| {
            if self.extract_running {
                // Кнопка остановки
                if ui.button(t.stop).clicked() {
                    self.stop_extraction();
                }
                ui.spinner();
                ui.label(egui::RichText::new("Распаковка...").color(egui::Color32::YELLOW));
            } else {
                // Кнопка распаковки
                let can_extract = self.extract_archive_path.is_some();
                if ui.add_enabled(can_extract, egui::Button::new(t.start_extraction)).clicked() {
                    self.start_local_extraction();
                }
            }
        });
        
        // === Результат ===
        if let Some(ref result) = self.extract_result {
            ui.add_space(15.0);
            ui.separator();
            ui.add_space(10.0);
            
            let color = if result.starts_with("✅") {
                egui::Color32::from_rgb(100, 200, 100)
            } else if result.starts_with("❌") {
                egui::Color32::from_rgb(200, 100, 100)
            } else {
                egui::Color32::WHITE
            };
            
            ui.label(egui::RichText::new(result).color(color));
        }
    }
}
