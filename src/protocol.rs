use serde::{Deserialize, Serialize};

/// Версия протокола
pub const PROTOCOL_VERSION: u8 = 2;

/// Порт по умолчанию
pub const DEFAULT_PORT: u16 = 9527;

/// Сообщения протокола
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    /// Начало передачи файла
    FileStart {
        filename: String,
        size: u64,
        /// Используется ли LZ4 сжатие
        compressed: bool,
        /// Смещение для возобновления (0 = с начала)
        #[serde(default)]
        offset: u64,
        /// Быстрый хэш для синхронизации
        #[serde(default)]
        quick_hash: u64,
    },
    /// Кусок данных файла (возможно сжатый)
    FileChunk {
        data: Vec<u8>,
        /// Размер оригинальных данных (до сжатия)
        #[serde(default)]
        original_size: usize,
    },
    /// Конец файла
    FileEnd,
    /// Подтверждение получения
    Ack,
    /// Ответ на FileStart для resume
    ResumeAck {
        /// Смещение с которого продолжить (0 = с начала, size = файл уже есть)
        offset: u64,
    },
    /// Ошибка
    Error(String),
    /// Завершение сессии
    Done,
    /// Отмена передачи (получатель отказался)
    Cancel,
    
    // === Сообщения для синхронизации ===
    
    /// Запрос списка файлов для синхронизации
    SyncRequest,
    /// Ответ со списком файлов на стороне получателя
    SyncFileList {
        files: Vec<SyncFileEntry>,
    },
    
    // === Сообщения для спидтеста ===
    
    /// Запрос на спидтест (размер данных в байтах)
    SpeedTestRequest {
        size: u64,
    },
    /// Готовность к спидтесту
    SpeedTestReady,
    /// Блок данных спидтеста
    SpeedTestData {
        data: Vec<u8>,
    },
    /// Завершение спидтеста
    SpeedTestEnd,
    /// Результат спидтеста (скорость в байтах/сек)
    SpeedTestResult {
        upload_speed: f64,
        download_speed: f64,
        latency_ms: f64,
    },
}

/// Запись о файле для синхронизации
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncFileEntry {
    pub relative_path: String,
    pub size: u64,
    pub quick_hash: u64,
}

impl Message {
    /// Сериализовать сообщение в байты с префиксом длины
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        let data = bincode::serialize(self)?;
        let len = (data.len() as u32).to_le_bytes();
        let mut result = Vec::with_capacity(4 + data.len());
        result.extend_from_slice(&len);
        result.extend(data);
        Ok(result)
    }

    /// Десериализовать сообщение из байтов (без префикса длины)
    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }
}

/// Статус файла в очереди
#[derive(Clone, Debug, PartialEq)]
pub enum FileStatus {
    Pending,
    Transferring,
    Completed,
    Error(String),
}

/// Информация о файле для передачи
#[derive(Clone, Debug)]
pub struct FileInfo {
    /// Полный путь к файлу на диске
    pub path: std::path::PathBuf,
    /// Имя файла для отображения
    pub name: String,
    /// Относительный путь для сохранения структуры папок
    pub relative_path: String,
    pub size: u64,
    pub transferred: u64,
    pub status: FileStatus,
}

impl FileInfo {
    /// Создать FileInfo для одиночного файла
    pub fn new(path: std::path::PathBuf) -> std::io::Result<Self> {
        let metadata = std::fs::metadata(&path)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        Ok(Self {
            path,
            name: name.clone(),
            relative_path: name, // Для одиночного файла - просто имя
            size: metadata.len(),
            transferred: 0,
            status: FileStatus::Pending,
        })
    }
    
    /// Создать FileInfo с относительным путём (для папок)
    pub fn with_relative_path(path: std::path::PathBuf, relative_path: String) -> std::io::Result<Self> {
        let metadata = std::fs::metadata(&path)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        Ok(Self {
            path,
            name,
            relative_path,
            size: metadata.len(),
            transferred: 0,
            status: FileStatus::Pending,
        })
    }

    pub fn progress(&self) -> f32 {
        if self.size == 0 {
            return 1.0;
        }
        self.transferred as f32 / self.size as f32
    }
}

/// Рекурсивно собрать все файлы из папки
pub fn collect_files_from_folder(folder: &std::path::Path) -> std::io::Result<Vec<FileInfo>> {
    let mut files = Vec::new();
    let folder_name = folder
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "folder".to_string());
    
    collect_files_recursive(folder, &folder_name, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(
    current_path: &std::path::Path,
    relative_base: &str,
    files: &mut Vec<FileInfo>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(current_path)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        
        // Формируем относительный путь
        let relative_path = if relative_base.is_empty() {
            file_name.clone()
        } else {
            format!("{}/{}", relative_base, file_name)
        };
        
        if path.is_dir() {
            // Рекурсивно обходим подпапки
            collect_files_recursive(&path, &relative_path, files)?;
        } else if path.is_file() {
            // Добавляем файл
            if let Ok(info) = FileInfo::with_relative_path(path, relative_path) {
                files.push(info);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    // === Тесты Message ===
    
    #[test]
    fn test_message_file_start_serialization() {
        let msg = Message::FileStart {
            filename: "test.txt".to_string(),
            size: 1024,
            compressed: true,
            offset: 0,
            quick_hash: 12345,
        };
        
        let bytes = msg.to_bytes().unwrap();
        assert!(bytes.len() > 4); // Минимум 4 байта на длину
        
        // Проверяем что первые 4 байта - это длина
        let len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        assert_eq!(len, bytes.len() - 4);
        
        // Десериализуем обратно
        let decoded = Message::from_bytes(&bytes[4..]).unwrap();
        match decoded {
            Message::FileStart { filename, size, compressed, offset, quick_hash } => {
                assert_eq!(filename, "test.txt");
                assert_eq!(size, 1024);
                assert!(compressed);
                assert_eq!(offset, 0);
                assert_eq!(quick_hash, 12345);
            }
            _ => panic!("Wrong message type"),
        }
    }
    
    #[test]
    fn test_message_file_chunk_serialization() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let msg = Message::FileChunk {
            data: data.clone(),
            original_size: 8,
        };
        
        let bytes = msg.to_bytes().unwrap();
        let decoded = Message::from_bytes(&bytes[4..]).unwrap();
        
        match decoded {
            Message::FileChunk { data: decoded_data, original_size } => {
                assert_eq!(decoded_data, data);
                assert_eq!(original_size, 8);
            }
            _ => panic!("Wrong message type"),
        }
    }
    
    #[test]
    fn test_message_ack_serialization() {
        let msg = Message::Ack;
        let bytes = msg.to_bytes().unwrap();
        let decoded = Message::from_bytes(&bytes[4..]).unwrap();
        assert!(matches!(decoded, Message::Ack));
    }
    
    #[test]
    fn test_message_resume_ack_serialization() {
        let msg = Message::ResumeAck { offset: 5000 };
        let bytes = msg.to_bytes().unwrap();
        let decoded = Message::from_bytes(&bytes[4..]).unwrap();
        
        match decoded {
            Message::ResumeAck { offset } => assert_eq!(offset, 5000),
            _ => panic!("Wrong message type"),
        }
    }
    
    #[test]
    fn test_message_error_serialization() {
        let msg = Message::Error("Test error".to_string());
        let bytes = msg.to_bytes().unwrap();
        let decoded = Message::from_bytes(&bytes[4..]).unwrap();
        
        match decoded {
            Message::Error(err) => assert_eq!(err, "Test error"),
            _ => panic!("Wrong message type"),
        }
    }
    
    #[test]
    fn test_message_done_serialization() {
        let msg = Message::Done;
        let bytes = msg.to_bytes().unwrap();
        let decoded = Message::from_bytes(&bytes[4..]).unwrap();
        assert!(matches!(decoded, Message::Done));
    }
    
    #[test]
    fn test_sync_file_list_serialization() {
        let msg = Message::SyncFileList {
            files: vec![
                SyncFileEntry {
                    relative_path: "file1.txt".to_string(),
                    size: 100,
                    quick_hash: 111,
                },
                SyncFileEntry {
                    relative_path: "dir/file2.txt".to_string(),
                    size: 200,
                    quick_hash: 222,
                },
            ],
        };
        
        let bytes = msg.to_bytes().unwrap();
        let decoded = Message::from_bytes(&bytes[4..]).unwrap();
        
        match decoded {
            Message::SyncFileList { files } => {
                assert_eq!(files.len(), 2);
                assert_eq!(files[0].relative_path, "file1.txt");
                assert_eq!(files[1].size, 200);
            }
            _ => panic!("Wrong message type"),
        }
    }
    
    // === Тесты FileInfo ===
    
    #[test]
    fn test_file_info_new() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello, World!").unwrap();
        
        let info = FileInfo::new(file_path.clone()).unwrap();
        
        assert_eq!(info.name, "test.txt");
        assert_eq!(info.relative_path, "test.txt");
        assert_eq!(info.size, 13);
        assert_eq!(info.transferred, 0);
        assert_eq!(info.status, FileStatus::Pending);
    }
    
    #[test]
    fn test_file_info_with_relative_path() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "Test content").unwrap();
        
        let info = FileInfo::with_relative_path(
            file_path,
            "folder/subfolder/test.txt".to_string(),
        ).unwrap();
        
        assert_eq!(info.name, "test.txt");
        assert_eq!(info.relative_path, "folder/subfolder/test.txt");
    }
    
    #[test]
    fn test_file_info_progress() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "1234567890").unwrap();
        
        let mut info = FileInfo::new(file_path).unwrap();
        
        assert_eq!(info.progress(), 0.0);
        
        info.transferred = 5;
        assert!((info.progress() - 0.5).abs() < 0.01);
        
        info.transferred = 10;
        assert!((info.progress() - 1.0).abs() < 0.01);
    }
    
    #[test]
    fn test_file_info_progress_empty_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("empty.txt");
        std::fs::write(&file_path, "").unwrap();
        
        let info = FileInfo::new(file_path).unwrap();
        assert_eq!(info.progress(), 1.0); // Пустой файл = 100%
    }
    
    // === Тесты FileStatus ===
    
    #[test]
    fn test_file_status_equality() {
        assert_eq!(FileStatus::Pending, FileStatus::Pending);
        assert_eq!(FileStatus::Transferring, FileStatus::Transferring);
        assert_eq!(FileStatus::Completed, FileStatus::Completed);
        assert_eq!(
            FileStatus::Error("test".to_string()),
            FileStatus::Error("test".to_string())
        );
        assert_ne!(FileStatus::Pending, FileStatus::Completed);
    }
    
    // === Тесты collect_files_from_folder ===
    
    #[test]
    fn test_collect_files_from_folder() {
        let dir = TempDir::new().unwrap();
        
        // Создаём структуру папок
        let sub1 = dir.path().join("sub1");
        let sub2 = dir.path().join("sub2");
        std::fs::create_dir(&sub1).unwrap();
        std::fs::create_dir(&sub2).unwrap();
        
        // Создаём файлы
        std::fs::write(dir.path().join("root.txt"), "root").unwrap();
        std::fs::write(sub1.join("file1.txt"), "file1").unwrap();
        std::fs::write(sub2.join("file2.txt"), "file2").unwrap();
        
        // Собираем файлы
        let files = collect_files_from_folder(dir.path()).unwrap();
        
        assert_eq!(files.len(), 3);
        
        // Проверяем что все файлы найдены
        let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
        assert!(paths.iter().any(|p| p.ends_with("root.txt")));
        assert!(paths.iter().any(|p| p.contains("sub1") && p.ends_with("file1.txt")));
        assert!(paths.iter().any(|p| p.contains("sub2") && p.ends_with("file2.txt")));
    }
    
    #[test]
    fn test_collect_files_empty_folder() {
        let dir = TempDir::new().unwrap();
        let files = collect_files_from_folder(dir.path()).unwrap();
        assert!(files.is_empty());
    }
}

