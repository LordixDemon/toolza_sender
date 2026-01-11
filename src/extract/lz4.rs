//! Распаковка LZ4 и tar.lz4 архивов

use super::types::ExtractResult;
use lz4_flex::frame::FrameDecoder;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Размер буфера чтения (64 МБ)
const READ_BUFFER_SIZE: usize = 64 * 1024 * 1024;
/// Размер буфера записи (64 МБ) - большой для NAS/сетевых дисков
const WRITE_BUFFER_SIZE: usize = 64 * 1024 * 1024;
/// Размер чанка записи (16 МБ) - оптимально для NAS
const WRITE_CHUNK_SIZE: usize = 16 * 1024 * 1024;

/// Распаковать обычный .lz4 файл (не архив) - потоковая распаковка
pub fn extract_lz4(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    extract_lz4_streaming(archive_path, output_dir, None)
}

/// Распаковать .lz4 файл с поддержкой остановки (потоковая)
pub fn extract_lz4_streaming(
    archive_path: &Path, 
    output_dir: &Path,
    stop_flag: Option<Arc<AtomicBool>>
) -> io::Result<ExtractResult> {
    let filename = archive_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    
    let output_name = if filename.to_lowercase().ends_with(".lz4") {
        &filename[..filename.len() - 4]
    } else {
        filename
    };
    
    let output_path = output_dir.join(output_name);
    
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let file = File::open(archive_path)?;
    let reader = BufReader::with_capacity(READ_BUFFER_SIZE, file);
    let mut decoder = FrameDecoder::new(reader);
    
    let output_file = File::create(&output_path)?;
    let mut writer = BufWriter::with_capacity(WRITE_BUFFER_SIZE, output_file);
    let mut total_size = 0u64;
    let mut buffer = vec![0u8; WRITE_CHUNK_SIZE];
    
    loop {
        // Проверяем флаг остановки
        if let Some(ref flag) = stop_flag {
            if flag.load(Ordering::Relaxed) {
                // Удаляем частично распакованный файл
                let _ = fs::remove_file(&output_path);
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Распаковка отменена"
                ));
            }
        }
        
        let bytes_read = decoder.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        
        writer.write_all(&buffer[..bytes_read])?;
        total_size += bytes_read as u64;
    }
    
    writer.flush()?;
    
    Ok(ExtractResult {
        files_count: 1,
        total_size,
    })
}

/// Распаковать tar.lz4 архив (потоковая распаковка - не грузит в RAM)
pub fn extract_tar_lz4(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    extract_tar_lz4_streaming(archive_path, output_dir, None)
}

/// Распаковать tar.lz4 архив с поддержкой остановки (потоковая)
pub fn extract_tar_lz4_streaming(
    archive_path: &Path, 
    output_dir: &Path,
    stop_flag: Option<Arc<AtomicBool>>
) -> io::Result<ExtractResult> {
    let file = File::open(archive_path)?;
    let reader = BufReader::with_capacity(READ_BUFFER_SIZE, file); // 64MB буфер
    
    // LZ4 FrameDecoder работает потоково - не грузит всё в RAM
    let decoder = FrameDecoder::new(reader);
    
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
    let mut buffer = vec![0u8; WRITE_CHUNK_SIZE];
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

/// Синхронная распаковка tar.lz4 (алиас для потоковой версии)
pub fn extract_tar_lz4_simple(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    extract_tar_lz4_streaming(archive_path, output_dir, None)
}


