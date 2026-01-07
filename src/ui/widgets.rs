//! Общие виджеты UI

use crate::app::{App, TargetStatus};
use toolza_sender::protocol::FileStatus;
use toolza_sender::utils::format_size;
use eframe::egui;

impl App {
    /// Отрисовать список получателей
    pub fn render_targets_list(&mut self, ui: &mut egui::Ui) {
        if self.targets.is_empty() {
            return;
        }
        
        egui::ScrollArea::vertical()
            .id_salt("targets_scroll")
            .max_height(100.0)
            .show(ui, |ui| {
                let mut to_remove = None;
                
                for (idx, target) in self.targets.iter().enumerate() {
                    ui.horizontal(|ui| {
                        // Иконка статуса
                        let (icon, color) = match &target.status {
                            TargetStatus::Pending => ("⏳", egui::Color32::GRAY),
                            TargetStatus::Connecting => ("🔄", egui::Color32::YELLOW),
                            TargetStatus::Transferring => ("📤", egui::Color32::LIGHT_BLUE),
                            TargetStatus::Completed => ("✅", egui::Color32::GREEN),
                            TargetStatus::Error(_) => ("❌", egui::Color32::RED),
                        };
                        ui.colored_label(color, icon);
                        ui.label(&target.address);
                        
                        // Прогресс для активных
                        if target.status == TargetStatus::Transferring {
                            let progress = target.files_completed as f32 / self.files.len().max(1) as f32;
                            ui.add(
                                egui::ProgressBar::new(progress)
                                    .desired_width(80.0)
                                    .text(format!("{}/{}", target.files_completed, self.files.len())),
                            );
                        }
                        
                        // Информация об ошибке
                        if let TargetStatus::Error(e) = &target.status {
                            ui.colored_label(egui::Color32::RED, e);
                        }
                        
                        // Кнопка удаления
                        if self.can_edit() && ui.small_button("❌").clicked() {
                            to_remove = Some(idx);
                        }
                    });
                }
                
                if let Some(idx) = to_remove {
                    self.targets.remove(idx);
                }
            });
    }
    
    /// Отрисовать список файлов для отправки
    pub fn render_files_list(&mut self, ui: &mut egui::Ui) {
        // Используем всё доступное пространство
        let available_height = ui.available_height().max(100.0);
        
        egui::ScrollArea::vertical()
            .id_salt("files_scroll")
            .max_height(available_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.files.is_empty() {
                    ui.colored_label(egui::Color32::GRAY, "Добавьте файлы или папки для отправки");
                    return;
                }
                
                let mut to_remove = None;
                
                for (idx, file) in self.files.iter().enumerate() {
                    ui.horizontal(|ui| {
                        // Иконка статуса
                        let icon = match &file.status {
                            FileStatus::Pending => "⏳",
                            FileStatus::Transferring => "📤",
                            FileStatus::Completed => "✅",
                            FileStatus::Error(_) => "❌",
                        };
                        ui.label(icon);
                        
                        // Путь файла (с обрезкой если слишком длинный)
                        let path_display = if file.relative_path.len() > 50 {
                            format!("...{}", &file.relative_path[file.relative_path.len()-47..])
                        } else {
                            file.relative_path.clone()
                        };
                        ui.label(path_display).on_hover_text(&file.relative_path);
                        
                        // Размер
                        ui.label(format!("({})", format_size(file.size)));
                        
                        // Прогресс
                        if file.status == FileStatus::Transferring {
                            ui.add(
                                egui::ProgressBar::new(file.progress())
                                    .desired_width(80.0)
                                    .show_percentage(),
                            );
                        } else if file.status == FileStatus::Completed {
                            ui.label("✓");
                        }
                        
                        // Кнопка удаления
                        if self.can_edit() && ui.small_button("❌").clicked() {
                            to_remove = Some(idx);
                        }
                    });
                }
                
                if let Some(idx) = to_remove {
                    self.files.remove(idx);
                }
            });
    }
    
    /// Отрисовать список полученных файлов
    pub fn render_received_files(&self, ui: &mut egui::Ui) {
        let available_height = ui.available_height().max(100.0);
        
        egui::ScrollArea::vertical()
            .id_salt("received_files_scroll")
            .max_height(available_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.received_files.is_empty() {
                    ui.colored_label(egui::Color32::GRAY, "Пока нет полученных файлов");
                } else {
                    for (name, size) in &self.received_files {
                        ui.horizontal(|ui| {
                            ui.label("✅");
                            // Обрезаем длинные пути
                            let name_display = if name.len() > 50 {
                                format!("...{}", &name[name.len()-47..])
                            } else {
                                name.clone()
                            };
                            ui.label(name_display).on_hover_text(name);
                            ui.label(format!("({})", format_size(*size)));
                        });
                    }
                }
            });
    }
}

