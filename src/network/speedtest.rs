//! Модуль спидтеста для измерения скорости между клиентами

use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::network::TransferEvent;
use crate::protocol::{Message, DEFAULT_PORT};

/// Размер данных для спидтеста по умолчанию (10 MB)
pub const DEFAULT_SPEEDTEST_SIZE: u64 = 10 * 1024 * 1024;

/// Размер чанка для спидтеста (64 KB)
const SPEEDTEST_CHUNK_SIZE: usize = 64 * 1024;

/// Результат спидтеста
#[derive(Debug, Clone)]
pub struct SpeedTestResult {
    /// Скорость загрузки (upload) в MB/s
    pub upload_speed: f64,
    /// Скорость скачивания (download) в MB/s
    pub download_speed: f64,
    /// Задержка (latency) в миллисекундах
    pub latency_ms: f64,
}

impl SpeedTestResult {
    /// Форматировать результат для отображения
    pub fn formatted(&self) -> String {
        format!(
            "Upload: {:.1} MB/s | Download: {:.1} MB/s | Ping: {:.1} ms",
            self.upload_speed, self.download_speed, self.latency_ms
        )
    }
}

/// Запустить спидтест к указанному серверу
pub async fn run_speedtest(
    addr: &str,
    size: u64,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
) -> Result<SpeedTestResult, String> {
    let target = if addr.contains(':') {
        addr.to_string()
    } else {
        format!("{}:{}", addr, DEFAULT_PORT)
    };

    let _ = event_tx.send(TransferEvent::SpeedTestStarted(target.clone()));

    // Подключаемся
    let stream = TcpStream::connect(&target)
        .await
        .map_err(|e| format!("Ошибка подключения: {}", e))?;
    
    stream.set_nodelay(true).ok();
    let (mut reader, mut writer) = stream.into_split();

    // Измеряем latency (ping)
    let latency_ms = measure_latency(&mut reader, &mut writer).await?;
    
    // Отправляем запрос на спидтест
    let request = Message::SpeedTestRequest { size };
    let request_bytes = request.to_bytes().map_err(|e| e.to_string())?;
    writer.write_all(&request_bytes).await.map_err(|e| e.to_string())?;

    // Ждём подтверждения готовности
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data).await.map_err(|e| e.to_string())?;
    
    match Message::from_bytes(&data) {
        Ok(Message::SpeedTestReady) => {}
        Ok(Message::Error(e)) => return Err(e),
        _ => return Err("Неожиданный ответ сервера".to_string()),
    }

    // === Upload test ===
    let _ = event_tx.send(TransferEvent::SpeedTestProgress("upload".to_string(), 0));
    let upload_speed = test_upload(&mut writer, size, &event_tx).await?;

    // Ждём подтверждения
    reader.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data).await.map_err(|e| e.to_string())?;
    
    // === Download test ===
    let _ = event_tx.send(TransferEvent::SpeedTestProgress("download".to_string(), 0));
    let download_speed = test_download(&mut reader, size, &event_tx).await?;

    // Отправляем Ack
    let ack = Message::Ack.to_bytes().map_err(|e| e.to_string())?;
    writer.write_all(&ack).await.map_err(|e| e.to_string())?;

    let result = SpeedTestResult {
        upload_speed,
        download_speed,
        latency_ms,
    };

    let _ = event_tx.send(TransferEvent::SpeedTestCompleted(
        result.upload_speed,
        result.download_speed,
        result.latency_ms,
    ));

    Ok(result)
}

/// Измерить latency (ping)
async fn measure_latency(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
) -> Result<f64, String> {
    let mut total_latency = 0.0;
    const PING_COUNT: u32 = 5;

    for _ in 0..PING_COUNT {
        let start = Instant::now();
        
        // Отправляем Ack как ping
        let ping = Message::Ack.to_bytes().map_err(|e| e.to_string())?;
        writer.write_all(&ping).await.map_err(|e| e.to_string())?;
        
        // Ждём ответ
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        reader.read_exact(&mut data).await.map_err(|e| e.to_string())?;
        
        total_latency += start.elapsed().as_secs_f64() * 1000.0;
    }

    Ok(total_latency / PING_COUNT as f64)
}

/// Тест upload скорости
async fn test_upload(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    size: u64,
    event_tx: &mpsc::UnboundedSender<TransferEvent>,
) -> Result<f64, String> {
    let chunk = vec![0xABu8; SPEEDTEST_CHUNK_SIZE];
    let mut sent = 0u64;
    let start = Instant::now();
    let mut last_update = Instant::now();

    while sent < size {
        let remaining = (size - sent) as usize;
        let to_send = remaining.min(SPEEDTEST_CHUNK_SIZE);
        
        let msg = Message::SpeedTestData {
            data: chunk[..to_send].to_vec(),
        };
        let msg_bytes = msg.to_bytes().map_err(|e| e.to_string())?;
        writer.write_all(&msg_bytes).await.map_err(|e| e.to_string())?;
        
        sent += to_send as u64;
        
        // Обновляем прогресс раз в секунду
        if last_update.elapsed().as_secs() >= 1 {
            let progress = ((sent as f64 / size as f64) * 100.0) as u8;
            let _ = event_tx.send(TransferEvent::SpeedTestProgress("upload".to_string(), progress));
            last_update = Instant::now();
        }
    }

    // Финальное обновление 100%
    let _ = event_tx.send(TransferEvent::SpeedTestProgress("upload".to_string(), 100));

    // Отправляем конец
    let end = Message::SpeedTestEnd.to_bytes().map_err(|e| e.to_string())?;
    writer.write_all(&end).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

    let elapsed = start.elapsed().as_secs_f64();
    let speed_mbps = (size as f64 / 1024.0 / 1024.0) / elapsed;

    Ok(speed_mbps)
}

/// Тест download скорости
async fn test_download(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
    size: u64,
    event_tx: &mpsc::UnboundedSender<TransferEvent>,
) -> Result<f64, String> {
    let mut received = 0u64;
    let start = Instant::now();
    let mut last_update = Instant::now();

    loop {
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        reader.read_exact(&mut data).await.map_err(|e| e.to_string())?;

        match Message::from_bytes(&data) {
            Ok(Message::SpeedTestData { data: chunk }) => {
                received += chunk.len() as u64;
                // Обновляем прогресс раз в секунду
                if last_update.elapsed().as_secs() >= 1 {
                    let progress = ((received as f64 / size as f64) * 100.0).min(100.0) as u8;
                    let _ = event_tx.send(TransferEvent::SpeedTestProgress("download".to_string(), progress));
                    last_update = Instant::now();
                }
            }
            Ok(Message::SpeedTestEnd) => break,
            Ok(Message::Error(e)) => return Err(e),
            _ => return Err("Неожиданное сообщение".to_string()),
        }
    }

    // Финальное обновление 100%
    let _ = event_tx.send(TransferEvent::SpeedTestProgress("download".to_string(), 100));

    let elapsed = start.elapsed().as_secs_f64();
    let speed_mbps = (received as f64 / 1024.0 / 1024.0) / elapsed;

    Ok(speed_mbps)
}

/// Обработать запрос спидтеста на стороне сервера
pub async fn handle_speedtest_server(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    size: u64,
) -> Result<(), String> {
    // Отправляем готовность
    let ready = Message::SpeedTestReady.to_bytes().map_err(|e| e.to_string())?;
    writer.write_all(&ready).await.map_err(|e| e.to_string())?;

    // === Принимаем upload ===
    let mut _received = 0u64;
    loop {
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        reader.read_exact(&mut data).await.map_err(|e| e.to_string())?;

        match Message::from_bytes(&data) {
            Ok(Message::SpeedTestData { data: chunk }) => {
                _received += chunk.len() as u64;
            }
            Ok(Message::SpeedTestEnd) => break,
            _ => return Err("Неожиданное сообщение в upload".to_string()),
        }
    }

    // Отправляем Ack после upload
    let ack = Message::Ack.to_bytes().map_err(|e| e.to_string())?;
    writer.write_all(&ack).await.map_err(|e| e.to_string())?;

    // === Отправляем download ===
    let chunk = vec![0xCDu8; SPEEDTEST_CHUNK_SIZE];
    let mut sent = 0u64;

    while sent < size {
        let remaining = (size - sent) as usize;
        let to_send = remaining.min(SPEEDTEST_CHUNK_SIZE);
        
        let msg = Message::SpeedTestData {
            data: chunk[..to_send].to_vec(),
        };
        let msg_bytes = msg.to_bytes().map_err(|e| e.to_string())?;
        writer.write_all(&msg_bytes).await.map_err(|e| e.to_string())?;
        
        sent += to_send as u64;
    }

    // Отправляем конец
    let end = Message::SpeedTestEnd.to_bytes().map_err(|e| e.to_string())?;
    writer.write_all(&end).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

    // Ждём Ack
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data).await.map_err(|e| e.to_string())?;

    Ok(())
}

/// Обработать запрос спидтеста через абстрактный транспорт
pub async fn handle_speedtest_server_transport(
    stream: &mut dyn super::transport::TransportStream,
    size: u64,
) -> Result<(), String> {
    // Отправляем готовность
    let ready = Message::SpeedTestReady.to_bytes().map_err(|e| e.to_string())?;
    stream.write_all(&ready).await.map_err(|e| e.to_string())?;

    // === Принимаем upload ===
    loop {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await.map_err(|e| e.to_string())?;

        match Message::from_bytes(&data) {
            Ok(Message::SpeedTestData { data: _ }) => {}
            Ok(Message::SpeedTestEnd) => break,
            _ => return Err("Неожиданное сообщение в upload".to_string()),
        }
    }

    // Отправляем Ack после upload
    let ack = Message::Ack.to_bytes().map_err(|e| e.to_string())?;
    stream.write_all(&ack).await.map_err(|e| e.to_string())?;

    // === Отправляем download ===
    let chunk = vec![0xCDu8; SPEEDTEST_CHUNK_SIZE];
    let mut sent = 0u64;

    while sent < size {
        let remaining = (size - sent) as usize;
        let to_send = remaining.min(SPEEDTEST_CHUNK_SIZE);
        
        let msg = Message::SpeedTestData {
            data: chunk[..to_send].to_vec(),
        };
        let msg_bytes = msg.to_bytes().map_err(|e| e.to_string())?;
        stream.write_all(&msg_bytes).await.map_err(|e| e.to_string())?;
        
        sent += to_send as u64;
    }

    // Отправляем конец
    let end = Message::SpeedTestEnd.to_bytes().map_err(|e| e.to_string())?;
    stream.write_all(&end).await.map_err(|e| e.to_string())?;
    stream.flush().await.map_err(|e| e.to_string())?;

    // Ждём Ack
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await.map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speedtest_result_formatted() {
        let result = SpeedTestResult {
            upload_speed: 100.5,
            download_speed: 95.3,
            latency_ms: 0.5,
        };
        
        let formatted = result.formatted();
        assert!(formatted.contains("100.5"));
        assert!(formatted.contains("95.3"));
        assert!(formatted.contains("0.5"));
    }

    #[test]
    fn test_speedtest_constants() {
        assert_eq!(DEFAULT_SPEEDTEST_SIZE, 10 * 1024 * 1024);
        assert_eq!(SPEEDTEST_CHUNK_SIZE, 64 * 1024);
    }
}

