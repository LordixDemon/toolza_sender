//! История передач - сохранение и загрузка

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Максимальное количество записей в истории
const MAX_HISTORY_ENTRIES: usize = 100;

/// Запись в истории передач
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Временная метка (Unix timestamp)
    pub timestamp: u64,
    /// Тип операции
    pub operation: OperationType,
    /// Направление (отправка/приём)
    pub direction: Direction,
    /// Количество файлов
    pub files_count: usize,
    /// Общий размер в байтах
    pub total_size: u64,
    /// Длительность в секундах
    pub duration_secs: f64,
    /// Средняя скорость (байт/сек)
    pub avg_speed: f64,
    /// Коэффициент сжатия
    pub compression_ratio: f64,
    /// Адреса (получатели или отправитель)
    pub addresses: Vec<String>,
    /// Успешно ли завершено
    pub success: bool,
    /// Сообщение об ошибке (если есть)
    pub error: Option<String>,
}

/// Тип операции
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum OperationType {
    Transfer,
    Sync,
}

/// Направление передачи
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Direction {
    Send,
    Receive,
}

impl HistoryEntry {
    /// Создать новую запись для отправки
    pub fn new_send(
        files_count: usize,
        total_size: u64,
        duration_secs: f64,
        compression_ratio: f64,
        addresses: Vec<String>,
        success: bool,
        error: Option<String>,
    ) -> Self {
        let avg_speed = if duration_secs > 0.0 {
            total_size as f64 / duration_secs
        } else {
            0.0
        };
        
        Self {
            timestamp: current_timestamp(),
            operation: OperationType::Transfer,
            direction: Direction::Send,
            files_count,
            total_size,
            duration_secs,
            avg_speed,
            compression_ratio,
            addresses,
            success,
            error,
        }
    }
    
    /// Создать новую запись для приёма
    pub fn new_receive(
        files_count: usize,
        total_size: u64,
        duration_secs: f64,
        address: String,
        success: bool,
        error: Option<String>,
    ) -> Self {
        let avg_speed = if duration_secs > 0.0 {
            total_size as f64 / duration_secs
        } else {
            0.0
        };
        
        Self {
            timestamp: current_timestamp(),
            operation: OperationType::Transfer,
            direction: Direction::Receive,
            files_count,
            total_size,
            duration_secs,
            avg_speed,
            compression_ratio: 1.0,
            addresses: vec![address],
            success,
            error,
        }
    }
    
    /// Форматировать дату/время
    pub fn formatted_time(&self) -> String {
        // Простое форматирование без внешних зависимостей
        let secs_per_day = 86400u64;
        let secs_per_hour = 3600u64;
        let secs_per_min = 60u64;
        
        // Приблизительное время (без учёта часовых поясов)
        let total_days = self.timestamp / secs_per_day;
        let day_secs = self.timestamp % secs_per_day;
        let hours = day_secs / secs_per_hour;
        let mins = (day_secs % secs_per_hour) / secs_per_min;
        
        // Вычисляем дату (приблизительно, начиная с 1970)
        let mut year = 1970u32;
        let mut remaining_days = total_days;
        
        loop {
            let days_in_year = if is_leap_year(year) { 366 } else { 365 };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }
        
        let (month, day) = days_to_month_day(remaining_days as u32, is_leap_year(year));
        
        format!("{:02}.{:02}.{} {:02}:{:02}", day, month, year, hours, mins)
    }
    
    /// Форматировать размер
    pub fn formatted_size(&self) -> String {
        crate::utils::format_size(self.total_size)
    }
    
    /// Форматировать скорость
    pub fn formatted_speed(&self) -> String {
        crate::stats::format_speed(self.avg_speed)
    }
    
    /// Форматировать длительность
    pub fn formatted_duration(&self) -> String {
        crate::stats::format_duration(std::time::Duration::from_secs_f64(self.duration_secs))
    }
}

/// История передач
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TransferHistory {
    pub entries: Vec<HistoryEntry>,
}

impl TransferHistory {
    /// Создать пустую историю
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
    
    /// Загрузить историю из файла
    pub fn load() -> Self {
        let path = history_file_path();
        
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Ok(history) = serde_json::from_str(&contents) {
                return history;
            }
        }
        
        Self::new()
    }
    
    /// Сохранить историю в файл
    pub fn save(&self) -> std::io::Result<()> {
        let path = history_file_path();
        
        // Создаём директорию если нужно
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)
    }
    
    /// Добавить запись
    pub fn add(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry); // Новые записи в начало
        
        // Ограничиваем размер
        if self.entries.len() > MAX_HISTORY_ENTRIES {
            self.entries.truncate(MAX_HISTORY_ENTRIES);
        }
        
        // Сохраняем
        let _ = self.save();
    }
    
    /// Очистить историю
    pub fn clear(&mut self) {
        self.entries.clear();
        let _ = self.save();
    }
    
    /// Получить общую статистику
    pub fn total_stats(&self) -> HistoryStats {
        let mut stats = HistoryStats::default();
        
        for entry in &self.entries {
            if entry.success {
                match entry.direction {
                    Direction::Send => {
                        stats.total_sent += entry.total_size;
                        stats.files_sent += entry.files_count;
                    }
                    Direction::Receive => {
                        stats.total_received += entry.total_size;
                        stats.files_received += entry.files_count;
                    }
                }
            }
        }
        
        stats.total_transfers = self.entries.len();
        stats.successful_transfers = self.entries.iter().filter(|e| e.success).count();
        
        stats
    }
}

/// Статистика истории
#[derive(Clone, Debug, Default)]
pub struct HistoryStats {
    pub total_transfers: usize,
    pub successful_transfers: usize,
    pub total_sent: u64,
    pub total_received: u64,
    pub files_sent: usize,
    pub files_received: usize,
}

/// Получить путь к файлу истории
fn history_file_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("toolza_sender")
        .join("history.json")
}

/// Текущий Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Проверка високосного года
fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Преобразовать день года в месяц и день
fn days_to_month_day(day_of_year: u32, leap: bool) -> (u32, u32) {
    let days_in_months: [u32; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    
    let mut remaining = day_of_year;
    for (month, &days) in days_in_months.iter().enumerate() {
        if remaining < days {
            return (month as u32 + 1, remaining + 1);
        }
        remaining -= days;
    }
    
    (12, 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // === Тесты HistoryEntry ===
    
    #[test]
    fn test_history_entry_new_send() {
        let entry = HistoryEntry::new_send(
            5,
            1024 * 1024, // 1 MB
            10.0,
            0.8,
            vec!["192.168.1.100".to_string()],
            true,
            None,
        );
        
        assert_eq!(entry.files_count, 5);
        assert_eq!(entry.total_size, 1024 * 1024);
        assert_eq!(entry.duration_secs, 10.0);
        assert!((entry.avg_speed - 104857.6).abs() < 1.0);
        assert_eq!(entry.compression_ratio, 0.8);
        assert_eq!(entry.direction, Direction::Send);
        assert_eq!(entry.operation, OperationType::Transfer);
        assert!(entry.success);
        assert!(entry.error.is_none());
    }
    
    #[test]
    fn test_history_entry_new_receive() {
        let entry = HistoryEntry::new_receive(
            3,
            500000,
            5.0,
            "192.168.1.50".to_string(),
            true,
            None,
        );
        
        assert_eq!(entry.files_count, 3);
        assert_eq!(entry.direction, Direction::Receive);
        assert_eq!(entry.addresses, vec!["192.168.1.50".to_string()]);
        assert_eq!(entry.compression_ratio, 1.0);
    }
    
    #[test]
    fn test_history_entry_speed_zero_duration() {
        let entry = HistoryEntry::new_send(
            1,
            1000,
            0.0, // Нулевая длительность
            1.0,
            vec![],
            true,
            None,
        );
        
        assert_eq!(entry.avg_speed, 0.0);
    }
    
    #[test]
    fn test_history_entry_with_error() {
        let entry = HistoryEntry::new_send(
            0,
            0,
            0.0,
            1.0,
            vec!["192.168.1.100".to_string()],
            false,
            Some("Connection refused".to_string()),
        );
        
        assert!(!entry.success);
        assert_eq!(entry.error, Some("Connection refused".to_string()));
    }
    
    // === Тесты TransferHistory ===
    
    #[test]
    fn test_transfer_history_new() {
        let history = TransferHistory::new();
        assert!(history.entries.is_empty());
    }
    
    #[test]
    fn test_transfer_history_add() {
        let mut history = TransferHistory::new();
        
        let entry = HistoryEntry::new_send(1, 100, 1.0, 1.0, vec![], true, None);
        history.entries.push(entry);
        
        assert_eq!(history.entries.len(), 1);
    }
    
    #[test]
    fn test_transfer_history_clear() {
        let mut history = TransferHistory::new();
        
        history.entries.push(HistoryEntry::new_send(1, 100, 1.0, 1.0, vec![], true, None));
        history.entries.push(HistoryEntry::new_send(2, 200, 2.0, 1.0, vec![], true, None));
        
        assert_eq!(history.entries.len(), 2);
        
        history.entries.clear();
        assert!(history.entries.is_empty());
    }
    
    #[test]
    fn test_transfer_history_total_stats() {
        let mut history = TransferHistory::new();
        
        // Успешная отправка
        history.entries.push(HistoryEntry::new_send(
            5, 1000, 1.0, 1.0, vec![], true, None,
        ));
        
        // Успешный приём
        history.entries.push(HistoryEntry::new_receive(
            3, 500, 0.5, "addr".to_string(), true, None,
        ));
        
        // Неуспешная отправка (не должна учитываться)
        history.entries.push(HistoryEntry::new_send(
            10, 10000, 0.0, 1.0, vec![], false, Some("error".to_string()),
        ));
        
        let stats = history.total_stats();
        
        assert_eq!(stats.total_transfers, 3);
        assert_eq!(stats.successful_transfers, 2);
        assert_eq!(stats.total_sent, 1000);
        assert_eq!(stats.total_received, 500);
        assert_eq!(stats.files_sent, 5);
        assert_eq!(stats.files_received, 3);
    }
    
    #[test]
    fn test_transfer_history_stats_empty() {
        let history = TransferHistory::new();
        let stats = history.total_stats();
        
        assert_eq!(stats.total_transfers, 0);
        assert_eq!(stats.successful_transfers, 0);
        assert_eq!(stats.total_sent, 0);
        assert_eq!(stats.total_received, 0);
    }
    
    // === Тесты вспомогательных функций ===
    
    #[test]
    fn test_is_leap_year() {
        assert!(!is_leap_year(1900)); // Делится на 100, но не на 400
        assert!(is_leap_year(2000));  // Делится на 400
        assert!(is_leap_year(2024));  // Делится на 4
        assert!(!is_leap_year(2023)); // Не делится на 4
    }
    
    #[test]
    fn test_days_to_month_day() {
        // 1 января (день 0)
        assert_eq!(days_to_month_day(0, false), (1, 1));
        
        // 31 января (день 30)
        assert_eq!(days_to_month_day(30, false), (1, 31));
        
        // 1 февраля (день 31)
        assert_eq!(days_to_month_day(31, false), (2, 1));
        
        // 29 февраля в високосный год (день 59)
        assert_eq!(days_to_month_day(59, true), (2, 29));
        
        // 1 марта в обычный год (день 59)
        assert_eq!(days_to_month_day(59, false), (3, 1));
    }
    
    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        // Должен быть больше чем 1 января 2024
        assert!(ts > 1704067200);
    }
    
    // === Тесты сериализации ===
    
    #[test]
    fn test_history_serialization() {
        let mut history = TransferHistory::new();
        history.entries.push(HistoryEntry::new_send(
            5, 1000, 10.0, 0.9, vec!["addr1".to_string()], true, None,
        ));
        
        let json = serde_json::to_string(&history).unwrap();
        let deserialized: TransferHistory = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.entries.len(), 1);
        assert_eq!(deserialized.entries[0].files_count, 5);
    }
    
    #[test]
    fn test_direction_equality() {
        assert_eq!(Direction::Send, Direction::Send);
        assert_eq!(Direction::Receive, Direction::Receive);
        assert_ne!(Direction::Send, Direction::Receive);
    }
    
    #[test]
    fn test_operation_type_equality() {
        assert_eq!(OperationType::Transfer, OperationType::Transfer);
        assert_eq!(OperationType::Sync, OperationType::Sync);
        assert_ne!(OperationType::Transfer, OperationType::Sync);
    }
}

