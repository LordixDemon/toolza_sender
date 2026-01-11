# 🚀 Toolza Sender

<div align="center">

![Rust](https://img.shields.io/badge/Rust-1.75+-orange?logo=rust&logoColor=white)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-blue)
![License](https://img.shields.io/badge/License-MIT-green)
![Version](https://img.shields.io/badge/Version-1.0.0-blue)

**⚡ Быстрая передача файлов по локальной сети**

*Современная альтернатива netcat с GUI и потоковой распаковкой архивов*

[🇬🇧 English](#-english) • [🇷🇺 Русский](#-русский)

</div>

---

## 🎯 Ключевые особенности

| Возможность | Описание |
|-------------|----------|
| 🔥 **4 протокола** | TCP, UDP, QUIC (шифрованный), KCP (сверхбыстрый) |
| 📦 **Потоковая распаковка** | tar.lz4, tar.zst — распаковка на лету без загрузки в RAM |
| 🗜️ **LZ4 сжатие** | Ускорение передачи текстовых файлов |
| 👥 **Мульти-отправка** | Одновременная отправка на несколько компьютеров |
| 🔄 **Докачка** | Автоматическое возобновление прерванных передач |
| 🌍 **Мультиязычность** | Русский, Украинский, Английский |

---

# 🇬🇧 English

## What is this?

A program for **quickly transferring files between computers** on the same network:

```
┌─────────────────┐         ┌─────────────────┐
│   Computer A    │         │   Computer B    │
│   (Receiver)    │◄────────│    (Sender)     │
│   Server mode   │  files  │   Client mode   │
└─────────────────┘         └─────────────────┘
```

## Quick Start

### 1️⃣ Build

```bash
git clone https://github.com/LordixDemon/toolza_sender.git
cd toolza_sender
cargo build --release
```

**Binaries in `target/release/`:**
- `toolza_sender` — GUI version
- `toolza_cli` — Terminal version

### 2️⃣ Receive Files (Computer A)

**GUI:** Run → "📥 Receive" → "▶ Start Server"

**Terminal:**
```bash
./toolza_cli receive
./toolza_cli receive -d ./downloads -x    # with auto-extract
./toolza_cli receive --transport kcp      # KCP protocol (faster)
```

### 3️⃣ Send Files (Computer B)

**GUI:** Run → "📤 Send" → Enter IP → Add files → "🚀 Send"

**Terminal:**
```bash
./toolza_cli send -t 192.168.1.100 file.zip
./toolza_cli send -t 192.168.1.100 -c ./folder/           # with compression
./toolza_cli send -t 192.168.1.100 --transport kcp ./data # KCP protocol
```

## CLI Reference

### `send` — Send files

```bash
toolza_cli send [OPTIONS] -t <TARGETS> <FILES>...

Options:
  -t, --targets <IP>     Receiver IP(s), comma-separated (required)
  -p, --port <PORT>      Port [default: 9527]
  -c, --compress         Enable LZ4 compression
  -s, --sync             Sync mode (only changed files)
  --flat                 Don't preserve folder structure
  --transport <TYPE>     Protocol: tcp, udp, quic, kcp [default: tcp]
```

### `receive` — Receive files (server mode)

```bash
toolza_cli receive [OPTIONS]

Options:
  -p, --port <PORT>      Listen port [default: 9527]
  -d, --dir <PATH>       Save directory [default: Downloads]
  -x, --extract          Auto-extract tar.lz4/tar.zst archives
  --transport <TYPE>     Protocol: tcp, udp, quic, kcp [default: tcp]
```

### `scan` — Find servers on network

```bash
toolza_cli scan [OPTIONS]

Options:
  -p, --port <PORT>      Port to check [default: 9527]
  -s, --subnets <LIST>   Subnets to scan (e.g., 192.168.1,10.0.0)
```

### `speedtest` — Test connection speed

```bash
toolza_cli speedtest <SERVER_IP> [OPTIONS]

Options:
  -p, --port <PORT>      Port [default: 9527]
  -m, --size <MB>        Test data size in MB [default: 10]
  --transport <TYPE>     Protocol: tcp, udp, quic, kcp [default: tcp]
```

## Protocols

| Protocol | Speed | Reliability | Encryption | Best for |
|----------|-------|-------------|------------|----------|
| **TCP** | ⭐⭐⭐ | ✅ Guaranteed | ❌ | Default, large files |
| **UDP** | ⭐⭐⭐⭐ | ❌ None | ❌ | Testing only |
| **QUIC** | ⭐⭐⭐ | ✅ Guaranteed | ✅ TLS 1.3 | Internet transfers |
| **KCP** | ⭐⭐⭐⭐⭐ | ✅ Guaranteed | ❌ | LAN, max speed (+30-40%) |

## Supported Archives (Auto-extract)

| Format | Streaming | Description |
|--------|-----------|-------------|
| `.tar.lz4` | ✅ On-the-fly | Fast LZ4 compression |
| `.tar.zst` | ✅ On-the-fly | Zstandard compression (supports `--long=31`) |
| `.tar.gz` / `.tgz` | ❌ | Standard gzip |
| `.tar` | ❌ | Uncompressed tar |
| `.zip` | ❌ | Standard zip |
| `.lz4` | ❌ | Raw LZ4 file |

> 💡 **Streaming extraction** means archives are unpacked directly from network stream without loading entire file into RAM. Perfect for huge archives (tested with 1.8TB+).

---

# 🇷🇺 Русский

## Что это?

Программа для **быстрой передачи файлов между компьютерами** в одной сети:

```
┌─────────────────┐         ┌─────────────────┐
│   Компьютер А   │         │   Компьютер Б   │
│   (Получатель)  │◄────────│  (Отправитель)  │
│  Режим сервера  │  файлы  │  Режим клиента  │
└─────────────────┘         └─────────────────┘
```

## Быстрый старт

### 1️⃣ Сборка

```bash
git clone https://github.com/LordixDemon/toolza_sender.git
cd toolza_sender
cargo build --release
```

**Бинарники в `target/release/`:**
- `toolza_sender` — GUI версия
- `toolza_cli` — Терминальная версия

### 2️⃣ Принять файлы (Компьютер А)

**GUI:** Запустить → "📥 Приём" → "▶ Запустить сервер"

**Терминал:**
```bash
./toolza_cli receive
./toolza_cli receive -d ./downloads -x    # с авто-распаковкой
./toolza_cli receive --transport kcp      # протокол KCP (быстрее)
```

### 3️⃣ Отправить файлы (Компьютер Б)

**GUI:** Запустить → "📤 Отправка" → Ввести IP → Добавить файлы → "🚀 Отправить"

**Терминал:**
```bash
./toolza_cli send -t 192.168.1.100 file.zip
./toolza_cli send -t 192.168.1.100 -c ./folder/           # со сжатием
./toolza_cli send -t 192.168.1.100 --transport kcp ./data # протокол KCP
```

## Справка по CLI

### `send` — Отправка файлов

```bash
toolza_cli send [ОПЦИИ] -t <АДРЕСА> <ФАЙЛЫ>...

Опции:
  -t, --targets <IP>     IP получателей, через запятую (обязательно)
  -p, --port <PORT>      Порт [по умолчанию: 9527]
  -c, --compress         Включить LZ4 сжатие
  -s, --sync             Режим синхронизации (только изменённые)
  --flat                 Не сохранять структуру папок
  --transport <TYPE>     Протокол: tcp, udp, quic, kcp [по умолчанию: tcp]
```

### `receive` — Приём файлов (режим сервера)

```bash
toolza_cli receive [ОПЦИИ]

Опции:
  -p, --port <PORT>      Порт прослушивания [по умолчанию: 9527]
  -d, --dir <PATH>       Папка для сохранения [по умолчанию: Загрузки]
  -x, --extract          Авто-распаковка tar.lz4/tar.zst архивов
  --transport <TYPE>     Протокол: tcp, udp, quic, kcp [по умолчанию: tcp]
```

### `scan` — Поиск серверов в сети

```bash
toolza_cli scan [ОПЦИИ]

Опции:
  -p, --port <PORT>      Порт для проверки [по умолчанию: 9527]
  -s, --subnets <LIST>   Подсети для сканирования (напр: 192.168.1,10.0.0)
```

### `speedtest` — Тест скорости

```bash
toolza_cli speedtest <IP_СЕРВЕРА> [ОПЦИИ]

Опции:
  -p, --port <PORT>      Порт [по умолчанию: 9527]
  -m, --size <МБ>        Размер тестовых данных в МБ [по умолчанию: 10]
  --transport <TYPE>     Протокол: tcp, udp, quic, kcp [по умолчанию: tcp]
```

## Протоколы

| Протокол | Скорость | Надёжность | Шифрование | Когда использовать |
|----------|----------|------------|------------|-------------------|
| **TCP** | ⭐⭐⭐ | ✅ Гарантирована | ❌ | По умолчанию, большие файлы |
| **UDP** | ⭐⭐⭐⭐ | ❌ Нет | ❌ | Только для тестов |
| **QUIC** | ⭐⭐⭐ | ✅ Гарантирована | ✅ TLS 1.3 | Передача через интернет |
| **KCP** | ⭐⭐⭐⭐⭐ | ✅ Гарантирована | ❌ | LAN, макс. скорость (+30-40%) |

## Поддерживаемые архивы (Авто-распаковка)

| Формат | Потоковая | Описание |
|--------|-----------|----------|
| `.tar.lz4` | ✅ На лету | Быстрое LZ4 сжатие |
| `.tar.zst` | ✅ На лету | Zstandard сжатие (поддержка `--long=31`) |
| `.tar.gz` / `.tgz` | ❌ | Стандартный gzip |
| `.tar` | ❌ | Несжатый tar |
| `.zip` | ❌ | Стандартный zip |
| `.lz4` | ❌ | Сырой LZ4 файл |

> 💡 **Потоковая распаковка** означает, что архивы распаковываются прямо из сетевого потока без загрузки всего файла в RAM. Идеально для огромных архивов (протестировано на 1.8TB+).

---

## 📁 Структура проекта

```
src/
├── main.rs                 # GUI точка входа
├── lib.rs                  # Общая библиотека
├── bin/cli.rs              # CLI бинарник
│
├── app/                    # Состояние приложения
│   ├── state.rs            # Структура App
│   ├── actions.rs          # Действия (старт/стоп сервера, отправка)
│   └── event_handler.rs    # Обработка событий передачи
│
├── network/                # Сетевая логика
│   ├── sender.rs           # Отправка файлов
│   ├── receiver/           # Приём файлов
│   │   ├── handlers.rs     # Обработчики подключений
│   │   ├── streaming.rs    # Потоковая распаковка
│   │   └── options.rs      # Опции сервера
│   ├── scanner.rs          # Сканирование сети
│   ├── speedtest.rs        # Тест скорости
│   ├── compression.rs      # LZ4 сжатие
│   └── transport/          # Транспортные протоколы
│       ├── tcp.rs
│       ├── udp.rs
│       ├── quic.rs
│       └── kcp.rs
│
├── extract/                # Распаковка архивов
│   ├── lz4.rs              # tar.lz4, lz4
│   ├── zst.rs              # tar.zst
│   ├── tar.rs              # tar, tar.gz
│   ├── zip.rs              # zip
│   └── types.rs            # Типы архивов
│
├── ui/                     # GUI интерфейс
│   ├── send_view.rs        # Вкладка отправки
│   ├── receive_view.rs     # Вкладка приёма
│   ├── extract_view.rs     # Вкладка распаковки
│   ├── history_view.rs     # История передач
│   ├── speedtest_view.rs   # Тест скорости
│   └── widgets.rs          # Общие виджеты
│
├── i18n/                   # Переводы
│   └── translations.rs     # RU, UA, EN
│
├── protocol.rs             # Бинарный протокол передачи
├── sync.rs                 # Синхронизация файлов
├── stats.rs                # Статистика передач
├── history.rs              # История
└── utils.rs                # Утилиты
```

## 🛠️ Сборка

```bash
# Полная сборка (TCP + QUIC + KCP)
cargo build --release

# Минимальная сборка (только TCP + UDP)
cargo build --release --no-default-features --features minimal

# Проверить фичи
cargo build --release --features "quic,kcp"
```

### Требования

- **Rust 1.75+** (из-за async traits)
- **Linux:** `libgtk-3-dev` для диалогов выбора файлов
- **Windows/macOS:** ничего дополнительного

## 📝 Лицензия

MIT License — используйте свободно!

---

<div align="center">

**Made with ❤️ and 🦀 Rust**

*Если проект полезен — поставьте ⭐ на GitHub!*

</div>
