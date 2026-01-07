//! Логика отправки файлов

use crate::protocol::{Message, FileInfo};
use crate::stats::{DEFAULT_CHUNK_SIZE, MIN_CHUNK_SIZE, MAX_CHUNK_SIZE};
use super::compression;
use super::events::TransferEvent;
use super::transport::{TransportType, TransportStream};
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::mpsc;

/// Опции отправки
#[derive(Clone, Debug)]
pub struct SendOptions {
    pub use_compression: bool,
    pub enable_resume: bool,
    pub transport_type: TransportType,
}

impl Default for SendOptions {
    fn default() -> Self {
        Self {
            use_compression: false,
            enable_resume: true,
            transport_type: TransportType::default(),
        }
    }
}

/// Отправить файлы на один сервер
pub async fn send_files_to_target(
    target_id: usize,
    addr: String,
    files: Vec<FileInfo>,
    use_compression: bool,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
) -> Result<(), String> {
    let options = SendOptions {
        use_compression,
        enable_resume: true,
        transport_type: TransportType::default(),
    };
    
    send_files_to_target_with_options(target_id, addr, files, options, event_tx).await
}

/// Отправить файлы на один сервер с расширенными опциями
pub async fn send_files_to_target_with_options(
    target_id: usize,
    addr: String,
    files: Vec<FileInfo>,
    options: SendOptions,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
) -> Result<(), String> {
    // Подключаемся через выбранный транспорт
    let mut stream = super::transport::connect(options.transport_type, &addr)
        .await
        .map_err(|e| format!("Ошибка подключения [{}]: {}", options.transport_type.name(), e))?;
    
    let _ = event_tx.send(TransferEvent::Connected(target_id, format!("{} [{}]", addr, options.transport_type.name())));
    
    // Адаптивный размер чанка
    let mut chunk_size = DEFAULT_CHUNK_SIZE;
    
    for (idx, file) in files.iter().enumerate() {
        let _ = event_tx.send(TransferEvent::FileStarted(target_id, idx));
        
        match send_single_file_transport(
            &mut *stream,
            file,
            target_id,
            idx,
            &options,
            &mut chunk_size,
            &event_tx,
        ).await {
            Ok(skipped) => {
                if skipped {
                    let _ = event_tx.send(TransferEvent::FileSkipped(target_id, idx));
                } else {
                    let _ = event_tx.send(TransferEvent::FileCompleted(target_id, idx));
                }
            }
            Err(e) => {
                let _ = event_tx.send(TransferEvent::FileError(target_id, idx, e.clone()));
                return Err(e);
            }
        }
    }
    
    // Отправляем сигнал завершения
    let done_msg = Message::Done.to_bytes().map_err(|e| e.to_string())?;
    stream.write_all(&done_msg).await.map_err(|e| e.to_string())?;
    
    let _ = event_tx.send(TransferEvent::TargetCompleted(target_id));
    Ok(())
}

/// Отправить файлы на несколько серверов параллельно
pub async fn send_files_to_multiple(
    targets: Vec<String>,
    files: Vec<FileInfo>,
    use_compression: bool,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
) {
    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let options = SendOptions {
        use_compression,
        enable_resume: true,
        transport_type: TransportType::default(),
    };
    send_files_to_multiple_with_stop(targets, files, options, event_tx, stop_flag).await;
}

/// Отправить файлы на несколько серверов параллельно с поддержкой остановки
pub async fn send_files_to_multiple_with_stop(
    targets: Vec<String>,
    files: Vec<FileInfo>,
    options: SendOptions,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
    stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut handles = Vec::new();
    
    for (target_id, addr) in targets.into_iter().enumerate() {
        let files = files.clone();
        let event_tx = event_tx.clone();
        let stop_flag = stop_flag.clone();
        let options = options.clone();
        
        let handle = tokio::spawn(async move {
            if let Err(e) = send_files_to_target_with_stop(target_id, addr, files, options, event_tx.clone(), stop_flag).await {
                let _ = event_tx.send(TransferEvent::ConnectionError(target_id, e));
            }
        });
        
        handles.push(handle);
    }
    
    // Ждём завершения всех передач
    for handle in handles {
        let _ = handle.await;
    }
    
    let _ = event_tx.send(TransferEvent::AllCompleted);
}

/// Отправить файлы на один сервер с поддержкой остановки
pub async fn send_files_to_target_with_stop(
    target_id: usize,
    addr: String,
    files: Vec<FileInfo>,
    options: SendOptions,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
    stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    
    // Подключаемся через выбранный транспорт
    let mut stream = super::transport::connect(options.transport_type, &addr)
        .await
        .map_err(|e| format!("Ошибка подключения [{}]: {}", options.transport_type.name(), e))?;
    
    let _ = event_tx.send(TransferEvent::Connected(target_id, format!("{} [{}]", addr, options.transport_type.name())));
    
    let mut chunk_size = DEFAULT_CHUNK_SIZE;
    
    for (idx, file) in files.iter().enumerate() {
        // Проверяем флаг остановки
        if stop_flag.load(Ordering::SeqCst) {
            return Err("Остановлено пользователем".to_string());
        }
        
        let _ = event_tx.send(TransferEvent::FileStarted(target_id, idx));
        
        match send_single_file_transport_with_stop(
            &mut *stream,
            file,
            target_id,
            idx,
            &options,
            &mut chunk_size,
            &event_tx,
            &stop_flag,
        ).await {
            Ok(skipped) => {
                if skipped {
                    let _ = event_tx.send(TransferEvent::FileSkipped(target_id, idx));
                } else {
                    let _ = event_tx.send(TransferEvent::FileCompleted(target_id, idx));
                }
            }
            Err(e) => {
                let _ = event_tx.send(TransferEvent::FileError(target_id, idx, e.clone()));
                return Err(e);
            }
        }
    }
    
    // Отправляем сигнал завершения
    let done_msg = Message::Done.to_bytes().map_err(|e| e.to_string())?;
    stream.write_all(&done_msg).await.map_err(|e| e.to_string())?;
    
    let _ = event_tx.send(TransferEvent::TargetCompleted(target_id));
    Ok(())
}

/// Отправить один файл через транспорт. Возвращает true если файл был пропущен
async fn send_single_file_transport(
    stream: &mut dyn TransportStream,
    file: &FileInfo,
    target_id: usize,
    file_idx: usize,
    options: &SendOptions,
    chunk_size: &mut usize,
    event_tx: &mpsc::UnboundedSender<TransferEvent>,
) -> Result<bool, String> {
    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    send_single_file_transport_with_stop(stream, file, target_id, file_idx, options, chunk_size, event_tx, &stop_flag).await
}

/// Отправить один файл через транспорт с поддержкой остановки
async fn send_single_file_transport_with_stop(
    stream: &mut dyn TransportStream,
    file: &FileInfo,
    target_id: usize,
    file_idx: usize,
    options: &SendOptions,
    chunk_size: &mut usize,
    event_tx: &mpsc::UnboundedSender<TransferEvent>,
    stop_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<bool, String> {
    use std::sync::atomic::Ordering;
    
    // Открываем файл
    let mut f = tokio::fs::File::open(&file.path)
        .await
        .map_err(|e| format!("Не удалось открыть файл: {}", e))?;
    
    // Вычисляем быстрый хэш для синхронизации
    let quick_hash = compute_quick_hash(&file.path).await.unwrap_or(0);
    
    // Отправляем заголовок
    let start_msg = Message::FileStart {
        filename: file.relative_path.clone(),
        size: file.size,
        compressed: options.use_compression,
        offset: 0,
        quick_hash,
    };
    let data = start_msg.to_bytes().map_err(|e| e.to_string())?;
    stream.write_all(&data).await.map_err(|e| e.to_string())?;
    
    // Ждём ответ (может быть Ack или ResumeAck)
    let start_offset = wait_resume_ack_transport(stream).await?;
    
    // Если offset == size, файл уже актуален
    if start_offset >= file.size {
        return Ok(true); // Файл пропущен
    }
    
    // Если есть offset, сообщаем о возобновлении
    if start_offset > 0 {
        let _ = event_tx.send(TransferEvent::FileResumed(target_id, file_idx, start_offset));
        f.seek(std::io::SeekFrom::Start(start_offset)).await.map_err(|e| e.to_string())?;
    }
    
    // Отправляем данные с адаптивным размером чанка
    let mut buffer = vec![0u8; MAX_CHUNK_SIZE];
    let mut transferred: u64 = start_offset;
    let mut total_original: u64 = 0;
    let mut total_compressed: u64 = 0;
    let mut last_speed_check = Instant::now();
    let mut last_progress_update = Instant::now();
    let mut bytes_since_check: u64 = 0;
    
    loop {
        // Проверяем флаг остановки
        if stop_flag.load(Ordering::SeqCst) {
            return Err("Остановлено пользователем".to_string());
        }
        
        // Читаем чанк текущего размера
        let read_size = (*chunk_size).min(buffer.len());
        let n = f.read(&mut buffer[..read_size]).await.map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        
        // Сжимаем данные если включено
        let (chunk_data, original_size) = if options.use_compression {
            let compressed = compression::compress(&buffer[..n]);
            (compressed, n)
        } else {
            (buffer[..n].to_vec(), n)
        };
        
        let compressed_size = chunk_data.len();
        total_original += original_size as u64;
        total_compressed += compressed_size as u64;
        
        let chunk_msg = Message::FileChunk {
            data: chunk_data,
            original_size,
        };
        let data = chunk_msg.to_bytes().map_err(|e| e.to_string())?;
        stream.write_all(&data).await.map_err(|e| e.to_string())?;
        
        transferred += n as u64;
        bytes_since_check += n as u64;
        
        // Отправляем прогресс раз в секунду (не чаще)
        if last_progress_update.elapsed().as_secs() >= 1 {
            let _ = event_tx.send(TransferEvent::Progress(
                target_id,
                file_idx,
                transferred,
                total_original,
                total_compressed,
            ));
            last_progress_update = Instant::now();
        }
        
        // Адаптируем размер чанка каждые 100ms
        let elapsed = last_speed_check.elapsed();
        if elapsed.as_millis() >= 100 {
            let speed = bytes_since_check as f64 / elapsed.as_secs_f64();
            adapt_chunk_size(chunk_size, speed);
            last_speed_check = Instant::now();
            bytes_since_check = 0;
        }
    }
    
    // Финальное обновление прогресса (100%)
    let _ = event_tx.send(TransferEvent::Progress(
        target_id,
        file_idx,
        transferred,
        total_original,
        total_compressed,
    ));
    
    // Отправляем конец файла
    let end_msg = Message::FileEnd;
    let data = end_msg.to_bytes().map_err(|e| e.to_string())?;
    stream.write_all(&data).await.map_err(|e| e.to_string())?;
    
    // Ждём подтверждение
    wait_ack_transport(stream).await?;
    
    Ok(false) // Файл был передан
}

/// Вычислить быстрый хэш файла
async fn compute_quick_hash(path: &std::path::Path) -> std::io::Result<u64> {
    let metadata = tokio::fs::metadata(path).await?;
    let size = metadata.len();
    
    if size == 0 {
        return Ok(0);
    }
    
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = FnvHasher::new();
    
    // Хэшируем размер
    hasher.update(&size.to_le_bytes());
    
    // Читаем первые 4KB
    let first_size = 4096usize.min(size as usize);
    let mut first_block = vec![0u8; first_size];
    file.read_exact(&mut first_block).await?;
    hasher.update(&first_block);
    
    // Если файл больше 4KB, читаем последние 4KB
    if size > 4096 {
        file.seek(std::io::SeekFrom::End(-4096)).await?;
        let mut last_block = vec![0u8; 4096];
        file.read_exact(&mut last_block).await?;
        hasher.update(&last_block);
    }
    
    Ok(hasher.finish())
}

/// Простой FNV-1a хэшер
struct FnvHasher {
    hash: u64,
}

impl FnvHasher {
    fn new() -> Self {
        Self { hash: 0xcbf29ce484222325 }
    }
    
    fn update(&mut self, data: &[u8]) {
        const FNV_PRIME: u64 = 0x100000001b3;
        for byte in data {
            self.hash ^= *byte as u64;
            self.hash = self.hash.wrapping_mul(FNV_PRIME);
        }
    }
    
    fn finish(self) -> u64 {
        self.hash
    }
}

/// Адаптировать размер чанка на основе скорости
fn adapt_chunk_size(chunk_size: &mut usize, speed_bytes_per_sec: f64) {
    // Целевое время чанка: 50-100ms
    let target_time_ms = 75.0;
    
    if speed_bytes_per_sec > 0.0 {
        let optimal = (speed_bytes_per_sec * target_time_ms / 1000.0) as usize;
        let new_size = optimal.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
        
        // Плавное изменение
        if new_size > *chunk_size {
            *chunk_size = (*chunk_size * 3 / 2).min(new_size);
        } else if new_size < *chunk_size {
            *chunk_size = (*chunk_size * 2 / 3).max(new_size);
        }
        
        *chunk_size = (*chunk_size).clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);
    }
}

/// Ждать Ack или ResumeAck через транспорт, возвращает offset
async fn wait_resume_ack_transport(stream: &mut dyn TransportStream) -> Result<u64, String> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
    let len = u32::from_le_bytes(len_buf) as usize;
    
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await.map_err(|e| e.to_string())?;
    
    let msg = Message::from_bytes(&data).map_err(|e| e.to_string())?;
    match msg {
        Message::Ack => Ok(0),
        Message::ResumeAck { offset } => Ok(offset),
        Message::Cancel => Err("⛔ Получатель отменил передачу".to_string()),
        Message::Error(e) => Err(e),
        _ => Err("Неожиданный ответ".to_string()),
    }
}

async fn wait_ack_transport(stream: &mut dyn TransportStream) -> Result<(), String> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
    let len = u32::from_le_bytes(len_buf) as usize;
    
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await.map_err(|e| e.to_string())?;
    
    let msg = Message::from_bytes(&data).map_err(|e| e.to_string())?;
    match msg {
        Message::Ack => Ok(()),
        Message::Cancel => Err("⛔ Получатель отменил передачу".to_string()),
        Message::Error(e) => Err(e),
        _ => Err("Неожиданный ответ".to_string()),
    }
}
