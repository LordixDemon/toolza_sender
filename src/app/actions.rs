//! Действия приложения

use super::state::{App, DialogResult, TargetInfo};
use std::sync::atomic::Ordering;
use toolza_sender::network;
use toolza_sender::protocol::{FileInfo, FileStatus, collect_files_from_folder};
use tokio::sync::mpsc;

impl App {
    // === Управление получателями ===
    
    /// Добавить получателя
    pub fn add_target(&mut self, address: String) {
        if address.is_empty() {
            return;
        }
        // Проверяем, не добавлен ли уже
        if !self.targets.iter().any(|t| t.address == address) {
            self.targets.push(TargetInfo::new(address));
        }
    }
    
    /// Удалить получателя
    #[allow(dead_code)]
    pub fn remove_target(&mut self, index: usize) {
        if index < self.targets.len() {
            self.targets.remove(index);
        }
    }
    
    /// Очистить список получателей
    pub fn clear_targets(&mut self) {
        self.targets.clear();
    }
    
    // === Управление файлами ===
    
    /// Добавить файлы через диалог (асинхронно)
    pub fn add_files_dialog(&mut self) {
        let tx = self.dialog_tx.clone();
        std::thread::spawn(move || {
            if let Some(paths) = rfd::FileDialog::new()
                .set_title("Выберите файлы для отправки")
                .pick_files()
            {
                let _ = tx.send(DialogResult::Files(paths));
            }
        });
    }
    
    /// Добавить папку через диалог (асинхронно)
    pub fn add_folder_dialog(&mut self) {
        let tx = self.dialog_tx.clone();
        std::thread::spawn(move || {
            if let Some(folder) = rfd::FileDialog::new()
                .set_title("Выберите папку для отправки")
                .pick_folder()
            {
                let _ = tx.send(DialogResult::Folder(folder));
            }
        });
    }
    
    /// Обработать результаты файловых диалогов
    pub fn process_dialog_results(&mut self) {
        // Собираем все результаты сначала, чтобы освободить borrow
        let results: Vec<_> = if let Some(ref mut rx) = self.dialog_rx {
            let mut res = Vec::new();
            while let Ok(result) = rx.try_recv() {
                res.push(result);
            }
            res
        } else {
            return;
        };
        
        // Теперь обрабатываем собранные результаты
        for result in results {
            match result {
                DialogResult::Files(paths) => {
                    for path in paths {
                        match FileInfo::new(path.clone()) {
                            Ok(info) => {
                                if !self.files.iter().any(|f| f.path == path) {
                                    self.files.push(info);
                                }
                            }
                            Err(e) => {
                                self.log(format!("Ошибка: {}", e));
                            }
                        }
                    }
                }
                DialogResult::Folder(folder) => {
                    match collect_files_from_folder(&folder) {
                        Ok(files) => {
                            let folder_name = folder
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "folder".to_string());
                            
                            let count = files.len();
                            for file in files {
                                if !self.files.iter().any(|f| f.path == file.path) {
                                    self.files.push(file);
                                }
                            }
                            self.log(format!("Добавлена папка '{}': {} файл(ов)", folder_name, count));
                        }
                        Err(e) => {
                            self.log(format!("Ошибка при сканировании папки: {}", e));
                        }
                    }
                }
                DialogResult::SaveDirectory(path) => {
                    self.save_directory = path;
                }
            }
        }
    }
    
    /// Удалить файл
    #[allow(dead_code)]
    pub fn remove_file(&mut self, index: usize) {
        if index < self.files.len() {
            self.files.remove(index);
        }
    }
    
    /// Очистить список файлов
    pub fn clear_files(&mut self) {
        self.files.clear();
    }
    
    /// Выбрать папку сохранения (асинхронно)
    pub fn select_save_directory(&mut self) {
        let tx = self.dialog_tx.clone();
        std::thread::spawn(move || {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Выберите папку для сохранения")
                .pick_folder()
            {
                let _ = tx.send(DialogResult::SaveDirectory(path));
            }
        });
    }
    
    // === Отправка ===
    
    /// Начать отправку файлов
    pub fn start_send(&mut self) {
        if self.files.is_empty() {
            self.status_message = "Добавьте файлы для отправки".to_string();
            return;
        }
        
        if self.targets.is_empty() {
            self.status_message = "Добавьте получателей".to_string();
            return;
        }
        
        let port: u16 = match self.target_port.parse() {
            Ok(p) => p,
            Err(_) => {
                self.status_message = "Неверный порт".to_string();
                return;
            }
        };
        
        // Формируем список адресов
        let targets: Vec<String> = self.targets
            .iter()
            .map(|t| {
                if t.address.contains(':') {
                    t.address.clone()
                } else {
                    format!("{}:{}", t.address, port)
                }
            })
            .collect();
        
        // Подготавливаем файлы - если не сохраняем структуру, используем только имена
        let files: Vec<FileInfo> = if self.preserve_structure {
            self.files.clone()
        } else {
            self.files.iter().map(|f| {
                let mut file = f.clone();
                file.relative_path = file.name.clone(); // Только имя файла
                file
            }).collect()
        };
        
        let (tx, rx) = mpsc::unbounded_channel();
        self.event_rx = Some(rx);
        
        // Сбрасываем состояние
        for file in &mut self.files {
            file.transferred = 0;
            file.status = FileStatus::Pending;
        }
        for target in &mut self.targets {
            target.reset();
        }
        
        self.is_running = true;
        
        // Сбрасываем флаг остановки
        self.reset_stop_flag();
        
        // Инициализируем статистику
        self.reset_stats();
        
        let compression_str = if self.use_compression { " (LZ4)" } else { "" };
        let structure_str = if self.preserve_structure { "" } else { " [плоско]" };
        let transport_str = format!(" [{}]", self.transport_type.name());
        self.status_message = format!("Отправка на {} получателей{}{}{}...", targets.len(), compression_str, structure_str, transport_str);
        self.log(format!("Начинаем отправку на {} получателей{}{}{}", targets.len(), compression_str, structure_str, transport_str));
        
        let options = toolza_sender::network::sender::SendOptions {
            use_compression: self.use_compression,
            enable_resume: true,
            transport_type: self.transport_type,
        };
        let stop_flag = self.stop_flag.clone();
        let handle = self.runtime.spawn(async move {
            network::send_files_to_multiple_with_stop(targets, files, options, tx, stop_flag).await;
        });
        self.current_task = Some(handle);
    }
    
    // === Приём ===
    
    /// Запустить сервер
    pub fn start_receive(&mut self) {
        let port: u16 = match self.listen_port.parse() {
            Ok(p) => p,
            Err(_) => {
                self.status_message = "Неверный порт".to_string();
                return;
            }
        };
        
        let save_dir = self.save_directory.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        self.event_rx = Some(rx);
        
        self.is_running = true;
        self.received_files.clear();
        
        // Сбрасываем флаг остановки
        self.reset_stop_flag();
        
        // Формируем строку с включёнными форматами
        let mut extract_formats = Vec::new();
        if self.auto_extract_tar_lz4 { extract_formats.push("tar.lz4"); }
        if self.auto_extract_lz4 { extract_formats.push("lz4"); }
        if self.auto_extract_tar { extract_formats.push("tar"); }
        if self.auto_extract_zip { extract_formats.push("zip"); }
        if self.auto_extract_rar { extract_formats.push("rar"); }
        
        let extract_str = if extract_formats.is_empty() {
            String::new()
        } else {
            format!(" [авто-распаковка: {}]", extract_formats.join(", "))
        };
        let transport_str = format!(" [{}]", self.transport_type.name());
        self.status_message = format!("Ожидание подключений на порту {}{}{}...", port, extract_str, transport_str);
        self.log(format!("Сервер запущен на порту {}{}{}", port, extract_str, transport_str));
        
        let options = network::ServerOptions {
            extract_options: network::ExtractOptions {
                tar_lz4: self.auto_extract_tar_lz4,
                lz4: self.auto_extract_lz4,
                tar: self.auto_extract_tar,
                zip: self.auto_extract_zip,
                rar: self.auto_extract_rar,
            },
            enable_resume: true,
            transport_type: self.transport_type,
        };
        let stop_flag = self.stop_flag.clone();
        let handle = self.runtime.spawn(async move {
            let _ = network::run_server_with_options_and_stop(port, save_dir, options, tx, stop_flag).await;
        });
        self.current_task = Some(handle);
    }
    
    // === Сканирование ===
    
    /// Начать сканирование сети
    pub fn start_scan(&mut self) {
        let port: u16 = match self.target_port.parse() {
            Ok(p) => p,
            Err(_) => {
                self.status_message = "Неверный порт".to_string();
                return;
            }
        };
        
        let (tx, rx) = mpsc::unbounded_channel();
        self.event_rx = Some(rx);
        
        self.is_scanning = true;
        self.scan_progress = 0;
        self.found_servers.clear();
        
        // Сбрасываем флаг остановки
        self.reset_stop_flag();
        
        // Парсим подсети если указаны
        let subnets_input = self.subnets_input.trim().to_string();
        
        if subnets_input.is_empty() {
            self.status_message = "Сканирование локальной сети...".to_string();
            self.log(format!("Сканирование локальной подсети на порту {}", port));
            
            let handle = self.runtime.spawn(async move {
                let _ = network::scan_network(port, tx).await;
            });
            self.current_task = Some(handle);
        } else {
            let subnets = network::parse_subnets(&subnets_input);
            
            if subnets.is_empty() {
                self.is_scanning = false;
                self.status_message = "Неверный формат подсетей".to_string();
                return;
            }
            
            let subnets_str: Vec<String> = subnets.iter().map(|s| s.to_string()).collect();
            self.status_message = format!("Сканирование {} подсетей...", subnets.len());
            self.log(format!("Сканирование подсетей: {}", subnets_str.join(", ")));
            
            let handle = self.runtime.spawn(async move {
                let _ = network::scan_subnets(subnets, port, tx).await;
            });
            self.current_task = Some(handle);
        }
    }
    
    // === Управление ===
    
    /// Остановить текущую операцию
    pub fn stop(&mut self) {
        // Устанавливаем флаг остановки
        self.stop_flag.store(true, Ordering::SeqCst);
        
        // Отменяем текущую задачу
        if let Some(handle) = self.current_task.take() {
            handle.abort();
        }
        
        self.is_running = false;
        self.is_scanning = false;
        self.speedtest_running = false;
        self.event_rx = None;
        self.status_message = "Остановлено".to_string();
        self.log("⏹ Операция остановлена");
    }
    
    /// Сбросить флаг остановки перед новой операцией
    fn reset_stop_flag(&mut self) {
        self.stop_flag.store(false, Ordering::SeqCst);
        if let Some(handle) = self.current_task.take() {
            handle.abort();
        }
    }
    
    // === Спидтест ===
    
    /// Запустить спидтест
    pub fn start_speedtest(&mut self) {
        if self.speedtest_target.is_empty() {
            self.status_message = "Укажите адрес сервера".to_string();
            return;
        }
        
        let port: u16 = match self.target_port.parse() {
            Ok(p) => p,
            Err(_) => {
                self.status_message = "Неверный порт".to_string();
                return;
            }
        };
        
        let target = if self.speedtest_target.contains(':') {
            self.speedtest_target.clone()
        } else {
            format!("{}:{}", self.speedtest_target, port)
        };
        
        let (tx, rx) = mpsc::unbounded_channel();
        self.event_rx = Some(rx);
        
        self.speedtest_running = true;
        self.speedtest_progress = 0;
        self.speedtest_direction = String::new();
        self.speedtest_result = None;
        
        // Сбрасываем флаг остановки
        self.reset_stop_flag();
        
        self.status_message = format!("🚀 Спидтест к {}...", target);
        self.log(format!("Начинаем спидтест к {}", target));
        
        let size = network::DEFAULT_SPEEDTEST_SIZE;
        let handle = self.runtime.spawn(async move {
            let _ = network::run_speedtest(&target, size, tx).await;
        });
        self.current_task = Some(handle);
    }
}

