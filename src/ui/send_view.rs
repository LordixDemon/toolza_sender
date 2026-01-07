//! UI режима отправки

use crate::app::App;
use eframe::egui;
use toolza_sender::network::TransportType;

impl App {
    pub fn render_send_mode(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        ui.heading(t.send_title);
        ui.add_space(10.0);
        
        // Информация о локальном IP и порт
        self.render_local_info(ui);
        
        ui.add_space(5.0);
        ui.separator();
        
        // Секция получателей
        self.render_targets_section(ui);
        
        ui.add_space(10.0);
        ui.separator();
        
        // Секция файлов
        self.render_files_section(ui);
    }
    
    fn render_local_info(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        ui.horizontal(|ui| {
            ui.label(t.your_ip);
            ui.label(&self.local_ip);
            ui.label(format!("  {}", t.port));
            ui.add_enabled(
                self.can_edit(),
                egui::TextEdit::singleline(&mut self.target_port)
                    .desired_width(60.0),
            );
        });
    }
    
    fn render_targets_section(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        ui.heading(t.recipients);
        
        // Добавление получателя
        ui.horizontal(|ui| {
            ui.label(t.ip_address);
            let response = ui.add_enabled(
                self.can_edit(),
                egui::TextEdit::singleline(&mut self.new_target_address)
                    .desired_width(150.0)
                    .hint_text("192.168.1.x"),
            );
            
            let can_add = self.can_edit() && !self.new_target_address.is_empty();
            let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            
            if ui.add_enabled(can_add, egui::Button::new(t.add)).clicked() || (enter_pressed && can_add) {
                let addr = self.new_target_address.clone();
                self.add_target(addr);
                self.new_target_address.clear();
            }
        });
        
        // Подсети для сканирования
        let t = self.t();
        ui.horizontal(|ui| {
            ui.label(t.subnets);
            ui.add_enabled(
                self.can_edit(),
                egui::TextEdit::singleline(&mut self.subnets_input)
                    .desired_width(200.0)
                    .hint_text(t.subnets_hint),
            ).on_hover_text(t.subnets_tooltip);
        });
        
        // Кнопки сканирования
        let t = self.t();
        ui.horizontal(|ui| {
            if self.is_scanning {
                ui.add(
                    egui::ProgressBar::new(self.scan_progress as f32 / 100.0)
                        .desired_width(200.0)
                        .show_percentage(),
                );
                if ui.button(t.cancel).clicked() {
                    self.stop();
                }
            } else {
                if ui.add_enabled(!self.is_running, egui::Button::new(t.find_servers)).clicked() {
                    self.start_scan();
                }
                
                if !self.targets.is_empty() && ui.add_enabled(self.can_edit(), egui::Button::new(t.clear)).clicked() {
                    self.clear_targets();
                }
            }
        });
        
        // Найденные серверы
        self.render_found_servers(ui);
        
        // Список получателей
        ui.add_space(5.0);
        self.render_targets_list(ui);
    }
    
    fn render_found_servers(&mut self, ui: &mut egui::Ui) {
        if self.found_servers.is_empty() {
            return;
        }
        
        let t = self.t();
        ui.add_space(5.0);
        ui.label(t.found_servers);
        
        ui.horizontal_wrapped(|ui| {
            for server in &self.found_servers.clone() {
                let server_ip = server.rsplit_once(':').map(|(ip, _)| ip).unwrap_or(server);
                let already_added = self.targets.iter().any(|t| 
                    t.address == server_ip || t.address == *server
                );
                
                if already_added {
                    ui.add_enabled(false, egui::Button::new(format!("✓ {}", server)));
                } else if ui.button(format!("🖥 {}", server)).clicked() {
                    self.add_target(server_ip.to_string());
                }
            }
        });
    }
    
    fn render_files_section(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        
        // Строка 1: Кнопки управления файлами
        ui.horizontal(|ui| {
            if ui.add_enabled(self.can_edit(), egui::Button::new(t.files)).clicked() {
                self.add_files_dialog();
            }
            
            if ui.add_enabled(self.can_edit(), egui::Button::new(t.folder)).clicked() {
                self.add_folder_dialog();
            }
            
            if ui.add_enabled(self.can_edit() && !self.files.is_empty(), egui::Button::new(t.clear)).clicked() {
                self.clear_files();
            }
        });
        
        ui.add_space(5.0);
        
        // Строка 2: Опции передачи
        let t = self.t();
        ui.horizontal(|ui| {
            ui.label(t.options);
            
            ui.add_enabled(
                self.can_edit(),
                egui::Checkbox::new(&mut self.use_compression, t.lz4_compression),
            ).on_hover_text(t.lz4_tooltip);
            
            ui.add_enabled(
                self.can_edit(),
                egui::Checkbox::new(&mut self.preserve_structure, t.preserve_structure),
            ).on_hover_text(t.preserve_structure_tooltip);
            
            ui.add_enabled(
                self.can_edit(),
                egui::Checkbox::new(&mut self.sync_mode, t.sync_mode),
            ).on_hover_text(t.sync_mode_tooltip);
        });
        
        // Строка 3: Выбор протокола
        let t = self.t();
        ui.horizontal(|ui| {
            ui.label(t.protocol);
            for transport in TransportType::all() {
                let label = match transport {
                    TransportType::Tcp => "TCP",
                    TransportType::Udp => "UDP",
                    #[cfg(feature = "quic")]
                    TransportType::Quic => "QUIC",
                    #[cfg(feature = "kcp")]
                    TransportType::Kcp => "KCP",
                };
                let tooltip = match transport {
                    TransportType::Tcp => t.tcp_description,
                    TransportType::Udp => t.udp_description,
                    #[cfg(feature = "quic")]
                    TransportType::Quic => t.quic_description,
                    #[cfg(feature = "kcp")]
                    TransportType::Kcp => t.kcp_description,
                };
                if ui.add_enabled(
                    self.can_edit(),
                    egui::RadioButton::new(self.transport_type == transport, label),
                ).on_hover_text(tooltip).clicked() {
                    self.transport_type = transport;
                }
            }
        });
        
        ui.add_space(5.0);
        
        // Строка 4: Кнопка отправки
        let t = self.t();
        ui.horizontal(|ui| {
            if self.is_running {
                if ui.button(t.stop).clicked() {
                    self.stop();
                }
            } else {
                let can_send = !self.files.is_empty() && !self.targets.is_empty();
                let btn_text = t.send_to_recipients.replace("{}", &self.targets.len().to_string());
                if ui.add_enabled(can_send, egui::Button::new(btn_text)).clicked() {
                    self.start_send();
                }
            }
        });
        
        ui.add_space(10.0);
        
        // Статистика во время передачи
        if self.is_running {
            let t = self.t();
            ui.horizontal(|ui| {
                let total_size: u64 = self.files.iter().map(|f| f.size).sum();
                let transferred: u64 = self.files.iter().map(|f| f.transferred).sum();
                let percent = if total_size > 0 {
                    (transferred as f64 / total_size as f64 * 100.0) as u32
                } else {
                    100
                };
                
                ui.add(
                    egui::ProgressBar::new(percent as f32 / 100.0)
                        .desired_width(200.0)
                        .text(format!(
                            "{} / {} ({}%)",
                            toolza_sender::utils::format_size(transferred),
                            toolza_sender::utils::format_size(total_size),
                            percent
                        )),
                );
                
                ui.label(format!("⚡ {}", self.current_speed()));
                ui.label(format!("{} {}", t.eta, self.current_eta()));
                
                if self.use_compression && self.bytes_original > 0 {
                    ui.label(format!("{} {}", t.compression_stats, self.compression_stats()));
                }
            });
            ui.add_space(5.0);
        }
        
        let t = self.t();
        ui.horizontal(|ui| {
            ui.heading(t.files_to_send);
            ui.colored_label(
                egui::Color32::GRAY, 
                t.or_drag_drop
            );
        });
        
        self.render_files_list(ui);
    }
}
