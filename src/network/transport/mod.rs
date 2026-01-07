//! Абстракция транспортного протокола (TCP, UDP, QUIC, KCP)

mod tcp;
mod udp;
#[cfg(feature = "quic")]
mod quic;
#[cfg(feature = "kcp")]
mod kcp;

pub use tcp::{TcpTransport, TcpStreamWrapper};
pub use udp::UdpTransport;
#[cfg(feature = "quic")]
pub use quic::QuicTransport;
#[cfg(feature = "kcp")]
pub use kcp::KcpTransport;

use async_trait::async_trait;
use std::io;

/// Тип транспортного протокола
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TransportType {
    #[default]
    Tcp,
    /// ⚠️ UDP не гарантирует доставку - только для тестов!
    Udp,
    #[cfg(feature = "quic")]
    Quic,
    #[cfg(feature = "kcp")]
    Kcp,
}

impl TransportType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
            #[cfg(feature = "quic")]
            Self::Quic => "QUIC",
            #[cfg(feature = "kcp")]
            Self::Kcp => "KCP",
        }
    }
    
    pub fn description(&self) -> &'static str {
        match self {
            Self::Tcp => "Надёжный, стандартный протокол",
            Self::Udp => "⚠️ Без гарантий доставки! Только для тестов",
            #[cfg(feature = "quic")]
            Self::Quic => "Быстрый, с шифрованием (UDP)",
            #[cfg(feature = "kcp")]
            Self::Kcp => "Сверхбыстрый, низкая задержка (UDP)",
        }
    }
    
    pub fn all() -> Vec<TransportType> {
        vec![
            Self::Tcp,
            Self::Udp,
            #[cfg(feature = "quic")]
            Self::Quic,
            #[cfg(feature = "kcp")]
            Self::Kcp,
        ]
    }
    
    /// Парсинг из строки
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "tcp" => Some(Self::Tcp),
            "udp" => Some(Self::Udp),
            #[cfg(feature = "quic")]
            "quic" => Some(Self::Quic),
            #[cfg(feature = "kcp")]
            "kcp" => Some(Self::Kcp),
            _ => None,
        }
    }
}

impl std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Абстракция потока для чтения/записи
#[async_trait]
pub trait TransportStream: Send {
    /// Читать данные в буфер
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    
    /// Читать точное количество байт
    async fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()>;
    
    /// Записать данные
    async fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;
    
    /// Сбросить буфер
    async fn flush(&mut self) -> io::Result<()>;
    
    /// Закрыть соединение
    async fn shutdown(&mut self) -> io::Result<()>;
}

/// Абстракция слушателя (сервера)
#[async_trait]
pub trait TransportListener: Send {
    /// Принять входящее соединение
    async fn accept(&mut self) -> io::Result<(Box<dyn TransportStream>, String)>;
    
    /// Принять с таймаутом (для проверки stop_flag)
    async fn accept_timeout(&mut self, timeout: std::time::Duration) -> io::Result<Option<(Box<dyn TransportStream>, String)>>;
}

/// Создать транспорт по типу
pub async fn connect(transport_type: TransportType, addr: &str) -> io::Result<Box<dyn TransportStream>> {
    match transport_type {
        TransportType::Tcp => {
            let transport = TcpTransport::new();
            Ok(Box::new(transport.connect(addr).await?))
        }
        TransportType::Udp => {
            Ok(Box::new(UdpTransport::connect(addr).await?))
        }
        #[cfg(feature = "quic")]
        TransportType::Quic => {
            let transport = QuicTransport::new();
            Ok(Box::new(transport.connect(addr).await?))
        }
        #[cfg(feature = "kcp")]
        TransportType::Kcp => {
            let transport = KcpTransport::new();
            Ok(Box::new(transport.connect(addr).await?))
        }
    }
}

/// Создать слушатель по типу
pub async fn bind(transport_type: TransportType, port: u16) -> io::Result<Box<dyn TransportListener>> {
    match transport_type {
        TransportType::Tcp => {
            let transport = TcpTransport::new();
            Ok(Box::new(transport.bind(port).await?))
        }
        TransportType::Udp => {
            Ok(Box::new(UdpTransport::bind(port).await?))
        }
        #[cfg(feature = "quic")]
        TransportType::Quic => {
            let transport = QuicTransport::new();
            Ok(Box::new(transport.bind(port).await?))
        }
        #[cfg(feature = "kcp")]
        TransportType::Kcp => {
            let transport = KcpTransport::new();
            Ok(Box::new(transport.bind(port).await?))
        }
    }
}

