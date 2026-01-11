//! Распаковка ZST и tar.zst архивов

use super::types::ExtractResult;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Распаковать tar.zst архив (потоковая распаковка - не грузит в RAM)
pub fn extract_tar_zst(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    extract_tar_zst_streaming(archive_path, output_dir, None)
}

/// Распаковать tar.zst архив с поддержкой остановки (потоковая)
pub fn extract_tar_zst_streaming(
    archive_path: &Path, 
    output_dir: &Path,
    stop_flag: Option<Arc<AtomicBool>>
) -> io::Result<ExtractResult> {
    let file = File::open(archive_path)?;
    let reader = BufReader::with_capacity(1024 * 1024, file); // 1MB буфер
    
    // ZST decoder работает потоково - не грузит всё в RAM
    let decoder = zstd::stream::Decoder::new(reader)?;
    
    // tar::Archive тоже работает потоково
    let mut archive = tar::Archive::new(decoder);
    
    let mut files_count = 0;
    let mut total_size = 0u64;
    
    for entry in archive.entries()? {
        // Проверяем флаг остановки
        if let Some(ref flag) = stop_flag {
            if flag.load(Ordering::Relaxed) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Распаковка отменена"
                ));
            }
        }
        
        let mut entry = entry?;
        let path = output_dir.join(entry.path()?);
        
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&path)?;
        } else if entry.header().entry_type().is_file() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let size = entry.header().size()?;
            entry.unpack(&path)?;
            files_count += 1;
            total_size += size;
        }
    }
    
    Ok(ExtractResult { files_count, total_size })
}

/// Синхронная распаковка tar.zst (алиас для потоковой версии)
pub fn extract_tar_zst_simple(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    extract_tar_zst_streaming(archive_path, output_dir, None)
}
