//! Интеграционные тесты для toolza_sender

use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::net::TcpListener;
use toolza_sender::protocol::{FileInfo, Message};
use toolza_sender::network::TransferEvent;

/// Тест: базовая сериализация/десериализация протокола
#[test]
fn test_protocol_roundtrip() {
    let messages = vec![
        Message::FileStart {
            filename: "test.txt".to_string(),
            size: 1024,
            compressed: true,
            offset: 0,
            quick_hash: 12345,
        },
        Message::FileChunk {
            data: vec![1, 2, 3, 4, 5],
            original_size: 5,
        },
        Message::FileEnd,
        Message::Ack,
        Message::ResumeAck { offset: 500 },
        Message::Error("Test error".to_string()),
        Message::Done,
    ];
    
    for msg in messages {
        let bytes = msg.to_bytes().expect("Serialization failed");
        let decoded = Message::from_bytes(&bytes[4..]).expect("Deserialization failed");
        
        // Проверяем что тип сообщения совпадает
        match (&msg, &decoded) {
            (Message::FileStart { .. }, Message::FileStart { .. }) => {}
            (Message::FileChunk { .. }, Message::FileChunk { .. }) => {}
            (Message::FileEnd, Message::FileEnd) => {}
            (Message::Ack, Message::Ack) => {}
            (Message::ResumeAck { .. }, Message::ResumeAck { .. }) => {}
            (Message::Error(_), Message::Error(_)) => {}
            (Message::Done, Message::Done) => {}
            _ => panic!("Message type mismatch"),
        }
    }
}

/// Тест: compression работает корректно
#[test]
fn test_compression_roundtrip() {
    use toolza_sender::network::compression;
    
    let test_cases = vec![
        b"Hello, World!".to_vec(),
        vec![0u8; 1000], // Повторяющиеся данные (хорошо сжимаются)
        (0..255).collect::<Vec<u8>>(), // Случайные данные
        vec![], // Пустые данные
    ];
    
    for original in test_cases {
        let compressed = compression::compress(&original);
        let decompressed = compression::decompress(&compressed)
            .expect("Decompression failed");
        
        assert_eq!(original, decompressed, "Data mismatch after roundtrip");
    }
}

/// Тест: парсинг подсетей
#[test]
fn test_subnet_parsing() {
    use toolza_sender::network::parse_subnets;
    
    // Простые случаи
    let subnets = parse_subnets("192.168.1");
    assert_eq!(subnets.len(), 1);
    
    // Несколько подсетей
    let subnets = parse_subnets("192.168.1, 10.0.0, 172.16.0");
    assert_eq!(subnets.len(), 3);
    
    // С CIDR нотацией
    let subnets = parse_subnets("192.168.1.0/24");
    assert_eq!(subnets.len(), 1);
    
    // Невалидные подсети игнорируются
    let subnets = parse_subnets("192.168.1, invalid, 10.0.0");
    assert_eq!(subnets.len(), 2);
    
    // Пустая строка
    let subnets = parse_subnets("");
    assert!(subnets.is_empty());
}

/// Тест: форматирование размера
#[test]
fn test_format_size() {
    use toolza_sender::utils::format_size;
    
    assert!(format_size(0).contains("Б"));
    assert!(format_size(1024).contains("КБ"));
    assert!(format_size(1024 * 1024).contains("МБ"));
    assert!(format_size(1024 * 1024 * 1024).contains("ГБ"));
}

/// Тест: форматирование скорости
#[test]
fn test_format_speed() {
    use toolza_sender::stats::format_speed;
    
    assert!(format_speed(100.0).contains("B/s"));
    assert!(format_speed(1500.0).contains("KB/s"));
    assert!(format_speed(1500000.0).contains("MB/s"));
    assert!(format_speed(1500000000.0).contains("GB/s"));
}

/// Тест: статистика передачи
#[test]
fn test_transfer_stats() {
    use toolza_sender::stats::TransferStats;
    
    let mut stats = TransferStats::new(1000, 5);
    
    assert_eq!(stats.total_bytes, 1000);
    assert_eq!(stats.files_total, 5);
    assert_eq!(stats.progress_percent(), 0.0);
    
    stats.transferred_bytes = 500;
    assert!((stats.progress_percent() - 50.0).abs() < 0.1);
    
    stats.file_completed();
    assert_eq!(stats.files_completed, 1);
}

/// Тест: история передач
#[test]
fn test_transfer_history() {
    use toolza_sender::history::{TransferHistory, HistoryEntry};
    
    let mut history = TransferHistory::new();
    assert!(history.entries.is_empty());
    
    let entry = HistoryEntry::new_send(
        5,
        1024,
        10.0,
        0.8,
        vec!["192.168.1.100".to_string()],
        true,
        None,
    );
    
    history.entries.push(entry);
    assert_eq!(history.entries.len(), 1);
    
    let stats = history.total_stats();
    assert_eq!(stats.total_transfers, 1);
    assert_eq!(stats.successful_transfers, 1);
}

/// Тест: синхронизация - определение изменений
#[test]
fn test_sync_diff() {
    use toolza_sender::sync::{SyncFileInfo, RemoteFileInfo, compute_sync_diff};
    
    let local = vec![
        SyncFileInfo {
            path: PathBuf::from("/new.txt"),
            relative_path: "new.txt".to_string(),
            size: 100,
            modified: 0,
            quick_hash: 111,
        },
        SyncFileInfo {
            path: PathBuf::from("/same.txt"),
            relative_path: "same.txt".to_string(),
            size: 200,
            modified: 0,
            quick_hash: 222,
        },
    ];
    
    let remote = vec![
        RemoteFileInfo {
            relative_path: "same.txt".to_string(),
            size: 200,
            modified: 0,
            quick_hash: 222,
        },
        RemoteFileInfo {
            relative_path: "old.txt".to_string(),
            size: 300,
            modified: 0,
            quick_hash: 333,
        },
    ];
    
    let diff = compute_sync_diff(&local, &remote);
    
    // new.txt должен передаваться (нет на remote)
    assert_eq!(diff.to_transfer.len(), 1);
    assert_eq!(diff.to_transfer[0].relative_path, "new.txt");
    
    // same.txt не изменился
    assert_eq!(diff.unchanged.len(), 1);
    
    // old.txt только на remote
    assert_eq!(diff.remote_only.len(), 1);
}

/// Тест: FileInfo создание
#[test]
fn test_file_info_creation() {
    use tempfile::TempDir;
    
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    std::fs::write(&file_path, "Hello, World!").unwrap();
    
    let info = FileInfo::new(file_path).unwrap();
    
    assert_eq!(info.name, "test.txt");
    assert_eq!(info.size, 13);
    assert_eq!(info.transferred, 0);
    assert_eq!(info.progress(), 0.0);
}

/// Тест: collect_files_from_folder
#[test]
fn test_collect_files() {
    use tempfile::TempDir;
    use toolza_sender::protocol::collect_files_from_folder;
    
    let dir = TempDir::new().unwrap();
    
    // Создаём файлы
    std::fs::write(dir.path().join("file1.txt"), "content1").unwrap();
    std::fs::write(dir.path().join("file2.txt"), "content2").unwrap();
    
    // Создаём подпапку с файлом
    let subdir = dir.path().join("subdir");
    std::fs::create_dir(&subdir).unwrap();
    std::fs::write(subdir.join("file3.txt"), "content3").unwrap();
    
    let files = collect_files_from_folder(dir.path()).unwrap();
    
    assert_eq!(files.len(), 3);
    
    // Проверяем что относительные пути правильные
    let paths: Vec<&str> = files.iter().map(|f| f.relative_path.as_str()).collect();
    assert!(paths.iter().any(|p| p.ends_with("file1.txt")));
    assert!(paths.iter().any(|p| p.ends_with("file2.txt")));
    assert!(paths.iter().any(|p| p.contains("subdir") && p.ends_with("file3.txt")));
}

/// Async тест: события сканирования
#[tokio::test]
async fn test_scan_events() {
    use toolza_sender::network::TransferEvent;
    
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // Отправляем тестовые события
    tx.send(TransferEvent::ScanProgress("192.168.1.1".to_string(), 10)).unwrap();
    tx.send(TransferEvent::ServerFound("192.168.1.100:9527".to_string())).unwrap();
    tx.send(TransferEvent::ScanCompleted).unwrap();
    
    // Проверяем получение
    let event1 = rx.recv().await.unwrap();
    assert!(matches!(event1, TransferEvent::ScanProgress(_, 10)));
    
    let event2 = rx.recv().await.unwrap();
    assert!(matches!(event2, TransferEvent::ServerFound(_)));
    
    let event3 = rx.recv().await.unwrap();
    assert!(matches!(event3, TransferEvent::ScanCompleted));
}

/// Async тест: события передачи
#[tokio::test]
async fn test_transfer_events() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // Симулируем события передачи
    tx.send(TransferEvent::Connected(0, "192.168.1.100:9527".to_string())).unwrap();
    tx.send(TransferEvent::FileStarted(0, 0)).unwrap();
    tx.send(TransferEvent::Progress(0, 0, 500, 1000, 800)).unwrap();
    tx.send(TransferEvent::FileCompleted(0, 0)).unwrap();
    tx.send(TransferEvent::TargetCompleted(0)).unwrap();
    tx.send(TransferEvent::AllCompleted).unwrap();
    
    // Собираем все события
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    
    assert_eq!(events.len(), 6);
    assert!(matches!(events[0], TransferEvent::Connected(0, _)));
    assert!(matches!(events[5], TransferEvent::AllCompleted));
}

/// Async тест: реальная передача файла через TCP
#[tokio::test]
async fn test_real_file_transfer() {
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    let test_content = b"Hello, World! This is a test file for transfer.";
    std::fs::write(&test_file, test_content).unwrap();
    
    // Запускаем сервер на случайном порту
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Запускаем приёмник в фоне
    let receive_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.set_nodelay(true).ok();
        
        let mut received = Vec::new();
        let mut buf = [0u8; 1024];
        
        loop {
            match socket.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => received.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }
        
        received
    });
    
    // Даём серверу время запуститься
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // Отправляем данные
    let mut client = TcpStream::connect(addr).await.unwrap();
    client.set_nodelay(true).ok();
    client.write_all(test_content).await.unwrap();
    drop(client); // Закрываем соединение
    
    // Ждём результат
    let received = receive_handle.await.unwrap();
    assert_eq!(received, test_content);
}

/// Тест: передача протокольного сообщения через TCP
#[tokio::test]
async fn test_protocol_message_transfer() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Приёмник
    let receive_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        
        // Читаем длину
        let mut len_buf = [0u8; 4];
        socket.read_exact(&mut len_buf).await.unwrap();
        let len = u32::from_le_bytes(len_buf) as usize;
        
        // Читаем сообщение
        let mut data = vec![0u8; len];
        socket.read_exact(&mut data).await.unwrap();
        
        Message::from_bytes(&data).unwrap()
    });
    
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    // Отправитель
    let msg = Message::FileStart {
        filename: "test.txt".to_string(),
        size: 1024,
        compressed: true,
        offset: 0,
        quick_hash: 12345,
    };
    
    let msg_bytes = msg.to_bytes().unwrap();
    
    let mut client = TcpStream::connect(addr).await.unwrap();
    client.write_all(&msg_bytes).await.unwrap();
    drop(client);
    
    // Проверяем полученное сообщение
    let received = receive_handle.await.unwrap();
    match received {
        Message::FileStart { filename, size, compressed, offset, quick_hash } => {
            assert_eq!(filename, "test.txt");
            assert_eq!(size, 1024);
            assert!(compressed);
            assert_eq!(offset, 0);
            assert_eq!(quick_hash, 12345);
        }
        _ => panic!("Wrong message type received"),
    }
}

/// Тест: несколько сообщений подряд
#[tokio::test]
async fn test_multiple_messages() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let receive_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut messages = Vec::new();
        
        for _ in 0..3 {
            let mut len_buf = [0u8; 4];
            if socket.read_exact(&mut len_buf).await.is_err() {
                break;
            }
            let len = u32::from_le_bytes(len_buf) as usize;
            
            let mut data = vec![0u8; len];
            socket.read_exact(&mut data).await.unwrap();
            
            messages.push(Message::from_bytes(&data).unwrap());
        }
        
        messages
    });
    
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    let mut client = TcpStream::connect(addr).await.unwrap();
    
    // Отправляем три сообщения
    let msg1 = Message::FileStart {
        filename: "file1.txt".to_string(),
        size: 100,
        compressed: false,
        offset: 0,
        quick_hash: 0,
    };
    let msg2 = Message::FileChunk {
        data: vec![1, 2, 3, 4, 5],
        original_size: 5,
    };
    let msg3 = Message::FileEnd;
    
    client.write_all(&msg1.to_bytes().unwrap()).await.unwrap();
    client.write_all(&msg2.to_bytes().unwrap()).await.unwrap();
    client.write_all(&msg3.to_bytes().unwrap()).await.unwrap();
    drop(client);
    
    let received = receive_handle.await.unwrap();
    assert_eq!(received.len(), 3);
    assert!(matches!(received[0], Message::FileStart { .. }));
    assert!(matches!(received[1], Message::FileChunk { .. }));
    assert!(matches!(received[2], Message::FileEnd));
}

/// Тест: большие данные
#[tokio::test]
async fn test_large_data_transfer() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    
    // Создаём большой блок данных (1 MB)
    let large_data: Vec<u8> = (0..1024*1024).map(|i| (i % 256) as u8).collect();
    let large_data_clone = large_data.clone();
    
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let receive_handle = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut received = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        
        loop {
            match socket.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => received.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }
        }
        
        received
    });
    
    tokio::time::sleep(Duration::from_millis(10)).await;
    
    let mut client = TcpStream::connect(addr).await.unwrap();
    client.write_all(&large_data_clone).await.unwrap();
    drop(client);
    
    let received = receive_handle.await.unwrap();
    assert_eq!(received.len(), large_data.len());
    assert_eq!(received, large_data);
}

/// Тест: сжатие больших данных
#[test]
fn test_compression_large_data() {
    use toolza_sender::network::compression;
    
    // 1 MB повторяющихся данных (хорошо сжимается)
    let original: Vec<u8> = (0..1024*1024).map(|i| ((i / 1024) % 256) as u8).collect();
    
    let compressed = compression::compress(&original);
    
    // Сжатые данные должны быть меньше
    assert!(compressed.len() < original.len());
    
    let decompressed = compression::decompress(&compressed).unwrap();
    assert_eq!(original, decompressed);
}

/// Тест: события TransferEvent все типы
#[test]
fn test_transfer_event_types() {
    let events = vec![
        TransferEvent::Connected(0, "addr".to_string()),
        TransferEvent::FileStarted(0, 0),
        TransferEvent::Progress(0, 0, 100, 200, 150),
        TransferEvent::FileCompleted(0, 0),
        TransferEvent::FileError(0, 0, "error".to_string()),
        TransferEvent::TargetCompleted(0),
        TransferEvent::AllCompleted,
        TransferEvent::ConnectionError(0, "error".to_string()),
        TransferEvent::FileSkipped(0, 0),
        TransferEvent::FileResumed(0, 0, 500),
        TransferEvent::Disconnected,
        TransferEvent::FileReceived("file".to_string(), 100),
        TransferEvent::ExtractionStarted("archive".to_string()),
        TransferEvent::ExtractionCompleted("archive".to_string(), 10, 1000),
        TransferEvent::ExtractionError("archive".to_string(), "error".to_string()),
        TransferEvent::ServerFound("addr".to_string()),
        TransferEvent::ScanProgress("ip".to_string(), 50),
        TransferEvent::ScanCompleted,
    ];
    
    // Просто проверяем что все типы существуют и создаются
    assert_eq!(events.len(), 18);
}

