//! Потоковая распаковка архивов

use crate::network::compression;
use crate::network::events::TransferEvent;
use crate::network::transport::TransportStream;
use crate::protocol::Message;
use lz4_flex::frame::FrameDecoder;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

/// FNV-1a хэшер для быстрого хэширования
pub(crate) struct FnvHasher {
    hash: u64,
}

impl FnvHasher {
    pub fn new() -> Self {
        Self { hash: 0xcbf29ce484222325 }
    }
    
    pub fn update(&mut self, data: &[u8]) {
        const FNV_PRIME: u64 = 0x100000001b3;
        for byte in data {
            self.hash ^= *byte as u64;
            self.hash = self.hash.wrapping_mul(FNV_PRIME);
        }
    }
    
    pub fn finish(self) -> u64 {
        self.hash
    }
}

/// Адаптер для чтения из канала как из std::io::Read
pub(crate) struct ChannelReader {
    receiver: std_mpsc::Receiver<Vec<u8>>,
    buffer: Vec<u8>,
    pos: usize,
}

impl ChannelReader {
    pub fn new(receiver: std_mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            receiver,
            buffer: Vec::new(),
            pos: 0,
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Если буфер пуст или весь прочитан, получаем следующий chunk
        if self.pos >= self.buffer.len() {
            match self.receiver.recv() {
                Ok(data) => {
                    self.buffer = data;
                    self.pos = 0;
                }
                Err(_) => {
                    // Канал закрыт - EOF
                    return Ok(0);
                }
            }
        }
        
        // Читаем из буфера
        let available = self.buffer.len() - self.pos;
        let to_read = buf.len().min(available);
        buf[..to_read].copy_from_slice(&self.buffer[self.pos..self.pos + to_read]);
        self.pos += to_read;
        Ok(to_read)
    }
}

/// Распаковка tar.lz4 из канала (потоковая, без буферизации всего файла)
pub(crate) fn extract_from_channel(
    rx: std_mpsc::Receiver<Vec<u8>>,
    output_dir: &PathBuf,
    filename: &str,
    event_tx: &mpsc::UnboundedSender<TransferEvent>,
) -> Result<(), String> {
    use std::fs::{self, File};
    
    // Создаём reader из канала
    let channel_reader = ChannelReader::new(rx);
    
    // LZ4 frame decoder поверх channel reader
    let lz4_reader = FrameDecoder::new(channel_reader);
    
    // Tar archive поверх LZ4 decoder
    let mut archive = tar::Archive::new(lz4_reader);
    
    let mut files_count = 0usize;
    let mut total_size = 0u64;
    
    // Читаем и распаковываем файлы по одному - ПОТОКОВО!
    for entry_result in archive.entries().map_err(|e| format!("Ошибка чтения tar: {}", e))? {
        let mut entry = entry_result.map_err(|e| format!("Ошибка записи tar: {}", e))?;
        
        let path = entry.path()
            .map_err(|e| format!("Ошибка пути: {}", e))?
            .to_path_buf();
        let full_path = output_dir.join(&path);
        
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&full_path)
                .map_err(|e| format!("Ошибка создания папки: {}", e))?;
        } else if entry.header().entry_type().is_file() {
            // Создаём родительскую директорию
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Ошибка создания папки: {}", e))?;
            }
            
            // Получаем размер до распаковки
            let size = entry.header().size().unwrap_or(0);
            
            // Распаковываем файл напрямую на диск - ПОТОКОВО!
            let mut file = File::create(&full_path)
                .map_err(|e| format!("Ошибка создания файла: {}", e))?;
            
            std::io::copy(&mut entry, &mut file)
                .map_err(|e| format!("Ошибка записи файла: {}", e))?;
            
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = entry.header().mode().unwrap_or(0o644);
                let _ = fs::set_permissions(&full_path, fs::Permissions::from_mode(mode));
            }
            
            files_count += 1;
            total_size += size;
        }
    }
    
    let _ = event_tx.send(TransferEvent::ExtractionCompleted(
        filename.to_string(),
        files_count,
        total_size,
    ));
    
    Ok(())
}

/// Распаковка tar.zst из канала (потоковая, без буферизации всего файла)
pub(crate) fn extract_from_channel_zst(
    rx: std_mpsc::Receiver<Vec<u8>>,
    output_dir: &PathBuf,
    filename: &str,
    event_tx: &mpsc::UnboundedSender<TransferEvent>,
) -> Result<(), String> {
    use std::fs::{self, File};
    
    // Создаём reader из канала
    let channel_reader = ChannelReader::new(rx);
    
    // Обёртываем в BufReader для лучшей производительности
    use std::io::BufReader;
    let buffered_reader = BufReader::with_capacity(1024 * 1024, channel_reader); // 1MB буфер
    
    // ZST decoder поверх buffered reader
    // Используем DecoderBuilder для настройки лимита памяти для больших фреймов
    // window_log_max=31 поддерживает архивы, сжатые с --long=31 (до 2GB окно)
    let zst_reader = match zstd::stream::Decoder::with_buffer(buffered_reader) {
        Ok(mut decoder) => {
            // Устанавливаем максимальный лимит памяти для декодирования (window_log_max=31 = до 2GB на фрейм)
            // Это необходимо для архивов, сжатых с параметром --long=31
            if let Err(e) = decoder.window_log_max(31) {
                let err_msg = format!("Ошибка настройки декодера (window_log_max=31): {}", e);
                let _ = event_tx.send(TransferEvent::ExtractionError(filename.to_string(), err_msg.clone()));
                return Err(err_msg);
            }
            decoder
        },
        Err(e) => {
            let err_msg = format!("Ошибка создания ZST декодера с буфером: {}", e);
            let _ = event_tx.send(TransferEvent::ExtractionError(filename.to_string(), err_msg.clone()));
            return Err(err_msg);
        }
    };
    
    // Tar archive поверх ZST decoder
    let mut archive = tar::Archive::new(zst_reader);
    
    let mut files_count = 0usize;
    let mut total_size = 0u64;
    
    // Читаем и распаковываем файлы по одному - ПОТОКОВО!
    let entries = match archive.entries() {
        Ok(e) => e,
        Err(e) => {
            let err_msg = format!("Ошибка чтения tar архива: {} (kind: {:?})", e, e.kind());
            let _ = event_tx.send(TransferEvent::ExtractionError(filename.to_string(), err_msg.clone()));
            return Err(err_msg);
        }
    };
    
    for entry_result in entries {
        let mut entry = match entry_result {
            Ok(e) => e,
            Err(e) => {
                // Если ошибка чтения записи - это может быть конец архива или повреждение
                // Проверяем, может быть это просто конец потока
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    // Конец данных - это нормально, возможно данные еще не все получены
                    // Не возвращаем ошибку, просто завершаем цикл
                    break;
                }
                let err_msg = format!("Ошибка чтения записи tar: {} (kind: {:?})", e, e.kind());
                let _ = event_tx.send(TransferEvent::ExtractionError(filename.to_string(), err_msg.clone()));
                return Err(err_msg);
            }
        };
        
        let path = entry.path()
            .map_err(|e| format!("Ошибка пути: {}", e))?
            .to_path_buf();
        let full_path = output_dir.join(&path);
        
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&full_path)
                .map_err(|e| format!("Ошибка создания папки: {}", e))?;
        } else if entry.header().entry_type().is_file() {
            // Создаём родительскую директорию
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Ошибка создания папки: {}", e))?;
            }
            
            // Получаем размер до распаковки
            let size = entry.header().size().unwrap_or(0);
            
            // Распаковываем файл напрямую на диск - ПОТОКОВО!
            let mut file = File::create(&full_path)
                .map_err(|e| format!("Ошибка создания файла: {}", e))?;
            
            std::io::copy(&mut entry, &mut file)
                .map_err(|e| format!("Ошибка записи файла: {}", e))?;
            
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = entry.header().mode().unwrap_or(0o644);
                let _ = fs::set_permissions(&full_path, fs::Permissions::from_mode(mode));
            }
            
            files_count += 1;
            total_size += size;
        }
    }
    
    let _ = event_tx.send(TransferEvent::ExtractionCompleted(
        filename.to_string(),
        files_count,
        total_size,
    ));
    
    Ok(())
}

/// ИСТИННАЯ потоковая распаковка tar.lz4 через транспорт с поддержкой резюме
pub(crate) async fn receive_and_extract_streaming_transport(
    stream: &mut dyn TransportStream,
    save_dir: &PathBuf,
    filename: &str,
    size: u64,
    compressed: bool,
    save_archive: bool, // Сохранять архив для возможности резюме
    event_tx: &mpsc::UnboundedSender<TransferEvent>,
    stop_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    use tokio::io::{AsyncWriteExt, AsyncSeekExt};
    
    // Путь к сырому архиву (для резюме) - только если включено сохранение
    let raw_file_path = save_dir.join(filename);
    
    // Проверяем есть ли частичный файл для резюме (только если сохраняем)
    let resume_offset = if save_archive {
        if let Ok(meta) = tokio::fs::metadata(&raw_file_path).await {
            let current_size = meta.len();
            if current_size < size {
                current_size // Есть частичный файл - продолжаем
            } else {
                0 // Файл полный или больше - качаем заново
            }
        } else {
            0 // Файла нет
        }
    } else {
        0 // Резюме отключено
    };
    
    // Отправляем ResumeAck или Ack
    if resume_offset > 0 {
        let resume_ack = crate::protocol::Message::ResumeAck { offset: resume_offset };
        let data = resume_ack.to_bytes().map_err(|e| e.to_string())?;
        stream.write_all(&data).await.map_err(|e| e.to_string())?;
        let _ = event_tx.send(TransferEvent::FileReceived(
            format!("🔄 Возобновление с {:.2} ГБ", resume_offset as f64 / 1024.0 / 1024.0 / 1024.0),
            0
        ));
    } else {
        super::send_ack_transport(stream).await?;
    }
    
    let _ = event_tx.send(TransferEvent::ExtractionStarted(filename.to_string()));
    
    // Открываем файл для записи сырых данных (только если включено сохранение)
    let mut raw_file: Option<tokio::fs::File> = if save_archive {
        Some(if resume_offset > 0 {
            let mut f = tokio::fs::OpenOptions::new()
                .write(true)
                .open(&raw_file_path)
                .await
                .map_err(|e| format!("Не удалось открыть файл: {}", e))?;
            f.seek(std::io::SeekFrom::Start(resume_offset)).await.map_err(|e| e.to_string())?;
            f
        } else {
            tokio::fs::File::create(&raw_file_path)
                .await
                .map_err(|e| format!("Не удалось создать файл: {}", e))?
        })
    } else {
        None
    };
    
    // Создаём канал для передачи данных в распаковщик (только если начинаем с нуля)
    let (tx, rx) = std_mpsc::sync_channel::<Vec<u8>>(32);
    let streaming_extract = resume_offset == 0; // Потоковая распаковка только с начала
    
    // Запускаем распаковщик в отдельном потоке (если не резюме)
    let output_dir = save_dir.clone();
    let event_tx_clone = event_tx.clone();
    let filename_clone = filename.to_string();
    
    // Определяем тип архива для выбора правильной функции распаковки
    let archive_type = crate::extract::ArchiveType::from_filename(filename);
    let is_tar_zst = archive_type == crate::extract::ArchiveType::TarZst;
    
    let extract_handle = if streaming_extract {
        Some(std::thread::spawn(move || {
            if is_tar_zst {
                extract_from_channel_zst(rx, &output_dir, &filename_clone, &event_tx_clone)
            } else {
                extract_from_channel(rx, &output_dir, &filename_clone, &event_tx_clone)
            }
        }))
    } else {
        drop(rx); // Не используем канал при резюме
        None
    };
    
    // Читаем данные из сети
    let mut received_bytes = resume_offset;
    let start_time = Instant::now();
    let mut last_progress_update = Instant::now();
    #[allow(unused_assignments)]
    let mut network_error: Option<String> = None;
    
    loop {
        // Проверяем флаг остановки
        if stop_flag.load(Ordering::SeqCst) {
            if let Some(ref mut f) = raw_file { let _ = f.flush().await; }
            drop(tx);
            if let Some(handle) = extract_handle {
                let _ = event_tx.send(TransferEvent::FileReceived(
                    "⏳ Ожидание завершения распаковки...".to_string(), 0
                ));
                let _ = handle.join();
            }
            let msg = if save_archive {
                format!("⏸️ Приостановлено: {:.2} ГБ сохранено", received_bytes as f64 / 1024.0 / 1024.0 / 1024.0)
            } else {
                "⛔ Остановлено".to_string()
            };
            let _ = event_tx.send(TransferEvent::FileReceived(msg, received_bytes));
            return Err("⛔ Остановлено пользователем".to_string());
        }
        
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) => {
                if let Some(ref mut f) = raw_file { let _ = f.flush().await; }
                network_error = Some(e.to_string());
                break;
            }
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        
        let mut data = vec![0u8; len];
        match stream.read_exact(&mut data).await {
            Ok(_) => {}
            Err(e) => {
                if let Some(ref mut f) = raw_file { let _ = f.flush().await; }
                network_error = Some(e.to_string());
                break;
            }
        }
        
        let msg = match Message::from_bytes(&data) {
            Ok(m) => m,
            Err(e) => {
                if let Some(ref mut f) = raw_file { let _ = f.flush().await; }
                network_error = Some(e.to_string());
                break;
            }
        };
        
        match msg {
            Message::FileChunk { data, original_size: _ } => {
                // Проверяем флаг остановки после каждого чанка
                if stop_flag.load(Ordering::SeqCst) {
                    if let Some(ref mut f) = raw_file { let _ = f.flush().await; }
                    drop(tx);
                    if let Some(handle) = extract_handle {
                        let _ = event_tx.send(TransferEvent::FileReceived(
                            "⏳ Ожидание завершения распаковки...".to_string(), 0
                        ));
                        let _ = handle.join();
                    }
                    let msg = if save_archive {
                        format!("⏸️ Приостановлено: {:.2} ГБ сохранено", received_bytes as f64 / 1024.0 / 1024.0 / 1024.0)
                    } else {
                        "⛔ Остановлено".to_string()
                    };
                    let _ = event_tx.send(TransferEvent::FileReceived(msg, received_bytes));
                    return Err("⛔ Остановлено пользователем".to_string());
                }
                
                let chunk_data = if compressed {
                    match compression::decompress(&data) {
                        Ok(d) => d,
                        Err(e) => {
                            if let Some(ref mut f) = raw_file { let _ = f.flush().await; }
                            network_error = Some(e);
                            break;
                        }
                    }
                } else {
                    data
                };
                
                // Сохраняем сырые данные в файл (только если включено)
                if let Some(ref mut f) = raw_file {
                    if let Err(e) = f.write_all(&chunk_data).await {
                        network_error = Some(e.to_string());
                        break;
                    }
                }
                
                received_bytes += chunk_data.len() as u64;
                
                if last_progress_update.elapsed().as_secs() >= 1 {
                    let _ = event_tx.send(TransferEvent::Progress(
                        0, 0, received_bytes, size, received_bytes,
                    ));
                    last_progress_update = Instant::now();
                }
                
                // Отправляем в распаковщик только если потоковая распаковка
                if streaming_extract {
                    match tx.send(chunk_data) {
                        Ok(()) => {},
                        Err(_) => {
                            // Распаковщик завершился - это может быть ошибка распаковки
                            // Не закрываем соединение, просто логируем и продолжаем получать данные
                            let _ = event_tx.send(TransferEvent::ExtractionError(
                                filename.to_string(),
                                format!("Распаковщик завершился раньше времени (получено {} байт)", received_bytes),
                            ));
                            // Продолжаем получать данные, но не отправляем в распаковщик
                            // Отключаем потоковую распаковку для следующих чанков
                        }
                    }
                }
            }
            Message::FileEnd => {
                // Сбрасываем буфер файла
                if let Some(ref mut f) = raw_file { let _ = f.flush().await; }
                
                let elapsed = start_time.elapsed().as_secs_f64();
                let speed_mbps = if elapsed > 0.0 { (received_bytes - resume_offset) as f64 / elapsed / 1024.0 / 1024.0 } else { 0.0 };
                
                let _ = event_tx.send(TransferEvent::Progress(
                    0, 0, received_bytes, size, received_bytes,
                ));
                
                // Закрываем канал
                drop(tx);
                
                if let Some(handle) = extract_handle {
                    // Потоковая распаковка - ждём завершения
                    let _ = event_tx.send(TransferEvent::FileReceived(
                        "⏳ Завершение распаковки...".to_string(), 0
                    ));
                    
                    match handle.join() {
                        Ok(Ok(())) => {
                            // Удаляем raw файл после успешной распаковки (если сохраняли)
                            if save_archive {
                                let _ = tokio::fs::remove_file(&raw_file_path).await;
                            }
                            let _ = event_tx.send(TransferEvent::FileReceived(
                                format!("✅ Потоковая распаковка завершена: {:.2} ГБ @ {:.1} MB/s", 
                                    received_bytes as f64 / 1024.0 / 1024.0 / 1024.0,
                                    speed_mbps),
                                received_bytes
                            ));
                        }
                        Ok(Err(e)) => {
                            let _ = event_tx.send(TransferEvent::ExtractionError(
                                filename.to_string(),
                                e,
                            ));
                        }
                        Err(_) => {
                            let _ = event_tx.send(TransferEvent::ExtractionError(
                                filename.to_string(),
                                "Поток распаковки завершился с паникой".to_string(),
                            ));
                        }
                    }
                } else if save_archive {
                    // Это было резюме - распаковываем из сохранённого файла
                    let _ = event_tx.send(TransferEvent::FileReceived(
                        format!("✅ Докачано: {:.2} ГБ @ {:.1} MB/s, распаковка...", 
                            received_bytes as f64 / 1024.0 / 1024.0 / 1024.0,
                            speed_mbps),
                        received_bytes
                    ));
                    
                    // Запускаем распаковку из файла
                    let output_dir = save_dir.clone();
                    let event_tx_clone = event_tx.clone();
                    let filename_clone = filename.to_string();
                    let raw_path = raw_file_path.clone();
                    
                    tokio::task::spawn_blocking(move || {
                        match crate::extract::extract_tar_lz4_simple(&raw_path, &output_dir) {
                            Ok(result) => {
                                let _ = event_tx_clone.send(TransferEvent::ExtractionCompleted(
                                    filename_clone,
                                    result.files_count,
                                    result.total_size,
                                ));
                                // Удаляем raw файл после распаковки
                                let _ = std::fs::remove_file(&raw_path);
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
                
                super::send_ack_transport(stream).await?;
                return Ok(());
            }
            _ => {
                if let Some(ref mut f) = raw_file { let _ = f.flush().await; }
                network_error = Some("Неожиданное сообщение при получении файла".to_string());
                break;
            }
        }
    }
    
    // Если вышли из цикла с ошибкой сети - сохраняем файл и ждём распаковщика
    if let Some(ref mut f) = raw_file { let _ = f.flush().await; }
    drop(tx);
    
    let msg = if save_archive {
        format!("⚠️ Соединение прервано. Сохранено: {:.2} ГБ (можно возобновить)", 
            received_bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    } else {
        format!("⚠️ Соединение прервано. Получено: {:.2} ГБ", 
            received_bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    };
    let _ = event_tx.send(TransferEvent::FileReceived(msg, received_bytes));
    
    // Ждём завершения распаковщика если он был запущен
    if let Some(handle) = extract_handle {
        let _ = handle.join();
    }
    
    if let Some(err) = network_error {
        Err(err)
    } else {
        Ok(())
    }
}

/// ИСТИННАЯ потоковая распаковка tar.lz4 (для TCP)
pub(crate) async fn receive_and_extract_streaming_tcp(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    save_dir: &PathBuf,
    filename: &str,
    size: u64,
    compressed: bool,
    event_tx: &mpsc::UnboundedSender<TransferEvent>,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    
    // Отправляем Ack
    let ack = Message::Ack.to_bytes().map_err(|e| e.to_string())?;
    writer.write_all(&ack).await.map_err(|e| e.to_string())?;
    
    let _ = event_tx.send(TransferEvent::ExtractionStarted(filename.to_string()));
    
    // Создаём канал для передачи данных в распаковщик
    let (tx, rx) = std_mpsc::sync_channel::<Vec<u8>>(32);
    
    // Запускаем распаковщик в отдельном потоке
    let output_dir = save_dir.clone();
    let event_tx_clone = event_tx.clone();
    let filename_clone = filename.to_string();
    
    // Определяем тип архива для выбора правильной функции распаковки
    let archive_type = crate::extract::ArchiveType::from_filename(&filename);
    let is_tar_zst = archive_type == crate::extract::ArchiveType::TarZst;
    
    let extract_handle = std::thread::spawn(move || {
        if is_tar_zst {
            extract_from_channel_zst(rx, &output_dir, &filename_clone, &event_tx_clone)
        } else {
            extract_from_channel(rx, &output_dir, &filename_clone, &event_tx_clone)
        }
    });
    
    // Читаем данные из сети и отправляем в канал
    let mut received_bytes = 0u64;
    let start_time = Instant::now();
    let mut last_progress_update = Instant::now();
    #[allow(unused_assignments)]
    let mut network_error: Option<String> = None;
    
    loop {
        let mut len_buf = [0u8; 4];
        match reader.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) => {
                network_error = Some(e.to_string());
                break;
            }
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        
        let mut data = vec![0u8; len];
        match reader.read_exact(&mut data).await {
            Ok(_) => {}
            Err(e) => {
                network_error = Some(e.to_string());
                break;
            }
        }
        
        let msg = match Message::from_bytes(&data) {
            Ok(m) => m,
            Err(e) => {
                network_error = Some(e.to_string());
                break;
            }
        };
        
        match msg {
            Message::FileChunk { data, original_size: _ } => {
                let chunk_data = if compressed {
                    match compression::decompress(&data) {
                        Ok(d) => d,
                        Err(e) => {
                            network_error = Some(e);
                            break;
                        }
                    }
                } else {
                    data
                };
                received_bytes += chunk_data.len() as u64;
                
                if last_progress_update.elapsed().as_secs() >= 1 {
                    let _ = event_tx.send(TransferEvent::Progress(
                        0, 0, received_bytes, size, received_bytes,
                    ));
                    last_progress_update = Instant::now();
                }
                
                if tx.send(chunk_data).is_err() {
                    return Err("Ошибка: распаковщик завершился раньше времени".to_string());
                }
            }
            Message::FileEnd => {
                let elapsed = start_time.elapsed().as_secs_f64();
                let speed_mbps = if elapsed > 0.0 { received_bytes as f64 / elapsed / 1024.0 / 1024.0 } else { 0.0 };
                
                let _ = event_tx.send(TransferEvent::Progress(
                    0, 0, received_bytes, size, received_bytes,
                ));
                
                // Закрываем канал и ждём завершения распаковки
                drop(tx);
                let _ = event_tx.send(TransferEvent::FileReceived(
                    "⏳ Завершение распаковки...".to_string(), 0
                ));
                
                match extract_handle.join() {
                    Ok(Ok(())) => {
                        let _ = event_tx.send(TransferEvent::FileReceived(
                            format!("✅ Потоковая распаковка завершена: {:.2} ГБ @ {:.1} MB/s", 
                                received_bytes as f64 / 1024.0 / 1024.0 / 1024.0,
                                speed_mbps),
                            received_bytes
                        ));
                    }
                    Ok(Err(e)) => {
                        let _ = event_tx.send(TransferEvent::ExtractionError(
                            filename.to_string(),
                            e,
                        ));
                    }
                    Err(_) => {
                        let _ = event_tx.send(TransferEvent::ExtractionError(
                            filename.to_string(),
                            "Поток распаковки завершился с паникой".to_string(),
                        ));
                    }
                }
                
                writer.write_all(&ack).await.map_err(|e| e.to_string())?;
                return Ok(());
            }
            _ => {
                network_error = Some("Неожиданное сообщение при получении файла".to_string());
                break;
            }
        }
    }
    
    // Если вышли из цикла с ошибкой сети - всё равно ждём завершения распаковки
    drop(tx);
    let _ = event_tx.send(TransferEvent::FileReceived(
        format!("⚠️ Клиент отключился, завершаем распаковку ({:.2} ГБ получено)...", 
            received_bytes as f64 / 1024.0 / 1024.0 / 1024.0),
        0
    ));
    
    match extract_handle.join() {
        Ok(Ok(())) => {
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed_mbps = if elapsed > 0.0 { received_bytes as f64 / elapsed / 1024.0 / 1024.0 } else { 0.0 };
            let _ = event_tx.send(TransferEvent::FileReceived(
                format!("✅ Распаковка завершена (частичная): {:.2} ГБ @ {:.1} MB/s", 
                    received_bytes as f64 / 1024.0 / 1024.0 / 1024.0,
                    speed_mbps),
                received_bytes
            ));
        }
        Ok(Err(e)) => {
            let _ = event_tx.send(TransferEvent::ExtractionError(
                filename.to_string(),
                format!("Ошибка распаковки после отключения: {}", e),
            ));
        }
        Err(_) => {
            let _ = event_tx.send(TransferEvent::ExtractionError(
                filename.to_string(),
                "Поток распаковки завершился с паникой".to_string(),
            ));
        }
    }
    
    if let Some(err) = network_error {
        Err(err)
    } else {
        Ok(())
    }
}

