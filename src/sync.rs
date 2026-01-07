//! Режим синхронизации - передача только изменённых файлов

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

/// Размер блока для хэширования (4 KB)
const HASH_BLOCK_SIZE: usize = 4 * 1024;

/// Информация о файле для синхронизации
#[derive(Clone, Debug)]
pub struct SyncFileInfo {
    pub path: PathBuf,
    pub relative_path: String,
    pub size: u64,
    pub modified: u64, // Unix timestamp
    pub quick_hash: u64, // Быстрый хэш (первые + последние блоки)
}

impl SyncFileInfo {
    /// Создать из пути
    pub fn from_path(path: &Path, relative_path: String) -> io::Result<Self> {
        let metadata = std::fs::metadata(path)?;
        let size = metadata.len();
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        
        let quick_hash = compute_quick_hash(path, size)?;
        
        Ok(Self {
            path: path.to_path_buf(),
            relative_path,
            size,
            modified,
            quick_hash,
        })
    }
    
    /// Проверить, нужно ли обновлять файл
    pub fn needs_update(&self, remote: &RemoteFileInfo) -> bool {
        // Разные размеры - точно нужно обновлять
        if self.size != remote.size {
            return true;
        }
        
        // Разные хэши - нужно обновлять
        if self.quick_hash != remote.quick_hash {
            return true;
        }
        
        // Файлы идентичны
        false
    }
}

/// Информация о файле на удалённой стороне
#[derive(Clone, Debug)]
pub struct RemoteFileInfo {
    pub relative_path: String,
    pub size: u64,
    pub modified: u64,
    pub quick_hash: u64,
}

/// Результат сравнения для синхронизации
#[derive(Clone, Debug)]
pub struct SyncDiff {
    /// Файлы для передачи (новые или изменённые)
    pub to_transfer: Vec<SyncFileInfo>,
    /// Файлы без изменений
    pub unchanged: Vec<String>,
    /// Файлы только на удалённой стороне (для удаления)
    pub remote_only: Vec<String>,
}

impl SyncDiff {
    /// Вычислить размер для передачи
    pub fn transfer_size(&self) -> u64 {
        self.to_transfer.iter().map(|f| f.size).sum()
    }
}

/// Вычислить быстрый хэш файла
/// Читает первый и последний блоки для скорости
fn compute_quick_hash(path: &Path, size: u64) -> io::Result<u64> {
    if size == 0 {
        return Ok(0);
    }
    
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    
    let mut hasher = QuickHasher::new();
    
    // Хэшируем размер
    hasher.update(&size.to_le_bytes());
    
    // Читаем первый блок
    let mut first_block = vec![0u8; HASH_BLOCK_SIZE.min(size as usize)];
    reader.read_exact(&mut first_block)?;
    hasher.update(&first_block);
    
    // Если файл больше одного блока, читаем последний
    if size > HASH_BLOCK_SIZE as u64 {
        use std::io::Seek;
        let last_offset = size - HASH_BLOCK_SIZE as u64;
        reader.seek(std::io::SeekFrom::Start(last_offset))?;
        
        let mut last_block = vec![0u8; HASH_BLOCK_SIZE];
        reader.read_exact(&mut last_block)?;
        hasher.update(&last_block);
    }
    
    Ok(hasher.finish())
}

/// Простой быстрый хэшер (FNV-1a)
struct QuickHasher {
    hash: u64,
}

impl QuickHasher {
    fn new() -> Self {
        Self {
            hash: 0xcbf29ce484222325, // FNV offset basis
        }
    }
    
    fn update(&mut self, data: &[u8]) {
        const FNV_PRIME: u64 = 0x100000001b3;
        
        for byte in data {
            self.hash ^= *byte as u64;
            self.hash = self.hash.wrapping_mul(FNV_PRIME);
        }
    }
    
    fn finish(self) -> u64 {
        self.hash
    }
}

/// Собрать информацию о локальных файлах для синхронизации
pub fn collect_sync_info(paths: &[PathBuf]) -> io::Result<Vec<SyncFileInfo>> {
    let mut files = Vec::new();
    
    for path in paths {
        if path.is_file() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".to_string());
            
            if let Ok(info) = SyncFileInfo::from_path(path, name) {
                files.push(info);
            }
        } else if path.is_dir() {
            collect_sync_info_recursive(path, &path.to_string_lossy(), &mut files)?;
        }
    }
    
    Ok(files)
}

/// Рекурсивно собрать информацию о файлах в папке
fn collect_sync_info_recursive(
    dir: &Path,
    _base: &str,
    files: &mut Vec<SyncFileInfo>,
) -> io::Result<()> {
    let dir_name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "folder".to_string());
    
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        
        let relative = format!("{}/{}", dir_name, name);
        
        if path.is_file() {
            if let Ok(info) = SyncFileInfo::from_path(&path, relative) {
                files.push(info);
            }
        } else if path.is_dir() {
            collect_sync_info_recursive(&path, &relative, files)?;
        }
    }
    
    Ok(())
}

/// Сравнить локальные и удалённые файлы
pub fn compute_sync_diff(
    local: &[SyncFileInfo],
    remote: &[RemoteFileInfo],
) -> SyncDiff {
    let remote_map: HashMap<&str, &RemoteFileInfo> = remote
        .iter()
        .map(|f| (f.relative_path.as_str(), f))
        .collect();
    
    let local_paths: std::collections::HashSet<&str> = local
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    
    let mut to_transfer = Vec::new();
    let mut unchanged = Vec::new();
    
    for local_file in local {
        if let Some(remote_file) = remote_map.get(local_file.relative_path.as_str()) {
            if local_file.needs_update(remote_file) {
                to_transfer.push(local_file.clone());
            } else {
                unchanged.push(local_file.relative_path.clone());
            }
        } else {
            // Файл отсутствует на удалённой стороне
            to_transfer.push(local_file.clone());
        }
    }
    
    // Файлы только на удалённой стороне
    let remote_only: Vec<String> = remote
        .iter()
        .filter(|f| !local_paths.contains(f.relative_path.as_str()))
        .map(|f| f.relative_path.clone())
        .collect();
    
    SyncDiff {
        to_transfer,
        unchanged,
        remote_only,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    // === Тесты QuickHasher ===
    
    #[test]
    fn test_quick_hasher_same_input() {
        let mut h1 = QuickHasher::new();
        h1.update(b"hello");
        
        let mut h2 = QuickHasher::new();
        h2.update(b"hello");
        
        assert_eq!(h1.finish(), h2.finish());
    }
    
    #[test]
    fn test_quick_hasher_different_input() {
        let mut h1 = QuickHasher::new();
        h1.update(b"hello");
        
        let mut h2 = QuickHasher::new();
        h2.update(b"world");
        
        assert_ne!(h1.finish(), h2.finish());
    }
    
    #[test]
    fn test_quick_hasher_incremental() {
        let mut h1 = QuickHasher::new();
        h1.update(b"hello");
        h1.update(b"world");
        
        let mut h2 = QuickHasher::new();
        h2.update(b"helloworld");
        
        assert_eq!(h1.finish(), h2.finish());
    }
    
    #[test]
    fn test_quick_hasher_empty() {
        let h = QuickHasher::new();
        // Пустой хэш должен быть начальным значением FNV
        assert_eq!(h.finish(), 0xcbf29ce484222325);
    }
    
    // === Тесты SyncFileInfo ===
    
    #[test]
    fn test_sync_file_info_from_path() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello, World!").unwrap();
        
        let info = SyncFileInfo::from_path(&file_path, "test.txt".to_string()).unwrap();
        
        assert_eq!(info.relative_path, "test.txt");
        assert_eq!(info.size, 13);
        assert!(info.quick_hash != 0);
    }
    
    #[test]
    fn test_sync_file_info_needs_update_different_size() {
        let local = SyncFileInfo {
            path: PathBuf::from("/test"),
            relative_path: "test.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 12345,
        };
        
        let remote = RemoteFileInfo {
            relative_path: "test.txt".to_string(),
            size: 200, // Разный размер
            modified: 0,
            quick_hash: 12345,
        };
        
        assert!(local.needs_update(&remote));
    }
    
    #[test]
    fn test_sync_file_info_needs_update_different_hash() {
        let local = SyncFileInfo {
            path: PathBuf::from("/test"),
            relative_path: "test.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 12345,
        };
        
        let remote = RemoteFileInfo {
            relative_path: "test.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 54321, // Разный хэш
        };
        
        assert!(local.needs_update(&remote));
    }
    
    #[test]
    fn test_sync_file_info_no_update_needed() {
        let local = SyncFileInfo {
            path: PathBuf::from("/test"),
            relative_path: "test.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 12345,
        };
        
        let remote = RemoteFileInfo {
            relative_path: "test.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 12345, // Одинаковый хэш
        };
        
        assert!(!local.needs_update(&remote));
    }
    
    // === Тесты SyncDiff ===
    
    #[test]
    fn test_sync_diff_transfer_size() {
        let diff = SyncDiff {
            to_transfer: vec![
                SyncFileInfo {
                    path: PathBuf::from("/a"),
                    relative_path: "a.txt".to_string(),
                    size: 100,
                    modified: 0,
                    quick_hash: 0,
                },
                SyncFileInfo {
                    path: PathBuf::from("/b"),
                    relative_path: "b.txt".to_string(),
                    size: 200,
                    modified: 0,
                    quick_hash: 0,
                },
            ],
            unchanged: vec![],
            remote_only: vec![],
        };
        
        assert_eq!(diff.transfer_size(), 300);
    }
    
    #[test]
    fn test_sync_diff_empty() {
        let diff = SyncDiff {
            to_transfer: vec![],
            unchanged: vec![],
            remote_only: vec![],
        };
        
        assert_eq!(diff.transfer_size(), 0);
    }
    
    // === Тесты compute_sync_diff ===
    
    #[test]
    fn test_compute_sync_diff_new_file() {
        let local = vec![SyncFileInfo {
            path: PathBuf::from("/new"),
            relative_path: "new.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 12345,
        }];
        
        let remote: Vec<RemoteFileInfo> = vec![];
        
        let diff = compute_sync_diff(&local, &remote);
        
        assert_eq!(diff.to_transfer.len(), 1);
        assert!(diff.unchanged.is_empty());
        assert!(diff.remote_only.is_empty());
    }
    
    #[test]
    fn test_compute_sync_diff_unchanged() {
        let local = vec![SyncFileInfo {
            path: PathBuf::from("/file"),
            relative_path: "file.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 12345,
        }];
        
        let remote = vec![RemoteFileInfo {
            relative_path: "file.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 12345,
        }];
        
        let diff = compute_sync_diff(&local, &remote);
        
        assert!(diff.to_transfer.is_empty());
        assert_eq!(diff.unchanged.len(), 1);
        assert!(diff.remote_only.is_empty());
    }
    
    #[test]
    fn test_compute_sync_diff_modified() {
        let local = vec![SyncFileInfo {
            path: PathBuf::from("/file"),
            relative_path: "file.txt".to_string(),
            size: 150, // Изменённый размер
            modified: 0,
            quick_hash: 99999,
        }];
        
        let remote = vec![RemoteFileInfo {
            relative_path: "file.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 12345,
        }];
        
        let diff = compute_sync_diff(&local, &remote);
        
        assert_eq!(diff.to_transfer.len(), 1);
        assert!(diff.unchanged.is_empty());
    }
    
    #[test]
    fn test_compute_sync_diff_remote_only() {
        let local: Vec<SyncFileInfo> = vec![];
        
        let remote = vec![RemoteFileInfo {
            relative_path: "old.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 12345,
        }];
        
        let diff = compute_sync_diff(&local, &remote);
        
        assert!(diff.to_transfer.is_empty());
        assert!(diff.unchanged.is_empty());
        assert_eq!(diff.remote_only.len(), 1);
        assert_eq!(diff.remote_only[0], "old.txt");
    }
    
    #[test]
    fn test_compute_sync_diff_complex() {
        let local = vec![
            SyncFileInfo {
                path: PathBuf::from("/new"),
                relative_path: "new.txt".to_string(),
                size: 100,
                modified: 0,
                quick_hash: 111,
            },
            SyncFileInfo {
                path: PathBuf::from("/same"),
                relative_path: "same.txt".to_string(),
                size: 200,
                modified: 0,
                quick_hash: 222,
            },
            SyncFileInfo {
                path: PathBuf::from("/changed"),
                relative_path: "changed.txt".to_string(),
                size: 350, // Изменён
                modified: 0,
                quick_hash: 999,
            },
        ];
        
        let remote = vec![
            RemoteFileInfo {
                relative_path: "same.txt".to_string(),
                size: 200,
                modified: 0,
                quick_hash: 222,
            },
            RemoteFileInfo {
                relative_path: "changed.txt".to_string(),
                size: 300,
                modified: 0,
                quick_hash: 333,
            },
            RemoteFileInfo {
                relative_path: "deleted.txt".to_string(),
                size: 400,
                modified: 0,
                quick_hash: 444,
            },
        ];
        
        let diff = compute_sync_diff(&local, &remote);
        
        // new.txt и changed.txt должны передаваться
        assert_eq!(diff.to_transfer.len(), 2);
        
        // same.txt не изменился
        assert_eq!(diff.unchanged.len(), 1);
        assert_eq!(diff.unchanged[0], "same.txt");
        
        // deleted.txt только на remote
        assert_eq!(diff.remote_only.len(), 1);
        assert_eq!(diff.remote_only[0], "deleted.txt");
    }
}

