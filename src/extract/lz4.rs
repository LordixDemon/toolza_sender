//! Распаковка LZ4 и tar.lz4 архивов

use super::types::ExtractResult;
use lz4_flex::frame::FrameDecoder;
use rayon::prelude::*;
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// Распаковать обычный .lz4 файл (не архив)
pub fn extract_lz4(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    let filename = archive_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    
    let output_name = if filename.to_lowercase().ends_with(".lz4") {
        &filename[..filename.len() - 4]
    } else {
        filename
    };
    
    let output_path = output_dir.join(output_name);
    
    let file = File::open(archive_path)?;
    let reader = BufReader::with_capacity(256 * 1024, file);
    
    let decompressed = match decompress_lz4_frame(reader) {
        Ok(data) => data,
        Err(_) => {
            let file = File::open(archive_path)?;
            decompress_lz4_blocks(file).map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("Не удалось распаковать LZ4: {}", e))
            })?
        }
    };
    
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let mut output_file = File::create(&output_path)?;
    output_file.write_all(&decompressed)?;
    
    let total_size = decompressed.len() as u64;
    
    Ok(ExtractResult {
        files_count: 1,
        total_size,
    })
}

/// Распаковать tar.lz4 архив (многопоточно)
pub fn extract_tar_lz4(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    let file = File::open(archive_path)?;
    let reader = BufReader::with_capacity(256 * 1024, file);
    
    // Пробуем LZ4 frame format
    let decompressed = match decompress_lz4_frame(reader) {
        Ok(data) => data,
        Err(_) => {
            let file = File::open(archive_path)?;
            match decompress_lz4_blocks(file) {
                Ok(data) => data,
                Err(e) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Не удалось распаковать LZ4: {}", e)
                    ));
                }
            }
        }
    };
    
    // Проверяем, является ли это tar архивом
    if !is_valid_tar(&decompressed) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Файл не содержит tar архив внутри LZ4"
        ));
    }
    
    extract_tar_parallel(&decompressed, output_dir)
}

/// Синхронная распаковка tar.lz4 (для небольших архивов)
pub fn extract_tar_lz4_simple(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    let file = File::open(archive_path)?;
    let reader = BufReader::with_capacity(256 * 1024, file);
    
    let decompressed = match decompress_lz4_frame(reader) {
        Ok(data) => data,
        Err(_) => {
            let file = File::open(archive_path)?;
            decompress_lz4_blocks(file)?
        }
    };
    
    let mut archive = tar::Archive::new(decompressed.as_slice());
    let mut files_count = 0;
    let mut total_size = 0u64;
    
    for entry in archive.entries()? {
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

/// Проверить, является ли данные tar архивом
pub(crate) fn is_valid_tar(data: &[u8]) -> bool {
    if data.len() < 512 {
        return false;
    }
    
    // Проверяем magic bytes "ustar"
    if data.len() >= 263 {
        let magic = &data[257..262];
        if magic == b"ustar" {
            return true;
        }
    }
    
    // Альтернативная проверка
    let header_bytes = &data[0..512];
    let non_zero = header_bytes.iter().any(|&b| b != 0);
    if !non_zero {
        return false;
    }
    
    // Проверяем checksum
    let checksum_field = &data[148..156];
    checksum_field.iter().all(|&b| 
        b == 0 || b == b' ' || (b >= b'0' && b <= b'7')
    )
}

/// Распаковать LZ4 frame format
pub(crate) fn decompress_lz4_frame<R: Read>(reader: R) -> io::Result<Vec<u8>> {
    let mut decoder = FrameDecoder::new(reader);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

/// Распаковать LZ4 block format
pub(crate) fn decompress_lz4_blocks<R: Read>(mut reader: R) -> io::Result<Vec<u8>> {
    let mut compressed = Vec::new();
    reader.read_to_end(&mut compressed)?;
    
    lz4_flex::decompress_size_prepended(&compressed)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
}

/// Параллельная распаковка tar архива
fn extract_tar_parallel(tar_data: &[u8], output_dir: &Path) -> io::Result<ExtractResult> {
    let mut archive = tar::Archive::new(tar_data);
    let mut entries_data: Vec<(PathBuf, Vec<u8>, u32)> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let full_path = output_dir.join(&path);
        
        if entry.header().entry_type().is_dir() {
            dirs.push(full_path);
        } else if entry.header().entry_type().is_file() {
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            let mode = entry.header().mode().unwrap_or(0o644);
            entries_data.push((full_path, data, mode));
        }
    }
    
    // Создаём директории последовательно
    for dir in &dirs {
        fs::create_dir_all(dir)?;
    }
    
    // Записываем файлы параллельно
    let results: Vec<io::Result<u64>> = entries_data
        .par_iter()
        .map(|(path, data, _mode)| {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            
            let mut file = File::create(path)?;
            file.write_all(data)?;
            
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(*_mode));
            }
            
            Ok(data.len() as u64)
        })
        .collect();
    
    let mut total_size = 0u64;
    let mut files_count = 0usize;
    
    for result in results {
        match result {
            Ok(size) => {
                total_size += size;
                files_count += 1;
            }
            Err(e) => return Err(e),
        }
    }
    
    Ok(ExtractResult { files_count, total_size })
}

