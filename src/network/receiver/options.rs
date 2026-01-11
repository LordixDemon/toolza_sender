//! Опции сервера для приёма файлов

use crate::network::transport::TransportType;

/// Опции автораспаковки
#[derive(Clone, Debug, Default)]
pub struct ExtractOptions {
    pub tar_lz4: bool,
    pub tar_zst: bool,
    pub lz4: bool,
    pub tar: bool,
    pub zip: bool,
    pub rar: bool,
}

impl ExtractOptions {
    pub fn any_enabled(&self) -> bool {
        self.tar_lz4 || self.tar_zst || self.lz4 || self.tar || self.zip || self.rar
    }
}

/// Опции сервера
#[derive(Clone, Debug)]
pub struct ServerOptions {
    pub extract_options: ExtractOptions,
    pub enable_resume: bool,
    pub transport_type: TransportType,
    /// Сохранять архив при потоковой распаковке (для возможности резюме)
    pub save_archive_for_resume: bool,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            extract_options: ExtractOptions::default(),
            enable_resume: true,
            transport_type: TransportType::default(),
            save_archive_for_resume: false, // По умолчанию чистая потоковая распаковка
        }
    }
}

impl ServerOptions {
    /// Проверить, нужно ли распаковывать файл
    pub fn should_extract(&self, filename: &str) -> bool {
        let archive_type = crate::extract::ArchiveType::from_filename(filename);
        match archive_type {
            crate::extract::ArchiveType::TarLz4 => self.extract_options.tar_lz4,
            crate::extract::ArchiveType::TarZst => self.extract_options.tar_zst,
            crate::extract::ArchiveType::Lz4 => self.extract_options.lz4,
            crate::extract::ArchiveType::Tar | crate::extract::ArchiveType::TarGz => self.extract_options.tar,
            crate::extract::ArchiveType::Zip => self.extract_options.zip,
            crate::extract::ArchiveType::Rar => self.extract_options.rar,
            _ => false,
        }
    }
}

