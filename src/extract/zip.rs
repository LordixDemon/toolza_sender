//! Распаковка ZIP архивов

use super::types::ExtractResult;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::Path;

/// Распаковать ZIP архив
pub fn extract_zip(archive_path: &Path, output_dir: &Path) -> io::Result<ExtractResult> {
    let file = File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    
    let mut files_count = 0;
    let mut total_size = 0u64;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        
        let outpath = match file.enclosed_name() {
            Some(path) => output_dir.join(path),
            None => continue,
        };
        
        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            
            let mut outfile = File::create(&outpath)?;
            let size = io::copy(&mut file, &mut outfile)?;
            files_count += 1;
            total_size += size;
            
            // Устанавливаем права на Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    let _ = fs::set_permissions(&outpath, fs::Permissions::from_mode(mode));
                }
            }
        }
    }
    
    Ok(ExtractResult { files_count, total_size })
}

