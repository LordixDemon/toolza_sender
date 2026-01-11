//! Распаковка tar и tar.gz архивов

use super::types::ExtractResult;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Размер буфера чтения (64 МБ)
const READ_BUFFER_SIZE: usize = 64 * 1024 * 1024;
/// Размер буфера записи (64 МБ) - для NAS/сетевых дисков
const WRITE_BUFFER_SIZE: usize = 64 * 1024 * 1024;
/// Размер чанка копирования (16 МБ)
const COPY_CHUNK_SIZE: usize = 16 * 1024 * 1024;

/// Распаковать обычный tar архив
pub fn extract_tar(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    extract_tar_streaming(archive_path, output_dir, None)
}

/// Распаковать tar с поддержкой остановки
pub fn extract_tar_streaming(
    archive_path: &Path, 
    output_dir: &Path,
    stop_flag: Option<Arc<AtomicBool>>
) -> io::Result<ExtractResult> {
    let file = File::open(archive_path)?;
    let mut archive = tar::Archive::new(BufReader::with_capacity(READ_BUFFER_SIZE, file));
    
    let mut files_count = 0;
    let mut total_size = 0u64;
    
    for entry in archive.entries()? {
        if let Some(ref flag) = stop_flag {
            if flag.load(Ordering::Relaxed) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "Распаковка отменена"));
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
            
            // Ручная распаковка с большим буфером записи для NAS
            let out_file = File::create(&path)?;
            let mut writer = BufWriter::with_capacity(WRITE_BUFFER_SIZE, out_file);
            buffered_copy(&mut entry, &mut writer)?;
            writer.flush()?;
            
            files_count += 1;
            total_size += size;
        }
    }
    
    Ok(ExtractResult { files_count, total_size })
}

/// Копирование с большим буфером (16 МБ чанки)
fn buffered_copy<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<u64> {
    let mut buffer = vec![0u8; COPY_CHUNK_SIZE];
    let mut total = 0u64;
    
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        writer.write_all(&buffer[..bytes_read])?;
        total += bytes_read as u64;
    }
    
    Ok(total)
}

/// Распаковать tar.gz архив
pub fn extract_tar_gz(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    extract_tar_gz_streaming(archive_path, output_dir, None)
}

/// Распаковать tar.gz с поддержкой остановки
pub fn extract_tar_gz_streaming(
    archive_path: &Path, 
    output_dir: &Path,
    stop_flag: Option<Arc<AtomicBool>>
) -> io::Result<ExtractResult> {
    let file = File::open(archive_path)?;
    let gz = flate2::read::GzDecoder::new(BufReader::with_capacity(READ_BUFFER_SIZE, file));
    let mut archive = tar::Archive::new(gz);
    
    let mut files_count = 0;
    let mut total_size = 0u64;
    
    for entry in archive.entries()? {
        if let Some(ref flag) = stop_flag {
            if flag.load(Ordering::Relaxed) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "Распаковка отменена"));
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
            
            // Ручная распаковка с большим буфером записи для NAS
            let out_file = File::create(&path)?;
            let mut writer = BufWriter::with_capacity(WRITE_BUFFER_SIZE, out_file);
            buffered_copy(&mut entry, &mut writer)?;
            writer.flush()?;
            
            files_count += 1;
            total_size += size;
        }
    }
    
    Ok(ExtractResult { files_count, total_size })
}

