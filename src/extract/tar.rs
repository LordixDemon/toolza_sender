//! Распаковка tar и tar.gz архивов

use super::types::ExtractResult;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::Path;

/// Распаковать обычный tar архив
pub fn extract_tar(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    let file = File::open(archive_path)?;
    let mut archive = tar::Archive::new(BufReader::new(file));
    
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

/// Распаковать tar.gz архив
pub fn extract_tar_gz(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    let file = File::open(archive_path)?;
    let gz = flate2::read::GzDecoder::new(BufReader::new(file));
    let mut archive = tar::Archive::new(gz);
    
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

