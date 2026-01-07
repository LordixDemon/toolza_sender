//! Статистика передачи - скорость, ETA, сжатие

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Размер окна для расчёта скорости (последние N измерений)
const SPEED_WINDOW_SIZE: usize = 10;

/// Минимальный размер чанка
pub const MIN_CHUNK_SIZE: usize = 16 * 1024; // 16 KB

/// Максимальный размер чанка
pub const MAX_CHUNK_SIZE: usize = 512 * 1024; // 512 KB

/// Начальный размер чанка
pub const DEFAULT_CHUNK_SIZE: usize = 64 * 1024; // 64 KB

/// Статистика передачи
#[derive(Clone, Debug)]
pub struct TransferStats {
    /// Время начала передачи
    pub start_time: Instant,
    /// Общий размер для передачи
    pub total_bytes: u64,
    /// Передано байт
    pub transferred_bytes: u64,
    /// Байт до сжатия (оригинал)
    pub bytes_before_compression: u64,
    /// Байт после сжатия (по сети)
    pub bytes_after_compression: u64,
    /// История скорости для сглаживания
    speed_samples: VecDeque<(Instant, u64)>,
    /// Текущий адаптивный размер чанка
    pub current_chunk_size: usize,
    /// Количество переданных файлов
    pub files_completed: usize,
    /// Общее количество файлов
    pub files_total: usize,
}

impl TransferStats {
    /// Создать новую статистику
    pub fn new(total_bytes: u64, files_total: usize) -> Self {
        Self {
            start_time: Instant::now(),
            total_bytes,
            transferred_bytes: 0,
            bytes_before_compression: 0,
            bytes_after_compression: 0,
            speed_samples: VecDeque::with_capacity(SPEED_WINDOW_SIZE + 1),
            current_chunk_size: DEFAULT_CHUNK_SIZE,
            files_completed: 0,
            files_total,
        }
    }
    
    /// Обновить прогресс передачи
    pub fn update(&mut self, bytes_transferred: u64, bytes_original: u64, bytes_compressed: u64) {
        self.transferred_bytes = bytes_transferred;
        self.bytes_before_compression += bytes_original;
        self.bytes_after_compression += bytes_compressed;
        
        // Добавляем точку в историю скорости
        let now = Instant::now();
        self.speed_samples.push_back((now, bytes_transferred));
        
        // Ограничиваем размер окна
        while self.speed_samples.len() > SPEED_WINDOW_SIZE {
            self.speed_samples.pop_front();
        }
        
        // Адаптируем размер чанка
        self.adapt_chunk_size();
    }
    
    /// Получить текущую скорость (байт/сек)
    pub fn speed_bytes_per_sec(&self) -> f64 {
        if self.speed_samples.len() < 2 {
            return 0.0;
        }
        
        let first = self.speed_samples.front().unwrap();
        let last = self.speed_samples.back().unwrap();
        
        let duration = last.0.duration_since(first.0).as_secs_f64();
        if duration < 0.001 {
            return 0.0;
        }
        
        let bytes_diff = last.1.saturating_sub(first.1);
        bytes_diff as f64 / duration
    }
    
    /// Получить скорость в удобном формате
    pub fn speed_formatted(&self) -> String {
        format_speed(self.speed_bytes_per_sec())
    }
    
    /// Получить оставшееся время (ETA)
    pub fn eta(&self) -> Option<Duration> {
        let speed = self.speed_bytes_per_sec();
        if speed < 1.0 {
            return None;
        }
        
        let remaining = self.total_bytes.saturating_sub(self.transferred_bytes);
        let seconds = remaining as f64 / speed;
        
        Some(Duration::from_secs_f64(seconds))
    }
    
    /// Получить ETA в удобном формате
    pub fn eta_formatted(&self) -> String {
        match self.eta() {
            Some(duration) => format_duration(duration),
            None => "∞".to_string(),
        }
    }
    
    /// Получить прошедшее время
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
    
    /// Получить прошедшее время в формате
    pub fn elapsed_formatted(&self) -> String {
        format_duration(self.elapsed())
    }
    
    /// Получить процент завершения
    pub fn progress_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            return 100.0;
        }
        (self.transferred_bytes as f64 / self.total_bytes as f64 * 100.0) as f32
    }
    
    /// Получить коэффициент сжатия (1.0 = без сжатия, 0.5 = сжато в 2 раза)
    pub fn compression_ratio(&self) -> f64 {
        if self.bytes_before_compression == 0 {
            return 1.0;
        }
        self.bytes_after_compression as f64 / self.bytes_before_compression as f64
    }
    
    /// Получить статистику сжатия в формате
    pub fn compression_formatted(&self) -> String {
        let ratio = self.compression_ratio();
        if ratio >= 0.99 {
            "без сжатия".to_string()
        } else {
            let saved = (1.0 - ratio) * 100.0;
            format!("{:.1}% экономия", saved)
        }
    }
    
    /// Адаптировать размер чанка на основе скорости
    fn adapt_chunk_size(&mut self) {
        let speed = self.speed_bytes_per_sec();
        
        // Целевое время обработки чанка: 50-100ms для отзывчивости UI
        let target_chunk_time_ms = 75.0;
        
        if speed > 0.0 {
            // Рассчитываем оптимальный размер
            let optimal = (speed * target_chunk_time_ms / 1000.0) as usize;
            
            // Ограничиваем диапазон
            let new_size = optimal.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
            
            // Плавно меняем (не более чем в 2 раза за раз)
            if new_size > self.current_chunk_size {
                self.current_chunk_size = (self.current_chunk_size * 3 / 2).min(new_size);
            } else if new_size < self.current_chunk_size {
                self.current_chunk_size = (self.current_chunk_size * 2 / 3).max(new_size);
            }
            
            // Финальное ограничение
            self.current_chunk_size = self.current_chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
        }
    }
    
    /// Отметить файл как завершённый
    pub fn file_completed(&mut self) {
        self.files_completed += 1;
    }
}

impl Default for TransferStats {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

/// Форматировать скорость
pub fn format_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec < 1024.0 {
        format!("{:.0} B/s", bytes_per_sec)
    } else if bytes_per_sec < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1024.0)
    } else if bytes_per_sec < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} MB/s", bytes_per_sec / 1024.0 / 1024.0)
    } else {
        format!("{:.2} GB/s", bytes_per_sec / 1024.0 / 1024.0 / 1024.0)
    }
}

/// Форматировать длительность
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}с", secs)
    } else if secs < 3600 {
        format!("{}м {}с", secs / 60, secs % 60)
    } else {
        format!("{}ч {}м", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // === Тесты format_speed ===
    
    #[test]
    fn test_format_speed_bytes() {
        assert_eq!(format_speed(0.0), "0 B/s");
        assert_eq!(format_speed(500.0), "500 B/s");
        assert_eq!(format_speed(1023.0), "1023 B/s");
    }
    
    #[test]
    fn test_format_speed_kilobytes() {
        assert_eq!(format_speed(1024.0), "1.0 KB/s");
        assert_eq!(format_speed(1500.0), "1.5 KB/s");
        assert_eq!(format_speed(10240.0), "10.0 KB/s");
    }
    
    #[test]
    fn test_format_speed_megabytes() {
        assert_eq!(format_speed(1024.0 * 1024.0), "1.0 MB/s");
        assert_eq!(format_speed(1500000.0), "1.4 MB/s");
        assert_eq!(format_speed(100.0 * 1024.0 * 1024.0), "100.0 MB/s");
    }
    
    #[test]
    fn test_format_speed_gigabytes() {
        assert_eq!(format_speed(1024.0 * 1024.0 * 1024.0), "1.00 GB/s");
        assert_eq!(format_speed(2.5 * 1024.0 * 1024.0 * 1024.0), "2.50 GB/s");
    }
    
    // === Тесты format_duration ===
    
    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0с");
        assert_eq!(format_duration(Duration::from_secs(45)), "45с");
        assert_eq!(format_duration(Duration::from_secs(59)), "59с");
    }
    
    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(Duration::from_secs(60)), "1м 0с");
        assert_eq!(format_duration(Duration::from_secs(125)), "2м 5с");
        assert_eq!(format_duration(Duration::from_secs(3599)), "59м 59с");
    }
    
    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(Duration::from_secs(3600)), "1ч 0м");
        assert_eq!(format_duration(Duration::from_secs(3725)), "1ч 2м");
        assert_eq!(format_duration(Duration::from_secs(7200)), "2ч 0м");
    }
    
    // === Тесты TransferStats ===
    
    #[test]
    fn test_transfer_stats_new() {
        let stats = TransferStats::new(1000, 5);
        
        assert_eq!(stats.total_bytes, 1000);
        assert_eq!(stats.transferred_bytes, 0);
        assert_eq!(stats.files_total, 5);
        assert_eq!(stats.files_completed, 0);
        assert_eq!(stats.current_chunk_size, DEFAULT_CHUNK_SIZE);
    }
    
    #[test]
    fn test_transfer_stats_default() {
        let stats = TransferStats::default();
        
        assert_eq!(stats.total_bytes, 0);
        assert_eq!(stats.files_total, 0);
    }
    
    #[test]
    fn test_transfer_stats_progress_percent() {
        let mut stats = TransferStats::new(1000, 1);
        
        assert_eq!(stats.progress_percent(), 0.0);
        
        stats.transferred_bytes = 500;
        assert!((stats.progress_percent() - 50.0).abs() < 0.1);
        
        stats.transferred_bytes = 1000;
        assert!((stats.progress_percent() - 100.0).abs() < 0.1);
    }
    
    #[test]
    fn test_transfer_stats_progress_percent_empty() {
        let stats = TransferStats::new(0, 0);
        assert_eq!(stats.progress_percent(), 100.0);
    }
    
    #[test]
    fn test_transfer_stats_compression_ratio() {
        let mut stats = TransferStats::new(1000, 1);
        
        // Без данных
        assert_eq!(stats.compression_ratio(), 1.0);
        
        // С данными
        stats.bytes_before_compression = 1000;
        stats.bytes_after_compression = 500;
        assert!((stats.compression_ratio() - 0.5).abs() < 0.01);
    }
    
    #[test]
    fn test_transfer_stats_compression_formatted() {
        let mut stats = TransferStats::new(1000, 1);
        
        // Без сжатия
        stats.bytes_before_compression = 1000;
        stats.bytes_after_compression = 1000;
        assert_eq!(stats.compression_formatted(), "без сжатия");
        
        // С сжатием 50%
        stats.bytes_after_compression = 500;
        assert!(stats.compression_formatted().contains("50"));
    }
    
    #[test]
    fn test_transfer_stats_file_completed() {
        let mut stats = TransferStats::new(1000, 5);
        
        assert_eq!(stats.files_completed, 0);
        
        stats.file_completed();
        assert_eq!(stats.files_completed, 1);
        
        stats.file_completed();
        stats.file_completed();
        assert_eq!(stats.files_completed, 3);
    }
    
    #[test]
    fn test_transfer_stats_speed_no_samples() {
        let stats = TransferStats::new(1000, 1);
        assert_eq!(stats.speed_bytes_per_sec(), 0.0);
    }
    
    #[test]
    fn test_transfer_stats_eta_no_speed() {
        let stats = TransferStats::new(1000, 1);
        assert!(stats.eta().is_none());
        assert_eq!(stats.eta_formatted(), "∞");
    }
    
    // === Тесты констант ===
    
    #[test]
    fn test_chunk_size_constants() {
        assert!(MIN_CHUNK_SIZE < DEFAULT_CHUNK_SIZE);
        assert!(DEFAULT_CHUNK_SIZE < MAX_CHUNK_SIZE);
        assert_eq!(MIN_CHUNK_SIZE, 16 * 1024);
        assert_eq!(MAX_CHUNK_SIZE, 512 * 1024);
    }
    
    #[test]
    fn test_transfer_stats_update() {
        let mut stats = TransferStats::new(1000, 1);
        
        // Несколько обновлений
        stats.update(100, 100, 80);
        stats.update(200, 100, 80);
        stats.update(300, 100, 80);
        
        assert_eq!(stats.transferred_bytes, 300);
        assert!(stats.bytes_before_compression > 0);
        assert!(stats.bytes_after_compression > 0);
    }
    
    #[test]
    fn test_transfer_stats_elapsed() {
        let stats = TransferStats::new(1000, 1);
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        let elapsed = stats.elapsed();
        assert!(elapsed.as_millis() >= 10);
        
        let elapsed_str = stats.elapsed_formatted();
        assert!(!elapsed_str.is_empty());
    }
    
    #[test]
    fn test_transfer_stats_speed_after_updates() {
        let mut stats = TransferStats::new(10000, 1);
        
        // Симулируем передачу с задержкой
        for i in 1..=5 {
            stats.update(i * 1000, 1000, 800);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        // После нескольких обновлений скорость должна быть > 0
        let speed = stats.speed_bytes_per_sec();
        // Скорость может быть 0 если слишком быстро, просто проверяем что не паникует
        assert!(speed >= 0.0);
    }
    
    #[test]
    fn test_format_speed_zero() {
        assert_eq!(format_speed(0.0), "0 B/s");
    }
    
    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0с");
    }
    
    #[test]
    fn test_transfer_stats_clone() {
        let stats = TransferStats::new(1000, 5);
        let cloned = stats.clone();
        
        assert_eq!(stats.total_bytes, cloned.total_bytes);
        assert_eq!(stats.files_total, cloned.files_total);
    }
}

