//! Toolza CLI - консольная версия для передачи файлов

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use tokio::sync::mpsc;
use toolza_sender::network::{self, TransferEvent, TransportType};
use toolza_sender::protocol::{FileInfo, collect_files_from_folder, DEFAULT_PORT};
use toolza_sender::utils::{format_size, get_local_ip_string};

/// Тип транспорта для CLI
#[derive(Clone, Copy, Debug, ValueEnum, Default)]
enum Transport {
    /// TCP - надёжный, стандартный
    #[default]
    Tcp,
    /// UDP - без гарантий доставки (только для тестов!)
    Udp,
    /// QUIC - быстрый, с шифрованием (UDP)
    #[cfg(feature = "quic")]
    Quic,
    /// KCP - сверхбыстрый, низкая задержка (UDP)
    #[cfg(feature = "kcp")]
    Kcp,
}

impl From<Transport> for TransportType {
    fn from(t: Transport) -> Self {
        match t {
            Transport::Tcp => TransportType::Tcp,
            Transport::Udp => TransportType::Udp,
            #[cfg(feature = "quic")]
            Transport::Quic => TransportType::Quic,
            #[cfg(feature = "kcp")]
            Transport::Kcp => TransportType::Kcp,
        }
    }
}

#[derive(Parser)]
#[command(name = "toolza_cli")]
#[command(author = "toolza")]
#[command(version = "1.0")]
#[command(about = "Быстрая передача файлов по локальной сети", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Отправить файлы на указанные адреса
    Send {
        /// Адреса получателей (IP или IP:порт), через запятую
        #[arg(short, long, value_delimiter = ',')]
        targets: Vec<String>,
        
        /// Файлы и папки для отправки
        #[arg(required = true)]
        files: Vec<PathBuf>,
        
        /// Порт (по умолчанию 9527)
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        
        /// Использовать LZ4 сжатие
        #[arg(short = 'c', long)]
        compress: bool,
        
        /// Не сохранять структуру папок (все файлы в одну папку)
        #[arg(long)]
        flat: bool,
        
        /// Режим синхронизации (передавать только изменённые файлы)
        #[arg(short = 's', long)]
        sync: bool,
        
        /// Транспортный протокол (tcp, quic, kcp)
        #[arg(long, value_enum, default_value_t = Transport::Tcp)]
        transport: Transport,
    },
    
    /// Принимать файлы (запустить сервер)
    Receive {
        /// Порт для прослушивания
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        
        /// Папка для сохранения файлов
        #[arg(short, long)]
        dir: Option<PathBuf>,
        
        /// Автоматически распаковывать tar.lz4 архивы
        #[arg(short = 'x', long)]
        extract: bool,
        
        /// Транспортный протокол (tcp, quic, kcp)
        #[arg(long, value_enum, default_value_t = Transport::Tcp)]
        transport: Transport,
    },
    
    /// Сканировать сеть на наличие серверов
    Scan {
        /// Порт для проверки
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        
        /// Подсети для сканирования (например: 192.168.1.0,10.0.0.0)
        /// Если не указаны, сканируется локальная подсеть
        #[arg(short, long, value_delimiter = ',')]
        subnets: Option<Vec<String>>,
    },
    
    /// Тест скорости соединения с сервером
    Speedtest {
        /// Адрес сервера (IP или IP:порт)
        #[arg(required = true)]
        target: String,
        
        /// Порт (по умолчанию 9527)
        #[arg(short, long, default_value_t = DEFAULT_PORT)]
        port: u16,
        
        /// Размер данных для теста в МБ (по умолчанию 10)
        #[arg(short = 'm', long, default_value_t = 10)]
        size: u64,
        
        /// Транспортный протокол (tcp, quic, kcp)
        #[arg(long, value_enum, default_value_t = Transport::Tcp)]
        transport: Transport,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Send { targets, files, port, compress, flat, sync, transport } => {
            let preserve_structure = !flat;
            send_files(targets, files, port, compress, preserve_structure, sync, transport.into()).await;
        }
        Commands::Receive { port, dir, extract, transport } => {
            receive_files(port, dir, extract, transport.into()).await;
        }
        Commands::Scan { port, subnets } => {
            scan_network(port, subnets).await;
        }
        Commands::Speedtest { target, port, size, transport } => {
            run_speedtest(target, port, size, transport.into()).await;
        }
    }
}

async fn send_files(targets: Vec<String>, paths: Vec<PathBuf>, port: u16, use_compression: bool, preserve_structure: bool, _sync_mode: bool, transport_type: TransportType) {
    if targets.is_empty() {
        eprintln!("Ошибка: укажите хотя бы один адрес получателя (-t)");
        std::process::exit(1);
    }
    
    if paths.is_empty() {
        eprintln!("Ошибка: укажите файлы для отправки");
        std::process::exit(1);
    }
    
    // Собираем файлы
    let mut files: Vec<FileInfo> = Vec::new();
    for path in paths {
        if path.is_dir() {
            match collect_files_from_folder(&path) {
                Ok(folder_files) => {
                    println!("📁 Папка '{}': {} файл(ов)", path.display(), folder_files.len());
                    files.extend(folder_files);
                }
                Err(e) => {
                    eprintln!("Ошибка сканирования папки '{}': {}", path.display(), e);
                }
            }
        } else if path.is_file() {
            match FileInfo::new(path.clone()) {
                Ok(info) => {
                    files.push(info);
                }
                Err(e) => {
                    eprintln!("Ошибка чтения файла '{}': {}", path.display(), e);
                }
            }
        } else {
            eprintln!("Путь не существует: {}", path.display());
        }
    }
    
    if files.is_empty() {
        eprintln!("Нет файлов для отправки");
        std::process::exit(1);
    }
    
    // Если не сохраняем структуру - используем только имена файлов
    if !preserve_structure {
        for file in &mut files {
            file.relative_path = file.name.clone();
        }
    }
    
    // Добавляем порт к адресам если нужно
    let targets: Vec<String> = targets
        .into_iter()
        .map(|t| {
            if t.contains(':') {
                t
            } else {
                format!("{}:{}", t, port)
            }
        })
        .collect();
    
    let total_size: u64 = files.iter().map(|f| f.size).sum();
    
    println!();
    println!("🚀 Отправка {} файл(ов) ({}) на {} получателей", 
        files.len(), 
        format_size(total_size),
        targets.len()
    );
    println!("🔌 Протокол: {}", transport_type.name());
    if use_compression {
        println!("🗜  LZ4 сжатие: включено");
    }
    if preserve_structure {
        println!("📂 Структура папок: сохраняется");
    } else {
        println!("📂 Структура папок: плоская (все файлы в одну папку)");
    }
    if _sync_mode {
        println!("🔄 Режим синхронизации: только изменённые файлы");
    }
    println!();
    
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // Создаём опции
    let options = network::SendOptions {
        use_compression,
        enable_resume: true,
        transport_type,
    };
    
    // Запускаем отправку
    let files_clone = files.clone();
    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    tokio::spawn(async move {
        network::send_files_to_multiple_with_stop(targets, files_clone, options, tx, stop_flag).await;
    });
    
    // Обрабатываем события
    let mut completed_targets = 0;
    let total_targets = files.len();
    
    while let Some(event) = rx.recv().await {
        match event {
            TransferEvent::Connected(_, addr) => {
                println!("✅ Подключено: {}", addr);
            }
            TransferEvent::FileStarted(target_id, file_idx) => {
                if let Some(file) = files.get(file_idx) {
                    println!("📤 [{}] Отправка: {} ({})", 
                        target_id, file.relative_path, format_size(file.size));
                }
            }
            TransferEvent::FileCompleted(target_id, file_idx) => {
                if let Some(file) = files.get(file_idx) {
                    println!("✅ [{}] Завершено: {}", target_id, file.relative_path);
                }
            }
            TransferEvent::FileSkipped(target_id, file_idx) => {
                if let Some(file) = files.get(file_idx) {
                    println!("⏭️ [{}] Пропущен (актуален): {}", target_id, file.relative_path);
                }
            }
            TransferEvent::FileResumed(target_id, file_idx, offset) => {
                if let Some(file) = files.get(file_idx) {
                    println!("🔄 [{}] Возобновление: {} @ {}", 
                        target_id, file.relative_path, format_size(offset));
                }
            }
            TransferEvent::TargetCompleted(target_id) => {
                completed_targets += 1;
                println!("🎉 Получатель {} завершён ({}/{})", 
                    target_id, completed_targets, total_targets);
            }
            TransferEvent::ConnectionError(target_id, err) => {
                eprintln!("❌ [{}] Ошибка: {}", target_id, err);
            }
            TransferEvent::FileError(target_id, file_idx, err) => {
                if let Some(file) = files.get(file_idx) {
                    eprintln!("❌ [{}] Ошибка отправки {}: {}", 
                        target_id, file.relative_path, err);
                }
            }
            TransferEvent::AllCompleted => {
                println!();
                println!("✅ Передача завершена!");
                break;
            }
            _ => {}
        }
    }
}

async fn receive_files(port: u16, save_dir: Option<PathBuf>, auto_extract: bool, transport_type: TransportType) {
    let save_dir = save_dir.unwrap_or_else(|| {
        dirs::download_dir().unwrap_or_else(|| PathBuf::from("."))
    });
    
    let local_ip = get_local_ip_string();
    
    println!();
    println!("📥 Сервер запущен");
    println!("   IP: {}", local_ip);
    println!("   Порт: {}", port);
    println!("   Протокол: {}", transport_type.name());
    println!("   Сохранение в: {}", save_dir.display());
    if auto_extract {
        println!("   📦 Авто-распаковка tar.lz4: включена");
    }
    println!();
    println!("Ожидание подключений... (Ctrl+C для выхода)");
    println!();
    
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // Создаём опции
    let options = network::ServerOptions {
        extract_options: network::ExtractOptions {
            tar_lz4: auto_extract,
            lz4: false,
            tar: false,
            zip: false,
            rar: false,
        },
        enable_resume: true,
        transport_type,
        save_archive_for_resume: false, // В CLI по умолчанию чистая потоковая распаковка
    };
    
    // Запускаем сервер
    let save_dir_clone = save_dir.clone();
    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    tokio::spawn(async move {
        if let Err(e) = network::run_server_with_options_and_stop(port, save_dir_clone, options, tx, stop_flag).await {
            eprintln!("Ошибка сервера: {}", e);
        }
    });
    
    // Обрабатываем события
    while let Some(event) = rx.recv().await {
        match event {
            TransferEvent::Connected(_, addr) => {
                println!("🔗 Подключение: {}", addr);
            }
            TransferEvent::FileReceived(name, size) => {
                println!("📥 Получен: {} ({})", name, format_size(size));
            }
            TransferEvent::ExtractionStarted(name) => {
                println!("📦 Распаковка: {}", name);
            }
            TransferEvent::ExtractionCompleted(name, files_count, total_size) => {
                println!("✅ Распаковано {}: {} файл(ов), {}", name, files_count, format_size(total_size));
            }
            TransferEvent::ExtractionError(name, err) => {
                eprintln!("❌ Ошибка распаковки {}: {}", name, err);
            }
            TransferEvent::Disconnected => {
                println!("🔌 Клиент отключился");
                println!();
            }
            TransferEvent::ConnectionError(_, err) => {
                eprintln!("❌ Ошибка: {}", err);
            }
            _ => {}
        }
    }
}

async fn scan_network(port: u16, subnets_input: Option<Vec<String>>) {
    let local_ip = get_local_ip_string();
    
    println!();
    println!("🔍 Сканирование сети...");
    println!("   Ваш IP: {}", local_ip);
    println!("   Порт: {}", port);
    
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // Парсим подсети или используем автоопределение
    if let Some(subnets_str) = subnets_input {
        let input = subnets_str.join(",");
        let subnets = network::parse_subnets(&input);
        
        if subnets.is_empty() {
            eprintln!("Ошибка: не удалось распознать подсети");
            std::process::exit(1);
        }
        
        println!("   Подсети:");
        for subnet in &subnets {
            println!("     - {}", subnet);
        }
        println!();
        
        tokio::spawn(async move {
            let _ = network::scan_subnets(subnets, port, tx).await;
        });
    } else {
        println!("   Подсеть: автоопределение");
        println!();
        
        tokio::spawn(async move {
            let _ = network::scan_network(port, tx).await;
        });
    }
    
    let mut found = Vec::new();
    
    // Обрабатываем события
    while let Some(event) = rx.recv().await {
        match event {
            TransferEvent::ServerFound(addr) => {
                println!("\r🟢 Найден сервер: {}                    ", addr);
                found.push(addr);
            }
            TransferEvent::ScanProgress(ip, progress) => {
                print!("\r   Проверка: {} ({}%)    ", ip, progress);
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
            TransferEvent::ScanCompleted => {
                println!();
                println!();
                if found.is_empty() {
                    println!("Серверы не найдены");
                } else {
                    println!("Найдено серверов: {}", found.len());
                    for server in &found {
                        println!("  - {}", server);
                    }
                }
                break;
            }
            _ => {}
        }
    }
}

async fn run_speedtest(target: String, port: u16, size_mb: u64, transport_type: TransportType) {
    let target_addr = if target.contains(':') {
        target
    } else {
        format!("{}:{}", target, port)
    };
    
    let size = size_mb * 1024 * 1024;
    
    println!();
    println!("🚀 Спидтест");
    println!("   Сервер: {}", target_addr);
    println!("   Протокол: {}", transport_type.name());
    println!("   Размер данных: {} MB", size_mb);
    println!();
    println!("💡 Убедитесь, что на сервере запущен режим \"receive\" с тем же протоколом");
    println!();
    
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    let target_addr_clone = target_addr.clone();
    let handle = tokio::spawn(async move {
        network::run_speedtest(&target_addr_clone, size, tx).await
    });
    
    // Обрабатываем события
    while let Some(event) = rx.recv().await {
        match event {
            TransferEvent::SpeedTestStarted(addr) => {
                println!("🔗 Подключено к {}", addr);
            }
            TransferEvent::SpeedTestProgress(direction, progress) => {
                let dir_str = if direction == "upload" { "⬆️  Upload" } else { "⬇️  Download" };
                print!("\r   {} {}%      ", dir_str, progress);
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
            TransferEvent::SpeedTestCompleted(upload, download, latency) => {
                println!("\r                              ");
                println!();
                println!("📊 Результаты:");
                println!("   ⬆️  Upload:   {:.1} MB/s", upload);
                println!("   ⬇️  Download: {:.1} MB/s", download);
                println!("   🏓 Ping:     {:.2} ms", latency);
                println!();
                
                // Оценка качества
                let avg_speed = (upload + download) / 2.0;
                let quality = if avg_speed >= 100.0 && latency < 1.0 {
                    "🌟 Превосходно"
                } else if avg_speed >= 50.0 && latency < 2.0 {
                    "✅ Отлично"
                } else if avg_speed >= 20.0 && latency < 5.0 {
                    "👍 Хорошо"
                } else if avg_speed >= 5.0 && latency < 10.0 {
                    "⚠️  Нормально"
                } else {
                    "❌ Медленно"
                };
                println!("   Качество соединения: {}", quality);
                break;
            }
            TransferEvent::SpeedTestError(err) => {
                eprintln!("\n❌ Ошибка: {}", err);
                break;
            }
            _ => {}
        }
    }
    
    let _ = handle.await;
}

