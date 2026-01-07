//! Модуль пользовательского интерфейса

mod send_view;
mod receive_view;
mod history_view;
mod speedtest_view;
mod widgets;

use crate::app::{App, Mode};
#[allow(unused_imports)]
use toolza_sender::protocol::FileStatus;
use toolza_sender::i18n::Language;
use eframe::egui;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Обрабатываем события
        self.process_events();
        
        // Обрабатываем результаты файловых диалогов
        self.process_dialog_results();
        
        // Обрабатываем Drag & Drop
        self.handle_drag_drop(ctx);
        
        // Запрашиваем перерисовку при активных операциях (раз в секунду, не чаще!)
        if self.is_running || self.is_scanning || self.speedtest_running {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        
        // Боковая панель
        self.render_sidebar(ctx);
        
        // Нижняя панель с логом (фиксированная высота)
        self.render_log_panel(ctx);
        
        // Основная панель (занимает оставшееся место)
        self.render_main_panel(ctx);
    }
}

impl App {
    fn handle_drag_drop(&mut self, ctx: &egui::Context) {
        // Проверяем dropped файлы
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                for dropped in &i.raw.dropped_files {
                    if let Some(path) = &dropped.path {
                        self.dropped_files.push(path.clone());
                    }
                }
            }
        });
        
        // Визуальный индикатор drag & drop
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) && self.can_edit() {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drag_drop_overlay"),
            ));
            
            let screen_rect = ctx.screen_rect();
            painter.rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(100, 150, 200, 100),
            );
            
            let drag_text = match self.language {
                Language::Russian => "📁 Перетащите файлы или папки сюда",
                Language::Ukrainian => "📁 Перетягніть файли або теки сюди",
                Language::English => "📁 Drag files or folders here",
            };
            painter.text(
                screen_rect.center(),
                egui::Align2::CENTER_CENTER,
                drag_text,
                egui::FontId::proportional(24.0),
                egui::Color32::WHITE,
            );
        }
    }
    
    fn render_sidebar(&mut self, ctx: &egui::Context) {
        let t = self.t();
        
        egui::SidePanel::left("mode_panel")
            .resizable(true)
            .min_width(120.0)
            .default_width(160.0)
            .max_width(400.0)
            .show(ctx, |ui| {
                ui.add_space(10.0);
                
                let mode_label = match self.language {
                    Language::Russian => "Режим",
                    Language::Ukrainian => "Режим",
                    Language::English => "Mode",
                };
                ui.heading(mode_label);
                ui.add_space(10.0);
                
                ui.selectable_value(&mut self.mode, Mode::Send, format!("📤 {}", t.mode_send));
                ui.selectable_value(&mut self.mode, Mode::Receive, format!("📥 {}", t.mode_receive));
                ui.selectable_value(&mut self.mode, Mode::SpeedTest, format!("🚀 {}", t.mode_speedtest));
                ui.selectable_value(&mut self.mode, Mode::History, format!("📊 {}", t.mode_history));
                
                ui.add_space(20.0);
                ui.separator();
                
                // Статистика передачи (если активна)
                if self.is_running {
                    ui.add_space(10.0);
                    let stats_label = match self.language {
                        Language::Russian => "📈 Статистика:",
                        Language::Ukrainian => "📈 Статистика:",
                        Language::English => "📈 Statistics:",
                    };
                    ui.label(stats_label);
                    ui.add_space(5.0);
                    
                    ui.horizontal(|ui| {
                        ui.label("⚡");
                        ui.label(&self.current_speed());
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("⏱");
                        ui.label(&self.current_eta());
                    });
                    
                    if self.use_compression {
                        ui.horizontal(|ui| {
                            ui.label("📦");
                            ui.label(&self.compression_stats());
                        });
                    }
                    
                    ui.add_space(10.0);
                    ui.separator();
                }
                
                ui.add_space(10.0);
                ui.label(t.status);
                
                // Статус с автоматической прокруткой - занимает оставшееся место
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label(&self.status_message);
                    });
            });
    }
    
    fn render_log_panel(&mut self, ctx: &egui::Context) {
        let t = self.t();
        
        egui::TopBottomPanel::bottom("log_panel")
            .resizable(true)
            .min_height(60.0)
            .default_height(100.0)
            .max_height(400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(t.log);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(t.clear).clicked() {
                            self.log_messages.clear();
                        }
                    });
                });
                
                egui::ScrollArea::vertical()
                    .id_salt("log_scroll")
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for msg in &self.log_messages {
                            ui.label(msg);
                        }
                        if self.log_messages.is_empty() {
                            let empty_log = match self.language {
                                Language::Russian => "Лог пуст",
                                Language::Ukrainian => "Лог порожній",
                                Language::English => "Log is empty",
                            };
                            ui.colored_label(egui::Color32::GRAY, empty_log);
                        }
                    });
            });
    }
    
    fn render_main_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Кнопки выбора языка вверху
            self.render_language_selector(ui);
            
            ui.separator();
            ui.add_space(5.0);
            
            match self.mode {
                Mode::Send => self.render_send_mode(ui),
                Mode::Receive => self.render_receive_mode(ui),
                Mode::SpeedTest => self.render_speedtest_mode(ui),
                Mode::History => self.render_history_mode(ui),
            }
        });
    }
    
    fn render_language_selector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("🌐");
            for lang in Language::all() {
                let text = format!("{} {}", lang.flag(), lang.native_name());
                let selected = self.language == *lang;
                
                if ui.selectable_label(selected, text).clicked() {
                    self.language = *lang;
                }
            }
        });
    }
}
