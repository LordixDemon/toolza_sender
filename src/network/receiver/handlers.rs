//! Обработчики клиентских подключений

use crate::extract;
use crate::network::compression;
use crate::network::events::TransferEvent;
use crate::network::transport::TransportStream;
use crate::protocol::Message;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncSeekExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use super::options::ServerOptions;
use super::streaming::{FnvHasher, receive_and_extract_streaming_transport, receive_and_extract_streaming_tcp};

/// Отправить Ack через транспорт
pub(crate) async fn send_ack_transport(stream: &mut dyn TransportStream) -> Result<(), String> {
    let ack = Message::Ack.to_bytes().map_err(|e| e.to_string())?;
    stream.write_all(&ack).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Отправить Cancel через транспорт
pub(crate) async fn send_cancel_transport(stream: &mut dyn TransportStream) -> Result<(), String> {
    let cancel = Message::Cancel.to_bytes().map_err(|e| e.to_string())?;
    stream.write_all(&cancel).await.map_err(|e| e.to_string())?;
    let _ = stream.flush().await;
    Ok(())
}

/// Обработчик клиента через абстрактный транспорт
pub(crate) async fn handle_client_transport(
    mut stream: Box<dyn TransportStream>,
    save_dir: PathBuf,
    options: ServerOptions,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), String> {
    // Логируем опции для диагностики
    let _ = event_tx.send(TransferEvent::FileReceived(
        format!("[DEBUG] handle_client_transport: extract_options={:?} transport={}", 
            options.extract_options, options.transport_type.name()),
        0
    ));
    
    loop {
        // Проверяем флаг остановки
        if stop_flag.load(Ordering::SeqCst) {
            let _ = send_cancel_transport(&mut *stream).await;
            let _ = event_tx.send(TransferEvent::FileReceived(
                "⛔ Передача отменена пользователем".to_string(), 0
            ));
            return Ok(());
        }
        
        // Читаем длину сообщения
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(()); // Клиент отключился
            }
            Err(e) => return Err(e.to_string()),
        }
        
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await.map_err(|e| e.to_string())?;
        
        let msg = Message::from_bytes(&data).map_err(|e| e.to_string())?;
        
        match msg {
            Message::FileStart { filename, size, compressed, offset: _, quick_hash } => {
                // Определяем тип архива и нужна ли распаковка
                let archive_type = extract::ArchiveType::from_filename(&filename);
                let should_extract = options.should_extract(&filename);
                let is_tar_lz4 = archive_type == extract::ArchiveType::TarLz4;
                let stream_extract = should_extract && is_tar_lz4;
                
                let _ = event_tx.send(TransferEvent::FileReceived(
                    format!("[DEBUG] FileStart: {} size={:.1}GB type={} extract={}", 
                        filename, 
                        size as f64 / (1024.0 * 1024.0 * 1024.0),
                        archive_type.name(),
                        should_extract
                    ),
                    0
                ));
                
                if stream_extract {
                    // Истинная потоковая распаковка
                    let result = receive_and_extract_streaming_transport(
                        &mut *stream,
                        &save_dir,
                        &filename,
                        size,
                        compressed,
                        options.save_archive_for_resume,
                        &event_tx,
                        &stop_flag,
                    ).await;
                    
                    if let Err(e) = result {
                        if stop_flag.load(Ordering::SeqCst) {
                            let _ = send_cancel_transport(&mut *stream).await;
                            return Err("⛔ Передача отменена".to_string());
                        }
                        return Err(e);
                    }
                    
                    let _ = event_tx.send(TransferEvent::FileReceived(
                        "[DEBUG] Распаковка завершена, ожидаем Done".to_string(), 0
                    ));
                } else {
                    // Обычное сохранение файла
                    let result = receive_file_transport(
                        &mut *stream,
                        &save_dir,
                        &filename,
                        size,
                        compressed,
                        quick_hash,
                        options.enable_resume,
                        &event_tx,
                        &stop_flag,
                    ).await;
                    
                    match result {
                        Ok(file_path) => {
                            // Если нужно распаковать (tar, zip, rar - не tar.lz4)
                            if should_extract && !is_tar_lz4 {
                                let _ = event_tx.send(TransferEvent::ExtractionStarted(filename.clone()));
                                
                                let output_dir = save_dir.clone();
                                let event_tx_clone = event_tx.clone();
                                let filename_clone = filename.clone();
                                let file_path_clone = file_path.clone();
                                
                                // Распаковываем в отдельном потоке
                                tokio::task::spawn_blocking(move || {
                                    match extract::extract_archive(&file_path_clone, &output_dir) {
                                        Ok(result) => {
                                            let _ = event_tx_clone.send(TransferEvent::ExtractionCompleted(
                                                filename_clone,
                                                result.files_count,
                                                result.total_size,
                                            ));
                                            // Удаляем архив после распаковки
                                            let _ = std::fs::remove_file(&file_path_clone);
                                        }
                                        Err(e) => {
                                            let _ = event_tx_clone.send(TransferEvent::ExtractionError(
                                                filename_clone,
                                                e.to_string(),
                                            ));
                                        }
                                    }
                                });
                            }
                        }
                        Err(e) => {
                            if stop_flag.load(Ordering::SeqCst) {
                                let _ = send_cancel_transport(&mut *stream).await;
                                return Err("⛔ Передача отменена".to_string());
                            }
                            return Err(e);
                        }
                    }
                }
            }
            Message::Done => {
                let _ = event_tx.send(TransferEvent::FileReceived(
                    "[DEBUG] Получен Done, завершаем".to_string(), 0
                ));
                return Ok(());
            }
            Message::Ack => {
                // Ping для спидтеста
                let ack = Message::Ack.to_bytes().map_err(|e| e.to_string())?;
                stream.write_all(&ack).await.map_err(|e| e.to_string())?;
            }
            Message::SpeedTestRequest { size } => {
                crate::network::speedtest::handle_speedtest_server_transport(&mut *stream, size).await?;
            }
            _ => {
                let err = Message::Error("Неожиданное сообщение".to_string());
                let data = err.to_bytes().map_err(|e| e.to_string())?;
                stream.write_all(&data).await.map_err(|e| e.to_string())?;
            }
        }
    }
}

/// Приём файла через абстрактный транспорт
pub(crate) async fn receive_file_transport(
    stream: &mut dyn TransportStream,
    save_dir: &PathBuf,
    filename: &str,
    size: u64,
    compressed: bool,
    quick_hash: u64,
    enable_resume: bool,
    event_tx: &mpsc::UnboundedSender<TransferEvent>,
    stop_flag: &Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    // Нормализуем путь
    let normalized_path = filename.replace('/', std::path::MAIN_SEPARATOR_STR);
    let file_path = save_dir.join(&normalized_path);
    
    // Создаём родительские папки если нужно
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Не удалось создать папку: {}", e))?;
    }
    
    // Проверяем возможность возобновления
    let resume_offset = if enable_resume {
        check_resume(&file_path, size, quick_hash).await
    } else {
        0
    };
    
    // Если файл уже полностью получен и хэш совпадает
    if resume_offset >= size {
        let resume_ack = Message::ResumeAck { offset: size };
        let data = resume_ack.to_bytes().map_err(|e| e.to_string())?;
        stream.write_all(&data).await.map_err(|e| e.to_string())?;
        
        let _ = event_tx.send(TransferEvent::FileReceived(filename.to_string(), size));
        return Ok(file_path);
    }
    
    // Открываем/создаём файл
    let mut file = if resume_offset > 0 {
        let f = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&file_path)
            .await
            .map_err(|e| format!("Не удалось открыть файл для resume: {}", e))?;
        
        let resume_ack = Message::ResumeAck { offset: resume_offset };
        let data = resume_ack.to_bytes().map_err(|e| e.to_string())?;
        stream.write_all(&data).await.map_err(|e| e.to_string())?;
        
        f
    } else {
        let f = tokio::fs::File::create(&file_path)
            .await
            .map_err(|e| format!("Не удалось создать файл: {}", e))?;
        
        send_ack_transport(stream).await?;
        f
    };
    
    // Если resume, перемещаемся в конец
    if resume_offset > 0 {
        file.seek(std::io::SeekFrom::Start(resume_offset)).await.map_err(|e| e.to_string())?;
    }
    
    // Трекинг прогресса
    let mut received_bytes = resume_offset;
    let start_time = std::time::Instant::now();
    let mut last_progress_update = std::time::Instant::now();
    
    // Принимаем данные
    loop {
        // Проверяем флаг остановки
        if stop_flag.load(Ordering::SeqCst) {
            return Err("⛔ Остановлено пользователем".to_string());
        }
        
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let len = u32::from_le_bytes(len_buf) as usize;
        
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await.map_err(|e| e.to_string())?;
        
        let msg = Message::from_bytes(&data).map_err(|e| e.to_string())?;
        
        match msg {
            Message::FileChunk { data, original_size: _ } => {
                if stop_flag.load(Ordering::SeqCst) {
                    return Err("⛔ Остановлено пользователем".to_string());
                }
                
                let write_data = if compressed {
                    compression::decompress(&data)?
                } else {
                    data
                };
                received_bytes += write_data.len() as u64;
                file.write_all(&write_data).await.map_err(|e| e.to_string())?;
                
                if last_progress_update.elapsed().as_secs() >= 1 {
                    let _ = event_tx.send(TransferEvent::Progress(
                        0, 0, received_bytes, size, received_bytes,
                    ));
                    last_progress_update = std::time::Instant::now();
                }
            }
            Message::FileEnd => {
                file.flush().await.map_err(|e| e.to_string())?;
                send_ack_transport(stream).await?;
                
                let elapsed = start_time.elapsed().as_secs_f64();
                let speed_mbps = if elapsed > 0.0 { received_bytes as f64 / elapsed / 1024.0 / 1024.0 } else { 0.0 };
                
                let _ = event_tx.send(TransferEvent::Progress(
                    0, 0, received_bytes, size, received_bytes,
                ));
                let _ = event_tx.send(TransferEvent::FileReceived(
                    format!("{} ({:.1} MB/s)", filename, speed_mbps),
                    size
                ));
                return Ok(file_path);
            }
            _ => {
                return Err("Неожиданное сообщение при получении файла".to_string());
            }
        }
    }
}

/// Проверка возможности возобновления загрузки
pub(crate) async fn check_resume(file_path: &PathBuf, expected_size: u64, quick_hash: u64) -> u64 {
    if quick_hash == 0 {
        return 0;
    }
    
    match tokio::fs::metadata(file_path).await {
        Ok(meta) => {
            let current_size = meta.len();
            if current_size >= expected_size {
                // Файл уже полный - проверяем хэш
                if let Ok(file_hash) = compute_quick_hash(file_path).await {
                    if file_hash == quick_hash {
                        return expected_size;
                    }
                }
                return 0; // Хэш не совпал - качаем заново
            }
            current_size
        }
        Err(_) => 0,
    }
}

/// Быстрый хэш файла (первые + последние 4KB)
pub(crate) async fn compute_quick_hash(file_path: &PathBuf) -> Result<u64, String> {
    let mut file = tokio::fs::File::open(file_path)
        .await
        .map_err(|e| e.to_string())?;
    
    let meta = file.metadata().await.map_err(|e| e.to_string())?;
    let size = meta.len();
    
    let mut hasher = FnvHasher::new();
    
    // Читаем первые 4KB
    let first_block_size = (size.min(4096)) as usize;
    let mut first_block = vec![0u8; first_block_size];
    file.read_exact(&mut first_block).await.map_err(|e| e.to_string())?;
    hasher.update(&first_block);
    
    // Читаем последние 4KB (если файл достаточно большой)
    if size > 4096 {
        file.seek(std::io::SeekFrom::End(-4096)).await.map_err(|e| e.to_string())?;
        let mut last_block = vec![0u8; 4096];
        file.read_exact(&mut last_block).await.map_err(|e| e.to_string())?;
        hasher.update(&last_block);
    }
    
    Ok(hasher.finish())
}

/// Обработчик клиента для TCP (устаревший, для совместимости)
pub(crate) async fn handle_client_tcp(
    stream: TcpStream,
    save_dir: PathBuf,
    options: ServerOptions,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
) -> Result<(), String> {
    let (mut reader, mut writer) = stream.into_split();
    
    loop {
        let mut len_buf = [0u8; 4];
        match reader.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(());
            }
            Err(e) => return Err(e.to_string()),
        }
        
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        reader.read_exact(&mut data).await.map_err(|e| e.to_string())?;
        
        let msg = Message::from_bytes(&data).map_err(|e| e.to_string())?;
        
        match msg {
            Message::FileStart { filename, size, compressed, offset: _, quick_hash } => {
                let is_tar_lz4 = extract::is_tar_lz4(&filename);
                let stream_extract = options.extract_options.tar_lz4 && is_tar_lz4;
                
                if stream_extract {
                    receive_and_extract_streaming_tcp(
                        &mut reader,
                        &mut writer,
                        &save_dir,
                        &filename,
                        size,
                        compressed,
                        &event_tx,
                    ).await?;
                } else {
                    let _file_path = receive_file_tcp(
                        &mut reader,
                        &mut writer,
                        &save_dir,
                        &filename,
                        size,
                        compressed,
                        quick_hash,
                        options.enable_resume,
                        &event_tx,
                    ).await?;
                }
            }
            Message::Done => {
                return Ok(());
            }
            Message::Ack => {
                let ack = Message::Ack.to_bytes().map_err(|e| e.to_string())?;
                writer.write_all(&ack).await.map_err(|e| e.to_string())?;
            }
            Message::SpeedTestRequest { size } => {
                crate::network::speedtest::handle_speedtest_server(&mut reader, &mut writer, size).await?;
            }
            _ => {
                let err = Message::Error("Неожиданное сообщение".to_string());
                let data = err.to_bytes().map_err(|e| e.to_string())?;
                writer.write_all(&data).await.map_err(|e| e.to_string())?;
            }
        }
    }
}

/// Приём файла для TCP (устаревший)
async fn receive_file_tcp(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    save_dir: &PathBuf,
    filename: &str,
    size: u64,
    compressed: bool,
    quick_hash: u64,
    enable_resume: bool,
    event_tx: &mpsc::UnboundedSender<TransferEvent>,
) -> Result<PathBuf, String> {
    let normalized_path = filename.replace('/', std::path::MAIN_SEPARATOR_STR);
    let file_path = save_dir.join(&normalized_path);
    
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Не удалось создать папку: {}", e))?;
    }
    
    let resume_offset = if enable_resume {
        check_resume(&file_path, size, quick_hash).await
    } else {
        0
    };
    
    if resume_offset >= size {
        let resume_ack = Message::ResumeAck { offset: size };
        let data = resume_ack.to_bytes().map_err(|e| e.to_string())?;
        writer.write_all(&data).await.map_err(|e| e.to_string())?;
        
        let _ = event_tx.send(TransferEvent::FileReceived(filename.to_string(), size));
        return Ok(file_path);
    }
    
    let mut file = if resume_offset > 0 {
        let f = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&file_path)
            .await
            .map_err(|e| format!("Не удалось открыть файл для resume: {}", e))?;
        
        let resume_ack = Message::ResumeAck { offset: resume_offset };
        let data = resume_ack.to_bytes().map_err(|e| e.to_string())?;
        writer.write_all(&data).await.map_err(|e| e.to_string())?;
        
        f
    } else {
        let f = tokio::fs::File::create(&file_path)
            .await
            .map_err(|e| format!("Не удалось создать файл: {}", e))?;
        
        let ack = Message::Ack.to_bytes().map_err(|e| e.to_string())?;
        writer.write_all(&ack).await.map_err(|e| e.to_string())?;
        f
    };
    
    if resume_offset > 0 {
        file.seek(std::io::SeekFrom::Start(resume_offset)).await.map_err(|e| e.to_string())?;
    }
    
    let mut received_bytes = resume_offset;
    let start_time = std::time::Instant::now();
    let mut last_progress_update = std::time::Instant::now();
    
    loop {
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let len = u32::from_le_bytes(len_buf) as usize;
        
        let mut data = vec![0u8; len];
        reader.read_exact(&mut data).await.map_err(|e| e.to_string())?;
        
        let msg = Message::from_bytes(&data).map_err(|e| e.to_string())?;
        
        match msg {
            Message::FileChunk { data, original_size: _ } => {
                let write_data = if compressed {
                    compression::decompress(&data)?
                } else {
                    data
                };
                received_bytes += write_data.len() as u64;
                file.write_all(&write_data).await.map_err(|e| e.to_string())?;
                
                if last_progress_update.elapsed().as_secs() >= 1 {
                    let _ = event_tx.send(TransferEvent::Progress(
                        0, 0, received_bytes, size, received_bytes,
                    ));
                    last_progress_update = std::time::Instant::now();
                }
            }
            Message::FileEnd => {
                file.flush().await.map_err(|e| e.to_string())?;
                
                let ack = Message::Ack.to_bytes().map_err(|e| e.to_string())?;
                writer.write_all(&ack).await.map_err(|e| e.to_string())?;
                
                let elapsed = start_time.elapsed().as_secs_f64();
                let speed_mbps = if elapsed > 0.0 { received_bytes as f64 / elapsed / 1024.0 / 1024.0 } else { 0.0 };
                
                let _ = event_tx.send(TransferEvent::Progress(
                    0, 0, received_bytes, size, received_bytes,
                ));
                let _ = event_tx.send(TransferEvent::FileReceived(
                    format!("{} ({:.1} MB/s)", filename, speed_mbps),
                    size
                ));
                return Ok(file_path);
            }
            _ => {
                return Err("Неожиданное сообщение при получении файла".to_string());
            }
        }
    }
}

