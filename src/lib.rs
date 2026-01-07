//! Toolza Sender - быстрая передача файлов по локальной сети
//! 
//! Общая библиотека для GUI и CLI версий.
//! 
//! # Модули
//! - `network` - сетевая передача (отправка, приём, сканирование)
//! - `protocol` - протокол передачи файлов
//! - `utils` - вспомогательные функции
//! - `extract` - распаковка tar.lz4 архивов
//! - `stats` - статистика передачи (скорость, ETA)
//! - `history` - история передач
//! - `sync` - режим синхронизации
//! - `i18n` - интернационализация (русский, украинский, английский)

pub mod extract;
pub mod history;
pub mod i18n;
pub mod network;
pub mod protocol;
pub mod stats;
pub mod sync;
pub mod utils;

