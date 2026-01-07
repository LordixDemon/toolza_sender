//! UI для спидтеста

use crate::app::App;
use toolza_sender::i18n::Language;
use eframe::egui;

impl App {
    pub fn render_speedtest_mode(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        ui.heading(t.speedtest_title);
        ui.add_space(10.0);
        
        let description = match self.language {
            Language::Russian => "Измерение скорости соединения между двумя экземплярами программы.",
            Language::Ukrainian => "Вимірювання швидкості з'єднання між двома екземплярами програми.",
            Language::English => "Measuring connection speed between two instances of the program.",
        };
        ui.label(description);
        ui.add_space(5.0);
        
        let hint = match self.language {
            Language::Russian => "💡 На целевом компьютере должен быть запущен режим \"Принять\"",
            Language::Ukrainian => "💡 На цільовому комп'ютері повинен бути запущений режим \"Прийом\"",
            Language::English => "💡 The target computer must be running in \"Receive\" mode",
        };
        ui.colored_label(egui::Color32::GRAY, hint);
        ui.add_space(15.0);
        
        // Адрес сервера
        let t = self.t();
        ui.horizontal(|ui| {
            ui.label(t.target_address);
            ui.add_enabled(
                !self.speedtest_running,
                egui::TextEdit::singleline(&mut self.speedtest_target)
                    .hint_text("192.168.1.100")
                    .desired_width(200.0),
            );
            ui.label(":");
            ui.add_enabled(
                !self.speedtest_running,
                egui::TextEdit::singleline(&mut self.target_port)
                    .desired_width(60.0),
            );
        });
        
        ui.add_space(10.0);
        
        // Выбор сервера из найденных
        if !self.found_servers.is_empty() {
            let found_label = match self.language {
                Language::Russian => "Найденные серверы",
                Language::Ukrainian => "Знайдені сервери",
                Language::English => "Found servers",
            };
            ui.collapsing(found_label, |ui| {
                for server in self.found_servers.clone() {
                    if ui.selectable_label(false, &server).clicked() {
                        // Извлекаем IP без порта
                        let ip = server.split(':').next().unwrap_or(&server).to_string();
                        self.speedtest_target = ip;
                    }
                }
            });
            ui.add_space(10.0);
        }
        
        // Кнопка запуска
        let t = self.t();
        ui.horizontal(|ui| {
            if self.speedtest_running {
                if ui.button(t.stop).clicked() {
                    self.stop();
                }
            } else {
                if ui.button(t.start_test).clicked() {
                    self.start_speedtest();
                }
            }
            
            // Кнопка сканирования
            if !self.is_scanning && !self.speedtest_running {
                if ui.button(t.find_servers).clicked() {
                    self.start_scan();
                }
            }
        });
        
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(10.0);
        
        // Прогресс
        if self.speedtest_running {
            let testing_label = match self.language {
                Language::Russian => "⏳ Тестирование...",
                Language::Ukrainian => "⏳ Тестування...",
                Language::English => "⏳ Testing...",
            };
            ui.heading(testing_label);
            ui.add_space(10.0);
            
            let direction = if self.speedtest_direction == "upload" {
                "⬆️ Upload"
            } else if self.speedtest_direction == "download" {
                "⬇️ Download"
            } else {
                "🏓 Ping"
            };
            
            ui.label(format!("{}: {}%", direction, self.speedtest_progress));
            
            let progress = self.speedtest_progress as f32 / 100.0;
            ui.add(egui::ProgressBar::new(progress).animate(true));
        }
        
        // Результаты
        if let Some(result) = &self.speedtest_result {
            ui.add_space(10.0);
            let results_label = match self.language {
                Language::Russian => "📊 Результаты",
                Language::Ukrainian => "📊 Результати",
                Language::English => "📊 Results",
            };
            ui.heading(results_label);
            ui.add_space(10.0);
            
            egui::Grid::new("speedtest_results")
                .num_columns(2)
                .spacing([20.0, 10.0])
                .show(ui, |ui| {
                    // Upload
                    ui.label("⬆️ Upload:");
                    ui.colored_label(
                        egui::Color32::from_rgb(100, 200, 100),
                        format!("{:.1} MB/s", result.upload_speed),
                    );
                    ui.end_row();
                    
                    // Download
                    ui.label("⬇️ Download:");
                    ui.colored_label(
                        egui::Color32::from_rgb(100, 150, 250),
                        format!("{:.1} MB/s", result.download_speed),
                    );
                    ui.end_row();
                    
                    // Ping
                    ui.label("🏓 Ping:");
                    let ping_color = if result.latency_ms < 1.0 {
                        egui::Color32::from_rgb(100, 200, 100)
                    } else if result.latency_ms < 5.0 {
                        egui::Color32::from_rgb(200, 200, 100)
                    } else {
                        egui::Color32::from_rgb(200, 100, 100)
                    };
                    ui.colored_label(ping_color, format!("{:.2} ms", result.latency_ms));
                    ui.end_row();
                });
            
            ui.add_space(20.0);
            
            // Визуализация скорости
            let max_speed = result.upload_speed.max(result.download_speed).max(1.0);
            let upload_bar = result.upload_speed as f32 / max_speed as f32;
            let download_bar = result.download_speed as f32 / max_speed as f32;
            
            ui.label("⬆️ Upload:");
            ui.add(egui::ProgressBar::new(upload_bar)
                .fill(egui::Color32::from_rgb(100, 200, 100))
                .text(format!("{:.1} MB/s", result.upload_speed)));
            
            ui.add_space(5.0);
            
            ui.label("⬇️ Download:");
            ui.add(egui::ProgressBar::new(download_bar)
                .fill(egui::Color32::from_rgb(100, 150, 250))
                .text(format!("{:.1} MB/s", result.download_speed)));
            
            ui.add_space(20.0);
            
            // Оценка качества соединения
            let quality = get_connection_quality(result.upload_speed, result.download_speed, result.latency_ms, self.language);
            let quality_label = match self.language {
                Language::Russian => "Качество соединения:",
                Language::Ukrainian => "Якість з'єднання:",
                Language::English => "Connection quality:",
            };
            ui.horizontal(|ui| {
                ui.label(quality_label);
                ui.colored_label(quality.1, quality.0);
            });
        }
    }
}

fn get_connection_quality(upload: f64, download: f64, latency: f64, lang: Language) -> (&'static str, egui::Color32) {
    let avg_speed = (upload + download) / 2.0;
    
    if avg_speed >= 100.0 && latency < 1.0 {
        let label = match lang {
            Language::Russian => "🌟 Превосходно",
            Language::Ukrainian => "🌟 Чудово",
            Language::English => "🌟 Excellent",
        };
        (label, egui::Color32::from_rgb(100, 255, 100))
    } else if avg_speed >= 50.0 && latency < 2.0 {
        let label = match lang {
            Language::Russian => "✅ Отлично",
            Language::Ukrainian => "✅ Відмінно",
            Language::English => "✅ Great",
        };
        (label, egui::Color32::from_rgb(150, 250, 150))
    } else if avg_speed >= 20.0 && latency < 5.0 {
        let label = match lang {
            Language::Russian => "👍 Хорошо",
            Language::Ukrainian => "👍 Добре",
            Language::English => "👍 Good",
        };
        (label, egui::Color32::from_rgb(200, 250, 100))
    } else if avg_speed >= 5.0 && latency < 10.0 {
        let label = match lang {
            Language::Russian => "⚠️ Нормально",
            Language::Ukrainian => "⚠️ Нормально",
            Language::English => "⚠️ Normal",
        };
        (label, egui::Color32::from_rgb(250, 200, 100))
    } else {
        let label = match lang {
            Language::Russian => "❌ Медленно",
            Language::Ukrainian => "❌ Повільно",
            Language::English => "❌ Slow",
        };
        (label, egui::Color32::from_rgb(250, 100, 100))
    }
}
