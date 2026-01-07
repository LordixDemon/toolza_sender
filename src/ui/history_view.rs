//! UI для истории передач

use crate::app::App;
use toolza_sender::history::Direction;
use toolza_sender::i18n::Language;
use eframe::egui;

impl App {
    pub fn render_history_mode(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        ui.heading(t.history_title);
        ui.add_space(10.0);
        
        // Общая статистика
        let stats = self.history.total_stats();
        
        let total_label = match self.language {
            Language::Russian => format!("Всего: {} передач | ✅ {} успешных", stats.total_transfers, stats.successful_transfers),
            Language::Ukrainian => format!("Всього: {} передач | ✅ {} успішних", stats.total_transfers, stats.successful_transfers),
            Language::English => format!("Total: {} transfers | ✅ {} successful", stats.total_transfers, stats.successful_transfers),
        };
        ui.horizontal(|ui| {
            ui.label(total_label);
        });
        
        let sent_label = match self.language {
            Language::Russian => format!("📤 Отправлено: {} файлов, {}", stats.files_sent, toolza_sender::utils::format_size(stats.total_sent)),
            Language::Ukrainian => format!("📤 Надіслано: {} файлів, {}", stats.files_sent, toolza_sender::utils::format_size(stats.total_sent)),
            Language::English => format!("📤 Sent: {} files, {}", stats.files_sent, toolza_sender::utils::format_size(stats.total_sent)),
        };
        ui.horizontal(|ui| {
            ui.label(sent_label);
        });
        
        let received_label = match self.language {
            Language::Russian => format!("📥 Получено: {} файлов, {}", stats.files_received, toolza_sender::utils::format_size(stats.total_received)),
            Language::Ukrainian => format!("📥 Отримано: {} файлів, {}", stats.files_received, toolza_sender::utils::format_size(stats.total_received)),
            Language::English => format!("📥 Received: {} files, {}", stats.files_received, toolza_sender::utils::format_size(stats.total_received)),
        };
        ui.horizontal(|ui| {
            ui.label(received_label);
        });
        
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        
        // Кнопка очистки
        let t = self.t();
        ui.horizontal(|ui| {
            if ui.button(t.clear_history).clicked() {
                self.history.clear();
            }
        });
        
        ui.add_space(10.0);
        
        // Список записей
        let t = self.t();
        if self.history.entries.is_empty() {
            ui.colored_label(egui::Color32::GRAY, t.no_history);
            return;
        }
        
        let files_label = match self.language {
            Language::Russian => "файл(ов)",
            Language::Ukrainian => "файл(ів)",
            Language::English => "file(s)",
        };
        
        let compression_label = match self.language {
            Language::Russian => "Сжатие",
            Language::Ukrainian => "Стиснення",
            Language::English => "Compression",
        };
        
        let addrs_label = match self.language {
            Language::Russian => "адр.",
            Language::Ukrainian => "адр.",
            Language::English => "addr.",
        };
        
        let error_label = match self.language {
            Language::Russian => "Ошибка",
            Language::Ukrainian => "Помилка",
            Language::English => "Error",
        };
        
        egui::ScrollArea::vertical()
            .id_salt("history_scroll")
            .show(ui, |ui| {
                for entry in &self.history.entries {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            // Иконка направления
                            let icon = match entry.direction {
                                Direction::Send => "📤",
                                Direction::Receive => "📥",
                            };
                            
                            // Статус
                            let status_icon = if entry.success { "✅" } else { "❌" };
                            
                            ui.label(format!("{} {}", icon, status_icon));
                            ui.label(&entry.formatted_time());
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(&entry.formatted_speed());
                            });
                        });
                        
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "{} {}, {}",
                                entry.files_count,
                                files_label,
                                entry.formatted_size()
                            ));
                            
                            if entry.compression_ratio < 0.99 {
                                let saved = (1.0 - entry.compression_ratio) * 100.0;
                                ui.label(format!("| {}: {:.0}%", compression_label, saved));
                            }
                        });
                        
                        ui.horizontal(|ui| {
                            ui.label(format!("⏱ {}", entry.formatted_duration()));
                            
                            if !entry.addresses.is_empty() {
                                let addrs = entry.addresses.join(", ");
                                if addrs.len() > 40 {
                                    ui.label(format!("| {} {}", entry.addresses.len(), addrs_label))
                                        .on_hover_text(&addrs);
                                } else {
                                    ui.label(format!("| {}", addrs));
                                }
                            }
                        });
                        
                        if let Some(err) = &entry.error {
                            ui.colored_label(egui::Color32::RED, format!("{}: {}", error_label, err));
                        }
                    });
                    ui.add_space(5.0);
                }
            });
    }
}
