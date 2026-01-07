//! UI режима приёма

use crate::app::App;
use eframe::egui;
use toolza_sender::network::TransportType;

impl App {
    pub fn render_receive_mode(&mut self, ui: &mut egui::Ui) {
        let t = self.t();
        ui.heading(t.receive_title);
        ui.add_space(10.0);
        
        // Адрес для подключения
        let t = self.t();
        ui.horizontal(|ui| {
            ui.label(t.your_address);
            ui.strong(format!("{}:{}", self.local_ip, self.listen_port));
            ui.label(format!("[{}]", self.transport_type.name()));
        });
        
        ui.add_space(5.0);
        
        // Порт
        let t = self.t();
        ui.horizontal(|ui| {
            ui.label(t.port);
            ui.add_enabled(
                self.can_edit(),
                egui::TextEdit::singleline(&mut self.listen_port)
                    .desired_width(60.0),
            );
        });
        
        ui.add_space(5.0);
        
        // Выбор протокола
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
        
        // Папка сохранения
        let t = self.t();
        ui.horizontal(|ui| {
            ui.label(t.save_folder);
            ui.label(self.save_directory.display().to_string());
            if ui.add_enabled(self.can_edit(), egui::Button::new(t.choose)).clicked() {
                self.select_save_directory();
            }
        });
        
        ui.add_space(5.0);
        
        // Галочки автораспаковки
        let t = self.t();
        ui.horizontal(|ui| {
            ui.label(t.auto_extract);
            
            ui.add_enabled(
                self.can_edit(),
                egui::Checkbox::new(&mut self.auto_extract_tar_lz4, "tar.lz4"),
            ).on_hover_text(t.extract_tooltip_tar_lz4);
            
            ui.add_enabled(
                self.can_edit(),
                egui::Checkbox::new(&mut self.auto_extract_lz4, "lz4"),
            ).on_hover_text(t.extract_tooltip_lz4);
            
            ui.add_enabled(
                self.can_edit(),
                egui::Checkbox::new(&mut self.auto_extract_tar, "tar"),
            ).on_hover_text(t.extract_tooltip_tar);
            
            ui.add_enabled(
                self.can_edit(),
                egui::Checkbox::new(&mut self.auto_extract_zip, "zip"),
            ).on_hover_text(t.extract_tooltip_zip);
            
            ui.add_enabled(
                self.can_edit(),
                egui::Checkbox::new(&mut self.auto_extract_rar, "rar"),
            ).on_hover_text(t.extract_tooltip_rar);
        });
        
        ui.add_space(10.0);
        
        // Кнопки управления
        let t = self.t();
        ui.horizontal(|ui| {
            if self.is_running {
                if ui.button(t.stop_server).clicked() {
                    self.stop();
                }
            } else {
                if ui.button(t.start_server).clicked() {
                    self.start_receive();
                }
            }
        });
        
        ui.add_space(10.0);
        
        // Полученные файлы
        let t = self.t();
        ui.heading(t.received_files);
        self.render_received_files(ui);
    }
}
