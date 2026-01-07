# 🚀 Toolza Sender

![Rust](https://img.shields.io/badge/Rust-1.70+-orange?logo=rust)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-blue)
![License](https://img.shields.io/badge/License-MIT-green)
![Tests](https://img.shields.io/badge/Tests-115%20passed-brightgreen)
![Coverage](https://img.shields.io/badge/Coverage-41%25-yellow)
![Version](https://img.shields.io/badge/Version-1.0.0-blue)
![Protocols](https://img.shields.io/badge/Protocols-TCP%20%7C%20UDP%20%7C%20QUIC%20%7C%20KCP-purple)

**Fast file transfer over local network** — a modern netcat alternative with GUI.

> ✅ **Tested on:** Windows 10/11, Linux (Ubuntu, Arch)  
> ⚠️ **macOS:** Should work, not fully tested

---

# 🇬🇧 English

## What is this?

A program for **quickly transferring files between computers** on the same network. Works like this:

1. **Computer A** (receiver) starts the server
2. **Computer B** (sender) connects and sends files
3. Done! Files appear on Computer A

## Quick Start (5 minutes)

### Step 1: Download or Build

```bash
# Clone the repository
git clone https://github.com/LordixDemon/toolza_sender.git
cd toolza_sender

# Build (requires Rust installed)
cargo build --release
```

Binaries will be in `target/release/`:
- `toolza_sender` — GUI version (with buttons and windows)
- `toolza_cli` — Terminal version (for servers or advanced users)

### Step 2: Receive Files (Computer A)

**GUI:**
1. Run `toolza_sender`
2. Click "📥 Receive" in the left menu
3. Click "▶ Start Server"
4. Note your IP address shown (e.g., `192.168.1.100:9527`)

**Terminal:**
```bash
./toolza_cli receive
```

### Step 3: Send Files (Computer B)

**GUI:**
1. Run `toolza_sender`
2. Click "📤 Send" in the left menu
3. Enter IP address of Computer A (e.g., `192.168.1.100`)
4. Click "➕ Add"
5. Click "➕ Files" or "📁 Folder" to select files
6. Click "🚀 Send"

**Terminal:**
```bash
# Send a single file
./toolza_cli send -t 192.168.1.100 myfile.zip

# Send a folder
./toolza_cli send -t 192.168.1.100 ./my_folder/

# Send with compression (faster for text files)
./toolza_cli send -t 192.168.1.100 -c ./my_folder/
```

## All CLI Commands

### Send files

```bash
toolza_cli send [OPTIONS] <FILES>...

# Required:
  -t, --targets <IP>     Receiver IP address(es), comma-separated

# Optional:
  -p, --port <PORT>      Port number (default: 9527)
  -c, --compress         Enable LZ4 compression
  -s, --sync             Sync mode (only changed files)
  --flat                 Don't preserve folder structure
  --transport <TYPE>     Protocol: tcp, udp, quic, kcp (default: tcp)

# Examples:
toolza_cli send -t 192.168.1.100 file.zip
toolza_cli send -t 192.168.1.100,192.168.1.101 -c ./folder/
toolza_cli send -t 192.168.1.100 --transport kcp ./files/
```

### Receive files

```bash
toolza_cli receive [OPTIONS]

# Optional:
  -p, --port <PORT>      Listen port (default: 9527)
  -d, --dir <PATH>       Save directory (default: Downloads)
  -x, --extract          Auto-extract .tar.lz4 archives
  --transport <TYPE>     Protocol: tcp, udp, quic, kcp (default: tcp)

# Examples:
toolza_cli receive
toolza_cli receive -d ./downloads -x
toolza_cli receive --transport kcp
```

### Find servers on network

```bash
toolza_cli scan [OPTIONS]

# Optional:
  -p, --port <PORT>      Port to check (default: 9527)
  -s, --subnets <LIST>   Subnets to scan (e.g., 192.168.1,10.0.0)

# Examples:
toolza_cli scan
toolza_cli scan -s 192.168.1,10.0.0
```

### Speed test

```bash
toolza_cli speedtest <SERVER_IP> [OPTIONS]

# Required:
  <SERVER_IP>            Server address

# Optional:
  -p, --port <PORT>      Port (default: 9527)
  -m, --size <MB>        Test data size in MB (default: 10)
  --transport <TYPE>     Protocol: tcp, udp, quic, kcp (default: tcp)

# Examples:
toolza_cli speedtest 192.168.1.100
toolza_cli speedtest 192.168.1.100 -m 50 --transport kcp
```

> ⚠️ Server must be running `receive` mode with the same protocol!

## Protocols

| Protocol | Description | Best for |
|----------|-------------|----------|
| **TCP** | Reliable, standard | Default choice, large files |
| **UDP** | Fast, no guarantees | Testing only! |
| **QUIC** | Encrypted, modern | Internet transfers |
| **KCP** | Fast, low latency | LAN, max speed (+30-40%) |

## Features

- ⚡ **Fast** — Adaptive chunk size (16KB-512KB)
- 🗜️ **Compression** — Optional LZ4 for faster transfers
- 📁 **Folders** — Transfer entire directories
- 👥 **Multi-target** — Send to multiple computers at once
- 🔄 **Resume** — Auto-resume interrupted transfers
- 📦 **Auto-extract** — Unpack `.tar.lz4` on receive
- 🔍 **Auto-discover** — Find servers on network
- 🌍 **Multi-language** — Russian, Ukrainian, English UI

## Building with all protocols

```bash
# TCP + QUIC + KCP (recommended)
cargo build --release

# Or explicitly:
cargo build --release --features "quic,kcp"
```

---

# 🇷🇺 Русский

## Что это?

Программа для **быстрой передачи файлов между компьютерами** в одной сети. Работает так:

1. **Компьютер А** (получатель) запускает сервер
2. **Компьютер Б** (отправитель) подключается и отправляет файлы
3. Готово! Файлы появляются на Компьютере А

## Быстрый старт (5 минут)

### Шаг 1: Скачать или собрать

```bash
# Клонируем репозиторий
git clone https://github.com/LordixDemon/toolza_sender.git
cd toolza_sender

# Собираем (нужен установленный Rust)
cargo build --release
```

Бинарники появятся в `target/release/`:
- `toolza_sender` — GUI версия (с кнопками и окошками)
- `toolza_cli` — Терминальная версия (для серверов или продвинутых)

### Шаг 2: Принять файлы (Компьютер А)

**GUI:**
1. Запустите `toolza_sender`
2. Нажмите "📥 Приём" в левом меню
3. Нажмите "▶ Запустить сервер"
4. Запомните показанный IP адрес (например, `192.168.1.100:9527`)

**Терминал:**
```bash
./toolza_cli receive
```

### Шаг 3: Отправить файлы (Компьютер Б)

**GUI:**
1. Запустите `toolza_sender`
2. Нажмите "📤 Отправка" в левом меню
3. Введите IP адрес Компьютера А (например, `192.168.1.100`)
4. Нажмите "➕ Добавить"
5. Нажмите "➕ Файлы" или "📁 Папку" для выбора файлов
6. Нажмите "🚀 Отправить"

**Терминал:**
```bash
# Отправить один файл
./toolza_cli send -t 192.168.1.100 myfile.zip

# Отправить папку
./toolza_cli send -t 192.168.1.100 ./my_folder/

# Отправить со сжатием (быстрее для текстов)
./toolza_cli send -t 192.168.1.100 -c ./my_folder/
```

## Все команды CLI

### Отправка файлов

```bash
toolza_cli send [ОПЦИИ] <ФАЙЛЫ>...

# Обязательно:
  -t, --targets <IP>     IP адрес(а) получателей, через запятую

# Опционально:
  -p, --port <PORT>      Порт (по умолчанию: 9527)
  -c, --compress         Включить LZ4 сжатие
  -s, --sync             Режим синхронизации (только изменённые)
  --flat                 Не сохранять структуру папок
  --transport <TYPE>     Протокол: tcp, udp, quic, kcp (по умолчанию: tcp)

# Примеры:
toolza_cli send -t 192.168.1.100 file.zip
toolza_cli send -t 192.168.1.100,192.168.1.101 -c ./folder/
toolza_cli send -t 192.168.1.100 --transport kcp ./files/
```

### Приём файлов

```bash
toolza_cli receive [ОПЦИИ]

# Опционально:
  -p, --port <PORT>      Порт прослушивания (по умолчанию: 9527)
  -d, --dir <PATH>       Папка для сохранения (по умолчанию: Загрузки)
  -x, --extract          Авто-распаковка .tar.lz4 архивов
  --transport <TYPE>     Протокол: tcp, udp, quic, kcp (по умолчанию: tcp)

# Примеры:
toolza_cli receive
toolza_cli receive -d ./downloads -x
toolza_cli receive --transport kcp
```

### Поиск серверов в сети

```bash
toolza_cli scan [ОПЦИИ]

# Опционально:
  -p, --port <PORT>      Порт для проверки (по умолчанию: 9527)
  -s, --subnets <LIST>   Подсети для сканирования (напр: 192.168.1,10.0.0)

# Примеры:
toolza_cli scan
toolza_cli scan -s 192.168.1,10.0.0
```

### Тест скорости

```bash
toolza_cli speedtest <IP_СЕРВЕРА> [ОПЦИИ]

# Обязательно:
  <IP_СЕРВЕРА>           Адрес сервера

# Опционально:
  -p, --port <PORT>      Порт (по умолчанию: 9527)
  -m, --size <МБ>        Размер тестовых данных в МБ (по умолчанию: 10)
  --transport <TYPE>     Протокол: tcp, udp, quic, kcp (по умолчанию: tcp)

# Примеры:
toolza_cli speedtest 192.168.1.100
toolza_cli speedtest 192.168.1.100 -m 50 --transport kcp
```

> ⚠️ На сервере должен быть запущен режим `receive` с тем же протоколом!

## Протоколы

| Протокол | Описание | Когда использовать |
|----------|----------|-------------------|
| **TCP** | Надёжный, стандартный | По умолчанию, большие файлы |
| **UDP** | Быстрый, без гарантий | Только для тестов! |
| **QUIC** | Шифрованный, современный | Передача через интернет |
| **KCP** | Быстрый, низкая задержка | LAN, макс. скорость (+30-40%) |

## Возможности

- ⚡ **Быстро** — Адаптивный размер чанков (16KB-512KB)
- 🗜️ **Сжатие** — Опциональное LZ4 для ускорения
- 📁 **Папки** — Передача целых директорий
- 👥 **Мульти-отправка** — На несколько компов одновременно
- 🔄 **Докачка** — Автоматическое возобновление
- 📦 **Авто-распаковка** — Распаковка `.tar.lz4` при получении
- 🔍 **Автопоиск** — Поиск серверов в сети
- 🌍 **Мультиязычность** — Русский, Украинский, Английский интерфейс

## Сборка со всеми протоколами

```bash
# TCP + QUIC + KCP (рекомендуется)
cargo build --release

# Или явно:
cargo build --release --features "quic,kcp"
```

---

## 📁 Project Structure / Структура проекта

```
src/
├── main.rs              # GUI entry point
├── lib.rs               # Shared library
├── bin/cli.rs           # CLI binary
├── app/                 # Application state & actions
├── network/             # Network logic
│   ├── sender.rs        # Send files
│   ├── receiver/        # Receive files (module)
│   ├── scanner.rs       # Network scanning
│   ├── speedtest.rs     # Speed test
│   └── transport/       # Protocol abstractions
│       ├── tcp.rs
│       ├── udp.rs
│       ├── quic.rs
│       └── kcp.rs
├── ui/                  # GUI views
├── extract/             # Archive extraction (module)
├── i18n/                # Translations
├── protocol.rs          # Binary protocol
├── stats.rs             # Transfer statistics
├── history.rs           # Transfer history
└── utils.rs             # Utilities
```

## 📝 License / Лицензия

MIT License — use freely! / MIT — используйте свободно!

---

**Made with ❤️ and 🦀 Rust**
