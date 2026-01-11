//! Обработка событий от сетевого модуля

use super::state::{App, TargetStatus};
use toolza_sender::history::HistoryEntry;
use toolza_sender::network::TransferEvent;
use toolza_sender::protocol::FileStatus;

impl App {
    /// Обработать все ожидающие события
    pub fn process_events(&mut self) {
        // Обрабатываем dropped файлы
        self.process_dropped_files();
        
        // Собираем все события в вектор
        let events: Vec<TransferEvent> = {
            let Some(rx) = &mut self.event_rx else { return };
            let mut events = Vec::new();
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
            events
        };
        
        // Обрабатываем события
        for event in events {
            self.handle_event(event);
        }
    }
    
    /// Обработать dropped файлы (Drag & Drop)
    fn process_dropped_files(&mut self) {
        if self.dropped_files.is_empty() || !self.can_edit() {
            return;
        }
        
        let paths = std::mem::take(&mut self.dropped_files);
        
        for path in paths {
            if path.is_file() {
                if let Ok(info) = toolza_sender::protocol::FileInfo::new(path.clone()) {
                    if !self.files.iter().any(|f| f.path == path) {
                        self.files.push(info);
                    }
                }
            } else if path.is_dir() {
                if let Ok(files) = toolza_sender::protocol::collect_files_from_folder(&path) {
                    let folder_name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "folder".to_string());
                    
                    let count = files.len();
                    for file in files {
                        if !self.files.iter().any(|f| f.path == file.path) {
                            self.files.push(file);
                        }
                    }
                    self.log(format!("📁 Добавлена папка '{}': {} файл(ов)", folder_name, count));
                }
            }
        }
    }
    
    fn handle_event(&mut self, event: TransferEvent) {
        match event {
            TransferEvent::Connected(target_id, addr) => {
                self.on_connected(target_id, addr);
            }
            TransferEvent::FileStarted(target_id, file_idx) => {
                self.on_file_started(target_id, file_idx);
            }
            TransferEvent::Progress(_target_id, file_idx, transferred, original, compressed) => {
                self.on_progress(file_idx, transferred, original, compressed);
            }
            TransferEvent::FileCompleted(target_id, file_idx) => {
                self.on_file_completed(target_id, file_idx);
            }
            TransferEvent::FileError(target_id, file_idx, err) => {
                self.on_file_error(target_id, file_idx, err);
            }
            TransferEvent::TargetCompleted(target_id) => {
                self.on_target_completed(target_id);
            }
            TransferEvent::AllCompleted => {
                self.on_all_completed();
            }
            TransferEvent::ConnectionError(target_id, err) => {
                self.on_connection_error(target_id, err);
            }
            TransferEvent::FileSkipped(target_id, file_idx) => {
                self.on_file_skipped(target_id, file_idx);
            }
            TransferEvent::FileResumed(target_id, file_idx, offset) => {
                self.on_file_resumed(target_id, file_idx, offset);
            }
            TransferEvent::Disconnected => {
                self.log("Клиент отключился");
            }
            TransferEvent::FileReceived(name, size) => {
                self.on_file_received(name, size);
            }
            TransferEvent::ExtractionStarted(name) => {
                self.on_extraction_started(name);
            }
            TransferEvent::ExtractionCompleted(name, files_count, total_size) => {
                self.on_extraction_completed(name, files_count, total_size);
            }
            TransferEvent::ExtractionError(name, err) => {
                self.on_extraction_error(name, err);
            }
            TransferEvent::ServerFound(addr) => {
                self.on_server_found(addr);
            }
            TransferEvent::ScanProgress(current_ip, progress) => {
                self.on_scan_progress(current_ip, progress);
            }
            TransferEvent::ScanCompleted => {
                self.on_scan_completed();
            }
            TransferEvent::SpeedTestStarted(addr) => {
                self.on_speedtest_started(addr);
            }
            TransferEvent::SpeedTestProgress(direction, progress) => {
                self.on_speedtest_progress(direction, progress);
            }
            TransferEvent::SpeedTestCompleted(upload, download, latency) => {
                self.on_speedtest_completed(upload, download, latency);
            }
            TransferEvent::SpeedTestError(err) => {
                self.on_speedtest_error(err);
            }
        }
    }
    
    // === Обработчики событий отправки ===
    
    fn on_connected(&mut self, target_id: usize, addr: String) {
        if target_id < self.targets.len() {
            self.targets[target_id].status = TargetStatus::Transferring;
        }
        self.log(format!("🔗 Подключено: {}", addr));
    }
    
    fn on_file_started(&mut self, target_id: usize, file_idx: usize) {
        if target_id < self.targets.len() {
            self.targets[target_id].current_file = file_idx;
        }
        
        if file_idx < self.files.len() && self.files[file_idx].status == FileStatus::Pending {
            self.files[file_idx].status = FileStatus::Transferring;
            let file_name = &self.files[file_idx].name;
            let target_addr = self.targets.get(target_id)
                .map(|t| t.address.as_str())
                .unwrap_or("?");
            self.log(format!("📤 {} → {}", file_name, target_addr));
        }
    }
    
    fn on_progress(&mut self, file_idx: usize, transferred: u64, original: u64, compressed: u64) {
        if file_idx < self.files.len() {
            if transferred > self.files[file_idx].transferred {
                self.files[file_idx].transferred = transferred;
            }
        }
        
        // Обновляем общую статистику
        self.bytes_original = self.bytes_original.max(original);
        self.bytes_compressed = self.bytes_compressed.max(compressed);
        
        // Обновляем статистику скорости
        // На стороне отправки: суммируем по файлам
        // На стороне приёма: используем transferred напрямую (files пуст)
        let total_transferred: u64 = if self.files.is_empty() {
            transferred // Приёмная сторона - используем переданное значение
        } else {
            self.files.iter().map(|f| f.transferred).sum()
        };
        
        // Для приёмной стороны устанавливаем total_bytes если не установлен
        if self.files.is_empty() && self.stats.total_bytes == 0 && original > 0 {
            self.stats = toolza_sender::stats::TransferStats::new(original, 1);
            self.transfer_start_time = Some(std::time::Instant::now());
        }
        
        self.stats.update(total_transferred, original, compressed);
        
        // Обновляем статус
        let speed = self.stats.speed_formatted();
        let eta = self.stats.eta_formatted();
        let progress_str = if original > 0 {
            let pct = (transferred as f64 / original as f64 * 100.0).min(100.0);
            format!(" ({:.1}%)", pct)
        } else {
            String::new()
        };
        self.status_message = format!("⚡ {} | ETA: {}{}", speed, eta, progress_str);
    }
    
    fn on_file_completed(&mut self, target_id: usize, file_idx: usize) {
        if target_id < self.targets.len() {
            self.targets[target_id].files_completed += 1;
        }
        
        let all_completed = self.targets.iter().all(|t| 
            t.files_completed > file_idx || 
            matches!(t.status, TargetStatus::Error(_))
        );
        
        if file_idx < self.files.len() && all_completed {
            self.files[file_idx].status = FileStatus::Completed;
            self.files[file_idx].transferred = self.files[file_idx].size;
            self.stats.file_completed();
        }
    }
    
    fn on_file_skipped(&mut self, target_id: usize, file_idx: usize) {
        if target_id < self.targets.len() {
            self.targets[target_id].files_completed += 1;
        }
        
        if file_idx < self.files.len() {
            self.files[file_idx].status = FileStatus::Completed;
            self.files[file_idx].transferred = self.files[file_idx].size;
            let name = &self.files[file_idx].name;
            self.log(format!("⏭️ Пропущен (актуален): {}", name));
            self.stats.file_completed();
        }
    }
    
    fn on_file_resumed(&mut self, target_id: usize, file_idx: usize, offset: u64) {
        if file_idx < self.files.len() {
            self.files[file_idx].transferred = offset;
            let name = &self.files[file_idx].name;
            let target_addr = self.targets.get(target_id)
                .map(|t| t.address.as_str())
                .unwrap_or("?");
            self.log(format!("🔄 Возобновление: {} @ {} → {}", name, format_size(offset), target_addr));
        }
    }
    
    fn on_file_error(&mut self, target_id: usize, file_idx: usize, err: String) {
        let file_name = self.files.get(file_idx)
            .map(|f| f.name.as_str())
            .unwrap_or("?");
        let target_addr = self.targets.get(target_id)
            .map(|t| t.address.as_str())
            .unwrap_or("?");
        self.log(format!("❌ Ошибка {} → {}: {}", file_name, target_addr, err));
    }
    
    fn on_target_completed(&mut self, target_id: usize) {
        if target_id < self.targets.len() {
            self.targets[target_id].status = TargetStatus::Completed;
            self.log(format!("✅ Завершено: {}", self.targets[target_id].address));
        }
        
        let completed = self.targets.iter()
            .filter(|t| t.status == TargetStatus::Completed)
            .count();
        self.status_message = format!("Завершено {}/{} получателей", completed, self.targets.len());
    }
    
    fn on_all_completed(&mut self) {
        self.is_running = false;
        
        let successful = self.targets.iter()
            .filter(|t| t.status == TargetStatus::Completed)
            .count();
        
        let duration = self.transfer_start_time
            .map(|t| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        
        let total_size: u64 = self.files.iter().map(|f| f.size).sum();
        let compression_ratio = if self.bytes_original > 0 {
            self.bytes_compressed as f64 / self.bytes_original as f64
        } else {
            1.0
        };
        
        // Добавляем в историю
        let addresses: Vec<String> = self.targets.iter()
            .filter(|t| t.status == TargetStatus::Completed)
            .map(|t| t.address.clone())
            .collect();
        
        let entry = HistoryEntry::new_send(
            self.files.len(),
            total_size,
            duration,
            compression_ratio,
            addresses,
            successful > 0,
            None,
        );
        self.history.add(entry);
        
        // Форматируем статистику
        let elapsed = self.stats.elapsed_formatted();
        let speed = self.stats.speed_formatted();
        let compression = if self.use_compression && compression_ratio < 0.99 {
            format!(", сжатие {:.0}%", (1.0 - compression_ratio) * 100.0)
        } else {
            String::new()
        };
        
        self.status_message = format!("✅ Готово! {} за {}{}", speed, elapsed, compression);
        self.log(format!("Передача завершена: {} файлов за {}", self.files.len(), elapsed));
        
        if successful > 0 {
            for file in &mut self.files {
                file.status = FileStatus::Completed;
                file.transferred = file.size;
            }
        }
    }
    
    fn on_connection_error(&mut self, target_id: usize, err: String) {
        if target_id < self.targets.len() {
            self.targets[target_id].status = TargetStatus::Error(err.clone());
            self.log(format!("❌ Ошибка {}: {}", self.targets[target_id].address, err));
        }
    }
    
    // === Обработчики событий приёма ===
    
    fn on_file_received(&mut self, name: String, size: u64) {
        self.received_files.push((name.clone(), size));
        self.log(format!("📥 Получен: {} ({})", name, format_size(size)));
    }
    
    fn on_extraction_started(&mut self, name: String) {
        self.status_message = format!("📦 Распаковка {}...", name);
        self.log(format!("📦 Распаковка: {}", name));
    }
    
    fn on_extraction_completed(&mut self, name: String, files_count: usize, total_size: u64) {
        self.status_message = "Ожидание подключений...".to_string();
        self.log(format!("✅ Распаковано {}: {} файл(ов), {}", name, files_count, format_size(total_size)));
        // Обновляем финальное состояние окна
        self.extraction_files_count = files_count;
        self.extraction_total_size = total_size;
        self.extraction_current_file = String::new();
        // Закрываем окно через небольшую задержку (можно закрыть сразу или через таймер)
        // Пока оставим открытым, чтобы пользователь видел результат
    }
    
    fn on_extraction_error(&mut self, name: String, err: String) {
        self.status_message = "Ожидание подключений...".to_string();
        self.log(format!("❌ Ошибка распаковки {}: {}", name, err));
        // Закрываем окно при ошибке
        self.extraction_window_open = false;
    }
    
    // === Обработчики событий сканирования ===
    
    fn on_server_found(&mut self, addr: String) {
        if !self.found_servers.contains(&addr) {
            self.found_servers.push(addr.clone());
            self.log(format!("🟢 Найден сервер: {}", addr));
        }
    }
    
    fn on_scan_progress(&mut self, current_ip: String, progress: u8) {
        self.scan_progress = progress;
        self.status_message = format!("🔍 {} ({}%)", current_ip, progress);
    }
    
    fn on_scan_completed(&mut self) {
        self.is_scanning = false;
        
        if self.found_servers.is_empty() {
            self.status_message = "Серверы не найдены".to_string();
            self.log("Сканирование завершено: серверы не найдены");
        } else {
            self.status_message = format!("Найдено серверов: {}", self.found_servers.len());
            self.log(format!("Сканирование завершено: найдено {} сервер(ов)", self.found_servers.len()));
        }
    }
    
    // === Обработчики событий спидтеста ===
    
    fn on_speedtest_started(&mut self, addr: String) {
        self.status_message = format!("🚀 Спидтест к {}...", addr);
    }
    
    fn on_speedtest_progress(&mut self, direction: String, progress: u8) {
        self.speedtest_progress = progress;
        self.speedtest_direction = direction.clone();
        
        let dir_str = if direction == "upload" { "⬆️ Upload" } else { "⬇️ Download" };
        self.status_message = format!("{}: {}%", dir_str, progress);
    }
    
    fn on_speedtest_completed(&mut self, upload: f64, download: f64, latency: f64) {
        self.speedtest_running = false;
        
        let result = toolza_sender::network::SpeedTestResult {
            upload_speed: upload,
            download_speed: download,
            latency_ms: latency,
        };
        
        self.status_message = format!(
            "✅ ⬆️ {:.1} MB/s | ⬇️ {:.1} MB/s | 🏓 {:.1} ms",
            upload, download, latency
        );
        self.log(format!(
            "Спидтест завершён: Upload {:.1} MB/s, Download {:.1} MB/s, Ping {:.1} ms",
            upload, download, latency
        ));
        
        self.speedtest_result = Some(result);
    }
    
    fn on_speedtest_error(&mut self, err: String) {
        self.speedtest_running = false;
        self.status_message = format!("❌ Ошибка спидтеста: {}", err);
        self.log(format!("Ошибка спидтеста: {}", err));
    }
}

fn format_size(bytes: u64) -> String {
    toolza_sender::utils::format_size(bytes)
}
