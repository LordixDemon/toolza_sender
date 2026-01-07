//! Модуль распаковки архивов
//!
//! Поддерживаемые форматы:
//! - tar.lz4, lz4
//! - tar, tar.gz
//! - zip

mod types;
mod tar;
mod lz4;
mod zip;

pub use types::{ArchiveType, ExtractResult, ExtractOptions};
pub use tar::{extract_tar, extract_tar_gz};
pub use lz4::{extract_lz4, extract_tar_lz4, extract_tar_lz4_simple};
pub use zip::extract_zip;

use std::io;
use std::path::Path;

/// Проверить, является ли файл tar.lz4 архивом
pub fn is_tar_lz4(filename: &str) -> bool {
    matches!(ArchiveType::from_filename(filename), ArchiveType::TarLz4 | ArchiveType::Lz4)
}

/// Проверить, является ли файл архивом любого поддерживаемого типа
pub fn is_archive(filename: &str) -> bool {
    !matches!(ArchiveType::from_filename(filename), ArchiveType::Unknown)
}

/// Распаковать архив в указанную папку (автоопределение типа)
pub fn extract_archive(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    let filename = archive_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    
    match ArchiveType::from_filename(filename) {
        ArchiveType::TarLz4 => extract_tar_lz4(archive_path, output_dir),
        ArchiveType::Lz4 => extract_lz4(archive_path, output_dir),
        ArchiveType::Tar => extract_tar(archive_path, output_dir),
        ArchiveType::TarGz => extract_tar_gz(archive_path, output_dir),
        ArchiveType::Zip => extract_zip(archive_path, output_dir),
        ArchiveType::Rar => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "RAR распаковка требует внешнего unrar. Используйте: unrar x archive.rar"
        )),
        ArchiveType::SevenZip => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "7z распаковка требует внешнего 7z. Используйте: 7z x archive.7z"
        )),
        ArchiveType::Unknown => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Неизвестный формат архива"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_tar_lz4() {
        assert!(is_tar_lz4("file.tar.lz4"));
        assert!(is_tar_lz4("file.TAR.LZ4"));
        assert!(is_tar_lz4("file.tlz4"));
        assert!(!is_tar_lz4("FILE.TLZR"));
        assert!(!is_tar_lz4("file.tar.gz"));
        assert!(is_tar_lz4("file.lz4"));
        assert!(!is_tar_lz4("file.tar"));
        assert!(!is_tar_lz4("file.zip"));
    }
    
    #[test]
    fn test_archive_type() {
        assert_eq!(ArchiveType::from_filename("test.tar.lz4"), ArchiveType::TarLz4);
        assert_eq!(ArchiveType::from_filename("test.lz4"), ArchiveType::Lz4);
        assert_eq!(ArchiveType::from_filename("test.tar"), ArchiveType::Tar);
        assert_eq!(ArchiveType::from_filename("test.tar.gz"), ArchiveType::TarGz);
        assert_eq!(ArchiveType::from_filename("test.zip"), ArchiveType::Zip);
        assert_eq!(ArchiveType::from_filename("test.rar"), ArchiveType::Rar);
        assert_eq!(ArchiveType::from_filename("test.7z"), ArchiveType::SevenZip);
        assert_eq!(ArchiveType::from_filename("test.txt"), ArchiveType::Unknown);
    }
    
    #[test]
    fn test_extract_result() {
        let result = ExtractResult {
            files_count: 10,
            total_size: 1024,
        };
        assert_eq!(result.files_count, 10);
        assert_eq!(result.total_size, 1024);
    }
    
    #[test]
    fn test_extract_options() {
        let mut opts = ExtractOptions::default();
        assert!(!opts.any_enabled());
        
        opts.tar_lz4 = true;
        assert!(opts.any_enabled());
        assert!(opts.should_extract(ArchiveType::TarLz4));
        assert!(!opts.should_extract(ArchiveType::Zip));
    }
}

