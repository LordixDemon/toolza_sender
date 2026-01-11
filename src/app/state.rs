//! Состояние приложения

use toolza_sender::history::TransferHistory;
use toolza_sender::i18n::{Language, Translations, t};
use toolza_sender::network::{TransferEvent, TransportType};
use toolza_sender::protocol::{FileInfo, DEFAULT_PORT};
use toolza_sender::stats::TransferStats;
use toolza_sender::utils::get_local_ip_string;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Тип результата файлового диалога
pub enum DialogResult {
    Files(Vec<PathBuf>),
    Folder(PathBuf),
    SaveDirectory(PathBuf),
    ArchiveFile(PathBuf),
    ExtractDestination(PathBuf),
    ExtractComplete(String),
}

/// Режим работы приложения
#[derive(PartialEq, Clone, Copy)]
pub enum Mode {
    Send,
    Receive,
    Extract,
    History,
    SpeedTest,
}

/// Информация о получателе
#[derive(Clone)]
pub struct TargetInfo {
    pub address: String,
    pub status: TargetStatus,
    pub current_file: usize,
    pub files_completed: usize,
}

impl TargetInfo {
    pub fn new(address: String) -> Self {
        Self {
            address,
            status: TargetStatus::Pending,
            current_file: 0,
            files_completed: 0,
        }
    }
    
    pub fn reset(&mut self) {
        self.status = TargetStatus::Connecting;
        self.current_file = 0;
        self.files_completed = 0;
    }
}

/// Статус получателя
#[derive(Clone, PartialEq)]
pub enum TargetStatus {
    Pending,
    Connecting,
    Transferring,
    Completed,
    Error(String),
}

/// Главная структура приложения
pub struct App {
    // Язык интерфейса
    pub language: Language,
    
    // Режим работы
    pub mode: Mode,
    
    // === Отправка ===
    pub new_target_address: String,
    pub target_port: String,
    pub targets: Vec<TargetInfo>,
    pub files: Vec<FileInfo>,
    /// Использовать LZ4 сжатие при передаче
    pub use_compression: bool,
    /// Сохранять структуру папок при передаче
    pub preserve_structure: bool,
    /// Режим синхронизации (передавать только изменённые файлы)
    pub sync_mode: bool,
    /// Тип транспортного протокола (TCP/QUIC)
    pub transport_type: TransportType,
    
    // === Приём ===
    pub listen_port: String,
    pub save_directory: PathBuf,
    pub received_files: Vec<(String, u64)>,
    /// Автоматически распаковывать tar.lz4 архивы
    pub auto_extract_tar_lz4: bool,
    /// Автоматически распаковывать tar.zst архивы
    pub auto_extract_tar_zst: bool,
    /// Автоматически распаковывать .lz4 файлы (не tar)
    pub auto_extract_lz4: bool,
    /// Автоматически распаковывать tar/tar.gz архивы
    pub auto_extract_tar: bool,
    /// Автоматически распаковывать zip архивы
    pub auto_extract_zip: bool,
    /// Автоматически распаковывать rar архивы (требует unrar)
    pub auto_extract_rar: bool,
    /// Сохранять архив при потоковой распаковке (для резюме)
    pub save_archive_for_resume: bool,
    
    // === Общее состояние ===
    pub is_running: bool,
    pub status_message: String,
    pub log_messages: Vec<String>,
    
    // === Сканирование ===
    pub is_scanning: bool,
    pub scan_progress: u8,
    pub found_servers: Vec<String>,
    pub local_ip: String,
    /// Подсети для сканирования (пустая строка = автоопределение)
    pub subnets_input: String,
    
    // === Статистика ===
    pub stats: TransferStats,
    pub transfer_start_time: Option<Instant>,
    /// Байты до сжатия (оригинал)
    pub bytes_original: u64,
    /// Байты после сжатия (по сети)
    pub bytes_compressed: u64,
    
    // === История ===
    pub history: TransferHistory,
    
    // === Drag & Drop ===
    pub dropped_files: Vec<PathBuf>,
    
    // === Спидтест ===
    pub speedtest_target: String,
    pub speedtest_running: bool,
    pub speedtest_progress: u8,
    pub speedtest_direction: String,
    pub speedtest_result: Option<toolza_sender::network::SpeedTestResult>,
    
    // === Локальная распаковка ===
    /// Путь к архиву для распаковки
    pub extract_archive_path: Option<PathBuf>,
    /// Папка назначения для распаковки
    pub extract_destination: PathBuf,
    /// Идёт распаковка
    pub extract_running: bool,
    /// Результат распаковки (кол-во файлов, размер)
    pub extract_result: Option<String>,
    /// Флаг остановки распаковки
    pub extract_stop_flag: Arc<AtomicBool>,
    
    // === Окно распаковки на лету ===
    /// Показывать окно распаковки
    pub extraction_window_open: bool,
    /// Имя текущего распаковываемого архива
    pub extraction_filename: String,
    /// Количество распакованных файлов
    pub extraction_files_count: usize,
    /// Общий размер распакованных данных
    pub extraction_total_size: u64,
    /// Текущий файл в процессе распаковки
    pub extraction_current_file: String,
    
    // === Runtime ===
    pub runtime: tokio::runtime::Runtime,
    pub event_rx: Option<mpsc::UnboundedReceiver<TransferEvent>>,
    /// Флаг для остановки текущей операции
    pub stop_flag: Arc<AtomicBool>,
    /// Handle текущей задачи для возможности отмены
    pub current_task: Option<JoinHandle<()>>,
    
    // === Файловые диалоги (асинхронные) ===
    pub dialog_tx: mpsc::UnboundedSender<DialogResult>,
    pub dialog_rx: Option<mpsc::UnboundedReceiver<DialogResult>>,
}

impl App {
    pub fn new() -> Self {
        let save_dir = dirs::download_dir()
            .unwrap_or_else(|| PathBuf::from("."));
        
        // Загружаем историю
        let history = TransferHistory::load();
        
        // Канал для результатов файловых диалогов
        let (dialog_tx, dialog_rx) = mpsc::unbounded_channel();
        
        Self {
            language: Language::default(),
            mode: Mode::Send,
            new_target_address: String::new(),
            target_port: DEFAULT_PORT.to_string(),
            targets: Vec::new(),
            files: Vec::new(),
            use_compression: false,
            preserve_structure: false,
            sync_mode: false,
            transport_type: TransportType::default(),
            listen_port: DEFAULT_PORT.to_string(),
            save_directory: save_dir.clone(),
            received_files: Vec::new(),
            auto_extract_tar_lz4: false,
            auto_extract_tar_zst: false,
            auto_extract_lz4: false,
            auto_extract_tar: false,
            auto_extract_zip: false,
            auto_extract_rar: false,
            save_archive_for_resume: false,
            is_running: false,
            status_message: String::new(),
            log_messages: Vec::new(),
            is_scanning: false,
            scan_progress: 0,
            found_servers: Vec::new(),
            local_ip: get_local_ip_string(),
            subnets_input: String::new(),
            stats: TransferStats::default(),
            transfer_start_time: None,
            bytes_original: 0,
            bytes_compressed: 0,
            history,
            dropped_files: Vec::new(),
            speedtest_target: String::new(),
            speedtest_running: false,
            speedtest_progress: 0,
            speedtest_direction: String::new(),
            speedtest_result: None,
            extract_archive_path: None,
            extract_destination: save_dir,
            extract_running: false,
            extract_result: None,
            extract_stop_flag: Arc::new(AtomicBool::new(false)),
            extraction_window_open: false,
            extraction_filename: String::new(),
            extraction_files_count: 0,
            extraction_total_size: 0,
            extraction_current_file: String::new(),
            runtime: tokio::runtime::Runtime::new().unwrap(),
            event_rx: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
            current_task: None,
            dialog_tx,
            dialog_rx: Some(dialog_rx),
        }
    }
    
    /// Добавить сообщение в лог
    pub fn log(&mut self, message: impl Into<String>) {
        self.log_messages.push(message.into());
    }
    
    /// Проверить, можно ли редактировать настройки
    pub fn can_edit(&self) -> bool {
        !self.is_running && !self.is_scanning
    }
    
    /// Получить текущую скорость передачи
    pub fn current_speed(&self) -> String {
        if self.is_running {
            self.stats.speed_formatted()
        } else {
            "—".to_string()
        }
    }
    
    /// Получить ETA
    pub fn current_eta(&self) -> String {
        if self.is_running {
            self.stats.eta_formatted()
        } else {
            "—".to_string()
        }
    }
    
    /// Получить статистику сжатия
    pub fn compression_stats(&self) -> String {
        if self.bytes_original > 0 && self.use_compression {
            let ratio = self.bytes_compressed as f64 / self.bytes_original as f64;
            if ratio < 0.99 {
                let saved = (1.0 - ratio) * 100.0;
                format!("{:.1}% экономия", saved)
            } else {
                "без эффекта".to_string()
            }
        } else {
            "—".to_string()
        }
    }
    
    /// Сбросить статистику для новой передачи
    pub fn reset_stats(&mut self) {
        let total_size: u64 = self.files.iter().map(|f| f.size).sum();
        self.stats = TransferStats::new(total_size, self.files.len());
        self.transfer_start_time = Some(Instant::now());
        self.bytes_original = 0;
        self.bytes_compressed = 0;
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Получить переводы для текущего языка
    pub fn t(&self) -> &'static Translations {
        t(self.language)
    }
}
