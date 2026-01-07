//! Модуль приёма файлов (сервер)
//!
//! Подмодули:
//! - `options` - опции сервера и автораспаковки
//! - `handlers` - обработчики клиентских подключений
//! - `streaming` - потоковая распаковка архивов

mod options;
mod handlers;
mod streaming;

pub use options::{ExtractOptions, ServerOptions};

use crate::network::events::TransferEvent;
use crate::network::transport::TransportType;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

// Re-export внутренних функций для использования в streaming
pub(crate) use handlers::send_ack_transport;

/// Запустить сервер для приёма файлов
pub async fn run_server(
    port: u16,
    save_dir: PathBuf,
    auto_extract: bool,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
) -> Result<(), String> {
    let options = ServerOptions {
        extract_options: ExtractOptions {
            tar_lz4: auto_extract,
            ..Default::default()
        },
        enable_resume: true,
        transport_type: TransportType::default(),
        save_archive_for_resume: false,
    };
    
    run_server_with_options(port, save_dir, options, event_tx).await
}

/// Запустить сервер с поддержкой остановки (устаревший API)
pub async fn run_server_with_stop(
    port: u16,
    save_dir: PathBuf,
    auto_extract: bool,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    let options = ServerOptions {
        extract_options: ExtractOptions {
            tar_lz4: auto_extract,
            ..Default::default()
        },
        enable_resume: true,
        transport_type: TransportType::default(),
        save_archive_for_resume: false,
    };
    run_server_with_options_and_stop(port, save_dir, options, event_tx, stop_flag).await
}

/// Запустить сервер с полными опциями и поддержкой остановки
pub async fn run_server_with_options_and_stop(
    port: u16,
    save_dir: PathBuf,
    options: ServerOptions,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut listener = crate::network::transport::bind(options.transport_type, port)
        .await
        .map_err(|e| format!("Не удалось запустить сервер [{}]: {}", options.transport_type.name(), e))?;
    
    loop {
        // Проверяем флаг остановки
        if stop_flag.load(Ordering::SeqCst) {
            return Ok(());
        }
        
        // Используем timeout для периодической проверки флага остановки
        match listener.accept_timeout(Duration::from_millis(100)).await {
            Ok(Some((stream, addr))) => {
                let _ = event_tx.send(TransferEvent::Connected(0, format!("{} [{}]", addr, options.transport_type.name())));
                
                let save_dir = save_dir.clone();
                let options = options.clone();
                let event_tx = event_tx.clone();
                let stop_flag = stop_flag.clone();
                
                tokio::spawn(async move {
                    if let Err(e) = handlers::handle_client_transport(stream, save_dir, options, event_tx.clone(), stop_flag).await {
                        let _ = event_tx.send(TransferEvent::ConnectionError(0, e));
                    }
                    let _ = event_tx.send(TransferEvent::Disconnected);
                });
            }
            Ok(None) => {
                // Timeout - просто продолжаем цикл для проверки флага остановки
                continue;
            }
            Err(e) => {
                let _ = event_tx.send(TransferEvent::ConnectionError(0, e.to_string()));
            }
        }
    }
}

/// Запустить сервер с расширенными опциями (без поддержки остановки)
pub async fn run_server_with_options(
    port: u16,
    save_dir: PathBuf,
    options: ServerOptions,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
) -> Result<(), String> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .map_err(|e| format!("Не удалось запустить сервер: {}", e))?;
    
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let _ = event_tx.send(TransferEvent::Connected(0, addr.to_string()));
                stream.set_nodelay(true).ok();
                
                let save_dir = save_dir.clone();
                let options = options.clone();
                let event_tx = event_tx.clone();
                
                tokio::spawn(async move {
                    if let Err(e) = handlers::handle_client_tcp(stream, save_dir, options, event_tx.clone()).await {
                        let _ = event_tx.send(TransferEvent::ConnectionError(0, e));
                    }
                    let _ = event_tx.send(TransferEvent::Disconnected);
                });
            }
            Err(e) => {
                let _ = event_tx.send(TransferEvent::ConnectionError(0, e.to_string()));
            }
        }
    }
}

