//! Типы для модуля распаковки

/// Тип архива
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveType {
    TarLz4,
    Lz4,
    Tar,
    TarGz,
    Zip,
    Rar,
    SevenZip,
    Unknown,
}

impl ArchiveType {
    /// Определить тип архива по имени файла
    pub fn from_filename(filename: &str) -> Self {
        let lower = filename.to_lowercase();
        
        if lower.ends_with(".tar.lz4") || lower.ends_with(".tlz4") {
            Self::TarLz4
        } else if lower.ends_with(".lz4") {
            Self::Lz4
        } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            Self::TarGz
        } else if lower.ends_with(".tar") {
            Self::Tar
        } else if lower.ends_with(".zip") {
            Self::Zip
        } else if lower.ends_with(".rar") {
            Self::Rar
        } else if lower.ends_with(".7z") {
            Self::SevenZip
        } else {
            Self::Unknown
        }
    }
    
    /// Имя формата
    pub fn name(&self) -> &'static str {
        match self {
            Self::TarLz4 => "tar.lz4",
            Self::Lz4 => "lz4",
            Self::Tar => "tar",
            Self::TarGz => "tar.gz",
            Self::Zip => "zip",
            Self::Rar => "rar",
            Self::SevenZip => "7z",
            Self::Unknown => "unknown",
        }
    }
}

/// Результат распаковки
pub struct ExtractResult {
    pub files_count: usize,
    pub total_size: u64,
}

/// Опции автораспаковки
#[derive(Debug, Clone, Default)]
pub struct ExtractOptions {
    pub tar_lz4: bool,
    pub lz4: bool,
    pub tar: bool,
    pub zip: bool,
    pub rar: bool,
}

impl ExtractOptions {
    /// Проверить, включена ли распаковка для данного типа архива
    pub fn should_extract(&self, archive_type: ArchiveType) -> bool {
        match archive_type {
            ArchiveType::TarLz4 => self.tar_lz4,
            ArchiveType::Lz4 => self.lz4,
            ArchiveType::Tar | ArchiveType::TarGz => self.tar,
            ArchiveType::Zip => self.zip,
            ArchiveType::Rar => self.rar,
            _ => false,
        }
    }
    
    /// Есть ли хоть одна опция включена
    pub fn any_enabled(&self) -> bool {
        self.tar_lz4 || self.tar || self.zip || self.rar
    }
}

