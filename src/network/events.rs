//! События сетевого модуля для GUI

/// События передачи для GUI
#[derive(Debug, Clone)]
pub enum TransferEvent {
    // === События отправки ===
    
    /// Соединение установлено (target_id, адрес)
    Connected(usize, String),
    /// Начало передачи файла (target_id, file_idx)
    FileStarted(usize, usize),
    /// Прогресс передачи (target_id, file_idx, transferred, original_bytes, compressed_bytes)
    Progress(usize, usize, u64, u64, u64),
    /// Файл завершён (target_id, file_idx)
    FileCompleted(usize, usize),
    /// Ошибка файла (target_id, file_idx, error)
    FileError(usize, usize, String),
    /// Все файлы переданы на получателя (target_id)
    TargetCompleted(usize),
    /// Все получатели завершены
    AllCompleted,
    /// Ошибка соединения (target_id, error)
    ConnectionError(usize, String),
    /// Файл пропущен (уже актуален) - для sync режима
    FileSkipped(usize, usize),
    /// Файл возобновлён с позиции (target_id, file_idx, offset)
    FileResumed(usize, usize, u64),
    
    // === События приёма ===
    
    /// Клиент отключился
    Disconnected,
    /// Получен файл (имя, размер)
    FileReceived(String, u64),
    /// Начата распаковка архива (имя файла)
    ExtractionStarted(String),
    /// Распаковка завершена (имя файла, кол-во файлов, общий размер)
    ExtractionCompleted(String, usize, u64),
    /// Ошибка распаковки (имя файла, ошибка)
    ExtractionError(String, String),
    
    // === События сканирования ===
    
    /// Найден сервер
    ServerFound(String),
    /// Прогресс сканирования (текущий IP, процент)
    ScanProgress(String, u8),
    /// Сканирование завершено
    ScanCompleted,
    
    // === События спидтеста ===
    
    /// Спидтест запущен (адрес)
    SpeedTestStarted(String),
    /// Прогресс спидтеста (направление: "upload"/"download", процент)
    SpeedTestProgress(String, u8),
    /// Спидтест завершён (upload MB/s, download MB/s, latency ms)
    SpeedTestCompleted(f64, f64, f64),
    /// Ошибка спидтеста
    SpeedTestError(String),
}

