//! Модуль интернационализации (i18n)
//! 
//! Поддерживаемые языки: русский, украинский, английский

mod translations;

pub use translations::*;

/// Поддерживаемые языки
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Russian,
    Ukrainian,
    English,
}

impl Language {
    /// Название языка на этом языке
    pub fn native_name(&self) -> &'static str {
        match self {
            Language::Russian => "Русский",
            Language::Ukrainian => "Українська",
            Language::English => "English",
        }
    }
    
    /// Флаг/эмодзи для языка
    pub fn flag(&self) -> &'static str {
        match self {
            Language::Russian => "🇷🇺",
            Language::Ukrainian => "🇺🇦",
            Language::English => "🇬🇧",
        }
    }
    
    /// Короткий код языка
    pub fn code(&self) -> &'static str {
        match self {
            Language::Russian => "ru",
            Language::Ukrainian => "uk",
            Language::English => "en",
        }
    }
    
    /// Все доступные языки
    pub fn all() -> &'static [Language] {
        &[Language::Russian, Language::Ukrainian, Language::English]
    }
}

/// Структура с переводами всех строк интерфейса
#[derive(Debug, Clone)]
pub struct Translations {
    // === Главное меню ===
    pub app_title: &'static str,
    pub mode_send: &'static str,
    pub mode_receive: &'static str,
    pub mode_history: &'static str,
    pub mode_speedtest: &'static str,
    
    // === Отправка ===
    pub send_title: &'static str,
    pub your_ip: &'static str,
    pub port: &'static str,
    pub recipients: &'static str,
    pub ip_address: &'static str,
    pub add: &'static str,
    pub subnets: &'static str,
    pub subnets_hint: &'static str,
    pub subnets_tooltip: &'static str,
    pub find_servers: &'static str,
    pub cancel: &'static str,
    pub clear: &'static str,
    pub found_servers: &'static str,
    pub files: &'static str,
    pub folder: &'static str,
    pub options: &'static str,
    pub lz4_compression: &'static str,
    pub lz4_tooltip: &'static str,
    pub preserve_structure: &'static str,
    pub preserve_structure_tooltip: &'static str,
    pub sync_mode: &'static str,
    pub sync_mode_tooltip: &'static str,
    pub protocol: &'static str,
    pub stop: &'static str,
    pub send_to_recipients: &'static str,
    pub files_to_send: &'static str,
    pub or_drag_drop: &'static str,
    pub eta: &'static str,
    pub compression_stats: &'static str,
    
    // === Приём ===
    pub receive_title: &'static str,
    pub your_address: &'static str,
    pub save_folder: &'static str,
    pub choose: &'static str,
    pub auto_extract: &'static str,
    pub start_server: &'static str,
    pub stop_server: &'static str,
    pub received_files: &'static str,
    pub extract_tooltip_tar_lz4: &'static str,
    pub extract_tooltip_lz4: &'static str,
    pub extract_tooltip_tar: &'static str,
    pub extract_tooltip_zip: &'static str,
    pub extract_tooltip_rar: &'static str,
    
    // === История ===
    pub history_title: &'static str,
    pub clear_history: &'static str,
    pub no_history: &'static str,
    pub direction_sent: &'static str,
    pub direction_received: &'static str,
    
    // === Спидтест ===
    pub speedtest_title: &'static str,
    pub target_address: &'static str,
    pub start_test: &'static str,
    pub testing: &'static str,
    pub ping: &'static str,
    pub upload: &'static str,
    pub download: &'static str,
    pub test_results: &'static str,
    
    // === Общее ===
    pub status: &'static str,
    pub log: &'static str,
    pub error: &'static str,
    pub success: &'static str,
    pub connecting: &'static str,
    pub connected: &'static str,
    pub disconnected: &'static str,
    pub transferring: &'static str,
    pub completed: &'static str,
    pub pending: &'static str,
    pub waiting_connections: &'static str,
    pub server_started: &'static str,
    pub file_received: &'static str,
    pub extraction_started: &'static str,
    pub extraction_completed: &'static str,
    pub extraction_error: &'static str,
    pub invalid_port: &'static str,
    pub no_files_selected: &'static str,
    pub no_recipients: &'static str,
    
    // === Протоколы ===
    pub tcp_description: &'static str,
    pub udp_description: &'static str,
    pub quic_description: &'static str,
    pub kcp_description: &'static str,
}

impl Translations {
    /// Получить переводы для указанного языка
    pub fn for_language(lang: Language) -> &'static Translations {
        match lang {
            Language::Russian => &translations::RU,
            Language::Ukrainian => &translations::UK,
            Language::English => &translations::EN,
        }
    }
}

/// Глобальный доступ к текущему языку (для удобства)
pub fn t(lang: Language) -> &'static Translations {
    Translations::for_language(lang)
}

